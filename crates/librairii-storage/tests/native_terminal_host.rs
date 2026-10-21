//! TEMP N3 composition proof only; this file must never enter a shipping branch.
//! H borrows the concrete R4 repository on the real Native-backed Database.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use librairii_app::{
    jobs::scoped::{
        self, Callbacks, Configuration, Delivery, Request, Retained, RunControl, WorkReply,
    },
    ports::operations::*,
};
use librairii_core::{
    OperationState,
    operations::{Counts, ItemState, RawOutcome},
};
use librairii_storage::{
    StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::{Database, TransactionOutcome, WorkCancellation},
    native::{NativeCloseFailure, NativeLibrary},
    operations_repository::TerminalRepository,
    platform::lock::LIBRARY_LOCK_FILE,
    profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations},
};
use librairii_testkit::TestRoots;
use std::{
    collections::VecDeque,
    error::Error,
    fs, io,
    path::Path,
    sync::{
        Barrier, Mutex, MutexGuard,
        atomic::{AtomicUsize, Ordering},
    },
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const GENERATION: u64 = 17;
const NATIVE_STATE: &str = ".librairii-native-state.json";
const MANIFEST: &[u8] =
    include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|error| error.into_inner())
}

fn assert_sync<T: Sync>(_: &T) {}

fn committed<T>(outcome: TransactionOutcome<T>) -> TestResult<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error.into()),
    }
}

fn prepare(roots: &TestRoots) -> TestResult<(IndependentCopy, ProfileLocations)> {
    let source = roots.library().join("legacy-source");
    fs::create_dir(&source)?;
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    let manifest: serde_json::Value = serde_json::from_slice(MANIFEST)?;
    for entry in manifest.as_array().ok_or("manifest array")? {
        let path = entry["path"].as_str().ok_or("manifest path")?;
        if entry["directory"].as_bool() == Some(true) {
            fs::create_dir(source.join(path))?;
        } else {
            fs::copy(corpus.join(path), source.join(path))?;
        }
    }
    let profile = Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("source UTF8")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    };
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
        ClosedManifest::decode(&serde_json::to_vec(&manifest)?)?,
        &WorkCancellation::default(),
    )?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy context")?;
    fs::create_dir_all(legacy)?;
    fs::write(legacy.join("settings.json"), serde_json::to_vec(&profile)?)?;
    let copy = input.copy_to_new_root(
        &locations.data.join("owned Éte # %2F"),
        &WorkCancellation::default(),
    )?;
    Ok((copy, locations))
}

fn state(root: &Path) -> TestResult<serde_json::Value> {
    Ok(serde_json::from_slice(&fs::read(root.join(NATIVE_STATE))?)?)
}

fn require_root_lock(root: &Path) -> TestResult {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    assert!(matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)));
    Ok(())
}

fn require_root_released(root: &Path) -> TestResult {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    file.try_lock()?;
    file.unlock()?;
    Ok(())
}

fn seed(db: &Database<'_>, rows: &[(String, i64)]) -> TestResult {
    let rows = rows.to_vec();
    committed(db.repair_transaction(move |tx| {
        tx.execute("INSERT OR IGNORE INTO stories(id,uuid) VALUES(42,'A0000000-0000-4000-8000-000000000042')", [])?;
        for (id, item) in rows {
            tx.execute("INSERT INTO file_operations(id,kind,status,total_items,total_bytes,created_at,
                started_at,export_source_type,export_source_shelf_ids,export_source_shelf_names,
                export_destination,error_code,error_message)
                VALUES(?,'import','queued',1,10,'raw CREATED+02','','shelves','[9, 2]',
                '[\"Été\",\"Été\"] ',' /raw # %2F destination ','',NULL)", [&id])?;
            tx.execute("INSERT INTO file_operation_items(id,operation_id,resolved_story_id,story_uuid,
                story_title,source_name,output_name,archive_relative_path,archive_sha256,
                status,completed_bytes,total_bytes,outcome_code,outcome_message)
                VALUES(?,?,41,'A0000000-0000-4000-8000-00000000000B','','Été # %2F.zip',
                NULL,'archives/RAW.zip',?,'pending',9,10,NULL,'')",
                rusqlite::params![item,id,"Ab".repeat(32)])?;
        }
        Ok(())
    })?.wait()?)?;
    Ok(())
}

fn snapshot(real: &dyn OperationTerminalStore, id: &str) -> TestResult<Box<OperationSnapshot>> {
    match real.snapshot(id) {
        SnapshotReply::Found(view) => Ok(view),
        other => Err(io::Error::other(format!("real snapshot unavailable: {other:?}")).into()),
    }
}

fn request(real: &dyn OperationTerminalStore, id: &str) -> TestResult<Request> {
    let saved = snapshot(real, id)?;
    let item = saved.items.first().ok_or("seeded item missing")?;
    let operation = saved.operation.immutable_scope();
    Ok(Request {
        complete: CompleteItemCommand {
            generation: GENERATION,
            operation: operation.clone(),
            item: item.immutable_scope(),
            story_id: if item.identity.id == 0 { 0 } else { 42 },
            state: ItemState::Succeeded,
            completed_bytes: 1,
            outcome: RawOutcome {
                code: Some(String::new()),
                message: Some(" raw item cause ".into()),
            },
        },
        finish: Some(FinishCommand {
            generation: GENERATION,
            operation: operation.clone(),
            state: OperationState::Succeeded,
            outcome: RawOutcome {
                code: Some(String::new()),
                message: Some(" raw finish ".into()),
            },
            finished_at: "RAW planned finish # %2F".into(),
        }),
        interrupt: InterruptCommand {
            generation: GENERATION,
            operation,
            code: " ".into(),
            message: " raw supplied interruption ".into(),
            finished_at: "RAW supplied interrupt".into(),
        },
    })
}

fn work(request: &Request) -> WorkReply {
    WorkReply::Completed {
        command: Box::new(request.complete.clone()),
        diagnostic: Some(StoreDiagnostic::new(io::Error::other(
            "independent controlled work cause",
        ))),
    }
}

fn copy_request(value: &Request) -> Request {
    Request {
        complete: value.complete.clone(),
        finish: value.finish.clone(),
        interrupt: value.interrupt.clone(),
    }
}

fn check_request(actual: &Request, expected: &Request) {
    assert_eq!(actual.complete, expected.complete);
    assert_eq!(actual.finish, expected.finish);
    assert_eq!(actual.interrupt, expected.interrupt);
}

#[derive(Debug)]
struct AcceptedReport {
    request: Request,
    execution: CompleteItemCommand,
    witness: CommandWitness,
    view: Box<TerminalView>,
    work_cause: String,
}

fn accept(reports: &Mutex<Vec<AcceptedReport>>, owner: &Retained) -> Delivery {
    assert!(!owner.unknown && !owner.different_terminal);
    assert!(owner.complete_acknowledged && !owner.work_panicked_before_db);
    let Some(execution) = owner.execution.as_ref() else {
        return Delivery::Retain;
    };
    let Some(view) = owner.view.as_ref() else {
        return Delivery::Retain;
    };
    let Some(cause) = owner.work_error.as_ref() else {
        return Delivery::Retain;
    };
    assert_eq!(execution, &owner.request.complete);
    assert_eq!(view.operation.state, OperationState::Succeeded);
    assert_eq!(
        view.operation.counts,
        Counts {
            completed: 1,
            total: 1
        }
    );
    lock(reports).push(AcceptedReport {
        request: copy_request(&owner.request),
        execution: execution.clone(),
        witness: owner.witness.clone(),
        view: view.clone(),
        work_cause: cause.to_string(),
    });
    Delivery::Accepted
}

fn fresh(
    real: &dyn OperationTerminalStore,
    witness: &CommandWitness,
) -> TestResult<Box<TerminalView>> {
    let reply = real.readback(witness);
    assert_eq!(&reply.witness, witness);
    assert!(
        reply.diagnostic.is_none(),
        "real readback cause: {:?}",
        reply.diagnostic
    );
    match reply.disposition {
        ReadbackDisposition::MatchesApplied(view) => Ok(view),
        other => Err(io::Error::other(format!("real typed command not applied: {other:?}")).into()),
    }
}

fn check_report(real: &dyn OperationTerminalStore, report: &AcceptedReport) -> TestResult {
    let complete = &report.execution;
    let finish = report
        .request
        .finish
        .as_ref()
        .ok_or("supplied Finish lost")?;
    assert_eq!(complete, &report.request.complete);
    assert_eq!(
        report.witness,
        CommandWitness::Finish(Box::new(finish.clone()))
    );
    assert_eq!(report.work_cause, "independent controlled work cause");
    let saved = fresh(real, &report.witness)?;
    assert_eq!(saved.operation.immutable_scope(), complete.operation);
    assert_eq!(
        saved.operation.identity.finished_at.as_deref(),
        Some(finish.finished_at.as_str())
    );
    assert_eq!(saved.operation.outcome, finish.stored_outcome());
    assert_eq!(
        saved.operation.counts,
        Counts {
            completed: 1,
            total: 1
        }
    );
    assert_eq!(saved.siblings, report.view.siblings);
    assert_eq!(saved.siblings.items, 1);
    assert_eq!(saved.siblings.terminal, 1);
    // Finish readback intentionally carries no target item. Read the Complete
    // witness in a SECOND coherent view to prove the SAME item was not counted twice.
    let item_view = fresh(
        real,
        &CommandWitness::CompleteItem(Box::new(complete.clone())),
    )?;
    let item = item_view
        .item
        .as_ref()
        .ok_or("complete readback item lost")?;
    assert_eq!(item.immutable_scope(), complete.item);
    assert_eq!(item.identity.story_id, complete.stored_story_id());
    assert_eq!(item.identity.resolved_story_id, Some(41));
    assert_eq!(item.state, complete.state);
    assert_eq!(item.progress.completed, complete.completed_bytes);
    assert_eq!(item.outcome, complete.stored_outcome());
    assert!(saved.operation.identity.typed_id().is_err());
    Ok(())
}

#[test]
fn native_host_two_occupied_workers_settle_report_then_close_and_reopen_next_epoch() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots)?;
    let root = copy.root().to_owned();
    let source = Path::new(&copy.source_profile().profile().root).join("db/librairii.sqlite3");
    let original_source = fs::read(&source)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    assert_sync(&store);
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|_| {
                io::Error::other("native constructor failed; negative fixture teardown")
            })?;
    assert_sync(&native);
    let mut db = Database::open_native(&native)?;
    assert_sync(&db);
    let rows = vec![
        (format!("{:.<36}", "native raw ZERO # %2F"), 0),
        (format!("{:.<36}", "Native RAW -seven"), -7),
    ];
    seed(&db, &rows)?;
    {
        let real = TerminalRepository::new(&db, GENERATION);
        assert_sync(&real);
        let plans = rows
            .iter()
            .map(|(id, _)| request(&real, id))
            .collect::<TestResult<Vec<_>>>()?;
        let expected: Vec<_> = plans.iter().map(copy_request).collect();
        let queue = Mutex::new(VecDeque::from(plans));
        let source = || lock(&queue).pop_front();
        let both_occupied = Barrier::new(2);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let calls = AtomicUsize::new(0);
        let controlled_work = |plan: &Request, control: &RunControl| {
            calls.fetch_add(1, Ordering::AcqRel);
            let occupied = active.fetch_add(1, Ordering::AcqRel) + 1;
            maximum.fetch_max(occupied, Ordering::AcqRel);
            both_occupied.wait();
            // Both accepted requests already have an occupied worker; intake
            // stops while those original owners continue required reporting.
            control.stop_intake();
            let result = work(plan);
            active.fetch_sub(1, Ordering::AcqRel);
            result
        };
        let reports = Mutex::new(Vec::new());
        let deliver = |owner: &Retained| accept(&reports, owner);
        let control = RunControl::default();
        let outcome = scoped::run(
            Configuration::new(2).ok_or("W2 configuration")?,
            &Callbacks {
                store: &real,
                source: &source,
                work: &controlled_work,
                deliver: &deliver,
            },
            &control,
        );
        assert!(outcome.positively_settled() && outcome.measurements.joined);
        assert_eq!(outcome.measurements.settled, 2);
        assert_eq!(maximum.load(Ordering::Acquire), 2);
        assert_eq!(calls.load(Ordering::Acquire), 2);
        assert_eq!(active.load(Ordering::Acquire), 0);
        assert!(outcome.measurements.max_retained <= 2);
        assert!(outcome.measurements.max_callback_owners <= 3);
        assert!(outcome.measurements.max_dispatched_handles <= 7);
        assert!(outcome.measurements.max_command_bytes > 0);
        assert!(outcome.measurements.max_store_field_copy_budget_bytes > 0);
        control.cancel_work();
        let accepted = lock(&reports);
        assert_eq!(accepted.len(), 2);
        for plan in &expected {
            let report = accepted
                .iter()
                .find(|report| report.request.complete == plan.complete)
                .ok_or("actual report absent")?;
            check_request(&report.request, plan);
            check_report(&real, report)?;
        }
        // Accepted sink values and H's fully settled result remain owned until
        // every exact Complete+Finish/readback assertion above has succeeded.
    }
    require_root_lock(&root)?;
    assert_eq!(state(&root)?["state"], "active");
    db.close()?;
    drop(db);
    require_root_lock(&root)?;
    let finished = native.finish();
    assert!(
        finished.is_ok(),
        "unexpected retained native close failure: {finished:?}"
    );
    drop(finished);
    require_root_released(&root)?;
    assert_eq!(state(&root)?["state"], "closed");
    assert_eq!(state(&root)?["epoch"], 1);
    // The profile store still owns its own OS lease after library finalization.
    assert!(NativeProfileStore::open_native(&locations.native).is_err());
    let native = NativeLibrary::reopen(&store, &locations, &WorkCancellation::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(state(&root)?["epoch"], 2);
    {
        let real = TerminalRepository::new(&db, GENERATION);
        for (id, signed_item) in &rows {
            let saved = snapshot(&real, id)?;
            assert_eq!(saved.operation.state, OperationState::Succeeded);
            assert_eq!(
                saved.operation.counts,
                Counts {
                    completed: 1,
                    total: 1
                }
            );
            assert_eq!(saved.items[0].identity.id, *signed_item);
            assert_eq!(saved.items[0].progress.completed, 1);
        }
    }
    db.close()?;
    drop(db);
    let finished = native.finish();
    assert!(finished.is_ok(), "next-epoch close failed: {finished:?}");
    drop(finished);
    assert_eq!(state(&root)?["state"], "closed");
    require_root_released(&root)?;
    assert_eq!(fs::read(&source)?, original_source);
    drop(store);
    let reopened_profile = NativeProfileStore::open_native(&locations.native)?;
    assert!(reopened_profile.load()?.is_some());
    Ok(())
}

#[test]
fn native_host_actual_lost_pending_ack_reports_truth_but_retains_typed_close_failure() -> TestResult
{
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    // This typed owner is intentionally OUTSIDE the inner Database/Native
    // scope and INSIDE the borrowed profile lifetime. It is never stringified,
    // made 'static, or dropped as a claim of uncertainty settlement.
    let retained_failure: Box<NativeCloseFailure<'_>>;
    let accepted_facts: Vec<AcceptedReport>;
    {
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|_| {
                    io::Error::other("native constructor failed; negative fixture teardown")
                })?;
        let mut db = Database::open_native(&native)?;
        let id = format!("{:.<36}", "native lost ACK raw operation");
        seed(&db, &[(id.clone(), -7)])?;
        {
            let real = TerminalRepository::new(&db, GENERATION);
            let plan = request(&real, &id)?;
            // Controlled SQL-command fixture: perform the SAME typed Complete
            // mutation on the SAME repair writer, then lose its REAL Pending
            // acknowledgement. No facade manufactures a durability reply.
            let command = plan.complete.clone();
            let pending = db.repair_transaction(move |tx| {
                let item = tx.execute(
                    "UPDATE file_operation_items SET story_id=?,status=?,
                    outcome_code=NULLIF(?,''),outcome_message=NULLIF(?,''),completed_bytes=?
                    WHERE id=? AND operation_id=? AND status IN ('pending','running')",
                    rusqlite::params![
                        command.stored_story_id(),
                        command.state.as_persisted(),
                        command.outcome.code.as_deref().unwrap_or(""),
                        command.outcome.message.as_deref().unwrap_or(""),
                        command.completed_bytes,
                        command.item.id,
                        command.operation.identity.id
                    ],
                )?;
                let parent = tx.execute(
                    "UPDATE file_operations SET completed_items=completed_items+1
                    WHERE id=? AND status IN ('queued','running') AND completed_items<total_items",
                    [&command.operation.identity.id],
                )?;
                if item != 1 || parent != 1 {
                    return Err(StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
                }
                Ok(())
            })?;
            drop(pending); // Actual sticky native fence, never rollback/cancel proof.
            let completed_view = fresh(
                &real,
                &CommandWitness::CompleteItem(Box::new(plan.complete.clone())),
            )?;
            assert_eq!(
                completed_view.operation.counts,
                Counts {
                    completed: 1,
                    total: 1
                }
            );
            assert_eq!(completed_view.operation.state, OperationState::Queued);
            let input = Mutex::new(Some(copy_request(&plan)));
            let source = || lock(&input).take();
            let calls = AtomicUsize::new(0);
            let controlled_work = |request: &Request, _: &RunControl| {
                calls.fetch_add(1, Ordering::AcqRel);
                work(request)
            };
            let reports = Mutex::new(Vec::new());
            let deliver = |owner: &Retained| accept(&reports, owner);
            let control = RunControl::default();
            let outcome = scoped::run(
                Configuration::new(1).ok_or("W1 configuration")?,
                &Callbacks {
                    store: &real,
                    source: &source,
                    work: &controlled_work,
                    deliver: &deliver,
                },
                &control,
            );
            assert!(outcome.positively_settled() && outcome.measurements.joined);
            assert_eq!(outcome.measurements.settled, 1);
            assert_eq!(calls.load(Ordering::Acquire), 1);
            control.stop_intake();
            control.cancel_work();
            let accepted = lock(&reports);
            assert_eq!(accepted.len(), 1);
            check_request(&accepted[0].request, &plan);
            check_report(&real, &accepted[0])?;
            drop(accepted);
            // Move the accepted owned typed facts outside the Native/DB scope;
            // they remain physically alongside the retained close failure.
            accepted_facts = std::mem::take(&mut *lock(&reports));
            // Actual domain truth/reporting is settled; the independent native
            // Pending-loss fence remains and cannot be cleared by this result.
        }
        db.close()?;
        drop(db);
        retained_failure = match native.finish() {
            Err(failure) => failure,
            Ok(()) => {
                return Err(io::Error::other("lost Pending acknowledgement minted Closed").into());
            }
        };
    }
    assert!(matches!(
        retained_failure.storage_error(),
        StorageError::RuntimeFenced
    ));
    assert_eq!(accepted_facts.len(), 1);
    assert_eq!(accepted_facts[0].execution.item.id, -7);
    assert_eq!(
        accepted_facts[0].view.operation.state,
        OperationState::Succeeded
    );
    assert_eq!(
        accepted_facts[0].work_cause,
        "independent controlled work cause"
    );
    require_root_lock(&root)?;
    let retained_record = fs::read(root.join(NATIVE_STATE))?;
    assert_eq!(state(&root)?["state"], "active");
    assert_eq!(state(&root)?["epoch"], 1);
    assert!(
        retained_failure
            .retained_library()
            .revalidate(&WorkCancellation::default())
            .is_err()
    );
    assert!(store.update(|_| Ok(())).is_err());
    assert!(NativeProfileStore::open_native(&locations.native).is_err());
    assert!(NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()).is_err());
    assert_eq!(fs::read(root.join(NATIVE_STATE))?, retained_record);
    require_root_lock(&root)?;
    // NEGATIVE FIXTURE TEARDOWN ONLY: these drops remove the temporary fixture;
    // they are not a repair, Closed receipt, restart permission, or settlement
    // of the retained native fence. The typed failure lives through all checks.
    drop(accepted_facts);
    drop(retained_failure);
    drop(store);
    Ok(())
}
