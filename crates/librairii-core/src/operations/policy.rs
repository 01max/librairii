use super::{ItemState, RawOutcome, StoredItemIdentity, is_terminal};
use crate::{OperationKind, OperationState};

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationPolicyError {
    #[error("operation is not active")]
    InactiveOperation,
    #[error("operation transition is invalid")]
    InvalidTransition,
    #[error("operation item progress is invalid")]
    InvalidProgress,
    #[error("operation item counts are invalid")]
    InvalidCounts,
    #[error("operation has unfinished items")]
    IncompleteOperation,
    #[error("export source is invalid")]
    InvalidExportSource,
    #[error("interruption requires a nonempty reason")]
    InvalidInterruption,
    #[error("operation byte count is negative")]
    NegativeByteCount,
    #[error("operation byte total exceeds the signed storage range")]
    ByteTotalOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Counts {
    pub completed: i64,
    pub total: i64,
}

impl Counts {
    /// Stored legacy operations may have no items. Creation of new work has a
    /// separate positive-total guard; it must not restrict recovery/history.
    pub fn validate(self) -> Result<(), OperationPolicyError> {
        if self.total < 0 || self.completed < 0 || self.completed > self.total {
            return Err(OperationPolicyError::InvalidCounts);
        }
        Ok(())
    }

    pub fn for_new_work(total: i64) -> Result<Self, OperationPolicyError> {
        if total <= 0 {
            return Err(OperationPolicyError::InvalidCounts);
        }
        Ok(Self {
            completed: 0,
            total,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    pub completed: i64,
    pub total: i64,
}

impl Progress {
    pub fn advance(self, state: ItemState, next: i64) -> Result<Self, OperationPolicyError> {
        if state != ItemState::Running
            || self.completed < 0
            || self.total < 0
            || next < self.completed
            || next > self.total
        {
            return Err(OperationPolicyError::InvalidProgress);
        }
        Ok(Self {
            completed: next,
            ..self
        })
    }
}

pub fn checked_byte_total(values: &[i64]) -> Result<i64, OperationPolicyError> {
    values.iter().try_fold(0_i64, |total, value| {
        if *value < 0 {
            return Err(OperationPolicyError::NegativeByteCount);
        }
        total
            .checked_add(*value)
            .ok_or(OperationPolicyError::ByteTotalOverflow)
    })
}

fn require_active(state: OperationState) -> Result<(), OperationPolicyError> {
    if is_terminal(state) {
        return Err(OperationPolicyError::InactiveOperation);
    }
    Ok(())
}

pub fn mark_running(state: OperationState) -> Result<OperationState, OperationPolicyError> {
    require_active(state)?;
    Ok(OperationState::Running)
}

pub fn mark_item_running(state: ItemState) -> Result<ItemState, OperationPolicyError> {
    if state != ItemState::Pending {
        return Err(OperationPolicyError::InvalidTransition);
    }
    Ok(ItemState::Running)
}

pub fn request_cancel(state: OperationState) -> Result<bool, OperationPolicyError> {
    require_active(state)?;
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionCandidate {
    pub item_state: ItemState,
    pub counts: Counts,
}

/// Pure preconditions on the supplied current values. The later repository must
/// guard and commit item+counter together; this value cannot acknowledge a write.
pub fn complete_item(
    parent: OperationState,
    current: ItemState,
    counts: Counts,
    terminal: ItemState,
) -> Result<CompletionCandidate, OperationPolicyError> {
    require_active(parent)?;
    counts.validate()?;
    if current.is_terminal() || !terminal.is_terminal() {
        return Err(OperationPolicyError::InvalidTransition);
    }
    if counts.completed == counts.total {
        return Err(OperationPolicyError::InvalidCounts);
    }
    Ok(CompletionCandidate {
        item_state: terminal,
        counts: Counts {
            completed: counts.completed + 1,
            ..counts
        },
    })
}

pub fn finish(
    current: OperationState,
    counts: Counts,
    target: OperationState,
) -> Result<OperationState, OperationPolicyError> {
    require_active(current)?;
    counts.validate()?;
    if !is_terminal(target) || target == OperationState::Interrupted {
        return Err(OperationPolicyError::InvalidTransition);
    }
    if counts.completed != counts.total {
        return Err(OperationPolicyError::IncompleteOperation);
    }
    Ok(target)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemRecord {
    pub identity: StoredItemIdentity,
    pub state: ItemState,
    pub progress: Progress,
    pub outcome: RawOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptionCandidate {
    pub state: OperationState,
    pub counts: Counts,
    pub items: Vec<ItemRecord>,
    pub reason: RawOutcome,
}

pub fn interrupt(
    current: OperationState,
    counts: Counts,
    items: &[ItemRecord],
    reason: &RawOutcome,
) -> Result<InterruptionCandidate, OperationPolicyError> {
    require_active(current)?;
    counts.validate()?;
    if reason.code.as_deref().unwrap_or_default().is_empty()
        || reason.message.as_deref().unwrap_or_default().is_empty()
    {
        return Err(OperationPolicyError::InvalidInterruption);
    }
    let total = i64::try_from(items.len()).map_err(|_| OperationPolicyError::InvalidCounts)?;
    if total != counts.total
        || i64::try_from(items.iter().filter(|item| item.state.is_terminal()).count())
            .map_err(|_| OperationPolicyError::InvalidCounts)?
            != counts.completed
    {
        return Err(OperationPolicyError::InvalidCounts);
    }
    let counts = Counts {
        completed: total,
        total,
    };
    counts.validate()?;
    let items = items
        .iter()
        .cloned()
        .map(|mut item| {
            if !item.state.is_terminal() {
                item.state = ItemState::Cancelled;
                item.outcome = reason.clone();
            }
            item
        })
        .collect();
    Ok(InterruptionCandidate {
        state: OperationState::Interrupted,
        counts,
        items,
        reason: reason.clone(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalResult {
    pub state: OperationState,
    pub code: String,
    pub message: String,
}

/// Fold terminal items in the order they were acknowledged. This mirrors Go's
/// operationRun policy, including its empty-code sentinel and kind-specific
/// skipped/conflict rules. Calling record does not make an acknowledgement durable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionAggregate {
    kind: OperationKind,
    counts: Counts,
    successes: i64,
    skipped: i64,
    conflicted: i64,
    failures: i64,
    cancelled: i64,
    failure_code: String,
    failure_message: String,
}

impl CompletionAggregate {
    pub fn new(kind: OperationKind, total: i64) -> Result<Self, OperationPolicyError> {
        let counts = Counts::for_new_work(total)?;
        Ok(Self {
            kind,
            counts,
            successes: 0,
            skipped: 0,
            conflicted: 0,
            failures: 0,
            cancelled: 0,
            failure_code: String::new(),
            failure_message: String::new(),
        })
    }

    pub const fn counts(&self) -> Counts {
        self.counts
    }

    pub fn record(
        &mut self,
        state: ItemState,
        outcome: &RawOutcome,
    ) -> Result<(), OperationPolicyError> {
        if !state.is_terminal() {
            return Err(OperationPolicyError::InvalidTransition);
        }
        if self.counts.completed == self.counts.total {
            return Err(OperationPolicyError::InvalidCounts);
        }
        match state {
            ItemState::Succeeded => self.successes += 1,
            ItemState::Skipped if self.kind != OperationKind::Export => self.successes += 1,
            ItemState::Skipped => self.skipped += 1,
            ItemState::Conflicted if self.kind == OperationKind::Export => self.conflicted += 1,
            ItemState::Cancelled => self.cancelled += 1,
            _ => {
                self.failures += 1;
                if self.failure_code.is_empty() {
                    self.failure_code = outcome.code.clone().unwrap_or_default();
                    self.failure_message = outcome.message.clone().unwrap_or_default();
                }
            }
        }
        self.counts.completed += 1;
        Ok(())
    }

    pub fn terminal_result(&self) -> Result<TerminalResult, OperationPolicyError> {
        if self.counts.completed != self.counts.total {
            return Err(OperationPolicyError::IncompleteOperation);
        }
        let result = if self.kind == OperationKind::Export {
            if self.failures == 0
                && self.cancelled == 0
                && self.skipped == 0
                && self.conflicted == 0
            {
                (OperationState::Succeeded, "", "")
            } else if self.successes > 0 {
                (
                    OperationState::PartiallySucceeded,
                    "partial_export",
                    "Some stories were not exported.",
                )
            } else if self.cancelled > 0 && self.failures == 0 {
                (
                    OperationState::Cancelled,
                    "cancelled",
                    "The export was cancelled.",
                )
            } else {
                return Ok(self.failure("export_failed", "No stories were exported."));
            }
        } else if self.failures == 0 && self.cancelled == 0 {
            (OperationState::Succeeded, "", "")
        } else if self.kind == OperationKind::MetadataSync {
            if self.failures == 0 {
                (
                    OperationState::Cancelled,
                    "catalog_cancelled",
                    "Official metadata refresh was cancelled.",
                )
            } else {
                return Ok(self.failure(
                    "catalog_refresh_failed",
                    "Official metadata could not be refreshed.",
                ));
            }
        } else if self.successes > 0 {
            (
                OperationState::PartiallySucceeded,
                "partial_failure",
                "Some files could not be imported.",
            )
        } else if self.failures == 0 {
            (
                OperationState::Cancelled,
                "cancelled",
                "The import was cancelled.",
            )
        } else {
            (
                OperationState::Failed,
                "import_failed",
                "No selected files were imported.",
            )
        };
        Ok(TerminalResult {
            state: result.0,
            code: result.1.into(),
            message: result.2.into(),
        })
    }

    fn failure(&self, code: &str, message: &str) -> TerminalResult {
        TerminalResult {
            state: OperationState::Failed,
            code: if self.failure_code.is_empty() {
                code.into()
            } else {
                self.failure_code.clone()
            },
            message: if self.failure_message.is_empty() {
                message.into()
            } else {
                self.failure_message.clone()
            },
        }
    }
}
