use super::{model::*, progress::*, runtime::*, terminal};
use librairii_core::{JobId, OperationKind, operations::HistoryKey};
use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    io,
    sync::{Arc, Condvar, Mutex, MutexGuard, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

type TestResult = Result<(), Box<dyn Error>>;

fn guard<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn until(mut condition: impl FnMut() -> bool) -> io::Result<()> {
    let limit = Instant::now() + Duration::from_secs(10);
    while !condition() {
        if Instant::now() >= limit {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "controlled observation",
            ));
        }
        thread::yield_now();
    }
    Ok(())
}

struct Gate {
    state: Mutex<(bool, usize)>,
    changed: Condvar,
}

impl Gate {
    fn new() -> Self {
        Self {
            state: Mutex::new((true, 0)),
            changed: Condvar::new(),
        }
    }

    fn close(&self) {
        guard(&self.state).0 = false;
    }

    fn open(&self) {
        guard(&self.state).0 = true;
        self.changed.notify_all();
    }

    fn pass(&self) {
        let mut state = guard(&self.state);
        state.1 += 1;
        self.changed.notify_all();
        while !state.0 {
            state = match self.changed.wait(state) {
                Ok(value) => value,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }

    fn entries(&self) -> usize {
        guard(&self.state).1
    }
}

#[derive(Clone)]
struct Gates {
    admit: Arc<Gate>,
    source: Arc<Gate>,
    execute: Arc<Gate>,
    report: Arc<Gate>,
    start_readback: Arc<Gate>,
    report_readback: Arc<Gate>,
    progress: Arc<Gate>,
    cancel: Arc<Gate>,
    cancel_readback: Arc<Gate>,
    terminal_attempt: Arc<Gate>,
    terminal_readback: Arc<Gate>,
}

impl Gates {
    fn new() -> Self {
        Self {
            admit: Arc::new(Gate::new()),
            source: Arc::new(Gate::new()),
            execute: Arc::new(Gate::new()),
            report: Arc::new(Gate::new()),
            start_readback: Arc::new(Gate::new()),
            report_readback: Arc::new(Gate::new()),
            progress: Arc::new(Gate::new()),
            cancel: Arc::new(Gate::new()),
            cancel_readback: Arc::new(Gate::new()),
            terminal_attempt: Arc::new(Gate::new()),
            terminal_readback: Arc::new(Gate::new()),
        }
    }

    fn release(&self) {
        for gate in [
            &self.admit,
            &self.source,
            &self.execute,
            &self.report,
            &self.start_readback,
            &self.report_readback,
            &self.progress,
            &self.cancel,
            &self.cancel_readback,
            &self.terminal_attempt,
            &self.terminal_readback,
        ] {
            gate.open();
        }
    }
}

#[derive(Default)]
struct Store {
    jobs: BTreeMap<HistoryKey, VecDeque<Item>>,
    accepted: Vec<JobId>,
    offered: Vec<(JobId, usize)>,
    recorded: Vec<Completed>,
    max_exclusions: usize,
    progress: Vec<Value>,
    terminal_truth: Vec<terminal::Resolution>,
    max_fences: usize,
    cancelled: Vec<Control>,
}

impl Store {
    fn create(&mut self, input: StartInput) -> Admission {
        if !self.accepted.contains(&input.job) {
            self.accepted.push(input.job);
        }
        if let Some(first) = input.items.front() {
            self.jobs.insert(first.order.clone(), input.items);
        }
        Admission::Created(input.job)
    }

    fn source(&mut self, request: SourceRequest) -> Page {
        self.max_fences = self.max_fences.max(request.fenced.len());
        self.max_exclusions = self.max_exclusions.max(request.excluded.len());
        let upper = request
            .cursor
            .upper
            .or_else(|| self.jobs.keys().next_back().cloned());
        for (key, items) in &mut self.jobs {
            if items.front().is_some_and(|item| {
                request.fenced.contains(&Control {
                    job: item.key.job,
                    generation: item.generation,
                })
            }) {
                continue;
            }
            if request
                .cursor
                .after
                .as_ref()
                .is_some_and(|after| key <= after)
                || upper.as_ref().is_some_and(|upper| key > upper)
            {
                continue;
            }
            let index = items
                .iter()
                .position(|item| !request.excluded.contains(&item.key));
            if let Some(mut item) = index.and_then(|index| items.remove(index)) {
                item.cancel_requested |= self.cancelled.contains(&Control {
                    job: item.key.job,
                    generation: item.generation,
                });
                self.offered.push((item.key.job, item.ordinal));
                return Page {
                    cursor: Cursor {
                        after: Some(key.clone()),
                        upper,
                    },
                    item: Some(item),
                    end_cycle: false,
                };
            }
        }
        Page {
            cursor: Cursor {
                after: request.cursor.after,
                upper,
            },
            item: None,
            end_cycle: true,
        }
    }

    fn save(&mut self, completed: Completed, saved: Option<terminal::Resolution>) -> Report {
        if let Some(truth) = saved {
            self.terminal_truth.push(truth);
        }
        self.recorded.push(completed);
        Report::Recorded {
            terminal_operation: saved.is_none_or(|s| s.observation.is_saved_terminal()),
        }
    }
}

mod terminal_tests;

fn input(number: u8, kind: OperationKind, count: usize, payload: usize) -> StartInput {
    let job = JobId::from_bytes([number; 16]);
    let order = HistoryKey {
        created_at: format!("{number:04}"),
        id: format!("Raw-{number}"),
    };
    let items = (0..count)
        .map(|ordinal| Item {
            key: ItemKey {
                job,
                row: ordinal as i64 + 1,
            },
            order: order.clone(),
            ordinal,
            kind,
            payload: vec![number; payload],
            generation: Generation(1),
            total_bytes: payload as i64,
            cancel_requested: false,
        })
        .collect();
    StartInput::new(job, kind, items)
}

#[derive(Clone, Copy)]
enum StartPolicy {
    Create,
    Uncertain,
    Reject,
}

#[derive(Clone, Copy)]
enum ControlPolicy {
    Record,
    Reject,
    Uncertain,
}

struct Rig {
    engine: Option<Engine>,
    handle: Handle,
    gates: Gates,
    store: Arc<Mutex<Store>>,
    on_execute: Arc<Mutex<Option<Handle>>>,
    on_progress: Arc<Mutex<Option<ExecuteHook>>>,
    control_policy: Arc<Mutex<ControlPolicy>>,
}

type ExecuteHook = Arc<dyn Fn(&Item, ProgressSink, WorkCancellation) + Send + Sync>;

impl Rig {
    fn new(
        workers: usize,
        gates: Gates,
        start_policy: StartPolicy,
        uncertain_report: bool,
    ) -> io::Result<Self> {
        Self::with_store(
            workers,
            gates,
            start_policy,
            uncertain_report,
            Arc::new(Mutex::new(Store::default())),
        )
    }

    fn with_store(
        workers: usize,
        gates: Gates,
        start_policy: StartPolicy,
        uncertain_report: bool,
        store: Arc<Mutex<Store>>,
    ) -> io::Result<Self> {
        Self::with_terminal(workers, gates, start_policy, uncertain_report, store, None)
    }

    fn with_terminal(
        workers: usize,
        gates: Gates,
        start_policy: StartPolicy,
        uncertain_report: bool,
        store: Arc<Mutex<Store>>,
        terminal: Option<terminal::Callbacks>,
    ) -> io::Result<Self> {
        let control_policy = Arc::new(Mutex::new(ControlPolicy::Record));
        let coordinator = CoordinatorCallbacks {
            admit: {
                let gates = gates.clone();
                let store = Arc::clone(&store);
                Arc::new(move |input| {
                    gates.admit.pass();
                    match start_policy {
                        StartPolicy::Uncertain => {
                            guard(&store).accepted.push(input.job);
                            Admission::Uncertain(input)
                        }
                        StartPolicy::Create => guard(&store).create(input),
                        StartPolicy::Reject => Admission::Rejected(input),
                    }
                })
            },
            readback_start: {
                let gate = Arc::clone(&gates.start_readback);
                let store = Arc::clone(&store);
                Arc::new(move |input| {
                    gate.pass();
                    guard(&store).create(input)
                })
            },
            progress: {
                let gate = Arc::clone(&gates.progress);
                let store = Arc::clone(&store);
                Arc::new(move |value| {
                    gate.pass();
                    let mut store = guard(&store);
                    if store
                        .recorded
                        .iter()
                        .any(|r| r.item.key == value.stamp.item)
                    {
                        return false;
                    }
                    store.progress.push(value);
                    true
                })
            },
            cancel: {
                let gate = Arc::clone(&gates.cancel);
                let store = Arc::clone(&store);
                let policy = Arc::clone(&control_policy);
                Arc::new(move |request| {
                    gate.pass();
                    match *guard(&policy) {
                        ControlPolicy::Record => {
                            guard(&store).cancelled.push(request);
                            ControlDecision::Recorded
                        }
                        ControlPolicy::Reject => ControlDecision::Rejected(request),
                        ControlPolicy::Uncertain => ControlDecision::Uncertain(request),
                    }
                })
            },
            readback_cancel: {
                let gate = Arc::clone(&gates.cancel_readback);
                let store = Arc::clone(&store);
                Arc::new(move |request| {
                    gate.pass();
                    guard(&store).cancelled.push(request);
                    ControlDecision::Recorded
                })
            },
            source: {
                let gate = Arc::clone(&gates.source);
                let store = Arc::clone(&store);
                Arc::new(move |request| {
                    gate.pass();
                    guard(&store).source(request)
                })
            },
        };
        let on_execute = Arc::new(Mutex::new(None::<Handle>));
        let on_progress = Arc::new(Mutex::new(None::<ExecuteHook>));
        let callbacks = WorkerCallbacks {
            terminal,
            execute: {
                let gate = Arc::clone(&gates.execute);
                let on_execute = Arc::clone(&on_execute);
                let on_progress = Arc::clone(&on_progress);
                Arc::new(move |item, progress, cancelled| {
                    gate.pass();
                    let hook = guard(&on_progress).clone();
                    if let Some(hook) = hook {
                        hook(&item, progress, cancelled);
                    }
                    if let Some(handle) = guard(&on_execute).clone() {
                        handle.stop();
                        let _ = handle.measurements();
                    }
                    Completed {
                        item,
                        fact: "observed-success".into(),
                        errors: vec!["independent-cleanup-diagnostic".into()],
                    }
                })
            },
            report: {
                let gate = Arc::clone(&gates.report);
                let store = Arc::clone(&store);
                Arc::new(move |completed, saved| {
                    gate.pass();
                    if uncertain_report {
                        Report::Uncertain(completed)
                    } else {
                        guard(&store).save(completed, saved)
                    }
                })
            },
            readback_report: {
                let gate = Arc::clone(&gates.report_readback);
                let store = Arc::clone(&store);
                Arc::new(move |completed, saved| {
                    gate.pass();
                    guard(&store).save(completed, saved)
                })
            },
        };
        let config = Configuration::new(workers)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "test workers"))?;
        let engine = Engine::start(config, coordinator, callbacks)?;
        Ok(Self {
            handle: engine.handle(),
            engine: Some(engine),
            gates,
            store,
            on_execute,
            on_progress,
            control_policy,
        })
    }

    fn start(&self, value: StartInput) -> Result<mpsc::Receiver<StartReply>, Box<dyn Error>> {
        self.handle
            .try_start(value)
            .map_err(|error| format!("start refused: {:?}", error.reason).into())
    }

    fn created(&self, value: StartInput) -> TestResult {
        let job = value.job;
        let reply = self.start(value)?.recv_timeout(Duration::from_secs(10))?;
        match reply {
            StartReply::Created(actual) => assert_eq!(actual, job),
            StartReply::Rejected(_) => return Err("unexpected proven rejection".into()),
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Measurements, Box<dyn Error>> {
        self.gates.release();
        self.handle.wake_repair();
        self.engine
            .take()
            .ok_or("missing engine")?
            .join()
            .map_err(Into::into)
    }

    fn joining(&mut self) -> Result<Joining, Box<dyn Error>> {
        self.handle.stop();
        let engine = self.engine.take().ok_or("missing engine")?;
        let (sender, receiver) = mpsc::sync_channel(1);
        let thread = thread::spawn(move || {
            let _ = sender.send(engine.join());
        });
        Ok(Joining {
            receiver,
            thread: Some(thread),
            gates: self.gates.clone(),
            handle: self.handle.clone(),
        })
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.gates.release();
        self.handle.wake_repair();
        if let Some(engine) = self.engine.take() {
            let _ = engine.join();
        }
    }
}

struct Joining {
    receiver: mpsc::Receiver<io::Result<Measurements>>,
    thread: Option<JoinHandle<()>>,
    gates: Gates,
    handle: Handle,
}

impl Joining {
    fn finish(mut self) -> Result<Measurements, Box<dyn Error>> {
        self.gates.release();
        self.handle.wake_repair();
        let result = self.receiver.recv_timeout(Duration::from_secs(10))??;
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| "join observer panicked")?;
        }
        Ok(result)
    }
}

impl Drop for Joining {
    fn drop(&mut self) {
        self.gates.release();
        self.handle.wake_repair();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[test]
fn j6_worker_configuration_matches_frozen_limits() {
    assert_eq!(Configuration::default().workers, 2);
    for workers in 1..=8 {
        assert_eq!(
            Configuration::new(workers).map(Configuration::capacity),
            Some(2 * workers)
        );
    }
    assert_eq!(Configuration::new(0), None);
    assert_eq!(Configuration::new(9), None);
}

#[test]
fn j6_busy_returns_same_input_before_admission_and_uncertainty_retains_it() -> TestResult {
    let gates = Gates::new();
    gates.admit.close();
    let rig = Rig::new(1, gates.clone(), StartPolicy::Uncertain, false)?;
    let first = rig.start(input(1, OperationKind::Import, 1, 1_024))?;
    until(|| gates.admit.entries() == 1)?;
    let second = input(2, OperationKind::Export, 1_001, 3);
    let allocation = second.items.front().map(|item| item.payload.as_ptr());
    let rejected = match rig.handle.try_start(second) {
        Err(value) => value,
        Ok(_) => return Err("busy start accepted".into()),
    };
    assert_eq!(rejected.reason, Refusal::Busy);
    assert_eq!(
        rejected
            .input
            .items
            .front()
            .map(|item| item.payload.as_ptr()),
        allocation
    );
    assert_eq!(gates.admit.entries(), 1);
    gates.admit.open();
    until(|| rig.handle.measurements().uncertain_starts == 1)?;
    assert!(matches!(first.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert_eq!(rig.handle.measurements().ingress, 1);
    rig.handle.stop();
    let mut rig = rig;
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    rig.handle.wake_repair();
    assert!(matches!(
        first.recv_timeout(Duration::from_secs(10))?,
        StartReply::Created(_)
    ));
    assert!(joining.finish()?.joined);
    assert_eq!(guard(&rig.store).accepted.len(), 1);
    Ok(())
}

#[test]
fn j6_full_pipeline_measures_handles_payload_and_all_blocked_call_owners() -> TestResult {
    for workers in [1, 2, 8] {
        let gates = Gates::new();
        gates.report.close();
        let rig = Rig::new(workers, gates.clone(), StartPolicy::Create, false)?;
        rig.created(input(1, OperationKind::Export, 100, 4096))?;
        until(|| {
            let m = rig.handle.measurements();
            m.dispatched_handles == 3 * workers + 1 && m.item_bytes >= (3 * workers + 1) * 4096
        })?;
        until(|| gates.report.entries() == workers)?;
        gates.admit.close();
        let pending = rig.start(input(2, OperationKind::Import, 100, 7))?;
        until(|| gates.admit.entries() == 2)?;
        let m = rig.handle.measurements();
        assert_eq!(m.calls, workers + 1);
        assert_eq!(m.dispatched_handles, 3 * workers + 1);
        assert_eq!(m.ingress, 1);
        assert!(m.ingress_bytes > 100 * 7);
        assert!(m.item_bytes >= (3 * workers + 1) * 4096);
        assert!(m.result_bytes > 0);
        assert!(!m.joined);
        drop(pending);
        let m = rig.finish()?;
        assert!(m.joined);
        assert!(m.calls <= workers + 1 && m.dispatched_handles <= 3 * workers + 1);
    }
    Ok(())
}

#[test]
fn j6_import_picker_limit_does_not_truncate_export_or_clone_item_payloads() -> TestResult {
    let rig = Rig::new(2, Gates::new(), StartPolicy::Create, false)?;
    for count in [0, 1_001] {
        let refused = rig
            .handle
            .try_start(input(1, OperationKind::Import, count, 1));
        assert!(matches!(
            refused,
            Err(Unaccepted {
                reason: Refusal::InvalidCount,
                ..
            })
        ));
    }
    rig.created(input(2, OperationKind::Import, 1_000, 1))?;
    until(|| guard(&rig.store).recorded.len() == 1_000)?;
    let exported = input(3, OperationKind::Export, 1_117, 17);
    let original = exported
        .items
        .front()
        .map(|item| item.payload.as_ptr() as usize);
    rig.created(exported)?;
    until(|| guard(&rig.store).recorded.len() == 2_117)?;
    {
        let store = guard(&rig.store);
        let ordered: Vec<_> = store
            .offered
            .iter()
            .filter(|(job, _)| *job == JobId::from_bytes([3; 16]))
            .map(|(_, ordinal)| *ordinal)
            .collect();
        assert_eq!(ordered, (0..1_117).collect::<Vec<_>>());
        let first = store.recorded.iter().find(|result| {
            result.item.key.job == JobId::from_bytes([3; 16]) && result.item.ordinal == 0
        });
        assert_eq!(
            first.map(|result| result.item.payload.as_ptr() as usize),
            original
        );
        assert!(store.max_exclusions <= 6);
    }
    assert!(rig.finish()?.dispatched_handles <= 7);
    Ok(())
}

#[test]
fn j6_one_metadata_reservation_lasts_until_report_acknowledgement() -> TestResult {
    let gates = Gates::new();
    gates.report.close();
    let rig = Rig::new(1, gates.clone(), StartPolicy::Create, false)?;
    rig.created(input(1, OperationKind::MetadataSync, 1, 1))?;
    until(|| gates.report.entries() == 1)?;
    assert!(matches!(
        rig.handle
            .try_start(input(2, OperationKind::MetadataSync, 1, 1)),
        Err(Unaccepted {
            reason: Refusal::MetadataActive,
            ..
        })
    ));
    gates.report.open();
    until(|| rig.handle.measurements().recorded == 1)?;
    rig.created(input(2, OperationKind::MetadataSync, 1, 1))?;
    until(|| rig.handle.measurements().recorded == 2)?;
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_unacknowledged_reports_retain_worker_budget_until_explicit_readback() -> TestResult {
    let rig = Rig::new(2, Gates::new(), StartPolicy::Create, true)?;
    rig.created(input(1, OperationKind::Export, 20, 31))?;
    // The peak includes a reserved source call, which may return no item.
    // Wait for seven actual offers before stopping the retained pipeline.
    until(|| {
        let m = rig.handle.measurements();
        m.uncertain_reports == 2
            && m.dispatched_handles == 7
            && guard(&rig.store).offered.len() == 7
    })?;
    assert_eq!(guard(&rig.store).recorded.len(), 0);
    assert_eq!(rig.gates.execute.entries(), 2);
    let mut rig = rig;
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    // The full2worker/4queue/1prefetch pipeline is fixed before stop. Drive
    // explicit readback wakes until every one of those7 owners settles, rather
    // than racing a temporarily equal execute/report counter.
    until(|| {
        rig.handle.wake_repair();
        rig.handle.measurements().recorded == 7
    })?;
    let m = joining.finish()?;
    assert!(m.calls <= 3 && m.workers <= 2 && m.joined);
    let store = guard(&rig.store);
    assert_eq!(
        store
            .recorded
            .iter()
            .filter(
                |r| r.fact == "observed-success" && r.errors == ["independent-cleanup-diagnostic"]
            )
            .count(),
        2
    );
    assert_eq!(
        store
            .recorded
            .iter()
            .filter(|r| r.fact == "observed-cancelled-before-execution" && r.errors.is_empty())
            .count(),
        5
    );
    Ok(())
}

#[test]
fn j6_stop_is_independent_of_blocked_source_and_returns_unissued_start() -> TestResult {
    let gates = Gates::new();
    gates.source.close();
    let mut rig = Rig::new(2, gates.clone(), StartPolicy::Create, false)?;
    until(|| gates.source.entries() == 1)?;
    let value = input(1, OperationKind::Import, 1, 123);
    let allocation = value.items.front().map(|item| item.payload.as_ptr());
    let reply = rig.start(value)?;
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(gates.admit.entries(), 0);
    assert!(matches!(
        rig.handle.try_start(input(2, OperationKind::Export, 1, 1)),
        Err(Unaccepted {
            reason: Refusal::Closed,
            ..
        })
    ));
    gates.source.open();
    match reply.recv_timeout(Duration::from_secs(10))? {
        StartReply::Rejected(value) => assert_eq!(
            value.items.front().map(|item| item.payload.as_ptr()),
            allocation
        ),
        StartReply::Created(_) => return Err("unissued start fabricated acceptance".into()),
    }
    assert!(joining.finish()?.joined);
    assert!(guard(&rig.store).accepted.is_empty());
    Ok(())
}

#[test]
fn j6_created_record_survives_disconnected_reply_consumer_and_stop() -> TestResult {
    let gates = Gates::new();
    gates.admit.close();
    let mut rig = Rig::new(1, gates.clone(), StartPolicy::Create, false)?;
    let ticket = rig.start(input(1, OperationKind::Export, 30, 3))?;
    until(|| gates.admit.entries() == 1)?;
    drop(ticket);
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    gates.admit.open();
    assert!(joining.finish()?.joined);
    assert_eq!(guard(&rig.store).accepted, [JobId::from_bytes([1; 16])]);
    Ok(())
}

#[test]
fn j6_finite_cursor_cycle_rotates_operations_and_keeps_each_offer_ordered() -> TestResult {
    let store = Arc::new(Mutex::new(Store::default()));
    let _ = guard(&store).create(input(1, OperationKind::Export, 4, 1));
    let _ = guard(&store).create(input(2, OperationKind::Export, 4, 1));
    let gates = Gates::new();
    gates.execute.close();
    let rig = Rig::with_store(
        1,
        gates.clone(),
        StartPolicy::Create,
        false,
        Arc::clone(&store),
    )?;
    until(|| guard(&store).offered.len() == 4)?;
    // The coordinator must keep servicing one ingress even with its prefetch
    // held behind a saturated work queue. New arrivals cannot extend a cycle.
    rig.created(input(3, OperationKind::Export, 4, 1))?;
    gates.execute.open();
    until(|| guard(&store).recorded.len() == 12)?;
    let offered = guard(&store).offered.clone();
    assert_eq!(offered[0], (JobId::from_bytes([1; 16]), 0));
    assert_eq!(offered[1], (JobId::from_bytes([2; 16]), 0));
    assert_eq!(offered[2], (JobId::from_bytes([1; 16]), 1));
    assert_eq!(offered[4], (JobId::from_bytes([1; 16]), 2));
    assert_eq!(offered[5], (JobId::from_bytes([2; 16]), 2));
    assert_eq!(offered[6], (JobId::from_bytes([3; 16]), 0));
    for number in 1..=3 {
        let values: Vec<_> = offered
            .iter()
            .filter(|(id, _)| *id == JobId::from_bytes([number; 16]))
            .map(|(_, ordinal)| *ordinal)
            .collect();
        assert_eq!(values, (0..4).collect::<Vec<_>>());
    }
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_proven_rejection_returns_owned_input_and_releases_metadata_reservation() -> TestResult {
    let rig = Rig::new(1, Gates::new(), StartPolicy::Reject, false)?;
    let value = input(1, OperationKind::MetadataSync, 1, 33);
    let allocation = value.items.front().map(|item| item.payload.as_ptr());
    match rig.start(value)?.recv_timeout(Duration::from_secs(10))? {
        StartReply::Rejected(value) => assert_eq!(
            value.items.front().map(|item| item.payload.as_ptr()),
            allocation
        ),
        StartReply::Created(_) => return Err("rejection fabricated creation".into()),
    }
    let next = rig.start(input(2, OperationKind::MetadataSync, 1, 1))?;
    assert!(matches!(
        next.recv_timeout(Duration::from_secs(10))?,
        StartReply::Rejected(_)
    ));
    rig.handle.wake_source();
    assert!(guard(&rig.store).accepted.is_empty());
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_executor_callback_reentry_never_holds_queue_mutex() -> TestResult {
    let rig = Rig::new(1, Gates::new(), StartPolicy::Create, false)?;
    *guard(&rig.on_execute) = Some(rig.handle.clone());
    rig.created(input(1, OperationKind::Export, 3, 1))?;
    until(|| rig.handle.measurements().recorded >= 1)?;
    assert!(matches!(
        rig.handle.try_start(input(2, OperationKind::Export, 1, 1)),
        Err(Unaccepted {
            reason: Refusal::Closed,
            ..
        })
    ));
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_progress_is_coalesced_with_one_scalar_per_worker_and_one_wake() -> TestResult {
    let gates = Gates::new();
    gates.progress.close();
    gates.report.close();
    let rig = Rig::new(2, gates.clone(), StartPolicy::Create, false)?;
    let sinks = Arc::new(Mutex::new(Vec::<(ProgressSink, Value)>::new()));
    let hook_sinks = Arc::clone(&sinks);
    let report = Arc::clone(&gates.report);
    *guard(&rig.on_progress) = Some(Arc::new(move |item, sink, _cancelled| {
        let stamp = Stamp {
            generation: item.generation,
            item: item.key,
        };
        for revision in 1..=10_000 {
            assert!(sink.publish(Value {
                stamp,
                revision,
                completed: revision as i64 * 100,
                total: 1_000_000
            }));
        }
        guard(&hook_sinks).push((
            sink,
            Value {
                stamp,
                revision: 10_001,
                completed: 1_000_000,
                total: 1_000_000,
            },
        ));
        report.pass();
    }));
    let mut request = input(31, OperationKind::Export, 2, 17);
    for item in &mut request.items {
        item.total_bytes = 1_000_000;
    }
    rig.created(request)?;
    until(|| gates.report.entries() == 2 && gates.progress.entries() == 1)?;
    let values = rig.handle.take_progress().ok_or("missing wake")?;
    assert_eq!(values.len(), 2);
    assert!(
        values
            .iter()
            .all(|v| v.completed == 1_000_000 && v.revision == 10_000)
    );
    assert!(rig.handle.take_progress().is_none());
    let measured = rig.handle.measurements();
    assert_eq!(measured.progress_slots, 2);
    assert_eq!(measured.progress_wakes, 1);
    assert_eq!(measured.calls, 3);
    // Terminal reporting can finish while the earlier progress attempt remains
    // blocked. The guarded backend refuses that late progress after release.
    gates.report.open();
    until(|| guard(&rig.store).recorded.len() == 2)?;
    let store = Arc::clone(&rig.store);
    let measured = rig.finish()?;
    assert!(measured.joined);
    assert_eq!(measured.unsaved_progress, 1);
    assert!(guard(&store).progress.is_empty());
    assert_eq!(guard(&store).recorded.len(), 2); // fetch survives lost/coalesced hints
    for (sink, value) in guard(&sinks).iter() {
        assert!(!sink.publish(*value));
    }
    Ok(())
}

#[test]
fn j6_progress_rejects_wrong_identity_regression_and_preserves_export_threshold() {
    let stamp = Stamp {
        generation: Generation(7),
        item: ItemKey {
            job: JobId::from_bytes([7; 16]),
            row: 1,
        },
    };
    let mut slot = Slot::new(stamp, 600_000);
    let first = Value {
        stamp,
        revision: 1,
        completed: 262_143,
        total: 600_000,
    };
    assert!(slot.replace(first, true));
    assert!(!slot.pending);
    assert!(!slot.replace(
        Value {
            revision: 2,
            completed: -1,
            ..first
        },
        true
    ));
    assert!(!slot.replace(
        Value {
            revision: 2,
            completed: 262_142,
            ..first
        },
        true
    ));
    assert!(!slot.replace(
        Value {
            revision: 2,
            total: 600_001,
            ..first
        },
        true
    ));
    let mut foreign = first;
    foreign.stamp.generation = Generation(8);
    foreign.revision = 2;
    assert!(!slot.replace(foreign, true));
    foreign.stamp = stamp;
    foreign.stamp.item.row = 2;
    assert!(!slot.replace(foreign, true));
    assert!(!slot.replace(first, true));
    assert!(slot.replace(
        Value {
            revision: 2,
            completed: 262_144,
            ..first
        },
        true
    ));
    assert!(slot.pending);
    slot.pending = false;
    slot.saved_bytes = 262_144;
    assert!(slot.replace(
        Value {
            revision: 3,
            completed: 524_287,
            ..first
        },
        true
    ));
    assert!(!slot.pending);
    assert!(slot.replace(
        Value {
            revision: 4,
            completed: 600_000,
            ..first
        },
        true
    ));
    assert!(slot.pending);
    slot.sealed = true;
    assert!(!slot.replace(
        Value {
            revision: 5,
            completed: 600_000,
            ..first
        },
        true
    ));
    let mut import = Slot::new(stamp, 10);
    assert!(import.replace(
        Value {
            stamp,
            revision: 1,
            completed: 10,
            total: 10
        },
        false
    ));
    assert!(!import.pending); // no fabricated import persistence callback
}

#[test]
fn j6_saved_terminal_and_generation_query_stamps_reject_stale_observations() {
    use librairii_core::{
        OperationState,
        operations::{ObservationKind, OperationObservation},
    };
    let job = JobId::from_bytes([44; 16]);
    let mut view = Projection {
        generation: Generation(9),
        query: QueryRevision(2),
        job,
        current: None,
    };
    let running = Snapshot {
        generation: Generation(9),
        query: QueryRevision(2),
        job,
        revision: 1,
        observation: OperationObservation {
            state: OperationState::Running,
            kind: ObservationKind::StoredSnapshot,
        },
    };
    assert!(!view.apply(Snapshot {
        generation: Generation(8),
        ..running
    }));
    assert!(!view.apply(Snapshot {
        query: QueryRevision(1),
        ..running
    }));
    assert!(!view.apply(Snapshot {
        job: JobId::from_bytes([45; 16]),
        ..running
    }));
    assert!(view.apply(running));
    let provisional = Snapshot {
        revision: 100,
        observation: OperationObservation {
            state: OperationState::Interrupted,
            kind: ObservationKind::ProvisionalPersistenceFailure,
        },
        ..running
    };
    assert!(view.apply(provisional));
    assert_eq!(view.saved_terminal(), None);
    let saved = Snapshot {
        revision: 2,
        observation: OperationObservation {
            state: OperationState::Succeeded,
            kind: ObservationKind::StoredSnapshot,
        },
        ..running
    };
    assert!(view.apply(saved)); // saved terminal wins over newer provisional fallback
    assert!(!view.apply(Snapshot {
        revision: 101,
        ..running
    }));
    assert!(!view.apply(Snapshot {
        revision: 102,
        ..provisional
    }));
    assert!(!view.apply(Snapshot {
        revision: 103,
        observation: OperationObservation {
            state: OperationState::Cancelled,
            kind: ObservationKind::StoredSnapshot
        },
        ..saved
    }));
    assert_eq!(view.saved_terminal(), Some(OperationState::Succeeded));
    view.query = QueryRevision(3);
    view.current = None;
    assert!(!view.apply(saved));
    assert!(view.apply(Snapshot {
        query: QueryRevision(3),
        ..saved
    }));
    view.generation = Generation(10);
    view.current = None;
    assert!(!view.apply(Snapshot {
        query: QueryRevision(3),
        ..saved
    }));
}

fn control(number: u8) -> Control {
    Control {
        job: JobId::from_bytes([number; 16]),
        generation: Generation(1),
    }
}

fn cancel(rig: &Rig, request: Control) -> Result<mpsc::Receiver<ControlReply>, Box<dyn Error>> {
    rig.handle
        .try_cancel(request)
        .map_err(|e| format!("cancel refused: {:?}", e.reason).into())
}

#[test]
fn j6_cancel_is_recorded_before_signal_and_queued_skip_preserves_returned_fact() -> TestResult {
    let gates = Gates::new();
    gates.cancel.close();
    gates.report.close();
    let rig = Rig::new(1, gates.clone(), StartPolicy::Create, false)?;
    let signal = Arc::new(Mutex::new(None::<WorkCancellation>));
    let seen = Arc::clone(&signal);
    let gate = Arc::clone(&gates.report);
    *guard(&rig.on_progress) = Some(Arc::new(move |_, _, work| {
        *guard(&seen) = Some(work);
        gate.pass(); // controlled executor remains active across cancellation
    }));
    rig.created(input(51, OperationKind::Export, 8, 37))?;
    until(|| rig.handle.measurements().dispatched_handles == 4 && guard(&signal).is_some())?;
    let reply = cancel(&rig, control(51))?;
    until(|| gates.cancel.entries() == 1)?;
    assert!(
        !guard(&signal)
            .as_ref()
            .ok_or("missing signal")?
            .is_requested()
    );
    let other = control(52);
    match rig.handle.try_cancel(other) {
        Err(value) => {
            assert_eq!(value.reason, Refusal::Busy);
            assert_eq!(value.request, other);
        }
        Ok(_) => return Err("second control accepted".into()),
    }
    let m = rig.handle.measurements();
    assert_eq!((m.control, m.calls, m.dispatched_handles), (1, 2, 4));
    assert_eq!(m.control_bytes, std::mem::size_of::<Control>());
    assert!(guard(&rig.store).cancelled.is_empty());
    gates.cancel.open();
    assert_eq!(
        reply.recv_timeout(Duration::from_secs(10))?,
        ControlReply::Recorded
    );
    assert!(
        guard(&signal)
            .as_ref()
            .ok_or("missing signal")?
            .is_requested()
    );
    gates.report.open();
    until(|| guard(&rig.store).recorded.len() == 8)?;
    assert_eq!(gates.execute.entries(), 1); // queued and future source items skip executor
    {
        let store = guard(&rig.store);
        assert_eq!(store.recorded[0].fact, "observed-success");
        assert_eq!(store.recorded[0].errors, ["independent-cleanup-diagnostic"]);
        assert!(
            store.recorded[1..]
                .iter()
                .all(|v| v.fact == "observed-cancelled-before-execution")
        );
        assert_eq!(
            store.offered.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
            (0..8).collect::<Vec<_>>()
        );
    }
    rig.handle.stop();
    match rig.handle.try_cancel(other) {
        Err(value) => {
            assert_eq!(value.reason, Refusal::Closed);
            assert_eq!(value.request, other);
        }
        Ok(_) => return Err("control accepted after stop".into()),
    }
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_rejected_or_other_generation_control_does_not_cancel_running_work() -> TestResult {
    let gates = Gates::new();
    gates.report.close();
    let rig = Rig::new(1, gates.clone(), StartPolicy::Create, false)?;
    let signal = Arc::new(Mutex::new(None::<WorkCancellation>));
    let seen = Arc::clone(&signal);
    let gate = Arc::clone(&gates.report);
    *guard(&rig.on_progress) = Some(Arc::new(move |_, _, work| {
        *guard(&seen) = Some(work);
        gate.pass();
    }));
    rig.created(input(61, OperationKind::Import, 1, 4))?;
    until(|| guard(&signal).is_some())?;
    *guard(&rig.control_policy) = ControlPolicy::Reject;
    assert_eq!(
        cancel(&rig, control(61))?.recv_timeout(Duration::from_secs(10))?,
        ControlReply::Rejected(control(61))
    );
    assert!(
        !guard(&signal)
            .as_ref()
            .ok_or("missing signal")?
            .is_requested()
    );
    assert!(guard(&rig.store).cancelled.is_empty());
    *guard(&rig.control_policy) = ControlPolicy::Record;
    let foreign = Control {
        generation: Generation(2),
        ..control(61)
    };
    assert_eq!(
        cancel(&rig, foreign)?.recv_timeout(Duration::from_secs(10))?,
        ControlReply::Recorded
    );
    assert!(
        !guard(&signal)
            .as_ref()
            .ok_or("missing signal")?
            .is_requested()
    );
    gates.report.open();
    until(|| rig.handle.measurements().recorded == 1)?;
    assert_eq!(guard(&rig.store).recorded[0].fact, "observed-success");
    assert!(rig.finish()?.joined);
    Ok(())
}

#[test]
fn j6_uncertain_control_and_symbolic_report_owner_survive_stop_until_actual_readback() -> TestResult
{
    let gates = Gates::new();
    gates.cancel_readback.close();
    gates.report_readback.close();
    let mut rig = Rig::new(1, gates.clone(), StartPolicy::Create, true)?;
    *guard(&rig.control_policy) = ControlPolicy::Uncertain;
    let request = input(71, OperationKind::MetadataSync, 1, 32);
    let allocation = request.items.front().map(|i| i.payload.as_ptr() as usize);
    rig.created(request)?;
    until(|| rig.handle.measurements().uncertain_reports == 1)?;
    let reply = cancel(&rig, control(71))?;
    until(|| rig.handle.measurements().uncertain_controls == 1)?;
    assert!(matches!(reply.try_recv(), Err(mpsc::TryRecvError::Empty)));
    assert!(guard(&rig.store).recorded.is_empty());
    assert!(guard(&rig.store).cancelled.is_empty());
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    rig.handle.wake_repair();
    until(|| gates.cancel_readback.entries() == 1 && gates.report_readback.entries() == 1)?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    gates.cancel_readback.open();
    assert_eq!(
        reply.recv_timeout(Duration::from_secs(10))?,
        ControlReply::Recorded
    );
    assert!(guard(&rig.store).recorded.is_empty());
    // A mere queued/readback intent did not transfer this move-only result.
    // The controlled backend now actually accepts the same allocation+facts.
    gates.report_readback.open();
    let measured = joining.finish()?;
    assert!(measured.joined && measured.workers == 1 && measured.control == 1);
    let store = guard(&rig.store);
    assert_eq!(store.recorded.len(), 1);
    assert_eq!(
        Some(store.recorded[0].item.payload.as_ptr() as usize),
        allocation
    );
    assert_eq!(store.recorded[0].fact, "observed-success");
    assert_eq!(store.recorded[0].errors, ["independent-cleanup-diagnostic"]);
    assert_eq!(gates.cancel.entries(), 1);
    assert_eq!(gates.cancel_readback.entries(), 1);
    Ok(())
}

#[test]
fn j6_stop_returns_unissued_control_without_claiming_cancel_recorded() -> TestResult {
    let gates = Gates::new();
    gates.source.close();
    let mut rig = Rig::new(1, gates.clone(), StartPolicy::Create, false)?;
    until(|| gates.source.entries() == 1)?;
    let request = control(81);
    let reply = cancel(&rig, request)?;
    let joining = rig.joining()?;
    assert!(matches!(
        joining.receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(gates.cancel.entries(), 0);
    gates.source.open();
    assert_eq!(
        reply.recv_timeout(Duration::from_secs(10))?,
        ControlReply::Rejected(request)
    );
    assert!(joining.finish()?.joined);
    assert!(guard(&rig.store).cancelled.is_empty());
    Ok(())
}
