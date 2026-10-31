//! Supplied terminal persistence observations, not storage/custody receipts.
use super::{model::Completed, progress::Stamp};
use librairii_core::{
    OperationState,
    operations::{Counts, OperationObservation, RawOutcome, is_terminal},
};
use std::{fmt, sync::Arc};

pub(super) const RETRY_DELAY: u64 = 10;
pub(super) const RECONCILE_DELAY: u64 = 1_000;
const CAUSE_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PaceError {
    Regression,
    Overflow,
}
impl fmt::Display for PaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PaceError {}

pub(super) fn checked_tick(current: u64, next: u64) -> Result<u64, PaceError> {
    if next < current {
        return Err(PaceError::Regression);
    }
    next.checked_add(RECONCILE_DELAY)
        .ok_or(PaceError::Overflow)?;
    Ok(next)
}

/// Latest retry diagnostics have fixed count and bounded text. Original
/// executor facts/errors are retained independently and are never truncated.
#[derive(Clone, Debug)]
pub(super) struct Cause {
    pub(super) text: String,
    pub(super) original_bytes: usize,
    pub(super) truncated: bool,
}
impl Cause {
    pub(super) fn new(text: &str) -> Self {
        let mut end = text.len().min(CAUSE_BYTES);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        Self {
            text: text[..end].into(),
            original_bytes: text.len(),
            truncated: end != text.len(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FinishIntent {
    pub(super) state: OperationState,
    pub(super) counts: Counts,
    pub(super) outcome: RawOutcome,
    pub(super) finished_at: Option<String>,
}
impl FinishIntent {
    fn valid(&self) -> bool {
        is_terminal(self.state)
            && self.state != OperationState::Interrupted
            && self.counts.validate().is_ok()
            && self.counts.completed == self.counts.total
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Command {
    CompleteItem { fact: String },
    Finish(FinishIntent),
    Interrupt { outcome: RawOutcome },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Witness {
    pub(super) stamp: Stamp,
    pub(super) command: Command,
}
impl Witness {
    fn bytes(&self) -> usize {
        let outcome_bytes = |outcome: &RawOutcome| {
            outcome.code.as_ref().map_or(0, String::capacity)
                + outcome.message.as_ref().map_or(0, String::capacity)
        };
        std::mem::size_of::<Self>()
            + match &self.command {
                Command::CompleteItem { fact } => fact.capacity(),
                Command::Finish(intent) => {
                    outcome_bytes(&intent.outcome)
                        + intent.finished_at.as_ref().map_or(0, String::capacity)
                }
                Command::Interrupt { outcome } => outcome_bytes(outcome),
            }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Applied {
    pub(super) witness: Witness,
    pub(super) observation: OperationObservation,
    pub(super) next_finish: Option<FinishIntent>,
}

pub(super) enum Attempt {
    Applied(Applied),
    ReportedNotApplied { witness: Witness, cause: Cause },
    Unknown { witness: Witness, cause: Cause },
}
pub(super) enum Readback {
    Matches(Applied),
    DifferentSavedTerminal {
        witness: Witness,
        observation: OperationObservation,
    },
    ReportedNotApplied {
        witness: Witness,
        cause: Cause,
    },
    Unavailable(Cause),
    Contradictory(Cause),
}

#[derive(Clone)]
pub(super) struct Callbacks {
    pub(super) attempt: AttemptCallback,
    /// A supplied coherent, fresh, writer-ordered answer. This fake callback
    /// cannot establish that a real storage barrier/view exists.
    pub(super) readback: ReadbackCallback,
}
type AttemptCallback = Arc<dyn Fn(&Witness, &Completed) -> Attempt + Send + Sync>;
type ReadbackCallback = Arc<dyn Fn(&Witness) -> Readback + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Resolution {
    pub(super) stamp: Stamp,
    pub(super) observation: OperationObservation,
    pub(super) different_terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Write,
    Readback,
    ClockBlocked,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Status {
    pub(super) due: u64,
    pub(super) action: Action,
    pub(super) attempts: u8,
    pub(super) observed_tick: u64,
}

pub(super) struct Driver {
    pub(super) witness: Witness,
    pub(super) attempts: u8,
    pub(super) due: u64,
    pub(super) action: Action,
    pub(super) reconciling: bool,
    pub(super) provisional: bool,
    pub(super) latest_retry: Option<Cause>,
    pub(super) latest_readback: Option<Cause>,
    pub(super) resolution: Option<Resolution>,
    pub(super) acknowledged_complete: bool,
}
impl Driver {
    pub(super) fn new(completed: &Completed, now: u64) -> Self {
        Self {
            witness: Witness {
                stamp: Stamp {
                    generation: completed.item.generation,
                    item: completed.item.key,
                },
                command: Command::CompleteItem {
                    fact: completed.fact.clone(),
                },
            },
            attempts: 0,
            due: now,
            action: Action::Write,
            reconciling: false,
            provisional: false,
            latest_retry: None,
            latest_readback: None,
            resolution: None,
            acknowledged_complete: false,
        }
    }

    pub(super) fn ready(&self, now: u64) -> Option<Action> {
        (self.resolution.is_none() && self.action != Action::ClockBlocked && now >= self.due)
            .then_some(self.action)
    }

    pub(super) fn before_write(&mut self) {
        if !self.reconciling {
            self.attempts += 1;
        }
    }

    fn defer(&mut self, now: u64, delay: u64, action: Action) {
        if let Some(due) = now.checked_add(delay) {
            self.due = due;
            self.action = action;
        } else {
            self.action = Action::ClockBlocked;
            self.latest_readback = Some(Cause::new("virtual persistence deadline overflow"));
        }
    }

    fn uncertain(&mut self, now: u64, cause: Cause, immediate: bool) {
        self.latest_readback = Some(cause);
        self.defer(
            now,
            if immediate { 0 } else { RECONCILE_DELAY },
            Action::Readback,
        );
    }

    fn not_applied(&mut self, now: u64, cause: Cause) {
        if self.acknowledged_complete
            && matches!(self.witness.command, Command::CompleteItem { .. })
        {
            self.uncertain(
                now,
                Cause::new("nonapplication contradicts acknowledged completion"),
                false,
            );
            return;
        }
        self.latest_retry = Some(cause);
        if self.reconciling {
            self.defer(now, RECONCILE_DELAY, Action::Write);
        } else if self.attempts < 3 {
            self.defer(now, RETRY_DELAY, Action::Write);
        } else if matches!(self.witness.command, Command::Interrupt { .. }) {
            self.provisional = true;
            self.reconciling = true;
            self.defer(now, RECONCILE_DELAY, Action::Write);
        } else {
            self.witness.command = Command::Interrupt {
                outcome: RawOutcome {
                    code: Some("persistence_failed".into()),
                    message: Some("Operation progress could not be saved.".into()),
                },
            };
            self.attempts = 0;
            self.defer(now, 0, Action::Write);
        }
    }

    fn applied(&mut self, now: u64, applied: Applied) {
        use librairii_core::operations::ObservationKind;
        if applied.witness != self.witness
            || applied.observation.kind != ObservationKind::StoredSnapshot
        {
            self.uncertain(
                now,
                Cause::new("applied response identity/provenance mismatch"),
                false,
            );
            return;
        }
        if matches!(self.witness.command, Command::CompleteItem { .. }) {
            self.acknowledged_complete = true;
        }
        if applied.observation.is_saved_terminal() {
            let different = match &self.witness.command {
                Command::Finish(intent) => applied.observation.state != intent.state,
                Command::Interrupt { .. } => {
                    applied.observation.state != OperationState::Interrupted
                }
                Command::CompleteItem { .. } => false,
            };
            self.resolve(applied.observation, different);
        } else if matches!(self.witness.command, Command::CompleteItem { .. }) {
            if let Some(intent) = applied.next_finish {
                if !intent.valid() {
                    self.uncertain(now, Cause::new("invalid supplied finish intent"), false);
                    return;
                }
                self.witness.command = Command::Finish(intent);
                self.attempts = 0;
                self.reconciling = false;
                self.defer(now, 0, Action::Write);
            } else {
                self.resolve(applied.observation, false);
            }
        } else {
            self.uncertain(
                now,
                Cause::new("terminal command returned nonterminal observation"),
                false,
            );
        }
    }

    fn resolve(&mut self, observation: OperationObservation, different_terminal: bool) {
        self.resolution = Some(Resolution {
            stamp: self.witness.stamp,
            observation,
            different_terminal,
        });
    }

    pub(super) fn accept_attempt(&mut self, now: u64, response: Attempt) {
        match response {
            Attempt::Applied(value) => self.applied(now, value),
            Attempt::ReportedNotApplied { witness, cause } if witness == self.witness => {
                if self.reconciling {
                    // Go's continued Interrupt failure is followed by Snapshot;
                    // do not blindly restart an initial three-write burst.
                    self.latest_retry = Some(cause);
                    self.defer(now, 0, Action::Readback);
                } else {
                    self.not_applied(now, cause);
                }
            }
            Attempt::Unknown { witness, cause } if witness == self.witness => {
                self.latest_retry = Some(cause);
                self.defer(now, 0, Action::Readback);
            }
            _ => self.uncertain(now, Cause::new("attempt command identity mismatch"), true),
        }
    }

    pub(super) fn accept_readback(&mut self, now: u64, response: Readback) {
        match response {
            Readback::Matches(value) => self.applied(now, value),
            Readback::DifferentSavedTerminal {
                witness,
                observation,
            } if witness == self.witness && observation.is_saved_terminal() => {
                self.resolve(observation, true)
            }
            Readback::ReportedNotApplied { witness, cause } if witness == self.witness => {
                self.not_applied(now, cause)
            }
            Readback::Unavailable(cause) | Readback::Contradictory(cause) => {
                self.uncertain(now, cause, false)
            }
            _ => self.uncertain(
                now,
                Cause::new("readback command identity/provenance mismatch"),
                false,
            ),
        }
    }

    pub(super) fn fenced(&self) -> bool {
        self.provisional || self.latest_retry.is_some() || self.latest_readback.is_some()
    }
    pub(super) fn witness_bytes(&self) -> usize {
        self.witness.bytes()
    }
    pub(super) fn diagnostic_bytes(&self) -> usize {
        self.latest_retry
            .iter()
            .chain(self.latest_readback.iter())
            .map(|cause| cause.text.capacity())
            .sum()
    }
}
