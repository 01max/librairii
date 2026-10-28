use super::*;
use crate::ports::{
    jobs::{JobStore, StartObservation, StoreResult},
    operations::*,
};
use std::{
    collections::VecDeque,
    sync::{Condvar, Mutex},
    thread,
    time::Duration,
};

const TICK: Duration = Duration::from_millis(10);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Busy,
    Closed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Admission {
    pub id: String,
    pub accepted: bool,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStatus {
    pub queued: usize,
    pub running: usize,
    pub stopping: bool,
    pub failed: bool,
    pub starting: bool,
    pub drained: bool,
}
#[derive(Debug)]
pub enum SessionError {
    Unavailable,
    ThreadStart,
    WorkerFailed,
    RecoveryRequired,
}
impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "operation session is unavailable",
            Self::ThreadStart => "operation threads could not start",
            Self::WorkerFailed => "operation worker failed; library requires recovery",
            Self::RecoveryRequired => "operation recovery is required",
        })
    }
}
impl std::error::Error for SessionError {}
#[derive(Debug)]
struct State {
    ingress: Option<Start>,
    admitting: bool,
    admission: Option<Admission>,
    queue: VecDeque<WorkItem>,
    inflight: Vec<(i64, String, Cancellation)>,
    running: usize,
    stopping: bool,
    done: bool,
    failed: bool,
    ready: bool,
    stop_requested_at: Option<std::time::Instant>,
    cancel: Option<String>,
}
#[derive(Debug)]
pub(super) struct Shared {
    state: Mutex<State>,
    changed: Condvar,
    config: Configuration,
    generation: Generation,
    progress: Vec<Arc<Mutex<super::progress::Slot>>>,
    dirty: Arc<AtomicBool>,
    revision: std::sync::atomic::AtomicU64,
    metadata_owner: AtomicBool,
}
/// Handles can outlive a session, but then return Closed. A new session has a
/// different Arc identity and cannot be reached through a stale handle.
#[derive(Clone, Debug)]
pub struct Handle(pub(super) Arc<Shared>);
impl Handle {
    /// Exact admission/worker quiescence for lifecycle switching. Queue and
    /// running counts alone omit an input being durably admitted.
    pub fn idle_for_relocation(&self) -> bool {
        let s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        s.ready
            && !s.failed
            && !s.stopping
            && s.ingress.is_none()
            && !s.admitting
            && s.queue.is_empty()
            && s.running == 0
            && s.inflight.is_empty()
    }
    pub fn generation(&self) -> Generation {
        self.0.generation.clone()
    }
    pub fn take_dirty(&self) -> bool {
        self.0.dirty.swap(false, Ordering::AcqRel)
    }
    pub fn has_critical_work(&self) -> bool {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        self.0.progress.iter().any(|slot| {
            slot.lock()
                .unwrap_or_else(|e| e.into_inner())
                .latest
                .as_ref()
                .is_some_and(|p| {
                    p.phase != super::Phase::Running
                        && state.inflight.iter().any(|(id, _, _)| *id == p.item)
                })
        })
    }
    pub fn take_progress(&self) -> Vec<ProgressUpdate> {
        self.0
            .progress
            .iter()
            .filter_map(|slot| {
                let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
                if !slot.dirty {
                    return None;
                }
                slot.dirty = false;
                slot.latest.clone()
            })
            .collect()
    }
    pub fn request_stamp(
        &self,
        query: Option<librairii_core::navigation::QueryStamp>,
    ) -> Option<RequestStamp> {
        let revision = self
            .0
            .revision
            .try_update(Ordering::AcqRel, Ordering::Acquire, |v| v.checked_add(1))
            .ok()?;
        Some(RequestStamp {
            generation: self.generation(),
            revision,
            query,
        })
    }
    pub(super) fn accepts_stamp(
        &self,
        stamp: &RequestStamp,
        nav: Option<&librairii_core::navigation::Navigation>,
    ) -> bool {
        let s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        !s.done
            && !s.failed
            && stamp.generation == self.0.generation
            && stamp
                .query
                .as_ref()
                .is_none_or(|q| nav.is_some_and(|nav| nav.accepts(q)))
    }

    pub(super) fn shutdown_observation(&self) -> (RuntimeStatus, Duration, Vec<i64>) {
        let s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        (
            RuntimeStatus {
                queued: s.queue.len(),
                running: s.running,
                stopping: s.stopping,
                failed: s.failed,
                starting: !s.ready && !s.done,
                drained: s.done && !s.failed,
            },
            s.stop_requested_at
                .map_or(Duration::ZERO, |at| at.elapsed()),
            s.inflight.iter().map(|(id, _, _)| *id).collect(),
        )
    }
    pub(super) fn take_phases(&self, active: &[i64]) -> Vec<Phase> {
        self.0
            .progress
            .iter()
            .filter_map(|slot| {
                slot.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .latest
                    .as_ref()
                    .filter(|p| active.contains(&p.item))
                    .map(|p| p.phase)
            })
            .collect()
    }
    pub fn try_start(&self, start: Start) -> Result<(), (Refusal, Start)> {
        let Ok(mut s) = self.0.state.try_lock() else {
            return Err((Refusal::Busy, start));
        };
        if s.stopping || s.done {
            return Err((Refusal::Closed, start));
        }
        if !s.ready || s.admitting || s.ingress.is_some() || s.admission.is_some() {
            return Err((Refusal::Busy, start));
        }
        s.ingress = Some(start);
        self.0.changed.notify_all();
        Ok(())
    }
    /// Acceptance here is asynchronous writer acceptance, distinct from merely
    /// offering a start command. At most one unread acknowledgement is retained.
    pub fn take_admission(&self) -> Option<Admission> {
        self.0.state.lock().ok()?.admission.take()
    }
    pub fn cancel(&self, id: String) -> Result<(), (Refusal, String)> {
        if id.capacity() > crate::ports::creation::MAX_COMMAND_BYTES {
            return Err((Refusal::Busy, id));
        }
        let Ok(mut s) = self.0.state.try_lock() else {
            return Err((Refusal::Busy, id));
        };
        if s.stopping || s.done {
            return Err((Refusal::Closed, id));
        }
        if s.cancel.is_some() {
            return Err((Refusal::Busy, id));
        }
        // Wake executing work promptly. SQL cancellation still follows on the
        // coordinator; these tokens do not acknowledge a database transition.
        for (_, operation, token) in &s.inflight {
            if *operation == id {
                token.cancel();
            }
        }
        s.cancel = Some(id);
        self.0.changed.notify_all();
        Ok(())
    }
    pub fn shutdown(&self) {
        if let Ok(mut s) = self.0.state.lock() {
            s.stopping = true;
            s.stop_requested_at
                .get_or_insert_with(std::time::Instant::now);
            for (_, _, c) in &s.inflight {
                c.cancel();
            }
            self.0.changed.notify_all();
        }
    }
    pub fn status(&self) -> RuntimeStatus {
        let s = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        RuntimeStatus {
            queued: s.queue.len(),
            running: s.running,
            stopping: s.stopping,
            failed: s.failed,
            starting: !s.ready && !s.done,
            drained: s.done && !s.failed,
        }
    }
}
fn guarded(store: &impl JobStore, shared: &Shared, run: impl FnOnce()) {
    if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        store.fence();
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.done = true;
        state.failed = true;
        state.stopping = true;
        for (_, _, token) in &state.inflight {
            token.cancel();
        }
        shared.changed.notify_all();
        drop(state);
        std::panic::resume_unwind(panic);
    }
}
fn now() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
fn pause() {
    thread::sleep(TICK);
}
fn admit(store: &impl JobStore, input: &NewOperation) -> bool {
    let at = now();
    let mut unknown = false;
    loop {
        if unknown {
            match store.creation_readback(input, &at) {
                StoreResult::Known(true) => return true,
                StoreResult::Known(false) => unknown = false,
                StoreResult::Rejected => return false,
                _ => {
                    pause();
                    continue;
                }
            }
        }
        match store.create(input, &at) {
            StoreResult::Known(()) => return true,
            StoreResult::Rejected => return false,
            StoreResult::Unknown => unknown = true,
            StoreResult::Retry => {}
        }
        pause();
    }
}
fn start(store: &impl JobStore, work: &WorkItem) -> bool {
    let at = now();
    let mut unknown = false;
    loop {
        if unknown {
            match store.start_readback(work) {
                StoreResult::Known(StartObservation::Running) => return true,
                StoreResult::Known(StartObservation::Pending) => unknown = false,
                StoreResult::Known(StartObservation::Interrupted) => return false,
                _ => {
                    pause();
                    continue;
                }
            }
        }
        match store.start_item(work, &at) {
            StoreResult::Known(()) => return true,
            StoreResult::Unknown | StoreResult::Rejected => unknown = true,
            StoreResult::Retry => {}
        }
        pause();
    }
}
fn attempt(store: &impl JobStore, command: &CommandWitness) -> AttemptDisposition {
    match command {
        CommandWitness::CompleteItem(c) => store.complete_item(c).disposition,
        CommandWitness::Finish(c) => store.finish(c).disposition,
        CommandWitness::Interrupt(c) => store.interrupt(c).disposition,
    }
}
fn command_scope(command: &CommandWitness) -> &OperationScope {
    match command {
        CommandWitness::CompleteItem(c) => &c.operation,
        CommandWitness::Finish(c) => &c.operation,
        CommandWitness::Interrupt(c) => &c.operation,
    }
}
fn interrupted(view: Box<TerminalView>) -> Option<OperationHeader> {
    (view.operation.state == librairii_core::OperationState::Interrupted).then_some(view.operation)
}
/// Three attempts, 10 ms apart. Unknown outcomes require fresh SAME-writer
/// readback before another original write. The original command and resource
/// owner remain live when escalation changes the reporting intent.
fn persist(
    store: &impl JobStore,
    command: &CommandWitness,
    shared: &Shared,
    progress: Option<&ProgressSink>,
) -> Option<OperationHeader> {
    let mut unknown = false;
    let mut writes = 0;
    let mut reads = 0;
    loop {
        if unknown {
            match store.readback(command).disposition {
                ReadbackDisposition::MatchesApplied(_) => return None,
                ReadbackDisposition::ProvenNotApplied(_) => unknown = false,
                ReadbackDisposition::TerminalConflict(view) => {
                    if let Some(header) = interrupted(view) {
                        return Some(header);
                    }
                }
                _ => {}
            }
            if unknown {
                reads += 1;
                if reads >= 3 {
                    break;
                }
                pause();
                continue;
            }
        }
        writes += 1;
        match attempt(store, command) {
            AttemptDisposition::Committed { .. } | AttemptDisposition::AlreadyMatching(_) => {
                return None;
            }
            AttemptDisposition::TerminalConflict(view) => {
                if let Some(header) = interrupted(view) {
                    return Some(header);
                }
            }
            AttemptDisposition::Unknown(_) => unknown = true,
            _ => {}
        }
        if writes >= 3 {
            break;
        }
        pause();
    }
    let scope = command_scope(command);
    {
        let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        for (_, id, token) in &s.inflight {
            if *id == scope.identity.id {
                token.cancel();
            }
        }
    }
    if let Some(progress) = progress {
        progress.owned_phase(Phase::Reconciling);
    }
    shared.dirty.store(true, Ordering::Release);
    let command = CommandWitness::Interrupt(Box::new(InterruptCommand {
        generation: store.generation(),
        operation: scope.clone(),
        code: "persistence_failed".into(),
        message: "Operation progress could not be saved.".into(),
        finished_at: now(),
    }));
    reconcile_interrupt(store, &command)
}
fn reconcile_interrupt(store: &impl JobStore, command: &CommandWitness) -> Option<OperationHeader> {
    let mut unknown = false;
    let mut attempts = 0;
    loop {
        if unknown {
            match store.readback(command).disposition {
                ReadbackDisposition::MatchesApplied(view) => return Some(view.operation),
                ReadbackDisposition::ProvenNotApplied(_) => unknown = false,
                ReadbackDisposition::TerminalConflict(view) => {
                    if let Some(header) = interrupted(view) {
                        return Some(header);
                    }
                }
                _ => {}
            }
        }
        if !unknown {
            match attempt(store, command) {
                AttemptDisposition::Committed { view: Some(view) }
                | AttemptDisposition::AlreadyMatching(view) => return Some(view.operation),
                AttemptDisposition::Committed { view: None } | AttemptDisposition::Unknown(_) => {
                    unknown = true
                }
                AttemptDisposition::TerminalConflict(view) => {
                    if let Some(header) = interrupted(view) {
                        return Some(header);
                    }
                }
                _ => {}
            }
        }
        attempts += 1;
        thread::sleep(if attempts < 3 {
            TICK
        } else {
            Duration::from_secs(1)
        });
    }
}
fn release(shared: &Shared, work: &WorkItem) {
    if work.operation.kind == librairii_core::OperationKind::MetadataSync {
        shared.metadata_owner.store(false, Ordering::Release);
    }
    let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
    s.inflight.retain(|(id, _, _)| *id != work.item.identity.id);
    s.running -= 1;
    shared.changed.notify_all();
}

fn worker(store: &impl JobStore, executor: &impl Executor, shared: &Shared, index: usize) {
    loop {
        let (work, token) = {
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if s.done {
                    return;
                }
                if let Some(work) = s.queue.pop_front() {
                    let Some(token) = s
                        .inflight
                        .iter()
                        .find(|(id, _, _)| *id == work.item.identity.id)
                        .map(|(_, _, c)| c.clone())
                    else {
                        store.fence();
                        s.done = true;
                        s.failed = true;
                        shared.changed.notify_all();
                        return;
                    };
                    if s.stopping {
                        token.cancel();
                    }
                    s.running += 1;
                    shared.changed.notify_all();
                    break (work, token);
                }
                if s.done {
                    return;
                }
                s = shared.changed.wait(s).unwrap_or_else(|e| e.into_inner());
            }
        };
        if !start(store, &work) {
            release(shared, &work);
            continue;
        }
        let progress = ProgressSink::bind(
            shared.progress[index].clone(),
            shared.generation.clone(),
            work.item.identity.id,
            work.item.progress,
            work.operation.kind == librairii_core::OperationKind::Export,
            shared.dirty.clone(),
        );
        let mut execution = if token.is_cancelled() {
            Execution {
                boundary: Boundary::BeforeCommit,
                report: Report::cancelled(&work),
                resources: None,
            }
        } else {
            executor.execute(&work, &token, &progress)
        };
        progress.seal();
        if execution.boundary == Boundary::Compensating {
            progress.owned_phase(Phase::Compensating);
            if let Some(resources) = execution.resources.as_mut() {
                while !executor.compensate(resources) {
                    pause();
                }
            } else {
                store.fence();
                let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                state.failed = true;
                state.done = true;
                shared.changed.notify_all();
                return;
            }
        }
        progress.owned_phase(Phase::Reporting);
        let c = CompleteItemCommand {
            generation: store.generation(),
            operation: work.operation.clone(),
            item: work.item.immutable_scope(),
            story_id: execution.report.story_id,
            state: execution.report.state,
            completed_bytes: execution.report.completed_bytes,
            outcome: execution.report.outcome,
        };
        let interruption = persist(
            store,
            &CommandWitness::CompleteItem(Box::new(c.clone())),
            shared,
            Some(&progress),
        );
        progress.owned_phase(Phase::Settling);
        while let Some(resources) = execution.resources.take() {
            match executor.settle(
                resources,
                &DurableReport {
                    command: &c,
                    interrupted: interruption.as_ref(),
                },
            ) {
                Ok(()) => {}
                Err(resources) => {
                    execution.resources = Some(resources);
                    pause();
                }
            }
        }
        shared.dirty.store(true, Ordering::Release);
        release(shared, &work);
    }
}
fn coordinator(store: &impl JobStore, shared: &Shared) {
    loop {
        match store.startup() {
            StoreResult::Known(()) => break,
            StoreResult::Rejected => {
                store.fence();
                let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                state.failed = true;
                state.done = true;
                shared.changed.notify_all();
                return;
            }
            _ => pause(),
        }
    }
    {
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        state.ready = true;
        shared.dirty.store(true, Ordering::Release);
        shared.changed.notify_all();
    }

    loop {
        if shared.state.lock().unwrap_or_else(|e| e.into_inner()).done {
            return;
        }
        let (ingress, cancel, stopping) = {
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            let ingress = s.ingress.take();
            if ingress.is_some() {
                s.admitting = true;
            }
            (ingress, s.cancel.take(), s.stopping)
        };
        if let Some(start) = ingress {
            let accepted = if start.input.kind == librairii_core::OperationKind::MetadataSync
                && shared.metadata_owner.load(Ordering::Acquire)
            {
                false
            } else {
                admit(store, &start.input)
            };
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            s.admitting = false;
            shared.dirty.store(true, Ordering::Release);
            s.admission = Some(Admission {
                id: start.input.id,
                accepted,
            });
            shared.changed.notify_all();
        }
        if let Some(id) = cancel {
            loop {
                if matches!(
                    store.request_cancel(&id),
                    StoreResult::Known(()) | StoreResult::Rejected
                ) {
                    break;
                }
                pause();
            }
        }
        if stopping {
            while !matches!(store.cancel_all(), StoreResult::Known(())) {
                pause();
            }
        }
        for slot in &shared.progress {
            let pending = {
                let slot = slot.lock().unwrap_or_else(|e| e.into_inner());
                slot.latest
                    .as_ref()
                    .filter(|v| {
                        !slot.sealed
                            && slot.export
                            && v.completed > slot.saved
                            && (v.completed - slot.saved >= 256 * 1024 || v.completed == v.total)
                    })
                    .cloned()
            };
            if let Some(pending) = pending
                && matches!(
                    store.save_progress(pending.item, pending.completed, pending.total),
                    StoreResult::Known(())
                )
            {
                let mut slot = slot.lock().unwrap_or_else(|e| e.into_inner());
                if slot.latest.as_ref().is_some_and(|v| v.item == pending.item) {
                    slot.saved = slot.saved.max(pending.completed);
                }
            }
        }
        let checks = {
            let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            s.inflight
                .iter()
                .map(|(_, id, c)| (id.clone(), c.clone()))
                .collect::<Vec<_>>()
        };
        for (id, c) in checks {
            if stopping || matches!(store.cancelled(&id), StoreResult::Known(true)) {
                c.cancel();
            }
        }
        let finishing_exclusions = {
            let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            s.inflight
                .iter()
                .map(|(_, id, _)| id.clone())
                .collect::<Vec<_>>()
        };
        let no_finished = match store.ready_to_finish(&finishing_exclusions) {
            StoreResult::Known(Some((operation, result))) => {
                let command = FinishCommand {
                    generation: store.generation(),
                    operation,
                    state: result.state,
                    outcome: RawOutcome {
                        code: Some(result.code),
                        message: Some(result.message),
                    },
                    finished_at: now(),
                };
                persist(
                    store,
                    &CommandWitness::Finish(Box::new(command)),
                    shared,
                    None,
                );
                shared.dirty.store(true, Ordering::Release);
                continue;
            }
            StoreResult::Known(None) => true,
            _ => false,
        };
        let excluded = {
            let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if s.queue.len() < shared.config.queue_capacity() {
                Some(s.inflight.iter().map(|(id, _, _)| *id).collect::<Vec<_>>())
            } else {
                None
            }
        };
        let mut empty = false;
        if let Some(excluded) = excluded {
            match store.next_pending(&excluded) {
                StoreResult::Known(Some(work)) => {
                    if work.operation.kind == librairii_core::OperationKind::MetadataSync {
                        shared.metadata_owner.store(true, Ordering::Release);
                    }
                    let token = Cancellation::default();
                    match store.cancelled(&work.operation.identity.id) {
                        StoreResult::Known(cancelled) => {
                            if stopping || cancelled {
                                token.cancel();
                            }
                        }
                        // The row is still pending in SQLite. Retry observation
                        // before queueing; an unavailable read is not permission.
                        _ => {
                            pause();
                            continue;
                        }
                    }
                    let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    if s.done {
                        return;
                    }
                    if s.stopping || s.cancel.as_ref() == Some(&work.operation.identity.id) {
                        token.cancel();
                    }
                    s.inflight.push((
                        work.item.identity.id,
                        work.operation.identity.id.clone(),
                        token,
                    ));
                    s.queue.push_back(work);
                    shared.changed.notify_all();
                    continue;
                }
                StoreResult::Known(None) => empty = true,
                _ => {}
            }
        }
        if empty
            && no_finished
            && shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .inflight
                .is_empty()
            && stopping
        {
            // A worker may have completed/released its item after the earlier
            // finishing scan excluded that in-flight operation. With no owners
            // left and intake stopped, reobserve the durable finish candidates
            // before certifying drain; the next loop persists any parent finish.
            match store.ready_to_finish(&[]) {
                StoreResult::Known(None) => {}
                StoreResult::Known(Some(_)) => continue,
                _ => {
                    pause();
                    continue;
                }
            }
            loop {
                match store.shutdown_cleanup() {
                    StoreResult::Known(()) => break,
                    StoreResult::Rejected => {
                        store.fence();
                        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                        state.failed = true;
                        break;
                    }
                    _ => pause(),
                }
            }
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            s.done = true;
            shared.changed.notify_all();
            return;
        }
        let s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        let _ = shared.changed.wait_timeout(s, TICK);
    }
}
/// The caller runs its event loop inside `run`. Returning or panicking requests
/// cancellation and drains workers; no detached thread can outlive the borrow.
pub fn with_runtime<S: JobStore, E: Executor, T>(
    store: &S,
    executor: &E,
    config: Configuration,
    run: impl FnOnce(Handle) -> T,
) -> Result<T, SessionError> {
    if !store.claim_session() {
        return Err(SessionError::Unavailable);
    }
    struct Session<'a, S: JobStore>(&'a S);
    impl<S: JobStore> Drop for Session<'_, S> {
        fn drop(&mut self) {
            self.0.release_session();
        }
    }
    let _session = Session(store);
    let shared = Arc::new(Shared {
        config,
        state: Mutex::new(State {
            ingress: None,
            admitting: false,
            admission: None,
            queue: VecDeque::with_capacity(config.queue_capacity()),
            inflight: Vec::with_capacity(config.workers() * 3 + 1),
            running: 0,
            stopping: false,
            done: false,
            failed: false,
            ready: false,
            stop_requested_at: None,
            cancel: None,
        }),
        changed: Condvar::new(),
        generation: Generation(Arc::new(())),
        progress: (0..config.workers())
            .map(|_| Arc::new(Mutex::new(super::progress::Slot::default())))
            .collect(),
        dirty: Arc::new(AtomicBool::new(false)),
        revision: std::sync::atomic::AtomicU64::new(1),
        metadata_owner: AtomicBool::new(false),
    });
    thread::scope(|scope| {
        let mut workers = Vec::with_capacity(config.workers());
        let stop_partial = || {
            let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            s.done = true;
            s.stopping = true;
            shared.changed.notify_all();
        };
        for index in 0..config.workers() {
            let s = &shared;
            match thread::Builder::new()
                .name(format!("librairii-job-{index}"))
                .spawn_scoped(scope, move || {
                    guarded(store, s, || worker(store, executor, s, index))
                }) {
                Ok(worker) => workers.push(worker),
                Err(_) => {
                    stop_partial();
                    for worker in workers {
                        if worker.join().is_err() {
                            store.fence();
                        }
                    }
                    return Err(SessionError::ThreadStart);
                }
            }
        }
        let s = &shared;
        let coordinator = match thread::Builder::new()
            .name("librairii-job-source".into())
            .spawn_scoped(scope, move || guarded(store, s, || coordinator(store, s)))
        {
            Ok(coordinator) => coordinator,
            Err(_) => {
                stop_partial();
                for worker in workers {
                    if worker.join().is_err() {
                        store.fence();
                    }
                }
                return Err(SessionError::ThreadStart);
            }
        };
        let handle = Handle(shared.clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(handle.clone())));
        handle.shutdown();
        let mut failed = coordinator.join().is_err();
        for worker in workers {
            failed |= worker.join().is_err();
        }
        if failed
            || shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .failed
        {
            store.fence();
            return Err(if failed {
                SessionError::WorkerFailed
            } else {
                SessionError::RecoveryRequired
            });
        }
        match result {
            Ok(result) => Ok(result),
            Err(panic) => {
                store.fence();
                std::panic::resume_unwind(panic)
            }
        }
    })
}
