//! Production blocking job session borrowing the admitted store and effect owner.
//! The session closure normally hosts the UI loop. Every owned scoped worker is
//! joined before this function returns and the Database/library borrow ends.
mod progress;
mod scheduler;
use crate::ports::{creation::NewOperation, jobs::WorkItem, operations::CompleteItemCommand};
use librairii_core::operations::{ItemState, RawOutcome};
pub use progress::{
    Generation, OperationProjection, ProgressSink, ProgressUpdate, RequestStamp, Response,
};
pub use scheduler::{Admission, Handle, Refusal, RuntimeStatus, SessionError, with_runtime};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Configuration {
    workers: usize,
}
impl Configuration {
    pub fn new(workers: usize) -> Option<Self> {
        (1..=8).contains(&workers).then_some(Self { workers })
    }
    pub const fn workers(self) -> usize {
        self.workers
    }
    pub const fn queue_capacity(self) -> usize {
        self.workers * 2
    }
}
impl Default for Configuration {
    fn default() -> Self {
        Self { workers: 2 }
    }
}

/// Move-only intake. Validation and capacity accounting happen before try_start.
#[derive(Debug)]
pub struct Start {
    pub(crate) input: Box<NewOperation>,
}
impl Start {
    pub fn new(input: NewOperation) -> Result<Self, Box<NewOperation>> {
        let count = input.items.len();
        let valid = match input.kind {
            librairii_core::OperationKind::Import => (1..=1000).contains(&count),
            librairii_core::OperationKind::MetadataSync => count == 1,
            librairii_core::OperationKind::Export => count > 0,
        };
        if valid && input.validate().is_ok() {
            Ok(Self {
                input: Box::new(input),
            })
        } else {
            Err(Box::new(input))
        }
    }
    pub fn input(&self) -> &NewOperation {
        &self.input
    }
}
#[derive(Clone, Debug, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
}
#[derive(Debug)]
pub struct Report {
    pub state: ItemState,
    pub story_id: i64,
    pub completed_bytes: i64,
    pub outcome: RawOutcome,
}
impl Report {
    pub(crate) fn cancelled(bytes: i64) -> Self {
        Self {
            state: ItemState::Cancelled,
            story_id: 0,
            completed_bytes: bytes,
            outcome: RawOutcome {
                code: Some("cancelled".into()),
                message: Some("The operation was cancelled.".into()),
            },
        }
    }
}
/// The effect adapter owns actual file/metadata/removal resources throughout
/// execution and hands them back even after an irreversible commit. Cancellation
/// after that boundary must not replace the committed result with Cancelled.
#[derive(Debug)]
pub struct Execution<R> {
    pub report: Report,
    pub resources: Option<R>,
}
/// Opaque acknowledgement created by the runtime only after a matching durable
/// terminal store result. It is not independent filesystem cleanup authority.
#[derive(Debug)]
pub struct DurableReport<'a> {
    command: &'a CompleteItemCommand,
}
impl DurableReport<'_> {
    pub fn command(&self) -> &CompleteItemCommand {
        self.command
    }
}
pub trait Executor: Sync {
    type Resources: Send;
    fn execute(
        &self,
        work: &WorkItem,
        cancel: &Cancellation,
        progress: &ProgressSink,
    ) -> Execution<Self::Resources>;
    /// On uncertainty return the original actual owner. The runtime retains it
    /// and retries without cancellation or shutdown deadlines.
    fn settle(
        &self,
        resources: Self::Resources,
        durable: &DurableReport<'_>,
    ) -> Result<(), Self::Resources>;
}
