use super::model::*;
use crate::ports::operations::*;
use librairii_core::{OperationState, operations::is_terminal};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Write,
    Readback,
    Deliver,
    Blocked,
}

pub(super) struct Driver {
    pub(super) owner: Retained,
    action: Action,
    attempts: u8,
    due: u64,
    reconciling: bool,
    acknowledged_command: bool,
}

impl Driver {
    pub(super) fn new(request: Request, now: u64) -> Self {
        let witness = CommandWitness::CompleteItem(Box::new(request.complete.clone()));
        Self {
            owner: Retained {
                request,
                execution: None,
                work_error: None,
                work_panicked_before_db: false,
                witness,
                complete_acknowledged: false,
                unknown: false,
                view: None,
                different_terminal: false,
                latest_store_cause: None,
                latest_delivery_cause: None,
            },
            action: Action::Write,
            attempts: 0,
            due: now,
            reconciling: false,
            acknowledged_command: false,
        }
    }

    pub(super) fn valid_request(&self) -> bool {
        let request = &self.owner.request;
        request.complete.generation == request.interrupt.generation
            && request.complete.operation == request.interrupt.operation
            && !request.interrupt.code.is_empty()
            && !request.interrupt.message.is_empty()
            && request.complete.item.operation_id == request.complete.operation.identity.id
            && request.finish.as_ref().is_none_or(|finish| {
                finish.generation == request.complete.generation
                    && finish.operation == request.complete.operation
            })
    }

    pub(super) fn invalid_request(&mut self) {
        self.owner.latest_store_cause =
            Some(Cause::new("supplied plans do not match immutable request"));
        self.action = Action::Blocked;
    }

    pub(super) fn accept_work(
        &mut self,
        reply: Result<WorkReply, Box<dyn std::any::Any + Send>>,
        now: u64,
    ) {
        match reply {
            Ok(WorkReply::Completed {
                command,
                diagnostic,
            }) if command.generation == self.owner.request.complete.generation
                && command.operation == self.owner.request.complete.operation
                && command.item == self.owner.request.complete.item =>
            {
                self.owner.witness = CommandWitness::CompleteItem(command.clone());
                self.owner.execution = Some(*command);
                self.owner.work_error = diagnostic;
            }
            Ok(WorkReply::Completed {
                command,
                diagnostic,
            }) => {
                self.owner.execution = Some(*command);
                self.owner.work_error = diagnostic;
                self.owner.latest_store_cause =
                    Some(Cause::new("work returned a different immutable scope"));
                self.interrupt(now);
            }
            Ok(WorkReply::Failed(cause)) => {
                self.owner.work_error = Some(cause);
                self.interrupt(now);
            }
            Err(_) => {
                // No store call has occurred: this is not an unknown DB attempt.
                self.owner.work_panicked_before_db = true;
                self.interrupt(now);
            }
        }
    }

    fn interrupt(&mut self, now: u64) {
        self.owner.witness =
            CommandWitness::Interrupt(Box::new(self.owner.request.interrupt.clone()));
        self.attempts = 0;
        self.acknowledged_command = false;
        self.reconciling = false;
        self.action = Action::Write;
        self.due = now;
    }

    fn defer(&mut self, now: u64, delay: u64, action: Action) {
        match now.checked_add(delay) {
            Some(due) => {
                self.due = due;
                self.action = action;
            }
            None => {
                self.action = Action::Blocked;
                self.owner.latest_store_cause = Some(Cause::new("virtual due overflow"));
            }
        }
    }

    pub(super) fn ready(&self, now: u64) -> bool {
        now >= self.due && self.action != Action::Blocked
    }
    pub(super) fn delivery(&self) -> bool {
        self.action == Action::Deliver
    }
    pub(super) fn fenced(&self) -> bool {
        self.owner.unknown
            || self.owner.latest_store_cause.is_some()
            || matches!(self.owner.witness, CommandWitness::Interrupt(_))
    }

    fn unknown(&mut self, now: u64, cause: impl std::fmt::Display, immediate: bool) {
        self.owner.unknown = true;
        self.owner.latest_store_cause = Some(Cause::new(cause));
        self.defer(
            now,
            if immediate && !self.reconciling {
                0
            } else {
                1000
            },
            Action::Readback,
        );
    }

    fn not_applied(&mut self, now: u64) {
        if self.owner.unknown || self.acknowledged_command {
            self.defer(now, 1000, Action::Readback);
        } else if self.reconciling {
            self.defer(now, 1000, Action::Write);
        } else if self.attempts < 3 {
            self.defer(now, 10, Action::Write);
        } else if matches!(self.owner.witness, CommandWitness::Interrupt(_)) {
            self.reconciling = true;
            self.defer(now, 1000, Action::Readback);
        } else {
            self.interrupt(now);
        }
    }

    pub(super) fn step(
        &mut self,
        store: &(dyn OperationTerminalStore + Sync),
        control: &RunControl,
    ) {
        match self.action {
            Action::Write => {
                if !self.reconciling {
                    self.attempts += 1;
                }
                // owner/witness is outside this catch boundary; callbacks BORROW
                // it, and may enqueue only their own DB-only field copy.
                let reply = catch_unwind(AssertUnwindSafe(|| match &self.owner.witness {
                    CommandWitness::CompleteItem(command) => store.complete_item(command),
                    CommandWitness::Finish(command) => store.finish(command),
                    CommandWitness::Interrupt(command) => store.interrupt(command),
                }));
                let now = control.observe().0;
                match reply {
                    Ok(reply) => self.attempt(now, reply),
                    Err(_) => self.unknown(
                        now,
                        "store callback panicked; admission/commit unknown",
                        true,
                    ),
                }
            }
            Action::Readback => {
                let reply = catch_unwind(AssertUnwindSafe(|| store.readback(&self.owner.witness)));
                let now = control.observe().0;
                match reply {
                    Ok(reply) => self.readback(now, reply),
                    Err(_) => self.unknown(now, "fresh readback callback panicked", false),
                }
            }
            Action::Deliver | Action::Blocked => {}
        }
    }

    fn attempt(&mut self, now: u64, reply: AttemptReply) {
        if reply.witness != self.owner.witness {
            self.unknown(now, "attempt witness mismatch", true);
            return;
        }
        if let Some(cause) = reply.diagnostic {
            self.owner.latest_store_cause = Some(Cause::new(cause));
        }
        match reply.disposition {
            AttemptDisposition::Committed { view } => {
                self.acknowledged_command = true;
                if matches!(self.owner.witness, CommandWitness::CompleteItem(_)) {
                    self.owner.complete_acknowledged = true;
                }
                self.owner.view = view;
                // A later missing view is independent of the PROVEN commit.
                // Always require exact fresh readback before a supplied Finish.
                self.defer(
                    now,
                    if self.reconciling { 1000 } else { 0 },
                    Action::Readback,
                );
            }
            AttemptDisposition::AlreadyMatching(view) => self.applied(now, view),
            AttemptDisposition::TerminalConflict(view) => self.conflict(now, view),
            AttemptDisposition::Unknown(reason) => {
                self.unknown(now, format_args!("{reason:?}"), true)
            }
            AttemptDisposition::NotAdmitted(reason) => {
                self.owner.latest_store_cause = Some(Cause::new(format_args!("{reason:?}")));
                self.not_applied(now);
            }
            AttemptDisposition::ProvenNotCommitted if self.reconciling => {
                self.defer(now, 1000, Action::Readback)
            }
            AttemptDisposition::ProvenNotCommitted => self.not_applied(now),
        }
    }

    fn readback(&mut self, now: u64, reply: ReadbackReply) {
        if reply.witness != self.owner.witness {
            self.unknown(now, "readback witness mismatch", false);
            return;
        }
        if let Some(cause) = reply.diagnostic {
            self.owner.latest_store_cause = Some(Cause::new(cause));
        }
        match reply.disposition {
            ReadbackDisposition::MatchesApplied(view) => self.applied(now, view),
            ReadbackDisposition::TerminalConflict(view) => self.conflict(now, view),
            ReadbackDisposition::ProvenNotApplied(view) if self.acknowledged_command => {
                self.owner.view = Some(view);
                self.unknown(
                    now,
                    "nonapplication contradicts acknowledged command",
                    false,
                );
            }
            ReadbackDisposition::ProvenNotApplied(view)
                if scope_matches(&self.owner.witness, &view)
                    && !is_terminal(view.operation.state) =>
            {
                self.owner.unknown = false;
                self.owner.view = Some(view);
                self.not_applied(now);
            }
            ReadbackDisposition::ProvenNotApplied(_) => {
                self.unknown(now, "contradictory nonapplication view", false)
            }
            ReadbackDisposition::NotAdmitted(reason) => {
                // Attempt-local queue refusal cannot erase an earlier unknown.
                self.owner.latest_store_cause = Some(Cause::new(format_args!("{reason:?}")));
                self.defer(now, 1000, Action::Readback);
            }
            ReadbackDisposition::Unknown(reason) => {
                self.unknown(now, format_args!("{reason:?}"), false)
            }
        }
    }

    fn applied(&mut self, now: u64, view: Box<TerminalView>) {
        if !matches_applied(&self.owner.witness, &view) {
            self.unknown(now, "matching view contradicts full command", false);
            return;
        }
        self.owner.unknown = false;
        self.acknowledged_command = true;
        if matches!(self.owner.witness, CommandWitness::CompleteItem(_)) {
            self.owner.complete_acknowledged = true;
            if !is_terminal(view.operation.state)
                && view.operation.counts.completed == view.operation.counts.total
                && let Some(finish) = &self.owner.request.finish
            {
                self.owner.witness = CommandWitness::Finish(Box::new(finish.clone()));
                self.attempts = 0;
                self.reconciling = false;
                self.acknowledged_command = false;
                self.owner.view = Some(view);
                self.defer(now, 0, Action::Write);
                return;
            }
        }
        self.owner.view = Some(view);
        self.action = Action::Deliver;
    }

    fn conflict(&mut self, now: u64, view: Box<TerminalView>) {
        // R4 can observe a different terminal target while its parent remains
        // active. Preserve that item truth without recounting or inventing a
        // parent terminal result/Finish. Immutable scope and sibling coherence
        // are still required for both forms of conflict.
        let terminal = is_terminal(view.operation.state)
            || view
                .item
                .as_ref()
                .is_some_and(|item| item.state.is_terminal());
        if !scope_matches(&self.owner.witness, &view) || !terminal {
            self.unknown(now, "conflict lacks coherent saved terminal truth", false);
            return;
        }
        self.owner.unknown = false;
        self.owner.different_terminal = true;
        self.owner.view = Some(view);
        self.action = Action::Deliver;
    }

    pub(super) fn deliver(
        &mut self,
        callback: &(dyn Fn(&Retained) -> Delivery + Sync),
        control: &RunControl,
    ) -> bool {
        let reply = catch_unwind(AssertUnwindSafe(|| callback(&self.owner)));
        let now = control.observe().0;
        match reply {
            Ok(Delivery::Accepted) => true,
            Ok(Delivery::Retain) => {
                self.defer(now, 1000, Action::Deliver);
                false
            }
            Err(_) => {
                self.owner.latest_delivery_cause = Some(Cause::new(
                    "delivery callback panicked; original record retained",
                ));
                self.defer(now, 1000, Action::Deliver);
                false
            }
        }
    }
}

fn scope_matches(witness: &CommandWitness, view: &TerminalView) -> bool {
    let scope = match witness {
        CommandWitness::CompleteItem(command) => &command.operation,
        CommandWitness::Finish(command) => &command.operation,
        CommandWitness::Interrupt(command) => &command.operation,
    };
    view.operation.immutable_scope() == *scope
        && view.operation.counts.validate().is_ok()
        && view.siblings.items == view.operation.counts.total
        && view.siblings.terminal == view.operation.counts.completed
        && match witness {
            CommandWitness::CompleteItem(command) => view
                .item
                .as_ref()
                .is_some_and(|item| item.immutable_scope() == command.item),
            _ => true,
        }
}

fn matches_applied(witness: &CommandWitness, view: &TerminalView) -> bool {
    if !scope_matches(witness, view) {
        return false;
    }
    match witness {
        CommandWitness::CompleteItem(command) => view.item.as_ref().is_some_and(|item| {
            item.state == command.state
                && item.identity.story_id == command.stored_story_id()
                && item.progress.completed == command.completed_bytes
                && item.outcome == command.stored_outcome()
        }),
        CommandWitness::Finish(command) => {
            view.operation.state == command.state
                && view.operation.identity.finished_at.as_ref() == Some(&command.finished_at)
                && view.operation.outcome == command.stored_outcome()
        }
        CommandWitness::Interrupt(command) => {
            view.operation.state == OperationState::Interrupted
                && view.operation.identity.finished_at.as_ref() == Some(&command.finished_at)
                && view.operation.outcome.code.as_ref() == Some(&command.code)
                && view.operation.outcome.message.as_ref() == Some(&command.message)
        }
    }
}
