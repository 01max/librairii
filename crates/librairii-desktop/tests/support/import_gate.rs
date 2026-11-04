//! Hold real read/copy boundaries; permit one chunk to publish actual progress.
use librairii_app::native_jobs::Cancellation;
use librairii_storage::imports::{ImportObserver, ImportPoint};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

#[derive(Debug, Default)]
pub struct ImportGate {
    armed: AtomicBool,
    allow_read: AtomicBool,
    entered: AtomicUsize,
    cancelled: AtomicUsize,
    timed_out: AtomicBool,
}

impl ImportGate {
    pub fn hold(&self) -> Held<'_> {
        assert!(!self.armed.swap(true, Ordering::SeqCst));
        Held(self)
    }
    pub fn entered(&self) -> usize {
        self.entered.load(Ordering::SeqCst)
    }
    pub fn advance_read(&self) {
        assert!(!self.allow_read.swap(true, Ordering::SeqCst));
    }
    pub fn cancelled(&self) -> usize {
        self.cancelled.load(Ordering::SeqCst)
    }
    pub fn timed_out(&self) -> bool {
        self.timed_out.load(Ordering::SeqCst)
    }
}

pub struct Held<'a>(&'a ImportGate);
impl Drop for Held<'_> {
    fn drop(&mut self) {
        self.0.armed.store(false, Ordering::SeqCst);
    }
}

impl ImportObserver for ImportGate {
    fn before(&self, point: ImportPoint, cancel: &Cancellation) -> librairii_storage::Result<()> {
        if !matches!(point, ImportPoint::BeforeRead | ImportPoint::AfterCopyChunk)
            || !self.armed.load(Ordering::SeqCst)
        {
            return Ok(());
        }
        self.entered.fetch_add(1, Ordering::SeqCst);
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.armed.load(Ordering::SeqCst) && !cancel.is_cancelled() {
            if point == ImportPoint::BeforeRead && self.allow_read.swap(false, Ordering::SeqCst) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                self.timed_out.store(true, Ordering::SeqCst);
                return Err(librairii_storage::StorageError::RuntimeFenced);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        if cancel.is_cancelled() {
            self.cancelled.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}
