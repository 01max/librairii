//! Separate complete-request diagnostic. Never an acceptance measurement.
use super::{Result, cpu, fixture::Story};
use librairii_core::{browsing::Page, query::QueryRequest};
use librairii_storage::{database::Database, query};
use serde_json::{Value, json};
use std::time::Instant;

fn ordinary(db: &Database<'_>) -> Result<(Page, Value)> {
    let cpu_start = cpu::milliseconds();
    let start = Instant::now();
    let page = db.search(QueryRequest::default(), "en-GB".into())?.wait()?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.;
    let process_cpu_ms = cpu::milliseconds() - cpu_start;
    Ok((
        page,
        json!({"wall_ms": wall_ms, "process_cpu_ms": process_cpu_ms}),
    ))
}

fn observed(db: &Database<'_>) -> Result<(Page, Value)> {
    let cpu_start = cpu::milliseconds();
    let start = Instant::now();
    let (page, worker_start_ms, worker_end_ms, boundaries) = db
        .read(move |tx| {
            let worker_start_ms = start.elapsed().as_secs_f64() * 1000.;
            let mut boundaries = Vec::with_capacity(53);
            let page = query::search_observed(tx, QueryRequest::default(), "en-GB", |phase| {
                // CPU reads surround rather than coincide with wall reads. They are
                // process-wide and cannot establish a thread's off-CPU intervals.
                let process_cpu_ms = cpu::milliseconds() - cpu_start;
                let wall_ms = start.elapsed().as_secs_f64() * 1000.;
                boundaries.push((phase, wall_ms, process_cpu_ms));
            })?;
            let worker_end_ms = start.elapsed().as_secs_f64() * 1000.;
            Ok((page, worker_start_ms, worker_end_ms, boundaries))
        })?
        .wait()?;
    let wall_ms = start.elapsed().as_secs_f64() * 1000.;
    let process_cpu_ms = cpu::milliseconds() - cpu_start;
    Ok((
        page,
        json!({"wall_ms": wall_ms, "process_cpu_ms": process_cpu_ms,
        "worker_start_ms": worker_start_ms, "worker_end_ms": worker_end_ms,
        "boundaries": boundaries}),
    ))
}

pub fn run(db: &Database<'_>, rows: &[Story]) -> Result<Value> {
    let (expected, ordinary_warmup) = ordinary(db)?;
    let (actual, observed_warmup) = observed(db)?;
    let mut ordered = rows.iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| {
        (&a.display_name_normalized, &a.uuid, a.id).cmp(&(
            &b.display_name_normalized,
            &b.uuid,
            b.id,
        ))
    });
    assert_eq!(expected.total_items, 5000);
    assert_eq!(
        expected.stories.iter().map(|s| s.id).collect::<Vec<_>>(),
        ordered.iter().take(24).map(|s| s.id).collect::<Vec<_>>()
    );
    assert_eq!(actual, expected, "complete observed warmup Page");
    let mut pairs = Vec::new();
    for index in 0..20 {
        let observed_first = index % 2 == 1;
        let (normal, instrumented) = if observed_first {
            let instrumented = observed(db)?;
            (ordinary(db)?, instrumented)
        } else {
            (ordinary(db)?, observed(db)?)
        };
        assert_eq!(normal.0, expected, "complete ordinary Page");
        assert_eq!(instrumented.0, expected, "complete instrumented Page");
        pairs.push(
            json!({"observed_first": observed_first, "ordinary": normal.1,
            "observed": instrumented.1, "complete_pages_equal": true}),
        );
    }
    Ok(
        json!({"format": "librairii.query-phase-experiment", "version": 1,
        "acceptance_evidence": false, "causal_fix_proven": false,
        "scope": "complete first-page search via database workers; diagnostic overhead included",
        "clock": "wall and process-wide CPU; not per-thread off-CPU time",
        "boundary_format": ["phase", "wall_ms_since_request", "process_cpu_ms_since_request"],
        "outside_worker": "dispatch/start includes admission, scheduling and transaction setup; finish/return includes transaction completion and delivery; neither is pure queue wait",
        "expected_page": expected,
        "warmups": {"ordinary": ordinary_warmup, "observed": observed_warmup}, "pairs": pairs}),
    )
}
