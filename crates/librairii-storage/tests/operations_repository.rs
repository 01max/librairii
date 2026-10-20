//! Real isolated Database/typed-port assertions. Execute only on pinned R+S+E
//! composition; D/R without the actual storage dependency is source-only.
use librairii_app::ports::operations::*;
use librairii_core::{
    OperationState,
    operations::{Counts, ItemState, RawOutcome},
};
use librairii_storage::{
    IsolatedLibrary, StorageError,
    database::{Database, TransactionOutcome},
    operations_repository::{TerminalRepository, command_bytes, snapshot_bytes},
};
use librairii_testkit::TestRoots;
use std::{error::Error, io};

type TestResult = Result<(), Box<dyn Error>>;
const GENERATION: u64 = 17;

fn owned_library() -> Result<(TestRoots, IsolatedLibrary), Box<dyn Error>> {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "A0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    Ok((roots, library))
}

fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T, Box<dyn Error>> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        other => Err(io::Error::other(match other {
            TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
                e.to_string()
            }
            TransactionOutcome::Committed(_) => "unexpected transaction result".into(),
        })
        .into()),
    }
}

fn seed(db: &Database<'_>, id: &str, items: i64) -> TestResult {
    let id = id.to_owned();
    committed(db.repair_transaction(move |tx| {
        tx.execute("INSERT OR IGNORE INTO stories(id,uuid) VALUES(42,'A0000000-0000-4000-8000-000000000042')", [])?;
        tx.execute("INSERT INTO file_operations(id,kind,status,total_items,total_bytes,created_at,
            started_at,export_source_type,export_source_shelf_ids,export_source_shelf_names,
            export_destination,error_code,error_message)
            VALUES(?,'import','queued',?,?,'raw CREATED+02','','shelves','[9, 2]',
            '[\"Été\",\"Été\"] ',' /raw # %2F destination ','',NULL)", rusqlite::params![id,items,items*10])?;
        for ordinal in (0..items).rev() {
            tx.execute("INSERT INTO file_operation_items(id,operation_id,resolved_story_id,story_uuid,
                story_title,source_name,output_name,archive_relative_path,archive_sha256,
                status,completed_bytes,total_bytes,outcome_code,outcome_message)
                VALUES(?,?,41,'A0000000-0000-4000-8000-00000000000B','','Été # %2F.zip',
                NULL,'archives/RAW.zip',?,'pending',9,10,NULL,'')",
                rusqlite::params![7+ordinal*24,id,"Ab".repeat(32)])?;
        }
        Ok(())
    })?.wait()?)?;
    Ok(())
}

fn snapshot(
    store: &dyn OperationTerminalStore,
    id: &str,
) -> Result<Box<OperationSnapshot>, Box<dyn Error>> {
    match store.snapshot(id) {
        SnapshotReply::Found(value) => Ok(value),
        other => Err(io::Error::other(format!("snapshot: {other:?}")).into()),
    }
}

fn complete(source: &OperationSnapshot, index: usize) -> CompleteItemCommand {
    CompleteItemCommand {
        generation: GENERATION,
        operation: source.operation.immutable_scope(),
        item: source.items[index].immutable_scope(),
        story_id: 42,
        state: ItemState::Succeeded,
        completed_bytes: 1,
        outcome: RawOutcome {
            code: Some(String::new()),
            message: Some(" ".into()),
        },
    }
}

fn finish(source: &OperationSnapshot) -> FinishCommand {
    FinishCommand {
        generation: GENERATION,
        operation: source.operation.immutable_scope(),
        state: OperationState::Succeeded,
        outcome: RawOutcome {
            code: Some(String::new()),
            message: Some(" raw ".into()),
        },
        finished_at: "EXACT finished+02".into(),
    }
}

fn interrupt(source: &OperationSnapshot) -> InterruptCommand {
    InterruptCommand {
        generation: GENERATION,
        operation: source.operation.immutable_scope(),
        code: " ".into(),
        message: " raw reason ".into(),
        finished_at: "RAW interrupted".into(),
    }
}

#[test]
fn terminal_sql_raw_snapshot_preserves_non_uuid_null_scope_and_row_order() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 2)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let source = snapshot(&store, &id)?;
        assert!(source.operation.identity.typed_id().is_err());
        assert_eq!(source.operation.identity.started_at.as_deref(), Some(""));
        assert_eq!(source.operation.identity.finished_at, None);
        assert_eq!(source.operation.outcome.code.as_deref(), Some(""));
        assert_eq!(source.operation.outcome.message, None);
        assert_eq!(
            source.operation.export_scope.group_ids.as_deref(),
            Some("[9, 2]")
        );
        assert_eq!(
            source.operation.export_scope.group_names.as_deref(),
            Some("[\"Été\",\"Été\"] ")
        );
        assert_eq!(
            source
                .items
                .iter()
                .map(|i| i.identity.id)
                .collect::<Vec<_>>(),
            [7, 31]
        );
        assert_eq!(source.items[0].story_title.as_deref(), Some(""));
        assert_eq!(source.items[0].output_name, None);
        assert_eq!(source.items[0].identity.story_id, None);
        assert_eq!(source.items[0].identity.resolved_story_id, Some(41));
        assert_eq!(source.items[0].source_name, "Été # %2F.zip");
        assert_eq!(
            source.items[0].archive_sha256.as_deref(),
            Some("Ab".repeat(32).as_str())
        );
        assert!(matches!(store.snapshot("missing"), SnapshotReply::Missing));
        let command = CommandWitness::CompleteItem(Box::new(complete(&source, 0)));
        assert!(command_bytes(&command) > std::mem::size_of_val(&command));
        assert!(
            snapshot_bytes(&source)
                > std::mem::size_of_val(source.as_ref())
                    + source.items.capacity() * std::mem::size_of::<OperationItem>()
        );
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_completion_is_atomic_exact_and_idempotent_across_siblings() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 2)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let original = snapshot(&store, &id)?;
        let first = complete(&original, 0);
        let second = complete(&original, 1);
        let witness = CommandWitness::CompleteItem(Box::new(first.clone()));
        assert!(matches!(
            store.readback(&witness).disposition,
            ReadbackDisposition::ProvenNotApplied(_)
        ));
        assert!(matches!(
            store.complete_item(&first).disposition,
            AttemptDisposition::Committed { .. }
        ));
        assert!(matches!(
            store.complete_item(&second).disposition,
            AttemptDisposition::Committed { .. }
        ));
        // The second sibling changed the parent count; matching is not old+1.
        assert!(
            matches!(store.readback(&witness).disposition, ReadbackDisposition::MatchesApplied(ref v) if v.operation.counts.completed==2 && v.siblings.terminal==2)
        );
        assert!(matches!(
            store.complete_item(&first).disposition,
            AttemptDisposition::AlreadyMatching(_)
        ));
        let saved = snapshot(&store, &id)?;
        assert_eq!(
            saved.operation.counts,
            Counts {
                completed: 2,
                total: 2
            }
        );
        assert_eq!(saved.items[0].identity.story_id, Some(42));
        assert_eq!(saved.items[0].identity.resolved_story_id, Some(41));
        assert_eq!(saved.items[0].progress.completed, 1); // May decrease at completion.
        assert_eq!(saved.items[0].outcome.code, None);
        assert_eq!(saved.items[0].outcome.message.as_deref(), Some(" "));
        assert_eq!(
            saved.items[0].immutable_scope(),
            original.items[0].immutable_scope()
        );
        assert_eq!(first.outcome.code.as_deref(), Some(""));
        let mut wrong = first.clone();
        wrong.completed_bytes = 2;
        assert!(matches!(
            store.complete_item(&wrong).disposition,
            AttemptDisposition::TerminalConflict(_)
        ));
        assert_eq!(snapshot(&store, &id)?.operation.counts.completed, 2);
        let done = finish(&saved);
        assert!(matches!(
            store.finish(&done).disposition,
            AttemptDisposition::Committed { .. }
        ));
        assert!(matches!(
            store.finish(&done).disposition,
            AttemptDisposition::AlreadyMatching(_)
        ));
        let mut different = done.clone();
        different.state = OperationState::Failed;
        assert!(
            matches!(store.finish(&different).disposition, AttemptDisposition::TerminalConflict(ref v) if v.operation.state==OperationState::Succeeded)
        );
        assert_eq!(
            snapshot(&store, &id)?
                .operation
                .identity
                .finished_at
                .as_deref(),
            Some("EXACT finished+02")
        );
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_second_guard_failure_rolls_back_first_item_mutation_and_trigger() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    committed(db.repair_transaction(|tx| {
        // Actual SQLite trigger forces the parent guard to fail AFTER the item
        // UPDATE. Returning Ok(refusal) would commit both injected mutations.
        tx.execute_batch("CREATE TRIGGER stop_parent_after_item AFTER UPDATE OF status ON file_operation_items
            BEGIN UPDATE file_operations SET status='failed' WHERE id=NEW.operation_id; END;")?;
        Ok(())
    })?.wait()?)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        let command = complete(&before, 0);
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::ProvenNotCommitted
        ));
        assert_eq!(snapshot(&store, &id)?, before);
        assert!(matches!(
            store.interrupt(&interrupt(&before)).disposition,
            AttemptDisposition::ProvenNotCommitted
        ));
        assert_eq!(snapshot(&store, &id)?, before);
        assert!(matches!(
            store
                .readback(&CommandWitness::CompleteItem(Box::new(command)))
                .disposition,
            ReadbackDisposition::ProvenNotApplied(_)
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_finish_and_interrupt_preserve_legacy_zero_totals() -> TestResult {
    for state in [
        OperationState::Succeeded,
        OperationState::PartiallySucceeded,
        OperationState::Failed,
        OperationState::Cancelled,
    ] {
        let (_roots, library) = owned_library()?;
        let mut db = Database::open_isolated(&library)?;
        let id = "Ab".repeat(18);
        seed(&db, &id, 0)?;
        {
            let store = TerminalRepository::new(&db, GENERATION);
            let source = snapshot(&store, &id)?;
            let mut command = finish(&source);
            command.state = state;
            assert!(matches!(
                store.finish(&command).disposition,
                AttemptDisposition::Committed { .. }
            ));
            let saved = snapshot(&store, &id)?;
            assert_eq!(saved.operation.state, state);
            assert_eq!(
                saved.operation.counts,
                Counts {
                    completed: 0,
                    total: 0
                }
            );
            assert_eq!(saved.operation.outcome.code, None);
            assert_eq!(saved.operation.outcome.message.as_deref(), Some(" raw "));
        }
        db.close()?;
    }
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 0)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let command = interrupt(&snapshot(&store, &id)?);
        assert!(matches!(
            store.interrupt(&command).disposition,
            AttemptDisposition::Committed { .. }
        ));
        assert!(
            matches!(store.readback(&CommandWitness::Interrupt(Box::new(command))).disposition,ReadbackDisposition::MatchesApplied(ref v) if v.siblings.items==0 && v.siblings.terminal==0)
        );
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_interrupt_preserves_terminal_items_and_all_bytes() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 2)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        assert!(matches!(
            store.complete_item(&complete(&before, 0)).disposition,
            AttemptDisposition::Committed { .. }
        ));
        let before = snapshot(&store, &id)?;
        let command = interrupt(&before);
        assert!(matches!(
            store.interrupt(&command).disposition,
            AttemptDisposition::Committed { .. }
        ));
        let saved = snapshot(&store, &id)?;
        assert_eq!(saved.items[0], before.items[0]);
        assert_eq!(saved.items[1].progress, before.items[1].progress);
        assert_eq!(saved.items[1].state, ItemState::Cancelled);
        assert_eq!(saved.items[1].outcome.code.as_deref(), Some(" "));
        assert_eq!(
            saved.operation.counts,
            Counts {
                completed: 2,
                total: 2
            }
        );
        assert_eq!(
            saved.operation.identity.finished_at.as_deref(),
            Some("RAW interrupted")
        );
        assert!(matches!(
            store.interrupt(&command).disposition,
            AttemptDisposition::AlreadyMatching(_)
        ));
        let mut different = command;
        different.message = "different actual reason".into();
        assert!(matches!(
            store.interrupt(&different).disposition,
            AttemptDisposition::TerminalConflict(_)
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_finish_guard_rolls_back_and_invalid_commands_never_admit() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        assert!(matches!(
            store.finish(&finish(&before)).disposition,
            AttemptDisposition::ProvenNotCommitted
        ));
        assert_eq!(snapshot(&store, &id)?, before);
        let mut command = complete(&before, 0);
        command.state = ItemState::Pending;
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::InvalidCommand)
        ));
        command = complete(&before, 0);
        command.completed_bytes = 11;
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::InvalidCommand)
        ));
        command = complete(&before, 0);
        command.generation += 1;
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::WrongGeneration)
        ));
        let mut reason = interrupt(&before);
        reason.code.clear();
        assert!(matches!(
            store.interrupt(&reason).disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::InvalidCommand)
        ));
        let mut done = finish(&before);
        done.state = OperationState::Interrupted;
        assert!(matches!(
            store.finish(&done).disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::InvalidCommand)
        ));
        assert_eq!(snapshot(&store, &id)?, before);
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_wrong_scope_missing_and_contradictory_counts_never_ack() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 2)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        let original = complete(&before, 0);
        let mut changed = original.clone();
        changed.item.resolved_story_id = None;
        assert!(matches!(
            store
                .readback(&CommandWitness::CompleteItem(Box::new(changed.clone())))
                .disposition,
            ReadbackDisposition::Unknown(UnknownOutcome::Contradictory)
        ));
        assert!(matches!(
            store.complete_item(&changed).disposition,
            AttemptDisposition::Unknown(UnknownOutcome::Contradictory)
        ));
        changed = original.clone();
        changed.item.id = 99;
        assert!(matches!(
            store
                .readback(&CommandWitness::CompleteItem(Box::new(changed)))
                .disposition,
            ReadbackDisposition::Unknown(UnknownOutcome::Missing)
        ));
        changed = original.clone();
        changed.operation.destination = None;
        assert!(matches!(
            store
                .readback(&CommandWitness::CompleteItem(Box::new(changed)))
                .disposition,
            ReadbackDisposition::Unknown(UnknownOutcome::Contradictory)
        ));
        let mut absent = finish(&before);
        absent.operation.identity.id = "Cd".repeat(18);
        assert!(matches!(
            store
                .readback(&CommandWitness::Finish(Box::new(absent)))
                .disposition,
            ReadbackDisposition::Unknown(UnknownOutcome::Missing)
        ));
        assert_eq!(snapshot(&store, &id)?, before);
        let raw = id.clone();
        committed(
            db.repair_transaction(move |tx| {
                // Legal schema counts, but not a coherent count of actual siblings.
                tx.execute(
                    "UPDATE file_operations SET completed_items=1 WHERE id=?",
                    [raw],
                )?;
                Ok(())
            })?
            .wait()?,
        )?;
        assert!(matches!(
            store
                .readback(&CommandWitness::CompleteItem(Box::new(original)))
                .disposition,
            ReadbackDisposition::Unknown(UnknownOutcome::Contradictory)
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_commit_error_is_unknown_then_reserved_fresh_readback_proves_state() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    committed(db.repair_transaction(|tx| {
        tx.execute_batch("CREATE TABLE deferred_terminal_probe(value INTEGER REFERENCES stories(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER fail_terminal_commit AFTER UPDATE OF status ON file_operation_items
            BEGIN INSERT INTO deferred_terminal_probe VALUES(999999); END;")?;
        Ok(())
    })?.wait()?)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        let command = complete(&before, 0);
        let unknown = store.complete_item(&command);
        assert!(matches!(
            unknown.disposition,
            AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost)
        ));
        assert!(unknown.diagnostic.is_some());
        // All four OWNED critical slots include replies not yet acknowledged.
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(db.repair_transaction(|_| Ok(()))?);
        }
        let refused = store.readback(&unknown.witness);
        assert!(matches!(
            refused.disposition,
            ReadbackDisposition::NotAdmitted(NonAdmission::QueueFull)
        ));
        assert!(matches!(
            unknown.disposition,
            AttemptDisposition::Unknown(_)
        )); // Earlier unknown survives.
        for pending in held {
            committed(pending.wait()?)?;
        }
        let proof = store.readback(&unknown.witness);
        assert!(matches!(
            proof.disposition,
            ReadbackDisposition::ProvenNotApplied(_)
        ));
        assert_eq!(snapshot(&store, &id)?, before);
        // Readback did not reset the Database's ordinary-write uncertainty fence.
        assert!(matches!(
            db.transaction(Default::default(), |_| Ok(())),
            Err(StorageError::RuntimeFenced)
        ));
        committed(
            db.repair_transaction(|tx| {
                tx.execute_batch("DROP TRIGGER fail_terminal_commit")?;
                Ok(())
            })?
            .wait()?,
        )?;
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::Committed { .. }
        ));
        assert!(
            matches!(store.readback(&unknown.witness).disposition,ReadbackDisposition::MatchesApplied(ref v) if v.operation.counts.completed==1)
        );
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::AlreadyMatching(_)
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_writer_barrier_matches_unacknowledged_prior_commit_without_recount() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let command = complete(&snapshot(&store, &id)?, 0);
        let fields = command.clone();
        let prior = db.repair_transaction(move |tx| {
            tx.execute(
                "UPDATE file_operation_items SET story_id=?,status=?,outcome_code=NULLIF(?,''),
                outcome_message=NULLIF(?,''),completed_bytes=? WHERE operation_id=? AND id=?",
                rusqlite::params![
                    fields.stored_story_id(),
                    fields.state.as_persisted(),
                    fields.outcome.code.as_deref().unwrap_or(""),
                    fields.outcome.message.as_deref().unwrap_or(""),
                    fields.completed_bytes,
                    fields.operation.identity.id,
                    fields.item.id
                ],
            )?;
            tx.execute(
                "UPDATE file_operations SET completed_items=1 WHERE id=?",
                [fields.operation.identity.id],
            )?;
            Ok(())
        })?;
        // Do NOT wait on the prior acknowledgement before the barrier call.
        let proof = store.readback(&CommandWitness::CompleteItem(Box::new(command.clone())));
        assert!(
            matches!(proof.disposition,ReadbackDisposition::MatchesApplied(ref v) if v.operation.counts.completed==1)
        );
        committed(prior.wait()?)?;
        assert!(matches!(
            store.complete_item(&command).disposition,
            AttemptDisposition::AlreadyMatching(_)
        ));
        assert_eq!(snapshot(&store, &id)?.operation.counts.completed, 1);
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_critical_refusal_retains_original_report_and_variable_command_bytes() -> TestResult
{
    struct Report {
        command: CompleteItemCommand,
        facts: Vec<String>,
    }
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let report = Report {
            command: complete(&snapshot(&store, &id)?, 0),
            facts: vec!["real returned execution fact".into()],
        };
        let original = CommandWitness::CompleteItem(Box::new(report.command.clone()));
        let original_bytes = command_bytes(&original);
        let mut larger = original.clone();
        if let CommandWitness::CompleteItem(c) = &mut larger {
            c.item.source_name.push_str(&"É".repeat(1000));
        }
        assert!(command_bytes(&larger) > original_bytes + 1000);
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(db.repair_transaction(|_| Ok(()))?);
        }
        let refusal = store.complete_item(&report.command);
        assert!(matches!(
            refusal.disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::QueueFull)
        ));
        assert_eq!(refusal.witness, original);
        assert_eq!(report.facts, ["real returned execution fact"]);
        assert_eq!(report.command.story_id, 42);
        for pending in held {
            committed(pending.wait()?)?;
        }
        assert_eq!(snapshot(&store, &id)?.operation.counts.completed, 0);
        assert!(matches!(
            store.complete_item(&report.command).disposition,
            AttemptDisposition::Committed { .. }
        ));
    }
    db.close()?;
    Ok(())
}

#[test]
fn terminal_sql_proven_commit_survives_postcommit_snapshot_refusal_and_drains() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 1)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let command = interrupt(&snapshot(&store, &id)?);
        db.stop_ordinary()?;
        let reply = store.interrupt(&command);
        assert!(matches!(
            reply.disposition,
            AttemptDisposition::Committed { view: None }
        ));
        assert!(reply.diagnostic.is_some());
        assert!(matches!(
            store.snapshot(&id),
            SnapshotReply::NotAdmitted {
                reason: NonAdmission::IntakeClosed,
                ..
            }
        ));
        let proof = store.readback(&reply.witness);
        assert!(
            matches!(proof.disposition,ReadbackDisposition::MatchesApplied(ref v) if v.operation.state==OperationState::Interrupted)
        );
    }
    // Only after synchronous report/readback owners settle does the DB close.
    db.close()?;
    library.lease().revalidate()?; // Actual cooperative lease outlives DB joins.
    Ok(())
}

#[test]
fn terminal_sql_two_callers_complete_distinct_siblings_on_one_writer() -> TestResult {
    let (_roots, library) = owned_library()?;
    let mut db = Database::open_isolated(&library)?;
    let id = "Ab".repeat(18);
    seed(&db, &id, 2)?;
    {
        let store = TerminalRepository::new(&db, GENERATION);
        let before = snapshot(&store, &id)?;
        let first = complete(&before, 0);
        let second = complete(&before, 1);
        std::thread::scope(|scope| -> TestResult {
            let a = scope.spawn(|| store.complete_item(&first));
            let b = scope.spawn(|| store.complete_item(&second));
            for handle in [a, b] {
                let reply = handle
                    .join()
                    .map_err(|_| io::Error::other("scoped caller panicked"))?;
                assert!(matches!(
                    reply.disposition,
                    AttemptDisposition::Committed { .. }
                ));
            }
            Ok(())
        })?;
        let proof = store.readback(&CommandWitness::CompleteItem(Box::new(first)));
        assert!(
            matches!(proof.disposition,ReadbackDisposition::MatchesApplied(ref v) if v.operation.counts.completed==2 && v.siblings.terminal==2)
        );
        assert_eq!(snapshot(&store, &id)?.operation.counts.completed, 2);
    }
    db.close()?;
    Ok(())
}
