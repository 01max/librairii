//! Fixed worker slots and correlated read responses, independent of toolkit.
use super::scheduler::Handle;
use crate::ports::operations::OperationSnapshot;
use librairii_core::{
    navigation::{Navigation, QueryStamp},
    operations::{Progress, is_terminal},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug)]
pub struct Generation(pub(super) Arc<()>);
impl PartialEq for Generation {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Generation {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressUpdate {
    pub generation: Generation,
    pub item: i64,
    pub revision: u64,
    pub completed: i64,
    pub total: i64,
}
#[derive(Debug)]
pub(super) struct Slot {
    pub epoch: Arc<()>,
    pub latest: Option<ProgressUpdate>,
    pub sealed: bool,
    pub dirty: bool,
    pub saved: i64,
    pub export: bool,
}
impl Default for Slot {
    fn default() -> Self {
        Self {
            epoch: Arc::new(()),
            latest: None,
            sealed: true,
            dirty: false,
            saved: 0,
            export: false,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ProgressSink {
    slot: Arc<Mutex<Slot>>,
    epoch: Arc<()>,
    wake: Arc<AtomicBool>,
}
impl ProgressSink {
    pub(super) fn bind(
        slot: Arc<Mutex<Slot>>,
        generation: Generation,
        item: i64,
        progress: Progress,
        export: bool,
        wake: Arc<AtomicBool>,
    ) -> Self {
        let epoch = Arc::new(());
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Slot {
            epoch: epoch.clone(),
            latest: Some(ProgressUpdate {
                generation,
                item,
                revision: 0,
                completed: progress.completed,
                total: progress.total,
            }),
            sealed: false,
            dirty: true,
            saved: progress.completed,
            export,
        };
        Self { slot, epoch, wake }
    }
    /// A late producer cannot write after sealing or into a later item. Each
    /// accepted value replaces one scalar slot; there is no progress event queue.
    pub fn advance(&self, completed: i64) -> bool {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if slot.sealed || !Arc::ptr_eq(&slot.epoch, &self.epoch) {
            return false;
        }
        let Some(previous) = slot.latest.as_ref() else {
            return false;
        };
        if (Progress {
            completed: previous.completed,
            total: previous.total,
        })
        .advance(librairii_core::operations::ItemState::Running, completed)
        .is_err()
        {
            return false;
        }
        let Some(revision) = previous.revision.checked_add(1) else {
            return false;
        };
        if let Some(latest) = slot.latest.as_mut() {
            latest.completed = completed;
            latest.revision = revision;
        }
        slot.dirty = true;
        self.wake.store(true, Ordering::Release);
        true
    }
    pub(super) fn seal(&self) {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        if Arc::ptr_eq(&slot.epoch, &self.epoch) {
            slot.sealed = true;
        }
    }
}
/// Stamp before beginning an asynchronous read, then attach only its resulting
/// observation. Query-independent operations use None and survive browser changes.
#[derive(Clone, Debug)]
pub struct RequestStamp {
    pub(super) generation: Generation,
    pub(super) revision: u64,
    pub(super) query: Option<QueryStamp>,
}
#[derive(Debug)]
pub struct Response<T> {
    stamp: RequestStamp,
    value: T,
}
impl RequestStamp {
    pub fn complete<T>(self, value: T) -> Response<T> {
        Response { stamp: self, value }
    }
}
impl<T> Response<T> {
    pub fn accept(self, handle: &Handle, navigation: Option<&Navigation>) -> Option<T> {
        (handle.accepts_stamp(&self.stamp, navigation)).then_some(self.value)
    }
}
#[derive(Debug)]
pub struct OperationProjection {
    id: String,
    current: Option<OperationSnapshot>,
    revision: u64,
    generation: Option<Generation>,
}
impl OperationProjection {
    pub fn new(id: String) -> Result<Self, crate::ports::creation::InputError> {
        if id.is_empty() || id.capacity() > crate::ports::creation::MAX_COMMAND_BYTES {
            return Err(crate::ports::creation::InputError::Invalid);
        }
        Ok(Self {
            id,
            current: None,
            revision: 0,
            generation: None,
        })
    }
    pub fn current(&self) -> Option<&OperationSnapshot> {
        self.current.as_ref()
    }
    pub fn apply(
        &mut self,
        response: Response<OperationSnapshot>,
        handle: &Handle,
        navigation: Option<&Navigation>,
    ) -> bool {
        if response.value.operation.identity.id != self.id
            || !handle.accepts_stamp(&response.stamp, navigation)
        {
            return false;
        }
        if self.generation.as_ref() != Some(&response.stamp.generation) {
            self.current = None;
            self.revision = 0;
        }
        if self.current.as_ref().is_some_and(|s| {
            s.operation.identity.id == response.value.operation.identity.id
                && (is_terminal(s.operation.state) || response.stamp.revision <= self.revision)
        }) {
            return false;
        }
        self.revision = response.stamp.revision;
        self.generation = Some(response.stamp.generation);
        self.current = Some(response.value);
        true
    }
}
