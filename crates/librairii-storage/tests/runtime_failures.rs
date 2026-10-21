#![cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "support/runtime.rs"]
mod runtime_support;

use librairii_app::{
    native_jobs::*,
    ports::{
        creation::{NewItem, NewOperation},
        jobs::{JobStore, StartObservation, StoreResult, WorkItem},
        operations::*,
    },
};
use librairii_core::{OperationKind, operations::TerminalResult};
use librairii_storage::{
    IsolatedLibrary, database::Database, operations_repository::TerminalRepository,
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
type TestResult = Result<(), Box<dyn Error>>;
struct Observed<'a, 'b> {
    cancel_created: AtomicBool,
    cancelled_refusals: AtomicUsize,
    pause_creation: AtomicBool,
    creation_entered: AtomicBool,
    allow_creation: AtomicBool,
    lose_ack: AtomicBool,
    refusals: AtomicUsize,
    readbacks: AtomicUsize,
    store: TerminalRepository<'a, 'b>,
    completes: Mutex<Vec<Instant>>,
    interrupts: Mutex<Vec<Instant>>,
}
impl OperationTerminalStore for Observed<'_, '_> {
    fn snapshot(&self, id: &str) -> SnapshotReply {
        self.store.snapshot(id)
    }
    fn complete_item(&self, c: &CompleteItemCommand) -> AttemptReply {
        let mut log = self.completes.lock().unwrap_or_else(|e| e.into_inner());
        if log.len() < 4 {
            log.push(Instant::now());
        }
        drop(log);
        let reply = self.store.complete_item(c);
        if self.lose_ack.swap(false, Ordering::AcqRel)
            && matches!(reply.disposition, AttemptDisposition::Committed { .. })
        {
            AttemptReply {
                witness: reply.witness,
                disposition: AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost),
                diagnostic: None,
            }
        } else {
            reply
        }
    }
    fn finish(&self, c: &FinishCommand) -> AttemptReply {
        self.store.finish(c)
    }
    fn interrupt(&self, c: &InterruptCommand) -> AttemptReply {
        let mut log = self.interrupts.lock().unwrap_or_else(|e| e.into_inner());
        if log.len() < 6 {
            log.push(Instant::now());
        }
        drop(log);
        self.store.interrupt(c)
    }
    fn readback(&self, w: &CommandWitness) -> ReadbackReply {
        self.readbacks.fetch_add(1, Ordering::SeqCst);
        if self
            .refusals
            .try_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_sub(1))
            .is_ok()
        {
            ReadbackReply {
                witness: w.clone(),
                disposition: ReadbackDisposition::NotAdmitted(NonAdmission::QueueFull),
                diagnostic: None,
            }
        } else {
            self.store.readback(w)
        }
    }
}
impl JobStore for Observed<'_, '_> {
    fn claim_session(&self) -> bool {
        self.store.claim_session()
    }
    fn release_session(&self) {
        self.store.release_session();
    }
    fn startup(&self) -> StoreResult<()> {
        self.store.startup()
    }
    fn shutdown_cleanup(&self) -> StoreResult<()> {
        self.store.shutdown_cleanup()
    }
    fn generation(&self) -> u64 {
        self.store.generation()
    }
    fn create(&self, i: &NewOperation, at: &str) -> StoreResult<()> {
        if self.pause_creation.load(Ordering::Acquire) {
            self.creation_entered.store(true, Ordering::Release);
            while !self.allow_creation.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
        }
        let result = self.store.create(i, at);
        if self.cancel_created.load(Ordering::Acquire) && matches!(result, StoreResult::Known(())) {
            assert!(matches!(
                self.store.request_cancel(&i.id),
                StoreResult::Known(())
            ));
        }
        result
    }
    fn creation_readback(&self, i: &NewOperation, at: &str) -> StoreResult<bool> {
        self.store.creation_readback(i, at)
    }
    fn next_pending(&self, e: &[i64]) -> StoreResult<Option<WorkItem>> {
        self.store.next_pending(e)
    }
    fn start_item(&self, w: &WorkItem, at: &str) -> StoreResult<()> {
        self.store.start_item(w, at)
    }
    fn start_readback(&self, w: &WorkItem) -> StoreResult<StartObservation> {
        self.store.start_readback(w)
    }
    fn ready_to_finish(
        &self,
        excluded: &[String],
    ) -> StoreResult<Option<(OperationScope, TerminalResult)>> {
        self.store.ready_to_finish(excluded)
    }
    fn save_progress(&self, i: i64, c: i64, t: i64) -> StoreResult<()> {
        self.store.save_progress(i, c, t)
    }
    fn cancelled(&self, id: &str) -> StoreResult<bool> {
        let result = self.store.cancelled(id);
        if matches!(result, StoreResult::Known(true))
            && self
                .cancelled_refusals
                .try_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_sub(1))
                .is_ok()
        {
            StoreResult::Retry
        } else {
            result
        }
    }
    fn request_cancel(&self, id: &str) -> StoreResult<()> {
        self.store.request_cancel(id)
    }
    fn cancel_all(&self) -> StoreResult<()> {
        self.store.cancel_all()
    }
    fn fence(&self) {
        self.store.fence();
    }
}
fn request(n: usize) -> NewOperation {
    NewOperation {
        id: format!("a0000000-0000-4000-8000-{n:012}"),
        kind: OperationKind::MetadataSync,
        source: None,
        destination: None,
        items: vec![NewItem {
            source_name: "fr".into(),
            ..Default::default()
        }],
    }
}
fn wait(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(Instant::now() < end, "durable reconciliation timed out");
        thread::sleep(Duration::from_millis(2));
    }
}
#[expect(
    clippy::panic,
    reason = "A failed prerequisite inside the scoped runtime must unwind its shutdown guard"
)]
fn must<T, E: std::fmt::Debug>(r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("test prerequisite: {e:?}"),
    }
}
#[test]
fn three_real_failed_writes_then_continued_interruption_retains_owner_and_metadata_reservation()
-> TestResult {
    use librairii_core::operations::{ItemState, RawOutcome};
    struct Effect {
        released: AtomicBool,
        settled: AtomicBool,
    }
    impl Executor for Effect {
        type Resources = Box<usize>;
        fn execute(
            &self,
            _: &WorkItem,
            _: &Cancellation,
            _: &ProgressSink,
        ) -> Execution<Self::Resources> {
            Execution {
                boundary: Boundary::Committed,
                report: Report {
                    state: ItemState::Succeeded,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome::default(),
                },
                resources: Some(Box::new(42)),
            }
        }
        fn compensate(&self, _: &mut Self::Resources) -> bool {
            true
        }
        fn settle(
            &self,
            r: Self::Resources,
            ack: &DurableReport<'_>,
        ) -> Result<(), Self::Resources> {
            assert_eq!(ack.command().state, ItemState::Succeeded);
            assert!(ack.interrupted().is_some());
            if !self.released.load(Ordering::Acquire) {
                return Err(r);
            }
            self.settled.store(true, Ordering::Release);
            Ok(())
        }
    }
    struct Release<'a>(&'a AtomicBool);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    assert!(matches!(db.transaction(Default::default(),|tx|{tx.execute_batch("CREATE TRIGGER fail_complete BEFORE UPDATE OF status ON file_operation_items WHEN NEW.status='succeeded' BEGIN SELECT RAISE(ABORT,'complete unavailable'); END; CREATE TRIGGER fail_interrupt BEFORE UPDATE OF status ON file_operations WHEN NEW.status='interrupted' BEGIN SELECT RAISE(ABORT,'interrupt unavailable'); END;")?;Ok(())})?.wait()?,librairii_storage::database::TransactionOutcome::Committed(())));
    let store = Observed {
        cancel_created: AtomicBool::new(false),
        cancelled_refusals: AtomicUsize::new(0),
        pause_creation: AtomicBool::new(false),
        creation_entered: AtomicBool::new(false),
        allow_creation: AtomicBool::new(true),
        lose_ack: AtomicBool::new(false),
        refusals: AtomicUsize::new(0),
        readbacks: AtomicUsize::new(0),
        store: TerminalRepository::new(&db, 1),
        completes: Mutex::new(Vec::with_capacity(4)),
        interrupts: Mutex::new(Vec::with_capacity(6)),
    };
    let effect = Effect {
        released: AtomicBool::new(false),
        settled: AtomicBool::new(false),
    };
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        let _release = Release(&effect.released);
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(1))),
        ));
        wait(|| handle.take_admission().is_some_and(|a| a.accepted));
        wait(|| {
            store
                .interrupts
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .len()
                >= 4
        });
        let times = store.completes.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(times.len(), 3);
        assert!(
            times
                .windows(2)
                .all(|w| w[1].duration_since(w[0]) >= Duration::from_millis(10))
        );
        drop(times);
        let interruptions = store.interrupts.lock().unwrap_or_else(|e| e.into_inner());
        assert!(interruptions[3].duration_since(interruptions[2]) >= Duration::from_secs(1));
        drop(interruptions);
        assert!(!effect.settled.load(Ordering::Acquire));
        assert_eq!(handle.status().running, 1);
        assert!(
            must(must(db.operation_snapshot(request(1).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Running)
        );
        assert!(matches!(
            must(
                must(db.repair_transaction(|tx| {
                    tx.execute_batch("DROP TRIGGER fail_interrupt")?;
                    Ok(())
                }))
                .wait()
            ),
            librairii_storage::database::TransactionOutcome::Committed(())
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(1).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Interrupted)
        });
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(2))),
        ));
        wait(|| handle.take_admission().is_some_and(|a| !a.accepted));
        assert!(must(must(db.operation_snapshot(request(2).id)).wait()).is_none());
        effect.released.store(true, Ordering::Release);
        wait(|| effect.settled.load(Ordering::Acquire));
    })?;
    let saved = db
        .operation_snapshot(request(1).id)?
        .wait()?
        .ok_or("interrupted")?;
    assert_eq!(
        saved.operation.outcome.code.as_deref(),
        Some("persistence_failed")
    );
    assert_eq!(saved.items[0].state, ItemState::Cancelled);
    db.close()?;
    Ok(())
}

#[test]
fn lost_real_commit_ack_then_barrier_refusal_cannot_erase_unknown_or_repeat_completion()
-> TestResult {
    use librairii_core::operations::{ItemState, RawOutcome};
    struct Effect;
    impl Executor for Effect {
        type Resources = ();
        fn execute(&self, _: &WorkItem, _: &Cancellation, _: &ProgressSink) -> Execution<()> {
            Execution {
                boundary: Boundary::Committed,
                report: Report {
                    state: ItemState::Succeeded,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome::default(),
                },
                resources: Some(()),
            }
        }
        fn compensate(&self, _: &mut ()) -> bool {
            true
        }
        fn settle(&self, _: (), ack: &DurableReport<'_>) -> Result<(), ()> {
            assert!(ack.interrupted().is_none());
            Ok(())
        }
    }
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = Observed {
        store: TerminalRepository::new(&db, 1),
        cancel_created: AtomicBool::new(false),
        cancelled_refusals: AtomicUsize::new(0),
        pause_creation: AtomicBool::new(false),
        creation_entered: AtomicBool::new(false),
        allow_creation: AtomicBool::new(true),
        lose_ack: AtomicBool::new(true),
        refusals: AtomicUsize::new(2),
        readbacks: AtomicUsize::new(0),
        completes: Mutex::new(Vec::new()),
        interrupts: Mutex::new(Vec::new()),
    };
    with_runtime(&store, &Effect, Configuration::default(), |handle| {
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(3))),
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(3).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Succeeded)
        });
    })?;
    assert_eq!(
        store
            .completes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len(),
        1
    );
    assert_eq!(store.readbacks.load(Ordering::SeqCst), 3);
    assert!(
        store
            .interrupts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_empty()
    );
    assert_eq!(
        db.operation_snapshot(request(3).id)?
            .wait()?
            .ok_or("saved")?
            .operation
            .counts
            .completed,
        1
    );
    db.close()?;
    Ok(())
}

#[test]
fn shutdown_during_admission_cancels_later_enqueued_work_without_entering_effects() -> TestResult {
    use librairii_core::operations::{ItemState, RawOutcome};
    struct Effect(AtomicUsize);
    impl Executor for Effect {
        type Resources = ();
        fn execute(&self, _: &WorkItem, _: &Cancellation, _: &ProgressSink) -> Execution<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Execution {
                boundary: Boundary::BeforeCommit,
                report: Report {
                    state: ItemState::Skipped,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome::default(),
                },
                resources: None,
            }
        }
        fn compensate(&self, _: &mut ()) -> bool {
            true
        }
        fn settle(&self, _: (), _: &DurableReport<'_>) -> Result<(), ()> {
            Ok(())
        }
    }
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = Observed {
        store: TerminalRepository::new(&db, 1),
        cancel_created: AtomicBool::new(false),
        cancelled_refusals: AtomicUsize::new(0),
        pause_creation: AtomicBool::new(true),
        creation_entered: AtomicBool::new(false),
        allow_creation: AtomicBool::new(false),
        lose_ack: AtomicBool::new(false),
        refusals: AtomicUsize::new(0),
        readbacks: AtomicUsize::new(0),
        completes: Mutex::new(Vec::new()),
        interrupts: Mutex::new(Vec::new()),
    };
    let effect = Effect(AtomicUsize::new(0));
    struct Release<'a>(&'a AtomicBool);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        let _release = Release(&store.allow_creation);
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(71))),
        ));
        wait(|| store.creation_entered.load(Ordering::Acquire));
        handle.shutdown();
        store.allow_creation.store(true, Ordering::Release);
        wait(|| handle.status().drained);
    })?;
    assert_eq!(effect.0.load(Ordering::SeqCst), 0);
    let saved = db
        .operation_snapshot(request(71).id)?
        .wait()?
        .ok_or("cancelled")?;
    assert_eq!(saved.items[0].state, ItemState::Cancelled);
    assert_eq!(
        saved.operation.state,
        librairii_core::OperationState::Cancelled
    );
    db.close()?;
    Ok(())
}

#[derive(Default)]
struct ReviewEffect {
    calls: AtomicUsize,
    import_entered: AtomicBool,
    release: AtomicBool,
}
impl Executor for ReviewEffect {
    type Resources = ();
    fn execute(&self, work: &WorkItem, _: &Cancellation, _: &ProgressSink) -> Execution<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if work.operation.kind == OperationKind::Import {
            self.import_entered.store(true, Ordering::Release);
            while !self.release.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(1));
            }
        }
        Execution {
            boundary: Boundary::Committed,
            report: Report {
                state: librairii_core::operations::ItemState::Succeeded,
                story_id: 0,
                completed_bytes: 0,
                outcome: Default::default(),
            },
            resources: None,
        }
    }
    fn compensate(&self, _: &mut ()) -> bool {
        true
    }
    fn settle(&self, _: (), _: &DurableReport<'_>) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn durable_queued_cancellation_survives_transient_cancellation_read_refusals() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = Observed {
        store: TerminalRepository::new(&db, 1),
        cancel_created: AtomicBool::new(true),
        cancelled_refusals: AtomicUsize::new(3),
        pause_creation: AtomicBool::new(false),
        creation_entered: AtomicBool::new(false),
        allow_creation: AtomicBool::new(true),
        lose_ack: AtomicBool::new(false),
        refusals: AtomicUsize::new(0),
        readbacks: AtomicUsize::new(0),
        completes: Mutex::new(Vec::new()),
        interrupts: Mutex::new(Vec::new()),
    };
    let effect = ReviewEffect::default();
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(72))),
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(72).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Cancelled)
        });
    })?;
    assert_eq!(effect.calls.load(Ordering::Acquire), 0);
    assert_eq!(store.cancelled_refusals.load(Ordering::Acquire), 0);
    db.close()?;
    Ok(())
}

#[test]
fn permanent_creation_rejection_releases_admission_and_allows_shutdown() -> TestResult {
    use librairii_core::operations::{ExportSource, ExportSourceType};
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let mut invalid = request(73);
    invalid.id = "x".into();
    assert!(Start::new(invalid).is_err());
    let mut unicode = request(73);
    unicode.id = "É".repeat(36);
    assert!(Start::new(unicode).is_ok());
    let mut input = request(73);
    input.kind = OperationKind::Export;
    input.destination = Some(roots.profile().to_string_lossy().into_owned());
    input.source = Some(ExportSource {
        source_type: ExportSourceType::Selection,
        collection_ids: Vec::new(),
        collection_names: Vec::new(),
        shelf_ids: Vec::new(),
        shelf_names: Vec::new(),
    });
    input.items = vec![NewItem {
        story_id: 99999,
        story_uuid: "c0000000-0000-4000-8000-000000000001".into(),
        story_title: "Deleted after planning".into(),
        source_name: "gone.zip".into(),
        output_name: "gone.zip".into(),
        archive_relative_path: "archives/gone.zip".into(),
        archive_sha256: "a".repeat(64),
        ..Default::default()
    }];
    let store = TerminalRepository::new(&db, 1);
    assert!(matches!(store.create(&input, "now"), StoreResult::Rejected));
    let effect = ReviewEffect::default();
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        must(runtime_support::offer(&handle, must(Start::new(input))));
        let mut rejected = false;
        wait(|| {
            if let Some(admission) = handle.take_admission() {
                rejected = !admission.accepted;
            }
            rejected
        });
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(74))),
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(74).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Succeeded)
        });
    })?;
    assert!(db.operation_snapshot(request(73).id)?.wait()?.is_none());
    assert_eq!(effect.calls.load(Ordering::Acquire), 1);
    db.close()?;
    Ok(())
}

#[test]
fn completed_metadata_finishes_while_unrelated_import_still_owns_its_resources() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = TerminalRepository::new(&db, 1);
    let effect = ReviewEffect::default();
    struct Release<'a>(&'a AtomicBool);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        let _release = Release(&effect.release);
        let mut import = request(75);
        import.kind = OperationKind::Import;
        must(runtime_support::offer(&handle, must(Start::new(import))));
        wait(|| effect.import_entered.load(Ordering::Acquire));
        wait(|| handle.take_admission().is_some());
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(76))),
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(76).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Succeeded)
        });
        assert!(!effect.release.load(Ordering::Acquire));
        assert_eq!(
            must(must(db.operation_snapshot(request(75).id)).wait()).map(|s| s.operation.state),
            Some(librairii_core::OperationState::Running)
        );
        wait(|| handle.take_admission().is_some());
        must(runtime_support::offer(
            &handle,
            must(Start::new(request(77))),
        ));
        wait(|| {
            must(must(db.operation_snapshot(request(77).id)).wait())
                .is_some_and(|s| s.operation.state == librairii_core::OperationState::Succeeded)
        });
    })?;
    db.close()?;
    Ok(())
}
