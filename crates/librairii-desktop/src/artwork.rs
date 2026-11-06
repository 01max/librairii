//! One joined lazy-artwork worker per open root. The qualified service coalesces
//! misses and owns publication; this queue bounds UI demand and retains library
//! admission through fetch, cache acknowledgement and thumbnail decoding.
use librairii_adapters::{lunii::ArtworkService, thumbnails::ThumbnailCache};
use librairii_app::lifecycle::LifecycleGate;
use librairii_core::Checksum;
use librairii_storage::{
    Result, StorageError,
    database::{Database, WorkCancellation},
    metadata::MetadataStorage,
};
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
/// One service uses a fresh cancellation token for each sequential request.
/// Actual uncertain publication stores stay owned here after a request fails.
#[derive(Debug)]
pub(crate) struct Repository<'db, 'library> {
    database: &'db Database<'library>,
    cancellation: Mutex<WorkCancellation>,
    retained: Mutex<Vec<MetadataStorage<'db, 'library>>>,
}
impl<'db, 'library> Repository<'db, 'library> {
    pub fn new(database: &'db Database<'library>) -> Self {
        Self {
            database,
            cancellation: Mutex::default(),
            retained: Mutex::default(),
        }
    }
    fn begin(&self, cancel: Arc<AtomicBool>) {
        *self
            .cancellation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            WorkCancellation::for_worker(cancel);
    }
    pub fn recovery_required(&self) -> bool {
        !self
            .retained
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }
}
fn metadata_error(error: StorageError) -> librairii_core::metadata::MetadataError {
    match error {
        StorageError::Metadata(error) => error,
        StorageError::Cancelled => librairii_core::metadata::MetadataError::Cancelled,
        _ => librairii_core::metadata::MetadataError::InvalidArtwork,
    }
}
impl librairii_app::metadata::ArtworkRepository for Repository<'_, '_> {
    fn registered_artwork(
        &self,
        id: Checksum,
    ) -> std::result::Result<
        librairii_app::metadata::RegisteredArtwork,
        librairii_core::metadata::MetadataError,
    > {
        self.database
            .read_registered_artwork(id)
            .map_err(metadata_error)
    }
    fn cache_artwork(
        &self,
        id: Checksum,
        asset: &librairii_app::metadata::ArtworkAsset,
        at: &str,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> std::result::Result<(), librairii_core::metadata::MetadataError> {
        let cancellation = self
            .cancellation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let store = MetadataStorage::for_work(self.database, cancellation);
        let result = store
            .persist_artwork(id, asset, at, etag, last_modified)
            .map_err(metadata_error);
        if !store.retained_failures().is_empty() {
            self.retained
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(store);
        }
        result
    }
}
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
    service: Option<&ArtworkService<&Repository<'_, '_>>>,
    store: &Repository<'_, '_>,
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
                        store.begin(cancel.clone());
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
                    if store.recovery_required() {
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
