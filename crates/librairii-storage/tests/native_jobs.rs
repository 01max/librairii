#![cfg(any(target_os = "macos", target_os = "linux"))]
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
            report: Report {
                state: ItemState::Skipped,
                story_id: 0,
                completed_bytes: 0,
                outcome: RawOutcome::default(),
            },
            resources: Some(Box::new(active)),
        }
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
        assert!(handle.try_start(must(Start::new(request))).is_ok());
        wait(|| handle.take_admission().is_some_and(|v| v.accepted));
        wait(|| handle.status().running == 2 && handle.status().queued == 4);
        assert_eq!(effect.maximum.load(Ordering::SeqCst), 2);
        effect.release.store(true, Ordering::Release);
        wait(|| {
            must(must(db.operation_snapshot(id.clone())).wait())
                .is_some_and(|s| s.operation.state == OperationState::Succeeded)
        });
        handle
    })?;
    assert_eq!(effect.settled.load(Ordering::SeqCst), 30);
    assert_eq!(stale.status().running, 0);
    assert!(matches!(
        stale.try_start(must(Start::new(input(OperationKind::Import, 1, 2)))),
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
                report: Report {
                    state: ItemState::Succeeded,
                    story_id: work.item.identity.historical_story_id(),
                    completed_bytes: 1_000_000,
                    outcome: RawOutcome::default(),
                },
                resources: None,
            }
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
        assert!(handle.try_start(must(Start::new(request))).is_ok());
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
