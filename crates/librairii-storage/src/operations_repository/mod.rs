//! Borrowed synchronous terminal SQL for an explicitly opened isolated Database.
//!
//! No activation, importer, worker pool, retry loop or lifetime extension lives
//! here. The caller keeps its original report/resources. Each call owns a reply
//! witness and one copied DB-only work witness until the same caller waits for
//! admission's result. Both copies contain variable text; neither is an item or
//! snapshot memory bound. Queue refusal consumes only that work copy.
//!
//! An attempt-local rollback/refusal never resolves an earlier unknown outcome.
//! Required readback always crosses the existing reserved SAME-writer barrier.
//! The caller must retain unresolved generation/drain ownership across close;
//! constructing this adapter or a public DTO cannot clear a storage fence.

mod creation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod destination;
mod history;
mod presentation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use destination::ExportDestination;
pub use presentation::{OperationCard, OperationHistory, OperationItems};
mod jobs;
mod policy;
mod recovery;
mod rows;

use crate::{
    StorageError,
    database::{Database, TransactionOutcome},
};
use librairii_app::ports::operations::{
    AttemptDisposition, AttemptReply, CommandWitness, CompleteItemCommand, FinishCommand,
    InterruptCommand, NonAdmission, OperationSnapshot, OperationTerminalStore, ReadbackDisposition,
    ReadbackReply, SnapshotReply, StoreDiagnostic, UnknownOutcome,
};
use policy::Inspection;

/// Owned command storage, including boxes and actual String capacities. This
/// measures one DB-only witness copy, not a quota or the caller's report/view.
/// An attempt temporarily holds both a reply witness and a submitted copy.
pub fn command_bytes(witness: &CommandWitness) -> usize {
    fn text(value: &Option<String>) -> usize {
        value.as_ref().map_or(0, String::capacity)
    }
    fn outcome(value: &librairii_core::operations::RawOutcome) -> usize {
        text(&value.code) + text(&value.message)
    }
    fn scope(value: &librairii_app::ports::operations::OperationScope) -> usize {
        value.identity.id.capacity()
            + value.identity.created_at.capacity()
            + text(&value.export_scope.discriminator)
            + text(&value.export_scope.group_ids)
            + text(&value.export_scope.group_names)
            + value
                .destination
                .as_ref()
                .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
    }
    std::mem::size_of::<CommandWitness>()
        + match witness {
            CommandWitness::CompleteItem(c) => {
                std::mem::size_of::<CompleteItemCommand>()
                    + scope(&c.operation)
                    + c.item.operation_id.capacity()
                    + text(&c.item.story_uuid)
                    + text(&c.item.story_title)
                    + c.item.source_name.capacity()
                    + c.item
                        .output_name
                        .as_ref()
                        .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
                    + c.item
                        .archive_relative_path
                        .as_ref()
                        .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
                    + text(&c.item.archive_sha256)
                    + text(&c.item.export_skip_code)
                    + outcome(&c.outcome)
            }
            CommandWitness::Finish(c) => {
                std::mem::size_of::<FinishCommand>()
                    + scope(&c.operation)
                    + outcome(&c.outcome)
                    + c.finished_at.capacity()
            }
            CommandWitness::Interrupt(c) => {
                std::mem::size_of::<InterruptCommand>()
                    + scope(&c.operation)
                    + c.code.capacity()
                    + c.message.capacity()
                    + c.finished_at.capacity()
            }
        }
}

/// Full owned raw snapshot storage, including Vec capacity and nullable text.
/// This is separate from the one affected-item view and is not capped by it.
pub fn snapshot_bytes(snapshot: &OperationSnapshot) -> usize {
    fn text(value: &Option<String>) -> usize {
        value.as_ref().map_or(0, String::capacity)
    }
    let h = &snapshot.operation;
    let header = h.identity.id.capacity()
        + h.identity.created_at.capacity()
        + text(&h.identity.started_at)
        + text(&h.identity.finished_at)
        + text(&h.export_scope.discriminator)
        + text(&h.export_scope.group_ids)
        + text(&h.export_scope.group_names)
        + h.destination
            .as_ref()
            .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
        + text(&h.outcome.code)
        + text(&h.outcome.message);
    let items: usize = snapshot
        .items
        .iter()
        .map(|item| {
            item.operation_id.capacity()
                + text(&item.identity.story_uuid)
                + text(&item.story_title)
                + item.source_name.capacity()
                + item
                    .output_name
                    .as_ref()
                    .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
                + item
                    .archive_relative_path
                    .as_ref()
                    .map_or(0, librairii_app::ports::operations::OperationPath::capacity)
                + text(&item.archive_sha256)
                + text(&item.export_skip_code)
                + text(&item.outcome.code)
                + text(&item.outcome.message)
        })
        .sum();
    std::mem::size_of::<OperationSnapshot>()
        + header
        + snapshot.items.capacity()
            * std::mem::size_of::<librairii_app::ports::operations::OperationItem>()
        + items
}

/// The Database and its isolated authority outlive every synchronous call.
/// `generation` is retained correlation, not a caller-created lease capability.
#[derive(Debug)]
pub struct TerminalRepository<'database, 'library> {
    database: &'database Database<'library>,
    generation: u64,
}

enum Work {
    Written,
    Observed(Inspection),
}

impl<'database, 'library> TerminalRepository<'database, 'library> {
    pub const fn new(database: &'database Database<'library>, generation: u64) -> Self {
        Self {
            database,
            generation,
        }
    }

    fn refusal(&self, witness: &CommandWitness) -> Option<NonAdmission> {
        if policy::generation(witness) != self.generation {
            Some(NonAdmission::WrongGeneration)
        } else if !policy::valid(witness) {
            Some(NonAdmission::InvalidCommand)
        } else {
            None
        }
    }

    fn attempt(&self, witness: CommandWitness) -> AttemptReply {
        if let Some(reason) = self.refusal(&witness) {
            return AttemptReply {
                witness,
                disposition: AttemptDisposition::NotAdmitted(reason),
                diagnostic: None,
            };
        }
        // Preserve the reply witness and the caller's original before a queue
        // may consume this closure even on refusal. Only DB fields are copied.
        let work = witness.clone();
        let pending = match self.database.repair_transaction(move |tx| {
            let observation = policy::inspect(tx, &work)?;
            if matches!(observation, Inspection::Unapplied(_)) {
                policy::write(tx, &work)?;
                Ok(Work::Written)
            } else {
                Ok(Work::Observed(observation))
            }
        }) {
            Ok(pending) => pending,
            Err(error) => {
                return AttemptReply {
                    witness,
                    disposition: AttemptDisposition::NotAdmitted(non_admission(&error)),
                    diagnostic: Some(StoreDiagnostic::new(error)),
                };
            }
        };
        let (disposition, diagnostic) = match pending.wait() {
            Ok(TransactionOutcome::Committed(Work::Written)) => {
                // Commit is already acknowledged. A subsequent ordinary read
                // can refuse/fail without erasing it, including after intake stop.
                let id = policy::scope(&witness).identity.id.clone();
                let item = policy::item_id(&witness);
                match self.database.read(move |tx| rows::view(tx, &id, item)) {
                    Ok(pending) => match pending.wait() {
                        Ok(Some(view)) => (
                            AttemptDisposition::Committed {
                                view: Some(Box::new(view)),
                            },
                            None,
                        ),
                        Ok(None) => (
                            AttemptDisposition::Committed { view: None },
                            Some(StoreDiagnostic::new(std::io::Error::other(
                                "committed operation view is unavailable",
                            ))),
                        ),
                        Err(error) => (
                            AttemptDisposition::Committed { view: None },
                            Some(StoreDiagnostic::new(error)),
                        ),
                    },
                    Err(error) => (
                        AttemptDisposition::Committed { view: None },
                        Some(StoreDiagnostic::new(error)),
                    ),
                }
            }
            Ok(TransactionOutcome::Committed(Work::Observed(observation))) => {
                (attempt_observation(observation), None)
            }
            Ok(TransactionOutcome::ProvenNotCommitted(error)) => (
                AttemptDisposition::ProvenNotCommitted,
                Some(StoreDiagnostic::new(error)),
            ),
            Ok(TransactionOutcome::CommitUncertain(error)) => (
                AttemptDisposition::Unknown(UnknownOutcome::AcknowledgementLost),
                Some(StoreDiagnostic::new(error)),
            ),
            Err(error) => (
                AttemptDisposition::Unknown(UnknownOutcome::Unavailable),
                Some(StoreDiagnostic::new(error)),
            ),
        };
        AttemptReply {
            witness,
            disposition,
            diagnostic,
        }
    }
}

fn non_admission(error: &StorageError) -> NonAdmission {
    match error {
        StorageError::QueueFull => NonAdmission::QueueFull,
        StorageError::IntakeClosed => NonAdmission::IntakeClosed,
        _ => NonAdmission::AuthorityUnavailable,
    }
}

fn attempt_observation(observation: Inspection) -> AttemptDisposition {
    match observation {
        Inspection::Matching(view) => AttemptDisposition::AlreadyMatching(view),
        Inspection::Conflict(view) => AttemptDisposition::TerminalConflict(view),
        Inspection::Unknown(reason) => AttemptDisposition::Unknown(reason),
        // Unapplied is consumed only by the guarded transaction body above.
        Inspection::Unapplied(_) => AttemptDisposition::Unknown(UnknownOutcome::Contradictory),
    }
}

impl OperationTerminalStore for TerminalRepository<'_, '_> {
    fn snapshot(&self, raw_operation_id: &str) -> SnapshotReply {
        let id = raw_operation_id.to_owned();
        let pending = match self.database.read(move |tx| rows::snapshot(tx, &id)) {
            Ok(pending) => pending,
            Err(error) => {
                return SnapshotReply::NotAdmitted {
                    reason: non_admission(&error),
                    diagnostic: Some(StoreDiagnostic::new(error)),
                };
            }
        };
        match pending.wait() {
            Ok(Some(snapshot)) => SnapshotReply::Found(Box::new(snapshot)),
            Ok(None) => SnapshotReply::Missing,
            Err(error) => SnapshotReply::Unavailable(StoreDiagnostic::new(error)),
        }
    }

    fn complete_item(&self, command: &CompleteItemCommand) -> AttemptReply {
        self.attempt(CommandWitness::CompleteItem(Box::new(command.clone())))
    }

    fn finish(&self, command: &FinishCommand) -> AttemptReply {
        self.attempt(CommandWitness::Finish(Box::new(command.clone())))
    }

    fn interrupt(&self, command: &InterruptCommand) -> AttemptReply {
        self.attempt(CommandWitness::Interrupt(Box::new(command.clone())))
    }

    fn readback(&self, witness: &CommandWitness) -> ReadbackReply {
        if let Some(reason) = self.refusal(witness) {
            return ReadbackReply {
                witness: witness.clone(),
                disposition: ReadbackDisposition::NotAdmitted(reason),
                diagnostic: None,
            };
        }
        let work = witness.clone();
        let pending = match self
            .database
            .barrier_read(move |tx| policy::inspect(tx, &work))
        {
            Ok(pending) => pending,
            Err(error) => {
                return ReadbackReply {
                    witness: witness.clone(),
                    disposition: ReadbackDisposition::NotAdmitted(non_admission(&error)),
                    diagnostic: Some(StoreDiagnostic::new(error)),
                };
            }
        };
        let (disposition, diagnostic) = match pending.wait() {
            Ok(Inspection::Unapplied(view)) => (ReadbackDisposition::ProvenNotApplied(view), None),
            Ok(Inspection::Matching(view)) => (ReadbackDisposition::MatchesApplied(view), None),
            Ok(Inspection::Conflict(view)) => (ReadbackDisposition::TerminalConflict(view), None),
            Ok(Inspection::Unknown(reason)) => (ReadbackDisposition::Unknown(reason), None),
            Err(error) => (
                ReadbackDisposition::Unknown(UnknownOutcome::Unavailable),
                Some(StoreDiagnostic::new(error)),
            ),
        };
        ReadbackReply {
            witness: witness.clone(),
            disposition,
            diagnostic,
        }
    }
}
