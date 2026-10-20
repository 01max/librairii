use super::model::*;
use super::progress::{Slot, Stamp, Value};
use std::{
    collections::VecDeque,
    io,
    sync::{
        Arc, Condvar, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
};

struct PendingStart {
    input: StartInput,
    reply: SyncSender<StartReply>,
}

struct Working {
    key: ItemKey,
    item_bytes: usize,
    result_bytes: usize,
    call: bool,
    progress: Slot,
    export: bool,
}

struct State {
    queue: VecDeque<Item>,
    prefetch: Option<Item>,
    source_reserved: bool,
    ingress: Option<PendingStart>,
    ingress_bytes: usize,
    ingress_occupied: bool,
    metadata_active: bool,
    workers: Vec<Option<Working>>,
    coordinator_call: bool,
    coordinator_done: bool,
    worker_failed: bool,
    version: u64,
    source_version: u64,
    repair_version: u64,
    measured: Measurements,
    progress_wake: bool,
    progress_cursor: usize,
}

impl State {
    fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.measure();
    }

    fn measure(&mut self) {
        let running = self.workers.iter().flatten().count();
        let fetched = usize::from(self.source_reserved || self.prefetch.is_some());
        let calls = usize::from(self.coordinator_call)
            + self.workers.iter().flatten().filter(|w| w.call).count();
        let item_bytes = self.queue.iter().map(Item::bytes).sum::<usize>()
            + self.prefetch.as_ref().map_or(0, Item::bytes)
            + self
                .workers
                .iter()
                .flatten()
                .map(|w| w.item_bytes)
                .sum::<usize>();
        self.measured.ready = self.measured.ready.max(self.queue.len());
        self.measured.workers = self.measured.workers.max(running);
        self.measured.prefetch = self.measured.prefetch.max(fetched);
        self.measured.ingress = self
            .measured
            .ingress
            .max(usize::from(self.ingress_occupied));
        self.measured.calls = self.measured.calls.max(calls);
        self.measured.progress_slots = self.measured.progress_slots.max(running);
        self.measured.progress_wakes = self
            .measured
            .progress_wakes
            .max(usize::from(self.progress_wake));
        self.measured.dispatched_handles = self
            .measured
            .dispatched_handles
            .max(self.queue.len() + running + fetched);
        self.measured.ingress_bytes = self.measured.ingress_bytes.max(self.ingress_bytes);
        self.measured.item_bytes = self.measured.item_bytes.max(item_bytes);
        self.measured.result_bytes = self
            .measured
            .result_bytes
            .max(self.workers.iter().flatten().map(|w| w.result_bytes).sum());
    }

    fn exclusions(&self) -> Vec<ItemKey> {
        self.queue
            .iter()
            .map(|item| item.key)
            .chain(self.prefetch.iter().map(|item| item.key))
            .chain(self.workers.iter().flatten().map(|worker| worker.key))
            .collect()
    }
}

struct Shared {
    configuration: Configuration,
    state: Mutex<State>,
    changed: Condvar,
    stopping: AtomicBool,
}

#[derive(Clone)]
pub(super) struct ProgressSink {
    shared: Arc<Shared>,
    index: usize,
    stamp: Stamp,
}

impl ProgressSink {
    pub(super) fn publish(&self, value: Value) -> bool {
        let mut state = lock(&self.shared.state);
        let Some(worker) = &mut state.workers[self.index] else {
            return false;
        };
        if self.stamp != worker.progress.stamp || !worker.progress.replace(value, worker.export) {
            return false;
        }
        state.progress_wake = true;
        state.source_version = state.source_version.wrapping_add(1);
        state.changed();
        self.shared.changed.notify_all();
        true
    }
}

// Callbacks never run with this mutex held. Recovering the guard lets owned
// threads be joined even after an internal panic; join still reports that panic.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn wait<'a>(shared: &'a Shared, guard: MutexGuard<'a, State>) -> MutexGuard<'a, State> {
    match shared.changed.wait(guard) {
        Ok(value) => value,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Clone)]
pub(super) struct Handle {
    shared: Arc<Shared>,
}

impl Handle {
    pub(super) fn try_start(&self, input: StartInput) -> Result<Receiver<StartReply>, Unaccepted> {
        let mut state = lock(&self.shared.state);
        let refusal = if self.shared.stopping.load(Ordering::Acquire) {
            Some(Refusal::Closed)
        } else if !input.valid_count() {
            Some(Refusal::InvalidCount)
        } else if input.kind == librairii_core::OperationKind::MetadataSync && state.metadata_active
        {
            Some(Refusal::MetadataActive)
        } else if state.ingress_occupied {
            Some(Refusal::Busy)
        } else {
            None
        };
        if let Some(reason) = refusal {
            return Err(Unaccepted { reason, input });
        }
        let (sender, receiver) = sync_channel(1);
        state.metadata_active |= input.kind == librairii_core::OperationKind::MetadataSync;
        state.ingress_bytes = input.bytes;
        state.ingress_occupied = true;
        state.ingress = Some(PendingStart {
            input,
            reply: sender,
        });
        state.source_version = state.source_version.wrapping_add(1);
        state.changed();
        self.shared.changed.notify_all();
        Ok(receiver)
    }

    pub(super) fn stop(&self) {
        self.shared.stopping.store(true, Ordering::Release);
        let mut state = lock(&self.shared.state);
        state.changed();
        self.shared.changed.notify_all();
    }

    pub(super) fn wake_source(&self) {
        let mut state = lock(&self.shared.state);
        state.source_version = state.source_version.wrapping_add(1);
        state.changed();
        self.shared.changed.notify_all();
    }

    /// An explicit controlled readback wake, not an automatic blind retry.
    pub(super) fn wake_repair(&self) {
        let mut state = lock(&self.shared.state);
        state.repair_version = state.repair_version.wrapping_add(1);
        state.changed();
        self.shared.changed.notify_all();
    }

    pub(super) fn measurements(&self) -> Measurements {
        lock(&self.shared.state).measured.clone()
    }

    /// Notification loss/coalescing is intentional. The receiver owns this
    /// bounded snapshot; terminal truth is fetched from the report backend.
    pub(super) fn take_progress(&self) -> Option<Vec<Value>> {
        let mut state = lock(&self.shared.state);
        if !state.progress_wake {
            return None;
        }
        state.progress_wake = false;
        Some(
            state
                .workers
                .iter()
                .flatten()
                .map(|w| w.progress.latest)
                .collect(),
        )
    }
}

pub(super) struct Engine {
    handle: Handle,
    threads: Vec<JoinHandle<()>>,
}

impl Engine {
    pub(super) fn start(
        configuration: Configuration,
        coordinator: CoordinatorCallbacks,
        worker: WorkerCallbacks,
    ) -> io::Result<Self> {
        if Configuration::new(configuration.workers).is_none() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "worker count"));
        }
        let shared = Arc::new(Shared {
            configuration,
            state: Mutex::new(State {
                queue: VecDeque::with_capacity(configuration.capacity()),
                prefetch: None,
                source_reserved: false,
                ingress: None,
                ingress_bytes: 0,
                ingress_occupied: false,
                metadata_active: false,
                workers: (0..configuration.workers).map(|_| None).collect(),
                coordinator_call: false,
                coordinator_done: false,
                worker_failed: false,
                version: 0,
                source_version: 0,
                repair_version: 0,
                measured: Measurements::default(),
                progress_wake: false,
                progress_cursor: 0,
            }),
            changed: Condvar::new(),
            stopping: AtomicBool::new(false),
        });
        let mut engine = Self {
            handle: Handle { shared },
            threads: Vec::with_capacity(configuration.workers + 1),
        };
        for index in 0..configuration.workers {
            let shared = Arc::clone(&engine.handle.shared);
            let callbacks = worker.clone();
            let thread = thread::Builder::new()
                .name(format!("librairii-job-{index}"))
                .spawn(move || {
                    let _exit = WorkerExit(Arc::clone(&shared));
                    worker_loop(shared, index, callbacks);
                });
            match thread {
                Ok(thread) => engine.threads.push(thread),
                Err(error) => {
                    lock(&engine.handle.shared.state).coordinator_done = true;
                    return Err(error);
                }
            }
        }
        let shared = Arc::clone(&engine.handle.shared);
        let thread = thread::Builder::new()
            .name("librairii-job-dispatch".into())
            .spawn(move || {
                let _exit = CoordinatorExit(Arc::clone(&shared));
                coordinator_loop(&shared, coordinator);
            });
        match thread {
            Ok(thread) => engine.threads.push(thread),
            Err(error) => {
                lock(&engine.handle.shared.state).coordinator_done = true;
                return Err(error);
            }
        }
        Ok(engine)
    }

    pub(super) fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub(super) fn join(mut self) -> io::Result<Measurements> {
        self.join_owned()?;
        let mut state = lock(&self.handle.shared.state);
        state.measured.joined = true;
        Ok(state.measured.clone())
    }

    fn join_owned(&mut self) -> io::Result<()> {
        self.handle.stop();
        let mut failed = false;
        for thread in self.threads.drain(..) {
            failed |= thread.join().is_err();
        }
        if failed {
            Err(io::Error::other("owned job thread panicked"))
        } else {
            Ok(())
        }
    }
}

struct CoordinatorExit(Arc<Shared>);

impl Drop for CoordinatorExit {
    fn drop(&mut self) {
        let mut state = lock(&self.0.state);
        state.coordinator_done = true;
        state.changed();
        self.0.changed.notify_all();
    }
}

struct WorkerExit(Arc<Shared>);

impl Drop for WorkerExit {
    fn drop(&mut self) {
        if thread::panicking() {
            self.0.stopping.store(true, Ordering::Release);
            let mut state = lock(&self.0.state);
            state.worker_failed = true;
            state.changed();
            self.0.changed.notify_all();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Dropping a JoinHandle would detach it. A blocked required port keeps
        // this drain pending; there is deliberately no deadline/detach path.
        let _ = self.join_owned();
    }
}

enum CallOwner {
    Coordinator,
    Worker(usize),
}

struct Call {
    shared: Arc<Shared>,
    owner: CallOwner,
}

impl Call {
    fn enter(shared: &Arc<Shared>, owner: CallOwner) -> Self {
        let mut state = lock(&shared.state);
        match owner {
            CallOwner::Coordinator => state.coordinator_call = true,
            CallOwner::Worker(index) => {
                if let Some(worker) = &mut state.workers[index] {
                    worker.call = true;
                }
            }
        }
        state.changed();
        shared.changed.notify_all();
        Self {
            shared: Arc::clone(shared),
            owner,
        }
    }
}

impl Drop for Call {
    fn drop(&mut self) {
        let mut state = lock(&self.shared.state);
        match self.owner {
            CallOwner::Coordinator => state.coordinator_call = false,
            CallOwner::Worker(index) => {
                if let Some(worker) = &mut state.workers[index] {
                    worker.call = false;
                }
            }
        }
        state.changed();
        self.shared.changed.notify_all();
    }
}

fn wait_repair(shared: &Shared, seen: u64) {
    let mut state = lock(&shared.state);
    while state.repair_version == seen {
        state = wait(shared, state);
    }
}

fn admit(shared: &Arc<Shared>, callbacks: &CoordinatorCallbacks, pending: PendingStart) {
    let metadata = pending.input.kind == librairii_core::OperationKind::MetadataSync;
    let mut seen = lock(&shared.state).repair_version;
    let mut response = {
        let _call = Call::enter(shared, CallOwner::Coordinator);
        (callbacks.admit)(pending.input)
    };
    let reply = loop {
        response = match response {
            Admission::Created(job) => {
                // A disconnected consumer cannot undo an acknowledged record.
                break StartReply::Created(job);
            }
            Admission::Rejected(input) => {
                if metadata {
                    lock(&shared.state).metadata_active = false;
                }
                break StartReply::Rejected(input);
            }
            Admission::Uncertain(input) => {
                {
                    let mut state = lock(&shared.state);
                    state.measured.uncertain_starts += 1;
                    state.changed();
                    shared.changed.notify_all();
                }
                wait_repair(shared, seen);
                seen = lock(&shared.state).repair_version;
                let _call = Call::enter(shared, CallOwner::Coordinator);
                (callbacks.readback_start)(input)
            }
        };
    };
    let mut state = lock(&shared.state);
    state.ingress_bytes = 0;
    state.ingress_occupied = false;
    state.source_version = state.source_version.wrapping_add(1);
    state.changed();
    shared.changed.notify_all();
    drop(state);
    let _ = pending.reply.send(reply);
}

fn coordinator_loop(shared: &Arc<Shared>, callbacks: CoordinatorCallbacks) {
    let mut cursor = Cursor::default();
    loop {
        let mut state = lock(&shared.state);
        let source_version = state.source_version;
        if shared.stopping.load(Ordering::Acquire) {
            if let Some(pending) = state.ingress.take() {
                state.ingress_occupied = false;
                state.ingress_bytes = 0;
                drop(state);
                let _ = pending.reply.send(StartReply::Rejected(pending.input));
                continue;
            }
            if state.worker_failed {
                return;
            }
            if state.prefetch.is_some() {
                if state.queue.len() == shared.configuration.capacity() {
                    drop(wait(shared, state));
                    continue;
                }
                if let Some(item) = state.prefetch.take() {
                    state.queue.push_back(item);
                    state.changed();
                    shared.changed.notify_all();
                }
            }
            return;
        }
        if let Some(pending) = state.ingress.take() {
            drop(state);
            admit(shared, &callbacks, pending);
            continue;
        }
        // Rotating bounded scalar mailboxes share the coordinator's sole call
        // owner. No callback is made under the queue mutex.
        let next = (0..shared.configuration.workers).find_map(|offset| {
            let index = (state.progress_cursor + offset) % shared.configuration.workers;
            state.workers[index]
                .as_ref()
                .filter(|w| w.progress.pending)
                .map(|w| (index, w.progress.latest))
        });
        if let Some((index, value)) = next {
            if let Some(worker) = &mut state.workers[index] {
                worker.progress.pending = false;
            }
            state.progress_cursor = (index + 1) % shared.configuration.workers;
            drop(state);
            let saved = {
                let _call = Call::enter(shared, CallOwner::Coordinator);
                (callbacks.progress)(value)
            };
            let mut state = lock(&shared.state);
            if saved {
                state.measured.progress_saves += 1;
                if let Some(worker) = &mut state.workers[index]
                    && worker.progress.stamp == value.stamp
                {
                    worker.progress.saved_bytes = value.completed;
                    worker.progress.pending = worker.progress.latest.completed > value.completed
                        && (worker.progress.latest.completed - value.completed >= 256 * 1024
                            || worker.progress.latest.completed == worker.progress.latest.total);
                }
            } else {
                state.measured.unsaved_progress += 1;
            }
            continue;
        }
        if state.prefetch.is_some() {
            if state.queue.len() < shared.configuration.capacity() {
                if let Some(item) = state.prefetch.take() {
                    state.queue.push_back(item);
                    state.changed();
                    shared.changed.notify_all();
                }
            } else {
                drop(wait(shared, state));
            }
            continue;
        }
        state.source_reserved = true;
        let excluded = state.exclusions();
        state.changed();
        drop(state);
        let page = {
            let _call = Call::enter(shared, CallOwner::Coordinator);
            (callbacks.source)(SourceRequest {
                cursor: cursor.clone(),
                excluded,
            })
        };
        let had_cycle = cursor.upper.is_some();
        cursor = page.cursor;
        let mut state = lock(&shared.state);
        state.source_reserved = false;
        if let Some(item) = page.item {
            state.prefetch = Some(item);
            state.changed();
            shared.changed.notify_all();
            continue;
        }
        if page.end_cycle {
            cursor = Cursor::default();
            if had_cycle {
                continue;
            }
        }
        // Call enter/exit updated the measurement version. A source-empty read
        // waits for an external mutation, not for its own measurement updates.
        if state.ingress.is_some() || shared.stopping.load(Ordering::Acquire) {
            continue;
        }
        while state.source_version == source_version
            && !shared.stopping.load(Ordering::Acquire)
            && state.ingress.is_none()
        {
            state = wait(shared, state);
        }
    }
}

fn worker_loop(shared: Arc<Shared>, index: usize, callbacks: WorkerCallbacks) {
    loop {
        let item = {
            let mut state = lock(&shared.state);
            loop {
                if let Some(item) = state.queue.pop_front() {
                    state.workers[index] = Some(Working {
                        key: item.key,
                        item_bytes: item.bytes(),
                        result_bytes: 0,
                        call: false,
                        progress: Slot::new(
                            Stamp {
                                generation: item.generation,
                                item: item.key,
                            },
                            item.total_bytes,
                        ),
                        export: item.kind == librairii_core::OperationKind::Export,
                    });
                    state.source_version = state.source_version.wrapping_add(1);
                    state.changed();
                    shared.changed.notify_all();
                    break item;
                }
                if state.coordinator_done && shared.stopping.load(Ordering::Acquire) {
                    return;
                }
                state = wait(&shared, state);
            }
        };
        let kind = item.kind;
        let sink = ProgressSink {
            shared: Arc::clone(&shared),
            index,
            stamp: Stamp {
                generation: item.generation,
                item: item.key,
            },
        };
        let completed = {
            let _call = Call::enter(&shared, CallOwner::Worker(index));
            (callbacks.execute)(item, sink)
        };
        {
            let mut state = lock(&shared.state);
            if let Some(worker) = &mut state.workers[index] {
                worker.progress.sealed = true;
                worker.result_bytes = completed.bytes() - completed.item.bytes();
            }
            state.changed();
        }
        let mut seen = lock(&shared.state).repair_version;
        let mut response = {
            let _call = Call::enter(&shared, CallOwner::Worker(index));
            (callbacks.report)(completed)
        };
        loop {
            response = match response {
                Report::Recorded { terminal_operation } => {
                    let mut state = lock(&shared.state);
                    if terminal_operation && kind == librairii_core::OperationKind::MetadataSync {
                        state.metadata_active = false;
                    }
                    state.measured.recorded += 1;
                    state.workers[index] = None;
                    state.source_version = state.source_version.wrapping_add(1);
                    state.changed();
                    shared.changed.notify_all();
                    break;
                }
                Report::Uncertain(completed) => {
                    {
                        let mut state = lock(&shared.state);
                        state.measured.uncertain_reports += 1;
                        state.changed();
                        shared.changed.notify_all();
                    }
                    wait_repair(&shared, seen);
                    seen = lock(&shared.state).repair_version;
                    let _call = Call::enter(&shared, CallOwner::Worker(index));
                    (callbacks.readback_report)(completed)
                }
            };
        }
    }
}
