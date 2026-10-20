use crate::ports::operations::*;
use std::{
    fmt,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Configuration {
    pub(super) workers: usize,
}
impl Default for Configuration {
    fn default() -> Self {
        Self { workers: 2 }
    }
}
impl Configuration {
    pub fn new(workers: usize) -> Option<Self> {
        (1..=8).contains(&workers).then_some(Self { workers })
    }
    pub fn queue_capacity(self) -> usize {
        self.workers * 2
    }
    pub fn workers(self) -> usize {
        self.workers
    }
}

/// One owned item, with explicit supplied terminal plans, never a scope vector.
#[derive(Debug)]
pub struct Request {
    pub complete: CompleteItemCommand,
    pub finish: Option<FinishCommand>,
    pub interrupt: InterruptCommand,
}

#[derive(Debug)]
pub enum WorkReply {
    Completed {
        command: Box<CompleteItemCommand>,
        diagnostic: Option<StoreDiagnostic>,
    },
    Failed(StoreDiagnostic),
}
impl WorkReply {
    pub fn completed(command: CompleteItemCommand) -> Self {
        Self::Completed {
            command: Box::new(command),
            diagnostic: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Delivery {
    Accepted,
    Retain,
}

/// Borrowed callables may refer to a concrete non-static repository. The store
/// object needs Sync explicitly: the broad port trait does not promise it.
pub struct Callbacks<'run> {
    pub store: &'run (dyn OperationTerminalStore + Sync),
    /// Return ONE item in offered order, or None at the finite cycle boundary.
    /// Keep source ownership outside this call until a value is returned.
    pub source: &'run (dyn Fn() -> Option<Request> + Sync),
    pub work: &'run (dyn Fn(&Request, &RunControl) -> WorkReply + Sync),
    /// Called on the same occupied worker. Retain/panic keeps the original
    /// record owned; an Accepted reply acknowledges only this controlled sink.
    pub deliver: &'run (dyn Fn(&Retained) -> Delivery + Sync),
}
impl fmt::Debug for Callbacks<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Callbacks").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub struct Cause {
    pub text: String,
    pub display_bytes: usize,
}
impl Cause {
    pub(super) fn new(value: impl fmt::Display) -> Self {
        struct Bounded {
            text: String,
            bytes: usize,
        }
        impl fmt::Write for Bounded {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                self.bytes = self.bytes.saturating_add(value.len());
                let mut end = value.len().min(1024 - self.text.len());
                while !value.is_char_boundary(end) {
                    end -= 1;
                }
                self.text.push_str(&value[..end]);
                Ok(())
            }
        }
        let mut output = Bounded {
            text: String::with_capacity(1024),
            bytes: 0,
        };
        let formatted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fmt::write(&mut output, format_args!("{value}"))
        }));
        if formatted.is_err() {
            output.text.clear();
            output.text.push_str("diagnostic formatting panicked");
        }
        Self {
            text: output.text,
            display_bytes: output.bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaceError {
    Regression,
    Overflow,
}

#[derive(Debug, Default)]
struct Signal {
    tick: u64,
    revision: u128,
}

/// Intake stop, cooperative work cancellation, and required-drain observation
/// are independent. Virtual ticks pace writes; no wall timer runs persistence.
#[derive(Debug, Default)]
pub struct RunControl {
    intake_stopped: AtomicBool,
    work_cancelled: AtomicBool,
    return_unresolved: AtomicBool,
    signal: Mutex<Signal>,
    wake: Condvar,
}
impl RunControl {
    pub fn stop_intake(&self) {
        self.intake_stopped.store(true, Ordering::Release);
        self.notify();
    }
    pub fn cancel_work(&self) {
        self.work_cancelled.store(true, Ordering::Release);
        self.notify();
    }
    pub fn work_cancelled(&self) -> bool {
        self.work_cancelled.load(Ordering::Acquire)
    }
    /// Request a truthful unresolved return, never a settled/closed deadline.
    /// A blocked callback still has to return before its thread can join.
    pub fn return_unresolved(&self) {
        self.return_unresolved.store(true, Ordering::Release);
        self.stop_intake();
    }
    pub fn advance(&self, next: u64) -> Result<(), PaceError> {
        let mut signal = self
            .signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if next < signal.tick {
            return Err(PaceError::Regression);
        }
        next.checked_add(1000).ok_or(PaceError::Overflow)?;
        signal.tick = next;
        signal.revision = signal.revision.wrapping_add(1);
        self.wake.notify_all();
        Ok(())
    }
    pub(super) fn stopping(&self) -> bool {
        self.intake_stopped.load(Ordering::Acquire)
    }
    pub(super) fn returning(&self) -> bool {
        self.return_unresolved.load(Ordering::Acquire)
    }
    pub(super) fn observe(&self) -> (u64, u128) {
        let signal = self
            .signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (signal.tick, signal.revision)
    }
    pub(super) fn notify(&self) {
        let mut signal = self
            .signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        signal.revision = signal.revision.wrapping_add(1);
        self.wake.notify_all();
    }
    pub(super) fn wait(&self, revision: u128) {
        let signal = self
            .signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        drop(
            self.wake
                .wait_while(signal, |signal| signal.revision == revision)
                .unwrap_or_else(|error| error.into_inner()),
        );
    }
}

/// Original typed owner lives outside every callback unwind boundary. It is
/// returned intact when the finite controlled run cannot establish settlement.
#[derive(Debug)]
pub struct Retained {
    pub request: Request,
    pub execution: Option<CompleteItemCommand>,
    pub work_error: Option<StoreDiagnostic>,
    pub work_panicked_before_db: bool,
    pub witness: CommandWitness,
    pub complete_acknowledged: bool,
    pub unknown: bool,
    pub view: Option<Box<TerminalView>>,
    pub different_terminal: bool,
    pub latest_store_cause: Option<Cause>,
    pub latest_delivery_cause: Option<Cause>,
}

#[derive(Clone, Debug, Default)]
pub struct Measurements {
    pub max_ready: usize,
    pub max_prefetch: usize,
    pub max_retained: usize,
    pub max_dispatched_handles: usize,
    pub max_callback_owners: usize,
    /// Logical owned string capacities, separate from handles. Opaque original
    /// work errors are retained; their arbitrary allocator internals are not a
    /// heap bound. Latest formatted retry causes have a fixed 1024-byte cap.
    pub max_command_bytes: usize,
    /// Two field copies per retained slot budgeted for the pinned R4 adapter
    /// (reply witness and DB work). Not an arbitrary provider heap bound.
    pub max_store_field_copy_budget_bytes: usize,
    pub max_view_bytes: usize,
    pub max_latest_diagnostic_bytes: usize,
    pub settled: usize,
    pub joined: bool,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub unresolved: Vec<Retained>,
    pub unstarted: Vec<Request>,
    pub source_failure_before_db: Option<Cause>,
    pub host_failure: Option<Cause>,
    pub measurements: Measurements,
}
impl RunOutcome {
    /// Does not authorize library activation or clear a storage fence.
    pub fn positively_settled(&self) -> bool {
        self.unresolved.is_empty()
            && self.unstarted.is_empty()
            && self.source_failure_before_db.is_none()
            && self.host_failure.is_none()
    }
}
