use super::{
    Result, cpu,
    fixture::{self, Story},
};
use image::ImageDecoder;
use librairii_core::{browsing::Page, collection_codec::Query, query::QueryRequest};
use librairii_storage::{
    database::{Database, WorkCancellation},
    query,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, io::Cursor, path::Path, time::Instant};
const ARTWORK_LOOKUP: &str = "SELECT embedded_artwork_path FROM stories WHERE id=?";

fn measured(
    mut action: impl FnMut() -> Result<Value>,
    expected: Value,
    budget_ms: f64,
    sample_count: usize,
) -> Result<Value> {
    assert_eq!(action()?, expected, "complete warmup result");
    let mut samples = Vec::new();
    let mut cpu_ms = Vec::new();
    for _ in 0..sample_count {
        let process_start = cpu::milliseconds();
        let started = Instant::now();
        let actual = action()?;
        samples.push(started.elapsed().as_secs_f64() * 1000.);
        cpu_ms.push(cpu::milliseconds() - process_start);
        assert_eq!(actual, expected, "result changed during measurement");
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    let p95 = sorted[(sample_count * 95).div_ceil(100) - 1];
    Ok(
        json!({"wall_ms":samples,"process_cpu_ms":cpu_ms,"p95_ms":p95,"budget_ms":budget_ms,"passed":p95<=budget_ms,"result":expected}),
    )
}

fn page_result(page: Page) -> Value {
    json!({"total":page.total_items,"ids":page.stories.iter().map(|s|s.id).collect::<Vec<_>>()})
}

fn expected(rows: &[Story], predicate: impl Fn(&Story) -> bool, page: usize) -> Value {
    let mut matching = rows.iter().filter(|row| predicate(row)).collect::<Vec<_>>();
    matching.sort_by(|a, b| {
        (&a.display_name_normalized, &a.uuid, a.id).cmp(&(
            &b.display_name_normalized,
            &b.uuid,
            b.id,
        ))
    });
    json!({"total":matching.len(),"ids":matching.iter().skip((page-1)*24).take(24).map(|s|s.id).collect::<Vec<_>>()})
}

pub fn run(db: &Database<'_>, rows: &[Story], root: &Path) -> Result<Value> {
    let queries = fixture::criteria()?;
    let mut metrics = serde_json::Map::new();
    let mut plans = serde_json::Map::new();
    let mut requests = Vec::new();
    requests.push((
        "collectionQuery",
        Query::default(),
        1,
        100.,
        expected(rows, |_| true, 1),
    ));
    requests.push((
        "substringSearch",
        queries[0].clone(),
        1,
        120.,
        expected(rows, |r| r.display_name_normalized.contains("moon"), 1),
    ));
    requests.push((
        "combinedFilters",
        queries[5].clone(),
        1,
        150.,
        expected(
            rows,
            |r| {
                r.display_name_normalized.contains("forest")
                    && r.language == "en-GB"
                    && r.validation_state == "valid"
                    && r.id % 3 == 0
            },
            1,
        ),
    ));
    requests.push((
        "deepPagination",
        Query::default(),
        200,
        120.,
        expected(rows, |_| true, 200),
    ));
    for (name, criteria, page, budget, wanted) in requests {
        let request = QueryRequest {
            criteria,
            page,
            page_size: 24,
            ..Default::default()
        };
        let explained = request.clone();
        let (plan, count_plan) = db
            .read(move |tx| {
                Ok((
                    query::explain_page(tx, explained.clone(), "en-GB")?
                        .ok_or(librairii_storage::StorageError::CompatibilityInvariant)?,
                    query::explain_count(tx, explained, "en-GB")?,
                ))
            })?
            .wait()?;
        eprintln!(
            "actual_query_plan; {name}; {}",
            serde_json::to_string(&plan)?
        );
        assert!(
            plan.steps.iter().any(|line| line.contains("INDEX")),
            "actual indexed {name} plan"
        );
        if name == "combinedFilters" {
            assert!(
                plan.steps.iter().any(|line| line.starts_with("SEARCH t")
                    && line.contains("INDEX")
                    && line.contains("story_id=?")
                    && line.contains("definition_id=?")
                    && line.contains("value_id")),
                "indexed assignment identity prefix and value lookup/range"
            );
            assert!(
                plan.steps
                    .iter()
                    .any(|line| line.contains("official_metadata")
                        || line.contains("official_story_metadata"))
            );
        }
        plans.insert(
            name.into(),
            json!({"page":plan,"count":count_plan,"page_number":page,"page_size":24}),
        );
        metrics.insert(
            name.into(),
            measured(
                || {
                    Ok(page_result(
                        db.search(request.clone(), "en-GB".into())?.wait()?,
                    ))
                },
                wanted,
                budget,
                20,
            )?,
        );
    }
    let count_plans = queries
        .into_iter()
        .map(|criteria| {
            Ok(db
                .read(move |tx| {
                    query::explain_count(
                        tx,
                        QueryRequest {
                            criteria,
                            ..Default::default()
                        },
                        "en-GB",
                    )
                })?
                .wait()?)
        })
        .collect::<Result<Vec<_>>>()?;
    plans.insert("collectionCounts".into(), json!(count_plans));
    metrics.insert(
        "collectionCounts".into(),
        measured(
            || {
                let summaries = fixture::committed(
                    db.collection_summaries("en-GB".into(), WorkCancellation::default())?
                        .wait()?,
                )?;
                Ok(json!(
                    summaries.into_iter().map(|s| s.count).collect::<Vec<_>>()
                ))
            },
            json!([624, 2500, 4000, 5000, 1666, 85]),
            250.,
            20,
        )?,
    );
    let first = db.search(QueryRequest::default(), "en-GB".into())?.wait()?;
    assert_eq!(first.stories.len(), 24);
    let ids = first.stories.iter().map(|s| s.id).collect::<Vec<_>>();
    let lookup = |id| -> Result<String> {
        Ok(db
            .read(move |tx| Ok(tx.query_row(ARTWORK_LOOKUP, [id], |r| r.get::<_, String>(0))?))?
            .wait()?)
    };
    let paths = ids
        .iter()
        .map(|id| lookup(*id))
        .collect::<Result<Vec<_>>>()?;
    let first_id = ids[0];
    let artwork_plan = db
        .read(move |tx| {
            Ok(tx
                .prepare(&format!("EXPLAIN QUERY PLAN {ARTWORK_LOOKUP}"))?
                .query_map([first_id], |r| r.get::<_, String>(3))?
                .collect::<rusqlite::Result<Vec<_>>>()?)
        })?
        .wait()?;
    plans.insert(
        "artworkLoad".into(),
        json!({"statement":ARTWORK_LOOKUP,"parameter_type":"integer","steps":artwork_plan,"ids":ids}),
    );
    assert_eq!(
        paths.iter().collect::<BTreeSet<_>>().len(),
        24,
        "all first-page artwork must be distinct"
    );
    let mut compressed = Vec::new();
    let mut pixels = vec![0; 320 * 400 * 3];
    let mut phases = Vec::new();
    let load = || -> Result<Value> {
        let mut witness = 0_u64;
        let mut phase_wall = [0.; 3];
        let mut phase_cpu = [0.; 3];
        for id in &ids {
            // Include the real record lookup for every local asset load.
            let cpu_start = cpu::milliseconds();
            let start = Instant::now();
            let path = lookup(*id)?;
            phase_wall[0] += start.elapsed().as_secs_f64() * 1000.;
            phase_cpu[0] += cpu::milliseconds() - cpu_start;
            let cpu_start = cpu::milliseconds();
            let start = Instant::now();
            compressed.clear();
            std::io::Read::read_to_end(&mut fs::File::open(root.join(path))?, &mut compressed)?;
            phase_wall[1] += start.elapsed().as_secs_f64() * 1000.;
            phase_cpu[1] += cpu::milliseconds() - cpu_start;
            let cpu_start = cpu::milliseconds();
            let start = Instant::now();
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(&compressed))?;
            assert_eq!(decoder.dimensions(), (320, 400));
            assert_eq!(decoder.total_bytes(), pixels.len() as u64);
            decoder.read_image(&mut pixels)?;
            // Match the qualified scratch probe's full-byte witness. This
            // batch has at most 24 * 320 * 400 * 3 * 255 < u64::MAX, so
            // wrapping addition is exact and avoids per-byte overflow checks.
            witness = std::hint::black_box(&pixels)
                .iter()
                .fold(witness, |sum, byte| sum.wrapping_add(u64::from(*byte)));
            phase_wall[2] += start.elapsed().as_secs_f64() * 1000.;
            phase_cpu[2] += cpu::milliseconds() - cpu_start;
        }
        phases.push(json!({"lookup_wall_ms":phase_wall[0],"lookup_cpu_ms":phase_cpu[0],
            "read_wall_ms":phase_wall[1],"read_cpu_ms":phase_cpu[1],"decode_wall_ms":phase_wall[2],"decode_cpu_ms":phase_cpu[2]}));
        Ok(json!({"files":24,"pixel_sum":witness}))
    };
    // Independent frozen generator RGB sum, not another decoder warmup.
    let wanted = json!({"files":24,"pixel_sum":1_173_893_696_u64});
    let mut artwork = measured(load, wanted, 50., 100)?;
    artwork["phases_warmup_then_100"] = json!(phases);
    metrics.insert("artworkLoad".into(), artwork);
    let passed = metrics.values().all(|metric| metric["passed"] == true);
    Ok(json!({"passed":passed,"metrics":metrics,"query_plans":plans}))
}
