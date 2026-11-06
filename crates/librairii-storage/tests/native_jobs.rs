#![cfg(any(target_os = "macos", target_os = "linux"))]
#[path = "support/runtime.rs"]
mod runtime_support;

use librairii_app::{
    native_jobs::*,
    ports::{
        creation::{NewItem, NewOperation},
        jobs::{JobStore, StoreResult, WorkItem},
    },
};
use librairii_core::{
    OperationKind, OperationState,
    operations::{ExportSource, ExportSourceType, ItemState, RawOutcome},
};
use librairii_storage::{
    IsolatedLibrary, database::Database, operations_repository::TerminalRepository,
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    thread,
    time::{Duration, Instant},
};
type TestResult = Result<(), Box<dyn Error>>;
struct Effect {
    release: AtomicBool,
    active: AtomicUsize,
    maximum: AtomicUsize,
    settled: AtomicUsize,
}
impl Effect {
    fn new() -> Self {
        Self {
            release: AtomicBool::new(false),
            active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
            settled: AtomicUsize::new(0),
        }
    }
}
impl Executor for Effect {
    type Resources = Box<usize>;
    fn execute(
        &self,
        _: &WorkItem,
        c: &Cancellation,
        _: &ProgressSink,
    ) -> Execution<Self::Resources> {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(active, Ordering::SeqCst);
        while !self.release.load(Ordering::Acquire) && !c.is_cancelled() {
            thread::sleep(Duration::from_millis(1));
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        Execution {
            boundary: Boundary::BeforeCommit,
            report: Report {
                state: ItemState::Skipped,
                story_id: 0,
                completed_bytes: 0,
                outcome: RawOutcome::default(),
            },
            resources: Some(Box::new(active)),
        }
    }
    fn compensate(&self, _: &mut Self::Resources) -> bool {
        true
    }
    fn settle(&self, _: Box<usize>, durable: &DurableReport<'_>) -> Result<(), Box<usize>> {
        assert_eq!(durable.command().state, ItemState::Skipped);
        self.settled.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn input(kind: OperationKind, count: usize, id: usize) -> NewOperation {
    NewOperation {
        id: format!("a0000000-0000-4000-8000-{id:012}"),
        kind,
        source: None,
        destination: None,
        items: (0..count)
            .map(|n| NewItem {
                source_name: format!("input-{n}.zip"),
                ..Default::default()
            })
            .collect(),
    }
}
#[expect(
    clippy::panic,
    reason = "Test prerequisites inside a runtime callback must fail the test and trigger its owned shutdown guard"
)]
fn must<T, E: std::fmt::Debug>(r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("test prerequisite failed: {e:?}"),
    }
}
fn wait(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < end, "runtime condition timed out");
        thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn actual_sql_source_bounds_queue_workers_and_keeps_borrow_through_settlement() -> TestResult {
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = TerminalRepository::new(&db, 1);
    let effect = Effect::new();
    let request = input(OperationKind::Import, 30, 1);
    let id = request.id.clone();
    let stale = with_runtime(&store, &effect, Configuration::default(), |handle| {
        wait(|| handle.idle_for_relocation());
        thread::sleep(Duration::from_millis(120));
        must(runtime_support::offer(&handle, must(Start::new(request))));
        wait(|| handle.take_admission().is_some_and(|v| v.accepted));
        wait(|| handle.status().running == 2 && handle.status().queued == 4);
        assert_eq!(effect.maximum.load(Ordering::SeqCst), 2);
        effect.release.store(true, Ordering::Release);
        wait(|| {
            must(must(db.operation_snapshot(id.clone())).wait())
                .is_some_and(|s| s.operation.state == OperationState::Succeeded)
        });
        wait(|| handle.idle_for_relocation());
        thread::sleep(Duration::from_millis(120));
        let next = input(OperationKind::Import, 1, 2);
        let next_id = next.id.clone();
        must(runtime_support::offer(&handle, must(Start::new(next))));
        wait(|| handle.take_admission().is_some_and(|v| v.accepted));
        wait(|| {
            must(must(db.operation_snapshot(next_id.clone())).wait())
                .is_some_and(|s| s.operation.state == OperationState::Succeeded)
        });
        wait(|| handle.idle_for_relocation());
        thread::sleep(Duration::from_millis(120));
        handle.shutdown();
        wait(|| handle.status().drained);
        handle
    })?;
    assert_eq!(effect.settled.load(Ordering::SeqCst), 31);
    assert_eq!(stale.status().running, 0);
    assert!(matches!(
        stale.try_start(must(Start::new(input(OperationKind::Import, 1, 3)))),
        Err((Refusal::Closed, _))
    ));
    db.close()?;
    Ok(())
}
#[test]
fn import_picker_limit_export_scope_and_metadata_conflict_are_distinct() -> TestResult {
    assert!(Start::new(input(OperationKind::Import, 1000, 1)).is_ok());
    assert!(Start::new(input(OperationKind::Import, 1001, 1)).is_err());
    let mut export = input(OperationKind::Export, 1001, 2);
    export.destination = Some("/chosen".into());
    export.source = Some(ExportSource {
        source_type: ExportSourceType::Selection,
        collection_ids: Vec::new(),
        collection_names: Vec::new(),
        shelf_ids: Vec::new(),
        shelf_names: Vec::new(),
    });
    for (n, item) in export.items.iter_mut().enumerate() {
        item.story_title = "Gone".into();
        item.story_uuid = format!("c0000000-0000-4000-8000-{n:012}");
        item.export_skip_code = "story_unavailable".into();
    }
    assert!(Start::new(export).is_ok());
    for workers in [1, 2, 8] {
        assert_eq!(
            Configuration::new(workers)
                .ok_or("worker configuration")?
                .queue_capacity(),
            workers * 2
        );
    }
    assert!(Configuration::new(0).is_none());
    assert!(Configuration::new(9).is_none());
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = TerminalRepository::new(&db, 1);
    assert!(matches!(
        store.create(&input(OperationKind::MetadataSync, 1, 3), "now"),
        StoreResult::Known(())
    ));
    assert!(matches!(
        store.create(&input(OperationKind::MetadataSync, 1, 4), "now"),
        StoreResult::Rejected
    ));
    assert!(
        db.operation_snapshot(input(OperationKind::MetadataSync, 1, 4).id)?
            .wait()?
            .is_none()
    );
    db.close()?;
    Ok(())
}

#[test]
fn progress_coalesces_persists_and_rejects_late_producers_and_stale_responses() -> TestResult {
    use librairii_core::{
        collection_codec::url::{Location, Scope},
        navigation::{Frame, Navigation},
        query::QueryRequest,
    };
    use std::sync::Mutex;
    struct Copy {
        release: AtomicBool,
        sink: Mutex<Option<ProgressSink>>,
    }
    impl Executor for Copy {
        type Resources = ();
        fn execute(&self, work: &WorkItem, c: &Cancellation, p: &ProgressSink) -> Execution<()> {
            *self.sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(p.clone());
            for n in 1..=10_000 {
                assert!(p.advance(n * 50));
            }
            assert!(!p.advance(-1));
            assert!(!p.advance(1_000_001));
            while !self.release.load(Ordering::Acquire) && !c.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            Execution {
                boundary: Boundary::BeforeCommit,
                report: Report {
                    state: ItemState::Succeeded,
                    story_id: work.item.identity.historical_story_id(),
                    completed_bytes: 1_000_000,
                    outcome: RawOutcome::default(),
                },
                resources: None,
            }
        }
        fn compensate(&self, _: &mut Self::Resources) -> bool {
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
    let store = TerminalRepository::new(&db, 1);
    assert!(matches!(
        db.transaction(Default::default(), |tx| {
            tx.execute(
                "INSERT INTO stories(id,uuid) VALUES(1,'c0000000-0000-4000-8000-000000000001')",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
        librairii_storage::database::TransactionOutcome::Committed(())
    ));
    let mut request = input(OperationKind::Export, 1, 5);
    request.source = Some(ExportSource {
        source_type: ExportSourceType::Selection,
        collection_ids: Vec::new(),
        collection_names: Vec::new(),
        shelf_ids: Vec::new(),
        shelf_names: Vec::new(),
    });
    request.destination = Some("/chosen".into());
    let item = &mut request.items[0];
    item.story_id = 1;
    item.story_uuid = "c0000000-0000-4000-8000-000000000001".into();
    item.story_title = "Copy".into();
    item.output_name = "copy.zip".into();
    item.archive_relative_path = "archives/content.zip".into();
    item.archive_sha256 = "a".repeat(64);
    item.total_bytes = 1_000_000;
    let id = request.id.clone();
    let effect = Copy {
        release: AtomicBool::new(false),
        sink: Mutex::new(None),
    };
    let stale = with_runtime(&store, &effect, Configuration::default(), |handle| {
        must(runtime_support::offer(&handle, must(Start::new(request))));
        wait(|| handle.take_admission().is_some_and(|v| v.accepted));
        wait(|| {
            must(must(db.operation_snapshot(id.clone())).wait())
                .is_some_and(|s| s.items[0].progress.completed == 500_000)
        });
        let latest = handle.take_progress();
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].completed, 500_000);
        assert_eq!(latest[0].revision, 10_000);
        assert!(handle.take_progress().is_empty());
        let old = must(handle.request_stamp(None).ok_or("stamp"));
        let active =
            must(must(db.operation_snapshot(id.clone())).wait()).unwrap_or_else(|| unreachable!());
        let mut nav = must(Navigation::new(
            must("b0000000-0000-4000-8000-000000000001".parse()),
            1,
            Location {
                scope: Scope::Library,
                request: QueryRequest::default(),
            },
        ));
        let query = must(handle.request_stamp(Some(nav.stamp())).ok_or("stamp"));
        must(nav.replace(Frame {
            location: Location {
                scope: Scope::Library,
                request: QueryRequest::default(),
            },
            selection: Vec::new(),
        }));
        assert!(query.complete(42).accept(&handle, Some(&nav)).is_none());
        effect.release.store(true, Ordering::Release);
        wait(|| {
            must(must(db.operation_snapshot(id.clone())).wait())
                .is_some_and(|s| s.operation.state == OperationState::Succeeded)
        });
        let final_state =
            must(must(db.operation_snapshot(id.clone())).wait()).unwrap_or_else(|| unreachable!());
        let mut projection = must(OperationProjection::new(id.clone()));
        let fresh = must(handle.request_stamp(None).ok_or("stamp"));
        assert!(projection.apply(fresh.complete(final_state), &handle, None));
        assert!(!projection.apply(old.complete(active), &handle, None));
        let late = effect
            .sink
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .unwrap_or_else(|| unreachable!());
        assert!(!late.advance(999_999));
        must(handle.request_stamp(None).ok_or("stamp")).complete(42)
    })?;
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        assert!(stale.accept(&handle, None).is_none());
    })?;
    db.close()?;
    Ok(())
}

#[test]
fn queued_cancel_committed_result_and_required_compensation_keep_distinct_boundaries() -> TestResult
{
    use std::{
        fs::{File, OpenOptions},
        io::{Read, Seek, Write},
    };
    struct Transition {
        file: File,
        boundary: Boundary,
        entered: AtomicBool,
        restore: AtomicBool,
        attempts: AtomicUsize,
        settled: AtomicBool,
    }
    impl Executor for Transition {
        type Resources = File;
        fn execute(&self, _: &WorkItem, c: &Cancellation, p: &ProgressSink) -> Execution<File> {
            let mut file = must(self.file.try_clone());
            if self.boundary != Boundary::BeforeCommit {
                must(file.write_all(b"effect"));
                must(file.sync_all());
                assert!(p.phase(Phase::Committing));
            }
            self.entered.store(true, Ordering::Release);
            while !c.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            Execution {
                boundary: self.boundary,
                report: Report {
                    state: if self.boundary == Boundary::Committed {
                        ItemState::Succeeded
                    } else {
                        ItemState::Cancelled
                    },
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome::default(),
                },
                resources: Some(file),
            }
        }
        fn compensate(&self, file: &mut File) -> bool {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            if !self.restore.load(Ordering::Acquire) {
                return false;
            }
            must(file.set_len(0));
            must(file.rewind());
            must(file.write_all(b"restored"));
            must(file.sync_all());
            true
        }
        fn settle(&self, _: File, _: &DurableReport<'_>) -> Result<(), File> {
            self.settled.store(true, Ordering::Release);
            Ok(())
        }
    }
    struct Restore<'a>(&'a AtomicBool);
    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }
    for (index, boundary) in [
        Boundary::BeforeCommit,
        Boundary::Committed,
        Boundary::Compensating,
    ]
    .into_iter()
    .enumerate()
    {
        let roots = TestRoots::new()?;
        let lib = IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "b0000000-0000-4000-8000-000000000001".parse()?,
        )?;
        let mut db = Database::open_isolated(&lib)?;
        let store = TerminalRepository::new(&db, 1);
        let path = roots.profile().join("effect");
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)?;
        let effect = Transition {
            file,
            boundary,
            entered: AtomicBool::new(false),
            restore: AtomicBool::new(false),
            attempts: AtomicUsize::new(0),
            settled: AtomicBool::new(false),
        };
        let request = input(OperationKind::Import, 4, index + 20);
        let id = request.id.clone();
        with_runtime(
            &store,
            &effect,
            must(Configuration::new(1).ok_or("workers")),
            |handle| {
                let _restore = Restore(&effect.restore);
                must(runtime_support::offer(&handle, must(Start::new(request))));
                wait(|| effect.entered.load(Ordering::Acquire) && handle.status().queued == 2);
                must(cancel(&handle, id.clone()));
                if boundary == Boundary::Compensating {
                    wait(|| effect.attempts.load(Ordering::SeqCst) >= 2);
                    let snapshot = must(must(db.operation_snapshot(id.clone())).wait())
                        .unwrap_or_else(|| unreachable!());
                    assert_eq!(snapshot.operation.state, OperationState::Running);
                    assert_eq!(snapshot.items[0].state, ItemState::Running);
                    assert!(!effect.settled.load(Ordering::Acquire));
                    effect.restore.store(true, Ordering::Release);
                }
                wait(|| {
                    must(must(db.operation_snapshot(id.clone())).wait())
                        .is_some_and(|s| librairii_core::operations::is_terminal(s.operation.state))
                });
            },
        )?;
        let saved = db.operation_snapshot(id)?.wait()?.ok_or("terminal")?;
        assert_eq!(
            saved.items[0].state,
            if boundary == Boundary::Committed {
                ItemState::Succeeded
            } else {
                ItemState::Cancelled
            }
        );
        assert!(
            saved.items[1..]
                .iter()
                .all(|i| i.state == ItemState::Cancelled)
        );
        assert!(effect.settled.load(Ordering::Acquire));
        let mut bytes = String::new();
        File::open(path)?.read_to_string(&mut bytes)?;
        assert_eq!(
            bytes,
            match boundary {
                Boundary::BeforeCommit => "",
                Boundary::Committed => "effect",
                Boundary::Compensating => "restored",
            }
        );
        db.close()?;
    }
    Ok(())
}

pub fn cancel(handle: &Handle, mut id: String) -> Result<(), Box<dyn Error>> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        match handle.cancel(id) {
            Ok(()) => return Ok(()),
            Err((Refusal::Busy, original)) if Instant::now() < end => {
                id = original;
                thread::sleep(Duration::from_millis(1));
            }
            Err(_) => return Err("runtime cancellation unavailable".into()),
        }
    }
}

#[test]
fn shutdown_joins_after_uncancelled_compensation_reporting_and_actual_owner_settlement()
-> TestResult {
    use std::{
        fs::{File, OpenOptions},
        io::{Seek, Write},
        sync::Mutex,
    };
    struct Effect {
        file: File,
        entered: AtomicBool,
        restore: AtomicBool,
        settling: AtomicBool,
        release: AtomicBool,
    }
    impl Executor for Effect {
        type Resources = File;
        fn execute(&self, _: &WorkItem, c: &Cancellation, p: &ProgressSink) -> Execution<File> {
            let mut file = must(self.file.try_clone());
            must(file.write_all(b"moved"));
            must(file.sync_all());
            self.entered.store(true, Ordering::Release);
            while !c.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            assert!(p.phase(Phase::Compensating));
            Execution {
                boundary: Boundary::Compensating,
                report: Report {
                    state: ItemState::Cancelled,
                    story_id: 0,
                    completed_bytes: 0,
                    outcome: RawOutcome::default(),
                },
                resources: Some(file),
            }
        }
        fn compensate(&self, file: &mut File) -> bool {
            if !self.restore.load(Ordering::Acquire) {
                return false;
            }
            must(file.set_len(0));
            must(file.rewind());
            must(file.write_all(b"restored"));
            must(file.sync_all());
            true
        }
        fn settle(&self, file: File, ack: &DurableReport<'_>) -> Result<(), File> {
            assert!(ack.interrupted().is_none());
            self.settling.store(true, Ordering::Release);
            if !self.release.load(Ordering::Acquire) {
                Err(file)
            } else {
                Ok(())
            }
        }
    }
    struct Release<'a>(&'a Effect);
    impl Drop for Release<'_> {
        fn drop(&mut self) {
            self.0.restore.store(true, Ordering::Release);
            self.0.release.store(true, Ordering::Release);
        }
    }
    let roots = TestRoots::new()?;
    let lib = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&lib)?;
    let store = TerminalRepository::new(&db, 1);
    let path = roots.profile().join("actual-owned-effect");
    let effect = Effect {
        file: OpenOptions::new()
            .create_new(true)
            .write(true)
            .read(true)
            .open(&path)?,
        entered: AtomicBool::new(false),
        restore: AtomicBool::new(false),
        settling: AtomicBool::new(false),
        release: AtomicBool::new(false),
    };
    let observed = Mutex::new(None);
    let ui_returned = AtomicBool::new(false);
    let joined = AtomicBool::new(false);
    thread::scope(|scope| -> TestResult {
        let _release = Release(&effect);
        let run = scope.spawn(|| {
            let result = with_runtime(&store, &effect, Configuration::default(), |handle| {
                must(runtime_support::offer(
                    &handle,
                    must(Start::new(input(OperationKind::Import, 1, 70))),
                ));
                wait(|| effect.entered.load(Ordering::Acquire));
                assert!(matches!(
                    with_runtime(&store, &effect, Configuration::default(), |_| ()),
                    Err(SessionError::Unavailable)
                ));
                *observed.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.clone());
                handle.shutdown();
                ui_returned.store(true, Ordering::Release);
            });
            joined.store(true, Ordering::Release);
            result
        });
        wait(|| ui_returned.load(Ordering::Acquire));
        let handle = observed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or("handle")?;
        wait(|| handle.shutdown_presentation().stage == ShutdownStage::RestoringChanges);
        assert!(!joined.load(Ordering::Acquire));
        let pending = db
            .operation_snapshot(input(OperationKind::Import, 1, 70).id)?
            .wait()?
            .ok_or("pending")?;
        assert_eq!(pending.items[0].state, ItemState::Running);
        effect.restore.store(true, Ordering::Release);
        wait(|| effect.settling.load(Ordering::Acquire));
        let reported = db
            .operation_snapshot(input(OperationKind::Import, 1, 70).id)?
            .wait()?
            .ok_or("reported")?;
        assert_eq!(reported.items[0].state, ItemState::Cancelled);
        assert_eq!(reported.operation.state, OperationState::Running);
        assert!(!joined.load(Ordering::Acquire));
        assert_eq!(
            handle.shutdown_presentation().stage,
            ShutdownStage::FinishingChanges
        );
        effect.release.store(true, Ordering::Release);
        run.join().map_err(|_| "runtime thread")??;
        assert!(joined.load(Ordering::Acquire));
        assert_eq!(
            handle.shutdown_presentation().stage,
            ShutdownStage::ReadyToClose
        );
        Ok(())
    })?;
    assert_eq!(std::fs::read(path)?, b"restored");
    with_runtime(&store, &effect, Configuration::default(), |handle| {
        assert_ne!(
            handle.generation(),
            observed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .unwrap_or_else(|| unreachable!())
                .generation()
        );
    })?;
    db.close()?;
    Ok(())
}
