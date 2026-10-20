use super::{model::*, runtime::*};
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
        self.max_exclusions = self.max_exclusions.max(request.excluded.len());
        let upper = request
            .cursor
            .upper
            .or_else(|| self.jobs.keys().next_back().cloned());
        for (key, items) in &mut self.jobs {
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
            if let Some(item) = index.and_then(|index| items.remove(index)) {
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

    fn save(&mut self, completed: Completed) -> Report {
        self.recorded.push(completed);
        Report::Recorded {
            terminal_operation: true,
        }
    }
}

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

struct Rig {
    engine: Option<Engine>,
    handle: Handle,
    gates: Gates,
    store: Arc<Mutex<Store>>,
    on_execute: Arc<Mutex<Option<Handle>>>,
}

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
        let callbacks = WorkerCallbacks {
            execute: {
                let gate = Arc::clone(&gates.execute);
                let on_execute = Arc::clone(&on_execute);
                Arc::new(move |item| {
                    gate.pass();
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
                Arc::new(move |completed| {
                    gate.pass();
                    if uncertain_report {
                        Report::Uncertain(completed)
                    } else {
                        guard(&store).save(completed)
                    }
                })
            },
            readback_report: {
                let gate = Arc::clone(&gates.report_readback);
                let store = Arc::clone(&store);
                Arc::new(move |completed| {
                    gate.pass();
                    guard(&store).save(completed)
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
            m.dispatched_handles == 3 * workers + 1
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
    until(|| {
        let m = rig.handle.measurements();
        m.uncertain_reports == 2 && m.dispatched_handles == 7
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
    assert!(guard(&rig.store).recorded.iter().all(|r| r.fact == "observed-success" && r.errors == ["independent-cleanup-diagnostic"]));
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
