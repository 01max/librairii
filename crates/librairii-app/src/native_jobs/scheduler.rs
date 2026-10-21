use super::*;
use crate::ports::{
    jobs::{JobStore, StoreResult},
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
}
#[derive(Debug)]
pub struct SessionError;
impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("operation worker failed; library fenced")
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
}
/// Handles can outlive a session, but then return Closed. A new session has a
/// different Arc identity and cannot be reached through a stale handle.
#[derive(Clone, Debug)]
pub struct Handle(pub(super) Arc<Shared>);
impl Handle {
    pub fn generation(&self) -> Generation {
        self.0.generation.clone()
    }
    pub fn take_dirty(&self) -> bool {
        self.0.dirty.swap(false, Ordering::AcqRel)
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

    pub fn try_start(&self, start: Start) -> Result<(), (Refusal, Start)> {
        let Ok(mut s) = self.0.state.try_lock() else {
            return Err((Refusal::Busy, start));
        };
        if s.stopping || s.done {
            return Err((Refusal::Closed, start));
        }
        if s.admitting || s.ingress.is_some() || s.admission.is_some() {
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
fn start(store: &impl JobStore, work: &WorkItem) {
    let at = now();
    let mut unknown = false;
    loop {
        if unknown {
            match store.start_readback(work) {
                StoreResult::Known(true) => return,
                StoreResult::Known(false) => unknown = false,
                _ => {
                    pause();
                    continue;
                }
            }
        }
        match store.start_item(work, &at) {
            StoreResult::Known(()) => return,
            StoreResult::Unknown => unknown = true,
            _ => {}
        }
        pause();
    }
}
/// Retains the exact command through refusal/unknown readback. Required reporting
/// never observes the work cancellation token.
fn persist(store: &impl JobStore, command: &CommandWitness) {
    let mut unknown = false;
    loop {
        if unknown {
            match store.readback(command).disposition {
                ReadbackDisposition::MatchesApplied(_) => return,
                ReadbackDisposition::ProvenNotApplied(_) => unknown = false,
                _ => {
                    pause();
                    continue;
                }
            }
        }
        let disposition = match command {
            CommandWitness::CompleteItem(c) => store.complete_item(c).disposition,
            CommandWitness::Finish(c) => store.finish(c).disposition,
            CommandWitness::Interrupt(c) => store.interrupt(c).disposition,
        };
        match disposition {
            AttemptDisposition::Committed { .. } | AttemptDisposition::AlreadyMatching(_) => return,
            AttemptDisposition::Unknown(_) => unknown = true,
            _ => {}
        }
        pause();
    }
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
        start(store, &work);
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
                report: Report::cancelled(work.item.progress.completed),
                resources: None,
            }
        } else {
            executor.execute(&work, &token, &progress)
        };
        progress.seal();
        let c = CompleteItemCommand {
            generation: store.generation(),
            operation: work.operation.clone(),
            item: work.item.immutable_scope(),
            story_id: execution.report.story_id,
            state: execution.report.state,
            completed_bytes: execution.report.completed_bytes,
            outcome: execution.report.outcome,
        };
        persist(store, &CommandWitness::CompleteItem(Box::new(c.clone())));
        while let Some(resources) = execution.resources.take() {
            match executor.settle(resources, &DurableReport { command: &c }) {
                Ok(()) => {}
                Err(resources) => {
                    execution.resources = Some(resources);
                    pause();
                }
            }
        }
        shared.dirty.store(true, Ordering::Release);
        let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        s.inflight.retain(|(id, _, _)| *id != work.item.identity.id);
        s.running -= 1;
        shared.changed.notify_all();
    }
}
fn coordinator(store: &impl JobStore, shared: &Shared) {
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
            let accepted = admit(store, &start.input);
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
                    let token = Cancellation::default();
                    if stopping
                        || matches!(
                            store.cancelled(&work.operation.identity.id),
                            StoreResult::Known(true)
                        )
                    {
                        token.cancel();
                    }
                    let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
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
            && shared
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .inflight
                .is_empty()
        {
            match store.ready_to_finish() {
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
                    persist(store, &CommandWitness::Finish(Box::new(command)));
                    shared.dirty.store(true, Ordering::Release);
                    continue;
                }
                StoreResult::Known(None) if stopping => {
                    let mut s = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                    s.done = true;
                    shared.changed.notify_all();
                    return;
                }
                _ => {}
            }
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
            cancel: None,
        }),
        changed: Condvar::new(),
        generation: Generation(Arc::new(())),
        progress: (0..config.workers())
            .map(|_| Arc::new(Mutex::new(super::progress::Slot::default())))
            .collect(),
        dirty: Arc::new(AtomicBool::new(false)),
        revision: std::sync::atomic::AtomicU64::new(1),
    });
    thread::scope(|scope| {
        let mut workers = Vec::with_capacity(config.workers());
        for index in 0..config.workers() {
            let s = &shared;
            workers
                .push(scope.spawn(move || guarded(store, s, || worker(store, executor, s, index))));
        }
        let s = &shared;
        let coordinator = scope.spawn(move || guarded(store, s, || coordinator(store, s)));
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
            return Err(SessionError);
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
