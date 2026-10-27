//! One joined lazy-artwork worker per open root. The qualified service coalesces
//! misses and owns publication; this queue bounds UI demand and retains library
//! admission through fetch, cache acknowledgement and thumbnail decoding.
use librairii_adapters::{lunii::ArtworkService, thumbnails::ThumbnailCache};
use librairii_app::lifecycle::LifecycleGate;
use librairii_core::Checksum;
use librairii_storage::{Result, StorageError, metadata::MetadataStorage};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
const QUEUE: usize = 32;
const BACKOFF: usize = 128;
#[derive(Debug, Default)]
struct State {
    queued: VecDeque<Checksum>,
    pending: BTreeSet<Checksum>,
    failures: VecDeque<(Checksum, Instant)>,
    active: Option<Arc<AtomicBool>>,
    stopping: bool,
}
#[derive(Debug, Default)]
struct Shared {
    enabled: bool,
    state: Mutex<State>,
    available: Condvar,
    dirty: AtomicBool,
    recovery: AtomicBool,
}
#[derive(Clone, Debug)]
pub(crate) struct Handle(Arc<Shared>);
impl Handle {
    pub fn request(&self, id: Checksum) {
        let Ok(mut state) = self.0.state.lock() else {
            self.0.recovery.store(true, Ordering::Release);
            return;
        };
        if !self.0.enabled
            || state.stopping
            || self.recovery_required()
            || state.pending.contains(&id)
            || state.queued.len() >= QUEUE
            || state
                .failures
                .iter()
                .any(|(failed, at)| *failed == id && at.elapsed() < Duration::from_secs(30))
        {
            return;
        }
        state.pending.insert(id);
        state.queued.push_back(id);
        self.0.available.notify_one();
    }
    pub fn take_dirty(&self) -> bool {
        self.0.dirty.swap(false, Ordering::AcqRel)
    }
    pub fn recovery_required(&self) -> bool {
        self.0.recovery.load(Ordering::Acquire)
    }
    pub fn cancel(&self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(active) = &state.active {
            active.store(true, Ordering::Release);
        }
        while let Some(id) = state.queued.pop_front() {
            state.pending.remove(&id);
        }
        self.0.available.notify_one();
    }
    fn stop(&self) {
        self.cancel();
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stopping = true;
        self.0.available.notify_one();
    }
}
struct Stop<'a>(&'a Handle);
impl Drop for Stop<'_> {
    fn drop(&mut self) {
        self.0.stop();
    }
}
pub(crate) fn with_worker<T>(
    gate: &LifecycleGate,
    service: Option<&ArtworkService<&MetadataStorage<'_, '_>>>,
    store: &MetadataStorage<'_, '_>,
    cache: &ThumbnailCache,
    run: impl FnOnce(Handle) -> Result<T>,
) -> Result<T> {
    let handle = Handle(Arc::new(Shared {
        enabled: service.is_some(),
        ..Default::default()
    }));
    let Some(service) = service else {
        return run(handle);
    };
    std::thread::scope(|scope| {
        let worker = handle.clone();
        let join = std::thread::Builder::new()
            .name("librairii-lazy-artwork".into())
            .spawn_scoped(scope, move || -> Result<()> {
                loop {
                    let (id, cancel) = {
                        let mut state = worker
                            .0
                            .state
                            .lock()
                            .map_err(|_| StorageError::RuntimeFenced)?;
                        while state.queued.is_empty() && !state.stopping {
                            state = worker
                                .0
                                .available
                                .wait(state)
                                .map_err(|_| StorageError::RuntimeFenced)?;
                        }
                        if state.stopping {
                            return Ok(());
                        }
                        let id = state
                            .queued
                            .pop_front()
                            .ok_or(StorageError::RuntimeFenced)?;
                        let cancel = Arc::new(AtomicBool::new(false));
                        state.active = Some(cancel.clone());
                        (id, cancel)
                    };
                    let result = (|| {
                        let admission = gate.enter().map_err(|_| StorageError::Cancelled)?;
                        let at = librairii_adapters::datetime::utc_now()
                            .ok_or(StorageError::InvalidProfile)?;
                        let asset = service
                            .load_blocking(id, &at, &cancel)
                            .map_err(StorageError::Metadata)?;
                        if !gate.accepts(admission.generation()) || cancel.load(Ordering::Acquire) {
                            return Err(StorageError::Cancelled);
                        }
                        let generation = cache.generation()?;
                        cache.resolve(&generation, Some(&asset), None)?;
                        Ok(())
                    })();
                    if !store.retained_failures().is_empty() {
                        worker.0.recovery.store(true, Ordering::Release);
                    }
                    let mut state = worker
                        .0
                        .state
                        .lock()
                        .map_err(|_| StorageError::RuntimeFenced)?;
                    state.active = None;
                    state.pending.remove(&id);
                    if result.is_err() && !cancel.load(Ordering::Acquire) {
                        if state.failures.len() >= BACKOFF {
                            state.failures.pop_front();
                        }
                        state.failures.push_back((id, Instant::now()));
                    }
                    worker.0.dirty.store(true, Ordering::Release);
                }
            })
            .map_err(|e| StorageError::Io {
                operation: "start lazy artwork worker",
                source: e,
            })?;
        let stopping = Stop(&handle);
        let result = run(handle.clone());
        drop(stopping);
        join.join().map_err(|_| StorageError::WorkerStopped)??;
        result
    })
}
