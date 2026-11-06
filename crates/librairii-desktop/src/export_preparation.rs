//! One bounded, owned preflight worker beside the shared file-job runtime.
//! Replacing/closing a preview cancels hashing; result serials confer no authority.
use librairii_core::export::{PreparationId, Preview, Scope};
use librairii_storage::{Result, StorageError, database::WorkCancellation, export::Exports};
use std::{
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};
#[derive(Debug)]
struct Request {
    serial: u64,
    scope: Scope,
    path: PathBuf,
    cancel: WorkCancellation,
}
#[derive(Debug, Default)]
struct State {
    serial: u64,
    queued: Option<Request>,
    active: Option<WorkCancellation>,
    result: Option<(u64, Result<Preview>)>,
    discards: Vec<PreparationId>,
    stopping: bool,
    failed: bool,
    recovery: bool,
}
#[derive(Debug, Default)]
struct Shared {
    state: Mutex<State>,
    available: Condvar,
}
#[derive(Clone, Debug)]
pub struct Handle(Arc<Shared>);
impl Handle {
    pub fn prepare(&self, scope: Scope, path: PathBuf) -> Result<u64> {
        let mut s = self
            .0
            .state
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        if s.stopping || s.failed || s.recovery {
            return Err(StorageError::IntakeClosed);
        }
        if let Some(c) = &s.active {
            c.cancel();
        }
        s.queued.take();
        if let Some((_, Ok(preview))) = s.result.take()
            && let Some(id) = preview.preparation
        {
            s.discards.push(id);
        }
        s.serial = s.serial.checked_add(1).ok_or(StorageError::RuntimeFenced)?;
        let serial = s.serial;
        s.queued = Some(Request {
            serial,
            scope,
            path,
            cancel: WorkCancellation::default(),
        });
        self.0.available.notify_one();
        Ok(serial)
    }
    pub fn discard(&self, id: PreparationId) -> Result<()> {
        let mut s = self
            .0
            .state
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        if s.discards.len() >= 8 {
            return Err(StorageError::QueueFull);
        }
        s.discards.push(id);
        self.0.available.notify_one();
        Ok(())
    }
    pub fn cancel(&self) {
        let mut s = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(c) = &s.active {
            c.cancel();
        }
        s.queued.take();
        s.serial = s.serial.saturating_add(1);
        if let Some((_, Ok(preview))) = s.result.take()
            && let Some(id) = preview.preparation
        {
            s.discards.push(id);
        }
        self.0.available.notify_one();
    }
    pub fn take_result(&self) -> Result<Option<(u64, Result<Preview>)>> {
        let mut s = self
            .0
            .state
            .lock()
            .map_err(|_| StorageError::RuntimeFenced)?;
        if s.recovery {
            return Err(StorageError::CustodyRecoveryRequired);
        }
        if s.failed {
            return Err(StorageError::RuntimeFenced);
        }
        Ok(s.result.take())
    }
    pub fn recovery_required(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recovery
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
struct Stop(Handle);
impl Drop for Stop {
    fn drop(&mut self) {
        self.0.stop();
    }
}
struct WorkerExit {
    handle: Handle,
    clean: bool,
}
impl Drop for WorkerExit {
    fn drop(&mut self) {
        if !self.clean {
            self.handle
                .0
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .failed = true;
        }
    }
}
pub fn with_worker<T>(
    exports: &Mutex<Exports<'_, '_>>,
    run: impl FnOnce(Handle) -> T,
) -> Result<T> {
    let handle = Handle(Arc::default());
    std::thread::scope(|scope| {
        let worker = handle.clone();
        let join = scope.spawn(move || {
            let mut exit = WorkerExit {
                handle: worker.clone(),
                clean: false,
            };
            loop {
                let (request, discards, stopping) = {
                    let s = worker
                        .0
                        .state
                        .lock()
                        .map_err(|_| StorageError::RuntimeFenced)?;
                    let mut s = worker
                        .0
                        .available
                        .wait_while(s, |s| {
                            !s.stopping && s.queued.is_none() && s.discards.is_empty()
                        })
                        .map_err(|_| StorageError::RuntimeFenced)?;
                    let request = s.queued.take();
                    s.active = request.as_ref().map(|r| r.cancel.clone());
                    (request, std::mem::take(&mut s.discards), s.stopping)
                };
                let mut e = exports.lock().map_err(|_| StorageError::RuntimeFenced)?;
                for id in discards {
                    e.discard(&id);
                }
                if stopping && e.retained_outputs() > 0 {
                    // Keep the actual uncertain probe and directory/file witnesses
                    // alive. A shutdown request cannot certify their settlement.
                    drop(e);
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    continue;
                }
                if stopping {
                    exit.clean = true;
                    return Ok::<(), StorageError>(());
                }
                let Some(request) = request else {
                    continue;
                };
                let result = e.prepare(
                    request.scope,
                    &request.path,
                    "en-GB".into(),
                    &request.cancel,
                );
                let mut s = worker
                    .0
                    .state
                    .lock()
                    .map_err(|_| StorageError::RuntimeFenced)?;
                s.active = None;
                if e.native_recovery_required() {
                    s.recovery = true;
                    s.queued.take();
                    s.result = None;
                    // The Exports owner remains in this scoped session, including
                    // when Close or replacement already cancelled the request.
                    continue;
                }
                if !s.stopping && s.serial == request.serial && !request.cancel.is_cancelled() {
                    // Publish only after releasing the registry, so a ready
                    // preview can immediately admit its single-use Start.
                    drop(e);
                    s.result = Some((request.serial, result));
                } else if let Ok(preview) = result
                    && let Some(id) = preview.preparation
                {
                    e.discard(&id);
                }
            }
        });
        let stop = Stop(handle.clone());
        let result = run(handle);
        drop(stop);
        join.join().map_err(|_| StorageError::RuntimeFenced)??;
        Ok(result)
    })
}
