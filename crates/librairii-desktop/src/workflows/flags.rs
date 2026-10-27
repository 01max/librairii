//! Independent flags and changed-only Undo authority owned by the controller.
use super::Targets;
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::broken::{FlagChange, FlagResult};
use librairii_storage::{Result, StorageError, database::TransactionOutcome};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Ticket(Arc<()>);
impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Ticket {}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Set { targets: Targets, broken: bool },
    Undo(Ticket),
}
#[derive(Clone, Debug)]
pub struct Failure {
    pub code: String,
    pub uncertain: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub result: Option<std::result::Result<FlagResult, Failure>>,
    pub restored: bool,
    pub undo: Option<Ticket>,
}
#[derive(Debug)]
struct Undo {
    ticket: Ticket,
    targets: Targets,
    changes: Vec<FlagChange>,
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    undo: Option<Undo>,
}
fn committed(outcome: TransactionOutcome<FlagResult>) -> Result<FlagResult> {
    match outcome {
        TransactionOutcome::Committed(result) => Ok(result),
        TransactionOutcome::ProvenNotCommitted(error) => Err(error),
        TransactionOutcome::CommitUncertain(_) => Err(StorageError::RuntimeFenced),
    }
}
impl Controller {
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        generation: LibraryGeneration,
        action: Action,
        revision: u64,
    ) {
        self.snapshot.revision = revision;
        self.snapshot.restored = matches!(action, Action::Undo(_));
        let result = if runtime.accepts(&generation) {
            self.apply(runtime, action)
        } else {
            Err(StorageError::SnapshotChanged)
        };
        if result.is_err() {
            self.undo = None;
        }
        self.snapshot.undo = self.undo.as_ref().map(|u| u.ticket.clone());
        self.snapshot.result = Some(result.map_err(|e| Failure {
            uncertain: matches!(e, StorageError::RuntimeFenced),
            code: e.to_string(),
        }));
    }
    fn apply(&mut self, runtime: &Runtime<'_, '_, '_>, action: Action) -> Result<FlagResult> {
        match action {
            Action::Set {
                mut targets,
                broken,
            } => {
                self.undo = None;
                if !runtime.accepts(&targets.generation) {
                    return Err(StorageError::SnapshotChanged);
                }
                let result = runtime.command(|db, _, _| {
                    committed(
                        db.set_broken_guarded(targets.stories.clone(), broken, Default::default())?
                            .wait()?,
                    )
                })?;
                if !result.changes.is_empty() {
                    let mut seen = std::collections::BTreeSet::new();
                    targets.stories.retain(|s| {
                        result.changes.iter().any(|c| c.story_id == s.id) && seen.insert(s.id)
                    });
                    // Writer result order follows the validated, deduplicated
                    // input, so the stored identity sequence remains exact.
                    self.undo = Some(Undo {
                        ticket: Ticket(Arc::new(())),
                        targets,
                        changes: result.changes.clone(),
                    });
                }
                Ok(result)
            }
            Action::Undo(ticket) => {
                let undo = self
                    .undo
                    .as_ref()
                    .filter(|u| u.ticket == ticket)
                    .ok_or(StorageError::SnapshotChanged)?;
                if !runtime.accepts(&undo.targets.generation) {
                    return Err(StorageError::SnapshotChanged);
                }
                let result = runtime.command(|db, _, _| {
                    committed(
                        db.restore_broken_guarded(
                            undo.targets.stories.clone(),
                            undo.changes.clone(),
                            Default::default(),
                        )?
                        .wait()?,
                    )
                })?;
                self.undo = None;
                Ok(result)
            }
        }
    }
}
