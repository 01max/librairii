//! Typed tag commands. Opaque deletion authority stays on the controller;
//! observations contain counts, labels and one bounded assignment workspace.
use super::Targets;
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::tagging::{
    AssignmentResult, AssignmentWorkspace, CatalogEntry, CreateDefinition, Kind, TagError,
};
use librairii_storage::{
    Result, StorageError,
    database::TransactionOutcome,
    tagging::{self, DeleteTarget, DeletionPlan},
};
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
    SaveDefinition {
        id: Option<i64>,
        key: String,
        label: String,
        color: String,
        kind: Kind,
    },
    SaveValue {
        definition: i64,
        id: Option<i64>,
        key: String,
        label: String,
    },
    MoveDefinition {
        id: i64,
        offset: i32,
    },
    MoveValue {
        definition: i64,
        id: i64,
        offset: i32,
    },
    Plan(DeleteTarget),
    Confirm(Ticket),
    Workspace(Targets),
    Assign {
        targets: Targets,
        definition: i64,
        value: Option<i64>,
        assigned: bool,
    },
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub ticket: Ticket,
    pub label: String,
    pub value: bool,
    pub assignments: i64,
    pub values: i64,
    pub collections: usize,
}
#[derive(Clone, Debug)]
pub enum Feedback {
    Ready,
    Saved,
    Deleted,
    Assignment(AssignmentResult),
    Error { code: String, uncertain: bool },
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub refresh_failed: bool,
    pub feedback: Option<Feedback>,
    pub preview: Option<Preview>,
    pub targets: Option<Targets>,
    pub workspace: Option<AssignmentWorkspace>,
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    plan: Option<(Ticket, DeletionPlan)>,
}
fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error) => Err(error),
        TransactionOutcome::CommitUncertain(_) => Err(StorageError::RuntimeFenced),
    }
}
fn reorder(mut ids: Vec<i64>, id: i64, offset: i32) -> Result<Vec<i64>> {
    if ![-1, 1].contains(&offset) {
        return Err(TagError::InvalidOrder.into());
    }
    let position = ids
        .iter()
        .position(|v| *v == id)
        .ok_or(TagError::InvalidOrder)?;
    let next = position
        .checked_add_signed(offset as isize)
        .filter(|i| *i < ids.len())
        .ok_or(TagError::InvalidOrder)?;
    ids.swap(position, next);
    Ok(ids)
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
        self.snapshot.refresh_failed = false;
        let result = if runtime.accepts(&generation) {
            self.apply(runtime, action)
        } else {
            Err(StorageError::SnapshotChanged)
        };
        self.snapshot.feedback = Some(match result {
            Ok(feedback) => feedback,
            Err(error) => Feedback::Error {
                uncertain: matches!(error, StorageError::RuntimeFenced),
                code: error.to_string(),
            },
        });
    }
    fn catalog(runtime: &Runtime<'_, '_, '_>) -> Result<Vec<CatalogEntry>> {
        runtime.command(|db, _, _| db.tag_catalog()?.wait())
    }
    fn edit(runtime: &Runtime<'_, '_, '_>, command: tagging::Command) -> Result<tagging::Response> {
        runtime.command(|db, _, _| committed(db.edit_tags(command, Default::default())?.wait()?))
    }
    fn apply(&mut self, runtime: &Runtime<'_, '_, '_>, action: Action) -> Result<Feedback> {
        match action {
            Action::SaveDefinition {
                id,
                key,
                label,
                color,
                kind,
            } => {
                let command = if let Some(id) = id {
                    tagging::Command::SaveDefinition { id, label, color }
                } else {
                    tagging::Command::CreateDefinition(CreateDefinition {
                        key,
                        label,
                        color,
                        kind,
                    })
                };
                Self::edit(runtime, command)?;
                Ok(Feedback::Saved)
            }
            Action::SaveValue {
                definition,
                id,
                key,
                label,
            } => {
                let entry = Self::catalog(runtime)?
                    .into_iter()
                    .find(|e| e.definition.id == definition)
                    .ok_or(TagError::DefinitionNotFound)?;
                let command = if let Some(id) = id {
                    if !entry.values.iter().any(|v| v.id == id) {
                        return Err(TagError::ValueNotFound.into());
                    }
                    tagging::Command::RenameValue { id, label }
                } else {
                    tagging::Command::CreateValue {
                        definition_id: definition,
                        key,
                        label,
                    }
                };
                Self::edit(runtime, command)?;
                Ok(Feedback::Saved)
            }
            Action::MoveDefinition { id, offset } => {
                let ids = Self::catalog(runtime)?
                    .into_iter()
                    .filter(|e| e.definition.source == "user")
                    .map(|e| e.definition.id)
                    .collect();
                Self::edit(
                    runtime,
                    tagging::Command::ReorderDefinitions(reorder(ids, id, offset)?),
                )?;
                Ok(Feedback::Saved)
            }
            Action::MoveValue {
                definition,
                id,
                offset,
            } => {
                let entry = Self::catalog(runtime)?
                    .into_iter()
                    .find(|e| e.definition.id == definition)
                    .ok_or(TagError::DefinitionNotFound)?;
                Self::edit(
                    runtime,
                    tagging::Command::ReorderValues {
                        definition_id: definition,
                        ids: reorder(entry.values.into_iter().map(|v| v.id).collect(), id, offset)?,
                    },
                )?;
                Ok(Feedback::Saved)
            }
            Action::Plan(target) => {
                let plan = runtime.command(|db, _, _| db.tag_deletion_plan(target)?.wait())?;
                let ticket = Ticket(Arc::new(()));
                self.snapshot.preview = Some(Preview {
                    ticket: ticket.clone(),
                    label: plan
                        .value
                        .as_ref()
                        .map(|v| v.label.clone())
                        .unwrap_or_else(|| plan.definition.label.clone()),
                    value: plan.value.is_some(),
                    assignments: plan.assignment_count,
                    values: plan.value_count,
                    collections: plan.affected_collection_ids.len(),
                });
                self.plan = Some((ticket, plan));
                Ok(Feedback::Ready)
            }
            Action::Confirm(ticket) => {
                let (expected, plan) = self.plan.as_ref().ok_or(TagError::DeletePlanStale)?;
                if *expected != ticket {
                    return Err(TagError::DeletePlanStale.into());
                }
                Self::edit(runtime, tagging::Command::Delete(Box::new(plan.clone())))?;
                self.plan = None;
                self.snapshot.preview = None;
                Ok(Feedback::Deleted)
            }
            Action::Workspace(targets) => {
                if !runtime.accepts(&targets.generation) {
                    return Err(StorageError::SnapshotChanged);
                }
                self.snapshot.workspace = None;
                let workspace = runtime.command(|db, _, _| {
                    db.assignment_workspace_guarded(targets.stories.clone())?
                        .wait()
                })?;
                self.snapshot.targets = Some(targets);
                self.snapshot.workspace = Some(workspace);
                Ok(Feedback::Ready)
            }
            Action::Assign {
                targets,
                definition,
                value,
                assigned,
            } => {
                if !runtime.accepts(&targets.generation) {
                    return Err(StorageError::SnapshotChanged);
                }
                let command = if let Some(value) = value {
                    tagging::AssignmentEdit::ChoiceToggle {
                        definition_id: definition,
                        value_id: value,
                        assigned,
                    }
                } else {
                    tagging::AssignmentEdit::Boolean {
                        definition_id: definition,
                        assigned,
                    }
                };
                let response = runtime.command(|db, _, _| {
                    committed(
                        db.edit_tags_guarded(targets.stories.clone(), command, Default::default())?
                            .wait()?,
                    )
                })?;
                let tagging::Response::Assignment(result) = response else {
                    return Err(StorageError::CompatibilityInvariant);
                };
                self.snapshot.workspace = match runtime.command(|db, _, _| {
                    db.assignment_workspace_guarded(targets.stories.clone())?
                        .wait()
                }) {
                    Ok(workspace) => Some(workspace),
                    Err(_) => {
                        self.snapshot.refresh_failed = true;
                        None
                    }
                };
                self.snapshot.targets = Some(targets);
                Ok(Feedback::Assignment(result))
            }
        }
    }
}
