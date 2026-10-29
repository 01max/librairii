//! Frozen review5000 outcomes plus actual large paging/detachment/deletion.
use super::{Result, fixture::committed};
use librairii_core::{CollectionId, ShelfId, collection_codec::Query, shelves::Shelf};
use librairii_storage::{
    StorageError, collections,
    database::{Database, TransactionOutcome, WorkCancellation},
    shelves::{self, Action, Decision},
};
use rusqlite::params;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path, time::Instant};

pub fn seed(db: &Database<'_>, repository: &Path) -> Result<Value> {
    let oracle: Value = serde_json::from_slice(&fs::read(
        repository.join("preservation/corpus/performance/review5000.json"),
    )?)?;
    let rows = oracle["initialAdditions"]
        .as_array()
        .ok_or("missing frozen additions")?
        .clone();
    assert_eq!(rows.len(), 5000);
    committed(db.transaction(WorkCancellation::default(),move |tx| {
        let mut story = tx.prepare("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized,is_broken,created_at) VALUES(?,?,?,?,?,'2026-01-01T00:00:00Z')")?;
        let mut archive = tx.prepare("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?,'story.zip','zip',?,1,?,'valid')")?;
        for (index,row) in rows.iter().enumerate() {
            let id=index as i64+1;
            let uuid=row["uuid"].as_str().ok_or(StorageError::CompatibilityInvariant)?;
            let title=row["title"].as_str().ok_or(StorageError::CompatibilityInvariant)?;
            let broken=row["broken"].as_bool().ok_or(StorageError::CompatibilityInvariant)?;
            story.execute(params![id,uuid,title,title.to_lowercase(),broken])?;
            archive.execute(params![id,format!("{index:064}"),format!("archives/{index}/story.zip")])?;
        }
        Ok(())
    })?.wait()?)?;
    for (name, text) in [("All shared", "shared"), ("Moon", "moon")] {
        let mut criteria = Query::default();
        criteria.name = text.into();
        let response = committed(
            db.edit_collections(
                collections::Command::Create {
                    name: name.into(),
                    criteria,
                },
                WorkCancellation::default(),
            )?
            .wait()?,
        )?;
        assert!(matches!(response, collections::Response::Collection(_)));
    }
    Ok(oracle)
}

fn create(db: &Database<'_>, name: &str) -> Result<Shelf> {
    match committed(
        db.edit_shelves(
            shelves::Command::Create { name: name.into() },
            WorkCancellation::default(),
        )?
        .wait()?,
    )? {
        shelves::Response::Shelf(shelf) => Ok(shelf),
        _ => Err("expected created Shelf".into()),
    }
}

fn preview(db: &Database<'_>, id: ShelfId) -> Result<shelves::UpdatePlan> {
    Ok(db.shelf_update_preview(id, "en-GB".into())?.wait()?)
}

fn candidates(plan: &shelves::UpdatePlan) -> Value {
    let rows = (0..plan.preview().addition_count().div_ceil(25)).flat_map(|page|plan.preview().additions_page(page)).map(|c|json!({
        "uuid":c.uuid,"title":c.title,"broken":c.story.as_ref().is_some_and(|s|s.broken),
        "sourceCollectionIds":c.sources.iter().filter_map(|s|s.collection_id.map(CollectionId::get)).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    json!(rows)
}

fn summary(s: &Shelf) -> Value {
    json!({"id":s.id.get(),"name":s.name,"position":s.position,"revision":s.revision,"count":s.count,
        "brokenCount":s.broken_count,"unavailableCount":s.unavailable_count,"sourceCount":s.source_count})
}

fn bounds(count: usize, mut page: impl FnMut(usize) -> usize) -> Value {
    let lengths = (0..count.div_ceil(25)).map(&mut page).collect::<Vec<_>>();
    assert!(lengths.iter().all(|n| *n > 0 && *n <= 25));
    assert_eq!(lengths.iter().sum::<usize>(), count);
    assert_eq!(page(count.div_ceil(25)), 0);
    assert_eq!(page(usize::MAX), 0);
    json!({"total":count,"page_lengths":lengths,"next_and_huge_empty":true})
}

fn source_bounds(preview: &shelves::SourceChangePreview) -> Value {
    bounds(preview.affected_count(), |page| {
        preview.affected_page(page).len()
    })
}

fn read_exclusions(db: &Database<'_>, id: ShelfId) -> Result<Vec<String>> {
    let mut rows = Vec::new();
    for page in 0..198 {
        let result = db.shelf_exclusions_page(id, page)?.wait()?;
        assert_eq!(result.total, 4950);
        assert_eq!(result.rows.len(), 25);
        rows.extend(result.rows.into_iter().map(|r| r.uuid));
    }
    assert!(db.shelf_exclusions_page(id, 198)?.wait()?.rows.is_empty());
    assert!(
        db.shelf_exclusions_page(id, usize::MAX)?
            .wait()?
            .rows
            .is_empty()
    );
    Ok(rows)
}

fn warning_ranges(db: &Database<'_>, moon: CollectionId, oracle: &Value) -> Result<Value> {
    let mut unhealthy = Vec::new();
    for n in 0..32 {
        let mut criteria = Query::default();
        criteria.name = "shared".into();
        let response = committed(
            db.edit_collections(
                collections::Command::Create {
                    name: format!("Warning {n:02}"),
                    criteria,
                },
                WorkCancellation::default(),
            )?
            .wait()?,
        )?;
        let collections::Response::Collection(c) = response else {
            return Err("expected warning source".into());
        };
        unhealthy.push(CollectionId::new(c.id)?);
    }
    let creation = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: "Warnings".into(),
                collection_ids: vec![unhealthy[0]],
                ..Default::default()
            },
            "en-GB".into(),
        )?
        .wait()?;
    let forest = creation.preview().candidates_page(0)[1].reference()?;
    assert_eq!(
        forest.as_stored(),
        oracle["initialAdditions"][1]["uuid"]
            .as_str()
            .ok_or("frozen forest UUID")?
    );
    let shelf = committed(
        db.create_shelf_reviewed(creation, vec![forest.clone()], WorkCancellation::default())?
            .wait()?,
    )?;
    let mut sources = vec![moon];
    sources.extend(unhealthy.iter().copied());
    committed(
        db.attach_shelf_sources(shelf.id, sources.clone(), WorkCancellation::default())?
            .wait()?,
    )?;
    let bad = unhealthy.iter().map(|id| id.get()).collect::<Vec<_>>();
    let mark = bad.clone();
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            // Owned corrupted/needs-attention fixture state, never a source/profile
            // mutation. Production evaluation must retain every unhealthy source.
            for id in mark {
                tx.execute(
                    "UPDATE collections SET validity_state='needs_attention' WHERE id=?",
                    [id],
                )?;
            }
            Ok(())
        })?
        .wait()?,
    )?;
    let review = preview(db, shelf.id)?;
    assert!(review.preview().incomplete);
    assert_eq!(review.preview().warning_count(), 32);
    assert_eq!(
        review.preview().removal_count(),
        0,
        "incomplete coverage cannot infer removal of the approved forest"
    );
    assert_eq!(
        review.preview().addition_count(),
        2500,
        "healthy Moon additions stay available"
    );
    let ranges = bounds(32, |page| review.preview().warnings_page(page).len());
    let warnings = (0..2)
        .flat_map(|page| review.preview().warnings_page(page))
        .map(|w| {
            assert_eq!(w.code, "source_needs_attention");
            w.source
                .as_ref()
                .and_then(|s| s.collection_id)
                .map(CollectionId::get)
                .ok_or("warning source identity")
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(warnings, bad);
    let add = review.preview().additions_page(0)[0].reference()?;
    let changed = committed(
        db.apply_shelf_updates(
            review,
            vec![Decision {
                uuid: add,
                action: Action::Add,
            }],
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    assert_eq!(changed.count, 2);
    assert!(
        db.shelf_members(shelf.id)?
            .wait()?
            .iter()
            .any(|m| m.uuid == forest.as_stored() && !m.manually_retained)
    );
    let fresh = preview(db, shelf.id)?;
    assert!(fresh.preview().incomplete);
    assert_eq!(fresh.preview().removal_count(), 0);
    let creation = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: "Incomplete creation".into(),
                collection_ids: sources,
                ..Default::default()
            },
            "en-GB".into(),
        )?
        .wait()?;
    assert!(creation.preview().incomplete);
    assert_eq!(creation.preview().warning_count(), 32);
    let creation_ranges = bounds(32, |page| creation.preview().warnings_page(page).len());
    Ok(
        json!({"warnings":warnings,"review_ranges":ranges,"creation_ranges":creation_ranges,
        "incomplete":fresh.preview().incomplete,"removal_count":fresh.preview().removal_count(),
        "healthy_additions":2500,"approved_forest_retained":true,"explicit_healthy_add_committed":true}),
    )
}

pub fn run(db: &Database<'_>, oracle: &Value) -> Result<Value> {
    let all = CollectionId::new(1)?;
    let moon = CollectionId::new(2)?;
    let shelf = create(db, "Review")?;
    committed(
        db.attach_shelf_sources(shelf.id, vec![all, moon], WorkCancellation::default())?
            .wait()?,
    )?;
    let warmup = preview(db, shelf.id)?;
    assert_eq!(candidates(&warmup), oracle["initialAdditions"]);
    let mut latest = None;
    let mut preview_ms = Vec::new();
    for _ in 0..4 {
        let start = Instant::now();
        let plan = preview(db, shelf.id)?;
        preview_ms.push(start.elapsed().as_secs_f64() * 1000.);
        assert_eq!(candidates(&plan), oracle["initialAdditions"]);
        assert_eq!(plan.preview().removal_count(), 0);
        latest = Some(plan);
    }
    let plan = latest.ok_or("missing measured review")?;
    let review_bounds = bounds(5000, |page| plan.preview().additions_page(page).len());
    let initial = candidates(&plan);
    let decisions = (0..2)
        .flat_map(|page| plan.preview().additions_page(page))
        .map(|c| {
            Ok(Decision {
                uuid: c.reference()?,
                action: Action::Add,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(
        json!(
            decisions
                .iter()
                .map(|d| json!({"uuid":d.uuid.as_stored(),"action":"add"}))
                .collect::<Vec<_>>()
        ),
        oracle["decisions"]
    );
    let start = Instant::now();
    let approved = committed(
        db.apply_shelf_updates(plan, decisions, WorkCancellation::default())?
            .wait()?,
    )?;
    let apply50_ms = start.elapsed().as_secs_f64() * 1000.;
    assert_eq!(summary(&approved), oracle["approved"]);
    let remaining = preview(db, shelf.id)?;
    assert_eq!(remaining.preview().addition_count(), 4950);
    let excluded = remaining.preview().bulk_decisions(Action::Exclude)?;
    committed(
        db.apply_shelf_updates(remaining, excluded, WorkCancellation::default())?
            .wait()?,
    )?;
    let mut exclusions = Vec::new();
    let mut exclusion_lengths = Vec::new();
    for page in 0..198 {
        let p = db.shelf_exclusions_page(shelf.id, page)?.wait()?;
        assert_eq!(p.total, 4950);
        assert_eq!(p.rows.len(), 25);
        exclusion_lengths.push(p.rows.len());
        exclusions.extend(p.rows.into_iter().map(|r| r.uuid));
    }
    assert!(
        db.shelf_exclusions_page(shelf.id, 198)?
            .wait()?
            .rows
            .is_empty()
    );
    assert!(
        db.shelf_exclusions_page(shelf.id, usize::MAX)?
            .wait()?
            .rows
            .is_empty()
    );
    assert_eq!(exclusions.iter().collect::<BTreeSet<_>>().len(), 4950);
    let expected_exclusions = oracle["initialAdditions"]
        .as_array()
        .ok_or("missing frozen additions")?[50..]
        .iter()
        .map(|r| r["uuid"].as_str().ok_or("missing frozen UUID"))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert_eq!(
        exclusions.iter().map(String::as_str).collect::<Vec<_>>(),
        expected_exclusions
    );
    let creation = db
        .shelf_creation_preview(
            shelves::CreationRequest {
                name: "Large".into(),
                collection_ids: vec![all, moon],
                ..Default::default()
            },
            "en-GB".into(),
        )?
        .wait()?;
    assert_eq!(creation.preview().candidate_count(), 5000);
    let creation_bounds = bounds(5000, |page| creation.preview().candidates_page(page).len());
    let selected = creation.preview().all_uuids()?;
    let large = committed(
        db.create_shelf_reviewed(creation, selected, WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(large.count, 5000);
    let detached = match committed(
        db.edit_shelves(
            shelves::Command::Duplicate {
                id: large.id,
                name: "Detach".into(),
            },
            WorkCancellation::default(),
        )?
        .wait()?,
    )? {
        shelves::Response::Shelf(s) => s,
        _ => return Err("expected independent duplicate".into()),
    };
    let detached_before = db.shelf_members(detached.id)?.wait()?;
    let change = db
        .shelf_source_change_preview(detached.id, vec![], "en-GB".into())?
        .wait()?;
    assert_eq!(change.preview().affected_count(), 5000);
    let detached_bounds = source_bounds(change.preview());
    let changed = committed(
        db.change_shelf_sources(change, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!((changed.count, changed.source_count), (5000, 0));
    let kept = db.shelf_members(detached.id)?.wait()?;
    assert_eq!(
        kept.iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>(),
        detached_before
            .iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>()
    );
    assert!(
        kept.iter()
            .all(|m| m.manually_retained && m.origins.iter().all(|s| !s.active))
    );
    let before = db.shelf_members(large.id)?.wait()?;
    let small_before = db.shelf_members(shelf.id)?.wait()?;
    let trigger = format!(
        "CREATE TRIGGER scale_delete_failure BEFORE DELETE ON collections WHEN OLD.id={} BEGIN SELECT CASE WHEN (SELECT COUNT(*) FROM shelf_sources WHERE collection_id=OLD.id AND active=1)=0 AND (SELECT COUNT(*) FROM shelf_stories WHERE shelf_id IN({},{}) AND protected=1)=2525 THEN RAISE(ABORT,'controlled scale rollback') ELSE RAISE(ABORT,'wrong scale rollback stage') END; END",
        all.get(),
        shelf.id.get(),
        large.id.get()
    );
    committed(
        db.transaction(WorkCancellation::default(), move |tx| {
            tx.execute_batch(&trigger)?;
            Ok(())
        })?
        .wait()?,
    )?;
    let deletion = db
        .linked_collection_deletion_preview(all, "en-GB".into())?
        .wait()?;
    assert_eq!(deletion.preview().shelf_count(), 2);
    let deletion_bounds = (0..2)
        .map(|n| {
            Ok(source_bounds(
                deletion
                    .preview()
                    .shelf(n)
                    .ok_or("missing affected Shelf")?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(deletion_bounds[0]["total"], 25);
    assert_eq!(deletion_bounds[1]["total"], 2500);
    assert!(deletion.preview().shelf(2).is_none());
    assert!(deletion.preview().shelf(usize::MAX).is_none());
    let rollback = db
        .delete_linked_collection(deletion, true, vec![], WorkCancellation::default())?
        .wait()?;
    match rollback {
        TransactionOutcome::ProvenNotCommitted(StorageError::Sqlite(
            rusqlite::Error::SqliteFailure(code, Some(message)),
        )) if code.extended_code == 1811 && message == "controlled scale rollback" => {}
        other => {
            return Err(format!(
                "expected proven rollback after guarded detachment writes, observed {other:?}"
            )
            .into());
        }
    }
    assert_eq!(db.shelf_members(large.id)?.wait()?, before);
    assert_eq!(db.shelf_members(shelf.id)?.wait()?, small_before);
    let rollback_exclusions = read_exclusions(db, shelf.id)?;
    assert_eq!(rollback_exclusions, exclusions);
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("DROP TRIGGER scale_delete_failure")?;
            Ok(())
        })?
        .wait()?,
    )?;
    let deletion = db
        .linked_collection_deletion_preview(all, "en-GB".into())?
        .wait()?;
    let receipt = committed(
        db.delete_linked_collection(deletion, true, vec![], WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(receipt.id(), all.get());
    let after = db.shelf_members(large.id)?.wait()?;
    assert_eq!(
        after
            .iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>(),
        before
            .iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>()
    );
    assert_eq!(db.shelf(large.id)?.wait()?.count, 5000);
    assert_eq!(db.shelf(shelf.id)?.wait()?.count, 50);
    let small_after = db.shelf_members(shelf.id)?.wait()?;
    assert_eq!(
        small_after
            .iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>(),
        small_before
            .iter()
            .map(|m| (&m.uuid, m.position))
            .collect::<Vec<_>>()
    );
    let final_exclusions = read_exclusions(db, shelf.id)?;
    assert_eq!(final_exclusions, exclusions);
    let warnings = warning_ranges(db, moon, oracle)?;
    let passed = preview_ms.iter().all(|ms| *ms <= 5000.) && apply50_ms <= 5000.;
    Ok(
        json!({"passed":passed,"preview_ms":preview_ms,"apply50_ms":apply50_ms,"ceiling_ms":5000,
        "initial_additions":initial,"approved":summary(&approved),"remaining_additions":4950,
        "review_ranges":review_bounds,"creation_ranges":creation_bounds,"detachment_ranges":detached_bounds,
        "exclusion_ranges":{"total":4950,"page_lengths":exclusion_lengths,"next_and_huge_empty":true},
        "exclusion_uuids":exclusions,"deletion_ranges":deletion_bounds,"one_shelf_at_a_time":true,
        "rollback_exclusion_uuids":rollback_exclusions,"final_exclusion_uuids":final_exclusions,
        "rollback":{"outcome":"proven_not_committed","error":"controlled scale rollback","intermediate_manual_members":2525},
        "small_member_uuids":small_after.iter().map(|m|&m.uuid).collect::<Vec<_>>(),"warnings":warnings,
        "failed_deletion_preserved_all_members":true,"successful_deletion_preserved_members_order_exclusions":true,
        "detached_manual_members":kept.len(),"retained_large_members":after.len(),
        "large_member_uuids":after.iter().map(|m|&m.uuid).collect::<Vec<_>>()}),
    )
}
