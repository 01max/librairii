#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::ports::{
    creation::{NewItem, NewOperation},
    operations::*,
};
use librairii_core::{
    OperationKind, OperationState,
    operations::{ExportSource, ExportSourceType, ItemState, RawOutcome},
};
use librairii_storage::{
    IsolatedLibrary,
    database::{Database, TransactionOutcome, WorkCancellation},
    operations_repository::TerminalRepository,
};
use librairii_testkit::TestRoots;
use std::error::Error;
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
fn committed<T>(v: TransactionOutcome<T>) -> TestResult<T> {
    match v {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}
fn library(roots: &TestRoots) -> TestResult<IsolatedLibrary> {
    Ok(IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?)
}
fn input(id: &str) -> NewOperation {
    NewOperation {
        id: id.into(),
        kind: OperationKind::Import,
        source: None,
        destination: None,
        items: vec![NewItem {
            source_name: "Été.zip".into(),
            ..Default::default()
        }],
    }
}
#[test]
fn real_creation_start_completion_readback_and_reopen_preserve_legacy_payloads() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let request = input(&"Z".repeat(36));
    let at = "2026-10-22T01:02:03.000000001Z".to_string();
    let created = committed(
        db.create_operation(request.clone(), at.clone(), WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(created.operation.state, OperationState::Queued);
    assert_eq!(created.items.len(), 1);
    let again = committed(
        db.create_operation(request.clone(), at.clone(), WorkCancellation::default())?
            .wait()?,
    )?;
    assert_eq!(again, created);
    let readback = db
        .readback_operation_creation(request.clone(), at)?
        .wait()?
        .ok_or("readback")?;
    assert_eq!(readback, created);
    let mut changed = request.clone();
    changed.items[0].source_name = "Different.zip".into();
    assert!(matches!(
        db.create_operation(
            changed,
            "2026-10-22T01:02:03.000000001Z".into(),
            WorkCancellation::default()
        )?
        .wait()?,
        TransactionOutcome::ProvenNotCommitted(_)
    ));
    committed(
        db.mark_operation_item_running(
            request.id.clone(),
            created.items[0].identity.id,
            "2026-10-22T01:02:04Z".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    let repo = TerminalRepository::new(&db, 1);
    let command = CompleteItemCommand {
        generation: 1,
        operation: created.operation.immutable_scope(),
        item: created.items[0].immutable_scope(),
        story_id: 0,
        state: ItemState::Skipped,
        completed_bytes: 0,
        outcome: RawOutcome {
            code: Some("future.outcome".into()),
            message: Some("Original text".into()),
        },
    };
    assert!(matches!(
        repo.complete_item(&command).disposition,
        AttemptDisposition::Committed { .. }
    ));
    let finish = FinishCommand {
        generation: 1,
        operation: created.operation.immutable_scope(),
        state: OperationState::Succeeded,
        outcome: RawOutcome::default(),
        finished_at: "2026-10-22T01:02:05Z".into(),
    };
    assert!(matches!(
        repo.finish(&finish).disposition,
        AttemptDisposition::Committed { .. }
    ));
    db.close()?;
    drop(db);
    let mut reopened = Database::open_isolated(&lib)?;
    let final_state = reopened
        .operation_snapshot(request.id)?
        .wait()?
        .ok_or("snapshot")?;
    let json = final_state.presentation()?;
    assert_eq!(json["id"], "Z".repeat(36));
    assert_eq!(json["items"][0]["outcomeCode"], "future.outcome");
    assert_eq!(json["items"][0]["outcomeMessage"], "Original text");
    assert!(json["items"][0].get("archiveRelativePath").is_none());
    reopened.close()?;
    Ok(())
}
#[test]
fn unavailable_exports_and_legacy_dynamic_names_keep_distinct_curated_identity() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    for (index, source_type) in [
        ExportSourceType::Shelf,
        ExportSourceType::Collection,
        ExportSourceType::CuratedShelf,
    ]
    .into_iter()
    .enumerate()
    {
        let collection = source_type == ExportSourceType::Collection;
        let source = ExportSource {
            source_type,
            collection_ids: if collection { vec![7] } else { Vec::new() },
            collection_names: if collection {
                vec!["<Été>\u{2028}".into()]
            } else {
                Vec::new()
            },
            shelf_ids: if collection { Vec::new() } else { vec![7] },
            shelf_names: if collection {
                Vec::new()
            } else {
                vec!["<Été>\u{2028}".into()]
            },
        };
        let request = NewOperation {
            id: format!("a0000000-0000-4000-8000-{index:012}"),
            kind: OperationKind::Export,
            source: Some(source),
            destination: Some("/".into()),
            items: vec![NewItem {
                source_name: "Gone".into(),
                story_title: "Gone".into(),
                story_uuid: "10000000-0000-4000-8000-000000000001".into(),
                export_skip_code: "story_unavailable".into(),
                ..Default::default()
            }],
        };
        let snapshot = committed(
            db.create_operation(
                request,
                "2026-10-22T00:00:00Z".into(),
                WorkCancellation::default(),
            )?
            .wait()?,
        )?;
        assert_eq!(snapshot.items[0].identity.story_id, None);
        assert_eq!(
            snapshot.operation.export_scope.group_names.as_deref(),
            Some("[\"\\u003cÉté\\u003e\\u2028\"]")
        );
        let json = snapshot.presentation()?;
        assert_eq!(json["sourceShelfIds"], serde_json::json!([7]));
        assert_eq!(json["destination"], "/");
        if source_type == ExportSourceType::CuratedShelf {
            assert!(json.get("sourceCollectionIds").is_none());
        } else {
            assert_eq!(json["sourceCollectionIds"], serde_json::json!([7]));
        }
    }
    db.close()?;
    Ok(())
}

#[test]
fn invalid_creation_rolls_back_and_changed_header_is_not_an_acknowledgement() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let mut request = input("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ");
    request.items.push(NewItem {
        source_name: "\u{2003}".into(),
        ..Default::default()
    });
    assert!(
        db.create_operation(request.clone(), "now".into(), WorkCancellation::default())
            .is_err()
    );
    assert!(db.operation_snapshot(request.id.clone())?.wait()?.is_none());
    request.items.pop();
    committed(
        db.create_operation(request.clone(), "now".into(), WorkCancellation::default())?
            .wait()?,
    )?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE file_operations SET total_bytes=1 WHERE id='ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ'",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(
        db.readback_operation_creation(request, "now".into())?
            .wait()
            .is_err()
    );
    db.close()?;
    Ok(())
}

#[test]
fn snapshot_byte_limits_refuse_oversized_legacy_text_without_changing_rows() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = library(&roots)?;
    let mut db = Database::open_isolated(&lib)?;
    let request = input("ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ");
    committed(
        db.create_operation(request.clone(), "now".into(), Default::default())?
            .wait()?,
    )?;
    committed(
        db.transaction(Default::default(), |tx| {
            tx.execute(
                "UPDATE file_operation_items SET outcome_message=CAST(zeroblob(16777217) AS TEXT)",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    assert!(matches!(
        TerminalRepository::new(&db, 1).snapshot(&request.id),
        SnapshotReply::Unavailable(_)
    ));
    assert_eq!(
        db.barrier_read(|tx| Ok(tx.query_row(
            "SELECT length(CAST(outcome_message AS BLOB)) FROM file_operation_items",
            [],
            |r| r.get::<_, i64>(0)
        )?))?
        .wait()?,
        16_777_217
    );
    db.close()?;
    Ok(())
}
