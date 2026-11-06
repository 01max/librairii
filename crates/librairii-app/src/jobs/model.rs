use librairii_core::{JobId, OperationKind, operations::HistoryKey};
use std::{collections::VecDeque, mem::size_of, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Configuration {
    pub(super) workers: usize,
}

impl Default for Configuration {
    fn default() -> Self {
        Self { workers: 2 }
    }
}

impl Configuration {
    pub(super) fn new(workers: usize) -> Option<Self> {
        (1..=8).contains(&workers).then_some(Self { workers })
    }

    pub(super) fn capacity(self) -> usize {
        self.workers * 2
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ItemKey {
    pub(super) job: JobId,
    pub(super) row: i64,
}

/// Move-only individual input. No scope vector/closure/Arc to one is retained.
#[derive(Debug)]
pub(super) struct Item {
    pub(super) key: ItemKey,
    pub(super) order: HistoryKey,
    pub(super) ordinal: usize,
    pub(super) kind: OperationKind,
    pub(super) payload: Vec<u8>,
    pub(super) generation: super::progress::Generation,
    pub(super) total_bytes: i64,
    pub(super) cancel_requested: bool,
}

impl Item {
    pub(super) fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.payload.capacity()
            + self.order.id.capacity()
            + self.order.created_at.capacity()
    }
}

/// Built off the intake call. Cached capacity makes try_start constant-time.
#[derive(Debug)]
pub(super) struct StartInput {
    pub(super) job: JobId,
    pub(super) kind: OperationKind,
    pub(super) items: VecDeque<Item>,
    pub(super) bytes: usize,
}

impl StartInput {
    pub(super) fn new(job: JobId, kind: OperationKind, items: VecDeque<Item>) -> Self {
        let bytes = size_of::<Self>()
            + items.capacity() * size_of::<Item>()
            + items
                .iter()
                .map(|item| item.bytes() - size_of::<Item>())
                .sum::<usize>();
        Self {
            job,
            kind,
            items,
            bytes,
        }
    }

    pub(super) fn valid_count(&self) -> bool {
        match self.kind {
            OperationKind::Import => (1..=1_000).contains(&self.items.len()),
            OperationKind::MetadataSync => self.items.len() == 1,
            OperationKind::Export => !self.items.is_empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Busy,
    Closed,
    InvalidCount,
    MetadataActive,
}

#[derive(Debug)]
pub(super) struct Unaccepted {
    pub(super) reason: Refusal,
    pub(super) input: StartInput,
}

#[derive(Debug)]
pub(super) enum Admission {
    Created(JobId),
    Rejected(StartInput),
    Uncertain(StartInput),
}

#[derive(Debug)]
pub(super) enum StartReply {
    Created(JobId),
    Rejected(StartInput),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Control {
    pub(super) job: JobId,
    pub(super) generation: super::progress::Generation,
}

pub(super) struct UnacceptedControl {
    pub(super) reason: Refusal,
    pub(super) request: Control,
}

pub(super) enum ControlDecision {
    Recorded,
    Rejected(Control),
    Uncertain(Control),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ControlReply {
    Recorded,
    Rejected(Control),
}

#[derive(Clone, Debug, Default)]
pub(super) struct Cursor {
    pub(super) after: Option<HistoryKey>,
    pub(super) upper: Option<HistoryKey>,
}

#[derive(Debug)]
pub(super) struct SourceRequest {
    pub(super) cursor: Cursor,
    pub(super) excluded: Vec<ItemKey>,
    pub(super) fenced: Vec<Control>,
}

#[derive(Debug)]
pub(super) struct Page {
    pub(super) cursor: Cursor,
    pub(super) item: Option<Item>,
    pub(super) end_cycle: bool,
}

/// An executor observation, not a commit/custody receipt. Independent fields
/// survive report/readback delays; no ambient stop rewrites this returned fact.
#[derive(Debug)]
pub(super) struct Completed {
    pub(super) item: Item,
    pub(super) fact: String,
    pub(super) errors: Vec<String>,
}

impl Completed {
    pub(super) fn bytes(&self) -> usize {
        self.item.bytes()
            + self.fact.capacity()
            + self.errors.capacity() * size_of::<String>()
            + self.errors.iter().map(String::capacity).sum::<usize>()
    }
}

#[derive(Debug)]
pub(super) enum Report {
    Recorded { terminal_operation: bool },
    Uncertain(Completed),
}

pub(super) type Admit = Arc<dyn Fn(StartInput) -> Admission + Send + Sync>;
pub(super) type Source = Arc<dyn Fn(SourceRequest) -> Page + Send + Sync>;
pub(super) type Execute = Arc<
    dyn Fn(Item, super::runtime::ProgressSink, super::runtime::WorkCancellation) -> Completed
        + Send
        + Sync,
>;
pub(super) type SaveProgress = Arc<dyn Fn(super::progress::Value) -> bool + Send + Sync>;
pub(super) type Save =
    Arc<dyn Fn(Completed, Option<super::terminal::Resolution>) -> Report + Send + Sync>;
pub(super) type Cancel = Arc<dyn Fn(Control) -> ControlDecision + Send + Sync>;

#[derive(Clone)]
pub(super) struct CoordinatorCallbacks {
    pub(super) admit: Admit,
    pub(super) readback_start: Admit,
    pub(super) source: Source,
    /// Controlled guarded progress attempt. False is an unsaved observation,
    /// never a terminal acknowledgement or permission to erase a domain fact.
    pub(super) progress: SaveProgress,
    pub(super) cancel: Cancel,
    pub(super) readback_cancel: Cancel,
}

#[derive(Clone)]
pub(super) struct WorkerCallbacks {
    pub(super) execute: Execute,
    pub(super) report: Save,
    pub(super) readback_report: Save,
    pub(super) terminal: Option<super::terminal::Callbacks>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Measurements {
    pub(super) ready: usize,
    pub(super) workers: usize,
    pub(super) prefetch: usize,
    pub(super) ingress: usize,
    pub(super) calls: usize,
    pub(super) dispatched_handles: usize,
    pub(super) ingress_bytes: usize,
    pub(super) item_bytes: usize,
    pub(super) result_bytes: usize,
    pub(super) recorded: usize,
    pub(super) uncertain_starts: usize,
    pub(super) uncertain_reports: usize,
    pub(super) joined: bool,
    pub(super) progress_slots: usize,
    pub(super) progress_wakes: usize,
    pub(super) progress_saves: usize,
    pub(super) unsaved_progress: usize,
    pub(super) control: usize,
    pub(super) control_bytes: usize,
    pub(super) uncertain_controls: usize,
    pub(super) repair_owners: usize,
    pub(super) fences: usize,
    pub(super) witness_bytes: usize,
    pub(super) retry_diagnostic_bytes: usize,
    pub(super) persistence_writes: usize,
    pub(super) persistence_readbacks: usize,
    pub(super) provisional: usize,
}
