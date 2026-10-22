//! Synchronous terminal persistence for one explicitly opened operation store.
//!
//! Implementations may borrow their database and library authority. This port
//! neither owns nor extends those lifetimes, starts threads, or grants activation,
//! filesystem, recovery or custody authority. Its owned data can cross a storage
//! worker boundary; no SQL value or transaction crosses this boundary.
//!
//! Ordinary snapshots use one read transaction. Acknowledgement readback uses
//! one fresh transaction on the SAME writer after earlier admitted writer jobs.
//! A pooled snapshot, a supplied state, a missing row or an error cannot prove
//! nonapplication. The implementation must compare the complete command and
//! immutable inputs, including sibling-consistent counters. These public values
//! describe implementation observations; constructing one does not prove a write.
//!
//! The caller retains its original command, report and any real resources before
//! submission. A storage closure can be consumed even on queue refusal, so only
//! an owned DB-only copy enters it. Count that copy separately. QueueFull and a
//! proved rollback of a later attempt cannot erase an earlier unknown outcome.
//! Required readback/reporting retains the same bounded owner independently of
//! work cancellation. Repair, barrier and backup share four owned FIFO slots,
//! including unacknowledged results; this port adds no queue, retry or waiter.
//! Drain required work before actual database close/join and lease release.
//!
//! This slice has no creation, source enumeration, public history/JSON codec,
//! import, runtime composition or host-path/reveal operation. Full snapshot
//! materialization has its own byte cost, separate from targeted readback.

use librairii_core::{
    OperationKind, OperationState,
    operations::{
        Counts, HistoryKey, ItemState, Progress, RawOutcome, StoredExportScope, StoredItemIdentity,
        StoredOperationIdentity,
    },
};
use std::{error::Error, fmt};

/// Exact stored columns, without Go's display COALESCE/JSON/basename projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationHeader {
    pub identity: StoredOperationIdentity,
    pub kind: OperationKind,
    pub state: OperationState,
    pub export_scope: StoredExportScope,
    pub destination: Option<String>,
    pub counts: Counts,
    pub total_bytes: i64,
    pub cancel_requested: bool,
    pub outcome: RawOutcome,
}

impl OperationHeader {
    /// Capture only immutable command scope, not state, progress or finish time.
    pub fn immutable_scope(&self) -> OperationScope {
        OperationScope {
            identity: self.identity.history_key(),
            kind: self.kind,
            export_scope: self.export_scope.clone(),
            destination: self.destination.clone(),
            total_items: self.counts.total,
            total_bytes: self.total_bytes,
        }
    }
}

/// Raw IDs and nullable scope text remain authoritative; JobId parsing is optional.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationScope {
    pub identity: HistoryKey,
    pub kind: OperationKind,
    pub export_scope: StoredExportScope,
    pub destination: Option<String>,
    pub total_items: i64,
    pub total_bytes: i64,
}

/// Live story_id is distinct from immutable historical resolved_story_id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationItem {
    pub operation_id: String,
    pub identity: StoredItemIdentity,
    pub story_title: Option<String>,
    pub source_name: String,
    pub output_name: Option<String>,
    pub archive_relative_path: Option<String>,
    pub archive_sha256: Option<String>,
    pub export_skip_code: Option<String>,
    pub state: ItemState,
    pub progress: Progress,
    pub outcome: RawOutcome,
}

impl OperationItem {
    pub fn immutable_scope(&self) -> ItemScope {
        ItemScope {
            operation_id: self.operation_id.clone(),
            id: self.identity.id,
            resolved_story_id: self.identity.resolved_story_id,
            story_uuid: self.identity.story_uuid.clone(),
            story_title: self.story_title.clone(),
            source_name: self.source_name.clone(),
            output_name: self.output_name.clone(),
            archive_relative_path: self.archive_relative_path.clone(),
            archive_sha256: self.archive_sha256.clone(),
            export_skip_code: self.export_skip_code.clone(),
            total_bytes: self.progress.total,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemScope {
    pub operation_id: String,
    pub id: i64,
    pub resolved_story_id: Option<i64>,
    pub story_uuid: Option<String>,
    pub story_title: Option<String>,
    pub source_name: String,
    pub output_name: Option<String>,
    pub archive_relative_path: Option<String>,
    pub archive_sha256: Option<String>,
    pub export_skip_code: Option<String>,
    pub total_bytes: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationSnapshot {
    pub operation: OperationHeader,
    /// The implementation returns row-ID order without re-encoding payloads.
    pub items: Vec<OperationItem>,
}

/// Counts observed over ALL sibling rows in the same transaction as the header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SiblingCounts {
    pub items: i64,
    pub terminal: i64,
}

/// A targeted coherent view, not a full history or a constructible capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalView {
    pub operation: OperationHeader,
    pub item: Option<OperationItem>,
    pub siblings: SiblingCounts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteItemCommand {
    /// Correlation checked against the implementation's retained authority.
    pub generation: u64,
    pub operation: OperationScope,
    pub item: ItemScope,
    /// Frozen Go input: zero is written as SQL NULL; other values are retained.
    pub story_id: i64,
    /// A terminal target is a use-case precondition, not a frozen SQL predicate.
    pub state: ItemState,
    /// Schema bounds apply; terminal completion is not a monotone progress call.
    pub completed_bytes: i64,
    pub outcome: RawOutcome,
}

impl CompleteItemCommand {
    pub const fn stored_story_id(&self) -> Option<i64> {
        if self.story_id == 0 {
            None
        } else {
            Some(self.story_id)
        }
    }

    /// Expected NULLIF(empty) values; the original command remains untouched.
    pub fn stored_outcome(&self) -> RawOutcome {
        null_if_empty(&self.outcome)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishCommand {
    pub generation: u64,
    pub operation: OperationScope,
    /// succeeded/partially_succeeded/failed/cancelled only, checked by the adapter.
    pub state: OperationState,
    pub outcome: RawOutcome,
    /// Exact planned TEXT. No ambient clock, parsing, NULL or preserve sentinel.
    pub finished_at: String,
}

impl FinishCommand {
    pub fn stored_outcome(&self) -> RawOutcome {
        null_if_empty(&self.outcome)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptCommand {
    pub generation: u64,
    pub operation: OperationScope,
    /// Exact nonempty strings, without trimming or NULLIF conversion.
    pub code: String,
    pub message: String,
    pub finished_at: String,
}

/// Full immutable command identity for attempt and fresh readback correlation.
/// This data never grants database activation, a lease or custody ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandWitness {
    CompleteItem(Box<CompleteItemCommand>),
    Finish(Box<FinishCommand>),
    Interrupt(Box<InterruptCommand>),
}

fn null_if_empty(outcome: &RawOutcome) -> RawOutcome {
    RawOutcome {
        code: outcome.code.clone().filter(|value| !value.is_empty()),
        message: outcome.message.clone().filter(|value| !value.is_empty()),
    }
}

/// Attempt-local refusal before admission; it says nothing about prior attempts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NonAdmission {
    QueueFull,
    IntakeClosed,
    InvalidCommand,
    WrongGeneration,
    AuthorityUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownOutcome {
    AcknowledgementLost,
    Unavailable,
    Missing,
    Contradictory,
}

/// An owned cause chain, without an app dependency on SQL or storage error types.
#[derive(Debug)]
pub struct StoreDiagnostic(Box<dyn Error + Send + Sync>);

impl StoreDiagnostic {
    pub fn new(cause: impl Error + Send + Sync + 'static) -> Self {
        Self(Box::new(cause))
    }
}

impl fmt::Display for StoreDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.0.as_ref(), f)
    }
}

impl Error for StoreDiagnostic {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

#[derive(Debug)]
pub enum SnapshotReply {
    Found(Box<OperationSnapshot>),
    Missing,
    NotAdmitted {
        reason: NonAdmission,
        diagnostic: Option<StoreDiagnostic>,
    },
    Unavailable(StoreDiagnostic),
}

#[derive(Debug)]
pub enum AttemptDisposition {
    NotAdmitted(NonAdmission),
    /// Commit acknowledgement remains a fact even if subsequent view loading
    /// fails. None is no observed view, not an absent committed operation.
    Committed {
        view: Option<Box<TerminalView>>,
    },
    AlreadyMatching(Box<TerminalView>),
    /// This attempt rolled back. Earlier command uncertainty is separate.
    ProvenNotCommitted,
    /// Actual legitimate saved terminal truth, never a forged matching success.
    TerminalConflict(Box<TerminalView>),
    Unknown(UnknownOutcome),
}

/// Confirmed facts and a later independent read/report failure can coexist.
#[derive(Debug)]
pub struct AttemptReply {
    pub witness: CommandWitness,
    pub disposition: AttemptDisposition,
    pub diagnostic: Option<StoreDiagnostic>,
}

#[derive(Debug)]
pub enum ReadbackDisposition {
    NotAdmitted(NonAdmission),
    MatchesApplied(Box<TerminalView>),
    /// Requires positive guarded evidence, not absence, a stale read or an error.
    ProvenNotApplied(Box<TerminalView>),
    TerminalConflict(Box<TerminalView>),
    Unknown(UnknownOutcome),
}

#[derive(Debug)]
pub struct ReadbackReply {
    pub witness: CommandWitness,
    pub disposition: ReadbackDisposition,
    pub diagnostic: Option<StoreDiagnostic>,
}

/// Borrowed synchronous adapters are allowed: no 'static/Send/Sync supertrait.
///
/// CompleteItem guards BOTH the current pending/running item and active parent
/// with `completed < total`; each UPDATE must affect one row or both roll back.
/// Matching terminal items never increment again. Finish accepts stored 0/0 and
/// requires active+completed==total. Interrupt preserves prior terminal rows and
/// all bytes, including cancelled rows with an independent reason; zero rows is
/// valid. No command rewrites immutable resolved story/export input.
///
/// Implementations wait for their admitted Pending on the SAME calling owner.
/// Disconnection means unavailable acknowledgement, never proved noncommit.
/// Required readback is uncancelled and cannot reset an uncertainty fence.
pub trait OperationTerminalStore {
    fn snapshot(&self, raw_operation_id: &str) -> SnapshotReply;
    fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply;
    fn finish(&self, command: &FinishCommand) -> AttemptReply;
    fn interrupt(&self, command: &InterruptCommand) -> AttemptReply;
    fn readback(&self, witness: &CommandWitness) -> ReadbackReply;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, io};

    fn snapshot() -> OperationSnapshot {
        let operation_id = "Ab".repeat(18);
        let item = OperationItem {
            operation_id: operation_id.clone(),
            identity: StoredItemIdentity {
                id: 7,
                story_id: Some(8),
                resolved_story_id: Some(41),
                story_uuid: Some("A0000000-0000-0000-0000-00000000000B".into()),
            },
            story_title: Some(String::new()),
            source_name: "Été # %2F.zip".into(),
            output_name: Some("RAW.output".into()),
            archive_relative_path: Some("archives/RAW.zip".into()),
            archive_sha256: Some("Ab".repeat(32)),
            export_skip_code: None,
            state: ItemState::Running,
            progress: Progress {
                completed: 9,
                total: 10,
            },
            outcome: RawOutcome {
                code: None,
                message: Some(String::new()),
            },
        };
        let mut second = item.clone();
        second.identity.id = 31;
        second.identity.story_id = None;
        second.identity.story_uuid = None;
        second.story_title = None;
        second.state = ItemState::Succeeded;
        second.progress.completed = 10;
        OperationSnapshot {
            operation: OperationHeader {
                identity: StoredOperationIdentity {
                    id: operation_id,
                    created_at: "raw CREATED+02".into(),
                    started_at: Some(String::new()),
                    finished_at: None,
                },
                kind: OperationKind::Export,
                state: OperationState::Running,
                export_scope: StoredExportScope {
                    discriminator: Some("shelves".into()),
                    group_ids: Some("[9, 2]".into()),
                    group_names: Some("[\"Été\",\"RAW\"] ".into()),
                },
                destination: Some(" /private/raw destination ".into()),
                counts: Counts {
                    completed: 1,
                    total: 2,
                },
                total_bytes: 20,
                cancel_requested: false,
                outcome: RawOutcome {
                    code: Some(String::new()),
                    message: None,
                },
            },
            items: vec![item, second],
        }
    }

    fn complete(snapshot: &OperationSnapshot) -> CompleteItemCommand {
        CompleteItemCommand {
            generation: 17,
            operation: snapshot.operation.immutable_scope(),
            item: snapshot.items[0].immutable_scope(),
            story_id: 42,
            state: ItemState::Succeeded,
            completed_bytes: 10,
            outcome: RawOutcome {
                code: Some("imported".into()),
                message: None,
            },
        }
    }

    fn witness(command: &CompleteItemCommand) -> CommandWitness {
        CommandWitness::CompleteItem(Box::new(command.clone()))
    }

    fn view(snapshot: &OperationSnapshot) -> TerminalView {
        TerminalView {
            operation: snapshot.operation.clone(),
            item: snapshot.items.first().cloned(),
            siblings: SiblingCounts {
                items: 2,
                terminal: 1,
            },
        }
    }

    /// No static state: both the provider and its counter borrow this test.
    struct BorrowedStore<'a> {
        snapshot: &'a OperationSnapshot,
        calls: &'a Cell<usize>,
    }

    impl OperationTerminalStore for BorrowedStore<'_> {
        fn snapshot(&self, raw_operation_id: &str) -> SnapshotReply {
            self.calls.set(self.calls.get() + 1);
            if raw_operation_id == self.snapshot.operation.identity.id {
                SnapshotReply::Found(Box::new(self.snapshot.clone()))
            } else {
                SnapshotReply::Missing
            }
        }
        fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply {
            // A refused worker consumes/drops its work closure, but only its
            // counted DB-only copy enters that closure, not the caller's report.
            let fields = command.clone();
            let work: Box<dyn FnOnce() + Send> = Box::new(move || drop(fields));
            drop(work);
            AttemptReply {
                witness: witness(command),
                disposition: AttemptDisposition::NotAdmitted(NonAdmission::QueueFull),
                diagnostic: None,
            }
        }
        fn finish(&self, command: &FinishCommand) -> AttemptReply {
            AttemptReply {
                witness: CommandWitness::Finish(Box::new(command.clone())),
                disposition: AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost),
                diagnostic: None,
            }
        }
        fn interrupt(&self, command: &InterruptCommand) -> AttemptReply {
            AttemptReply {
                witness: CommandWitness::Interrupt(Box::new(command.clone())),
                disposition: AttemptDisposition::Committed { view: None },
                diagnostic: Some(StoreDiagnostic::new(io::Error::other(
                    "post-commit snapshot",
                ))),
            }
        }
        fn readback(&self, command: &CommandWitness) -> ReadbackReply {
            ReadbackReply {
                witness: command.clone(),
                disposition: ReadbackDisposition::Unknown(UnknownOutcome::Unavailable),
                diagnostic: None,
            }
        }
    }

    fn assert_owned<T: Send + 'static>(_: T) {}

    #[test]
    fn terminal_port_accepts_borrowed_synchronous_provider_and_owned_results() {
        let source = snapshot();
        let calls = Cell::new(0);
        let provider = BorrowedStore {
            snapshot: &source,
            calls: &calls,
        };
        let port: &dyn OperationTerminalStore = &provider;
        let reply = port.snapshot(&source.operation.identity.id);
        assert!(matches!(&reply, SnapshotReply::Found(value) if **value == source));
        assert_eq!(calls.get(), 1);
        assert_owned(reply);
        assert_owned(port.complete_item(&complete(&source)));
    }

    #[test]
    fn terminal_port_raw_snapshot_keeps_non_uuid_ids_nulls_and_scope_bytes() {
        let source = snapshot();
        assert_eq!(source.operation.identity.id.len(), 36);
        assert!(source.operation.identity.typed_id().is_err());
        assert_eq!(source.operation.identity.started_at.as_deref(), Some(""));
        assert_eq!(source.operation.identity.finished_at, None);
        assert_eq!(
            source.operation.export_scope.discriminator.as_deref(),
            Some("shelves")
        );
        assert_eq!(
            source.operation.export_scope.group_ids.as_deref(),
            Some("[9, 2]")
        );
        assert_eq!(
            source.operation.export_scope.group_names.as_deref(),
            Some("[\"Été\",\"RAW\"] ")
        );
        assert_eq!(
            source
                .items
                .iter()
                .map(|item| item.identity.id)
                .collect::<Vec<_>>(),
            [7, 31]
        );
        assert_eq!(source.items[0].story_title.as_deref(), Some(""));
        assert_eq!(source.items[1].story_title, None);
        assert_eq!(source.items[0].identity.story_id, Some(8));
        assert_eq!(source.items[0].identity.resolved_story_id, Some(41));
        assert_eq!(
            source.items[0].archive_sha256.as_deref(),
            Some("Ab".repeat(32).as_str())
        );
    }

    #[test]
    fn terminal_port_immutable_scope_excludes_live_state_but_keeps_resolved_input() {
        let mut source = snapshot();
        let parent = source.operation.immutable_scope();
        let item = source.items[0].immutable_scope();
        source.operation.state = OperationState::Interrupted;
        source.operation.counts.completed = 2;
        source.operation.cancel_requested = true;
        source.operation.identity.started_at = None;
        source.operation.identity.finished_at = Some("raw finish".into());
        source.items[0].identity.story_id = None;
        source.items[0].state = ItemState::Cancelled;
        source.items[0].progress.completed = 1;
        source.items[0].outcome.code = Some("independent".into());
        assert_eq!(source.operation.immutable_scope(), parent);
        assert_eq!(source.items[0].immutable_scope(), item);
        source.items[0].identity.resolved_story_id = Some(42);
        assert_ne!(source.items[0].immutable_scope(), item);
        source.operation.destination = None;
        assert_ne!(source.operation.immutable_scope(), parent);
    }

    #[test]
    fn terminal_port_full_witness_keeps_generation_scope_and_exact_payload() {
        let source = snapshot();
        let command = complete(&source);
        let original = witness(&command);
        let mut changed = command.clone();
        changed.generation += 1;
        assert_ne!(witness(&changed), original);
        changed = command.clone();
        changed.item.id += 1;
        assert_ne!(witness(&changed), original);
        changed = command.clone();
        changed.operation.export_scope.group_ids = Some("[2,9]".into());
        assert_ne!(witness(&changed), original);
        changed = command.clone();
        changed.story_id = 41;
        assert_ne!(witness(&changed), original);
        changed = command.clone();
        changed.completed_bytes = 9;
        assert_ne!(witness(&changed), original);
        changed = command.clone();
        changed.outcome.message = Some(String::new());
        assert_ne!(witness(&changed), original);
        assert_eq!(changed.stored_outcome(), command.stored_outcome());
    }

    #[test]
    fn terminal_port_completion_null_rules_retain_raw_input_and_terminal_bytes() {
        let source = snapshot();
        let mut command = complete(&source);
        command.story_id = 0;
        command.completed_bytes = 1; // Less than observed progress is representable.
        command.outcome = RawOutcome {
            code: Some(String::new()),
            message: Some(" ".into()),
        };
        let before = command.clone();
        assert_eq!(command.stored_story_id(), None);
        assert_eq!(
            command.stored_outcome(),
            RawOutcome {
                code: None,
                message: Some(" ".into())
            }
        );
        assert_eq!(command, before);
        assert_eq!(command.completed_bytes, 1);
        assert_eq!(command.item.resolved_story_id, Some(41));
        command.story_id = -1;
        assert_eq!(command.stored_story_id(), Some(-1)); // Validation must not hide invalid data.
    }

    #[test]
    fn terminal_port_finish_and_interrupt_have_distinct_null_and_time_rules() {
        let source = snapshot();
        let finish = FinishCommand {
            generation: 17,
            operation: source.operation.immutable_scope(),
            state: OperationState::Failed,
            outcome: RawOutcome {
                code: Some(String::new()),
                message: Some(" ".into()),
            },
            finished_at: "RAW+00:00".into(),
        };
        assert_eq!(
            finish.stored_outcome(),
            RawOutcome {
                code: None,
                message: Some(" ".into())
            }
        );
        assert_eq!(finish.outcome.code.as_deref(), Some(""));
        let interrupt = InterruptCommand {
            generation: 17,
            operation: finish.operation.clone(),
            code: " ".into(),
            message: " raw ".into(),
            finished_at: finish.finished_at.clone(),
        };
        assert_eq!(interrupt.code, " ");
        assert_eq!(interrupt.message, " raw ");
        assert_eq!(interrupt.finished_at, "RAW+00:00");
        assert_ne!(
            CommandWitness::Finish(Box::new(finish)),
            CommandWitness::Interrupt(Box::new(interrupt))
        );
    }

    #[test]
    fn terminal_port_confirmed_commit_survives_missing_snapshot_and_owned_cause() {
        let source = snapshot();
        let calls = Cell::new(0);
        let provider = BorrowedStore {
            snapshot: &source,
            calls: &calls,
        };
        let command = InterruptCommand {
            generation: 17,
            operation: source.operation.immutable_scope(),
            code: "interrupted".into(),
            message: "retained".into(),
            finished_at: "raw time".into(),
        };
        let reply = provider.interrupt(&command);
        assert!(matches!(
            reply.disposition,
            AttemptDisposition::Committed { view: None }
        ));
        assert_eq!(reply.witness, CommandWitness::Interrupt(Box::new(command)));
        assert_eq!(
            reply
                .diagnostic
                .as_ref()
                .and_then(Error::source)
                .map(ToString::to_string)
                .as_deref(),
            Some("post-commit snapshot")
        );
        assert_owned(reply);
    }

    #[test]
    fn terminal_port_queue_refusal_retains_original_move_only_report() {
        struct Report {
            command: CompleteItemCommand,
            facts: Vec<String>,
        }
        let source = snapshot();
        let calls = Cell::new(0);
        let provider = BorrowedStore {
            snapshot: &source,
            calls: &calls,
        };
        let report = Report {
            command: complete(&source),
            facts: vec!["actual returned story".into()],
        };
        let earlier_unknown = AttemptReply {
            witness: witness(&report.command),
            disposition: AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost),
            diagnostic: None,
        };
        let refused = provider.complete_item(&report.command);
        assert!(matches!(
            refused.disposition,
            AttemptDisposition::NotAdmitted(NonAdmission::QueueFull)
        ));
        assert_eq!(refused.witness, earlier_unknown.witness);
        assert!(matches!(
            earlier_unknown.disposition,
            AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost)
        ));
        assert_eq!(report.facts, ["actual returned story"]);
        assert_eq!(report.command.story_id, 42);
    }

    #[test]
    fn terminal_port_barrier_results_retain_actual_conflict_and_unknown_evidence() {
        let source = snapshot();
        let command = witness(&complete(&source));
        let mut saved = view(&source);
        saved.operation.state = OperationState::Cancelled;
        let actual = saved.clone();
        let conflict = ReadbackReply {
            witness: command.clone(),
            disposition: ReadbackDisposition::TerminalConflict(Box::new(saved)),
            diagnostic: None,
        };
        assert!(
            matches!(&conflict.disposition, ReadbackDisposition::TerminalConflict(value) if **value == actual)
        );
        for reason in [
            UnknownOutcome::Unavailable,
            UnknownOutcome::Missing,
            UnknownOutcome::Contradictory,
        ] {
            let readback = ReadbackReply {
                witness: command.clone(),
                disposition: ReadbackDisposition::Unknown(reason),
                diagnostic: None,
            };
            assert!(
                matches!(readback.disposition, ReadbackDisposition::Unknown(value) if value == reason)
            );
        }
        assert_owned(conflict);
    }

    #[test]
    fn terminal_port_legacy_zero_item_view_has_no_new_work_count_guard() {
        let mut source = snapshot();
        source.operation.counts = Counts {
            completed: 0,
            total: 0,
        };
        source.operation.total_bytes = 0;
        source.items.clear();
        assert!(source.operation.counts.validate().is_ok());
        assert_eq!(source.operation.immutable_scope().total_items, 0);
        let view = TerminalView {
            operation: source.operation.clone(),
            item: None,
            siblings: SiblingCounts {
                items: 0,
                terminal: 0,
            },
        };
        let finish = FinishCommand {
            generation: 17,
            operation: source.operation.immutable_scope(),
            state: OperationState::Succeeded,
            outcome: RawOutcome::default(),
            finished_at: "raw 0/0".into(),
        };
        let reply = AttemptReply {
            witness: CommandWitness::Finish(Box::new(finish)),
            disposition: AttemptDisposition::Committed {
                view: Some(Box::new(view)),
            },
            diagnostic: None,
        };
        assert_owned(reply);
    }
}
