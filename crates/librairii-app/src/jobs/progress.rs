use super::model::ItemKey;
use librairii_core::{
    JobId, OperationState,
    operations::{ItemState, OperationObservation, Progress},
};

/// Correlation only. These stamps confer no activation or custody authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Generation(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct QueryRevision(pub(super) u64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Stamp {
    pub(super) generation: Generation,
    pub(super) item: ItemKey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Value {
    pub(super) stamp: Stamp,
    pub(super) revision: u64,
    pub(super) completed: i64,
    pub(super) total: i64,
}

/// Exactly one scalar latest value per worker. Persistence and presentation
/// have separate dirty flags, rather than event queues or extra producer tasks.
pub(super) struct Slot {
    pub(super) stamp: Stamp,
    pub(super) latest: Value,
    pub(super) pending: bool,
    pub(super) saved_bytes: i64,
    pub(super) sealed: bool,
}

impl Slot {
    pub(super) fn new(stamp: Stamp, total: i64) -> Self {
        Self {
            stamp,
            latest: Value {
                stamp,
                revision: 0,
                completed: 0,
                total,
            },
            pending: false,
            saved_bytes: 0,
            sealed: false,
        }
    }

    pub(super) fn replace(&mut self, value: Value, export: bool) -> bool {
        if self.sealed
            || value.stamp != self.stamp
            || value.total != self.latest.total
            || value.revision <= self.latest.revision
            || (Progress {
                completed: self.latest.completed,
                total: self.latest.total,
            })
            .advance(ItemState::Running, value.completed)
            .is_err()
        {
            return false;
        }
        self.latest = value;
        // Frozen Go export policy; this is not an invented import byte callback.
        self.pending |= export
            && (value.completed - self.saved_bytes >= 256 * 1024 || value.completed == value.total);
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    pub(super) generation: Generation,
    pub(super) query: QueryRevision,
    pub(super) job: JobId,
    pub(super) revision: u64,
    pub(super) observation: OperationObservation,
}

/// One consumer-owned projection, not a global operation dictionary, event log
/// or durable authority. A coalesced wake requires a fresh snapshot fetch.
pub(super) struct Projection {
    pub(super) generation: Generation,
    pub(super) query: QueryRevision,
    pub(super) job: JobId,
    pub(super) current: Option<Snapshot>,
}

impl Projection {
    pub(super) fn apply(&mut self, next: Snapshot) -> bool {
        if next.generation != self.generation || next.query != self.query || next.job != self.job {
            return false;
        }
        if let Some(current) = self.current
            && (current.observation.is_saved_terminal()
                || (next.revision <= current.revision && !next.observation.is_saved_terminal()))
        {
            return false;
        }
        self.current = Some(next);
        true
    }

    pub(super) fn saved_terminal(&self) -> Option<OperationState> {
        self.current
            .filter(|s| s.observation.is_saved_terminal())
            .map(|s| s.observation.state)
    }
}
