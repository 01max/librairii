//! Collection forms, copied unions and controller-owned replacement/deletion plans.
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::{
    CollectionId,
    collection_codec::{
        Query,
        url::{Location, Scope},
    },
    collections::{Collection, Inspection},
    navigation::{Frame, Navigation},
};
use librairii_storage::{
    Result, StorageError,
    collections::{self, Observation},
    database::TransactionOutcome,
    shelves,
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
    Inspect(i64),
    Open(i64),
    EditQuery(i64),
    Create { name: String, criteria: Query },
    SaveUnion { name: String, ids: Vec<i64> },
    Rename { id: i64, name: String },
    Duplicate { id: i64, name: String },
    Move { id: i64, offset: i32 },
    PlanReplace { id: i64, criteria: Query },
    PlanDelete(i64),
    PreviewPage { ticket: Ticket, page: usize },
    Confirm(Ticket),
    PreviewUnion(Vec<i64>),
    OpenUnion(Vec<i64>),
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub ticket: Ticket,
    pub name: String,
    pub replacing: bool,
    pub shelves: Vec<(String, usize, bool)>,
    pub shelf_count: usize,
    pub page: usize,
    pub replacement: Option<Query>,
}
#[derive(Clone, Debug)]
pub enum Feedback {
    Ready,
    Saved(i64),
    Deleted,
    Error { code: String, uncertain: bool },
}
#[derive(Clone, Debug)]
pub struct Union {
    pub sources: Vec<(i64, String, i64)>,
    pub criteria: Query,
    pub unique: i64,
    pub overlap: i64,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub feedback: Option<Feedback>,
    pub inspection: Option<Inspection>,
    pub preview: Option<Preview>,
    pub union: Option<Union>,
    pub union_error: bool,
    pub navigation_error: bool,
}
#[derive(Debug)]
enum Plan {
    Replace(collections::ReplacementPlan),
    Delete(shelves::CollectionDeletionPlan),
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    plan: Option<(Ticket, Plan)>,
    union_ids: Vec<i64>,
}
fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T> {
    match outcome {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) => Err(e),
        TransactionOutcome::CommitUncertain(_) => Err(StorageError::RuntimeFenced),
    }
}
fn name(collection: &Collection) -> String {
    String::from_utf8_lossy(&collection.name).into_owned()
}
fn deletion_preview(
    plan: &shelves::CollectionDeletionPlan,
    ticket: Ticket,
    page: usize,
) -> Preview {
    let preview = plan.preview();
    let page = page.min(preview.shelf_count().saturating_sub(1));
    let shelves = preview
        .shelf(page)
        .into_iter()
        .map(|s| (s.shelf_name.clone(), s.affected_count(), s.incomplete))
        .collect();
    Preview {
        ticket,
        name: name(&preview.collection),
        replacing: false,
        shelves,
        shelf_count: preview.shelf_count(),
        page,
        replacement: None,
    }
}
fn edit(
    runtime: &Runtime<'_, '_, '_>,
    command: collections::Command,
) -> Result<collections::Response> {
    runtime.command(|db, _, _| committed(db.edit_collections(command, Default::default())?.wait()?))
}
fn changed(response: collections::Response) -> Result<Collection> {
    match response {
        collections::Response::Collection(c) => Ok(c),
        _ => Err(librairii_core::collections::CollectionError::NeedsAttention.into()),
    }
}
fn observe_union(runtime: &Runtime<'_, '_, '_>, ids: Vec<i64>) -> Result<Union> {
    let preview = runtime.command(|db, _, _| {
        committed(
            db.collection_selection_preview(ids, "en-GB".into(), Default::default())?
                .wait()?,
        )
    })?;
    let Observation::Ready(preview) = preview else {
        return Err(librairii_core::collections::CollectionError::NeedsAttention.into());
    };
    Ok(Union {
        sources: preview
            .sources
            .into_iter()
            .map(|s| (s.collection.id, name(&s.collection), s.count))
            .collect(),
        criteria: preview.criteria,
        unique: preview.unique_count,
        overlap: preview.overlap_count,
    })
}
fn open(runtime: &Runtime<'_, '_, '_>, navigation: &mut Navigation, id: i64) -> Result<()> {
    let mut request = navigation.current().location.request.clone();
    request.page = 1;
    let evaluation = runtime.command(|db, _, _| {
        committed(
            db.evaluate_collection(id, request.clone(), "en-GB".into(), Default::default())?
                .wait()?,
        )
    })?;
    let Observation::Ready(evaluation) = evaluation else {
        return Err(librairii_core::collections::CollectionError::NeedsAttention.into());
    };
    navigation
        .push(Frame {
            location: Location {
                scope: Scope::Collection(
                    CollectionId::new(id).map_err(|_| StorageError::InvalidNavigation)?,
                ),
                request: librairii_core::query::QueryRequest {
                    criteria: evaluation.criteria,
                    ..request
                },
            },
            selection: Vec::new(),
        })
        .map_err(|_| StorageError::InvalidNavigation)
}
impl Controller {
    pub fn refresh_union(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        known: &[crate::browsing::Group],
    ) -> Result<()> {
        self.union_ids.retain(|id| {
            known
                .iter()
                .any(|g| matches!(g.scope,Scope::Collection(current) if current.get()==*id))
        });
        self.snapshot.union_error = false;
        if self.union_ids.is_empty() {
            self.snapshot.union = None;
            return Ok(());
        }
        match observe_union(runtime, self.union_ids.clone()) {
            Ok(union) => self.snapshot.union = Some(union),
            Err(StorageError::Collection(_)) => {
                self.snapshot.union = None;
                self.snapshot.union_error = true;
            }
            Err(error) => {
                self.snapshot.union = None;
                self.snapshot.union_error = true;
                return Err(error);
            }
        }
        Ok(())
    }
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        generation: LibraryGeneration,
        action: Action,
        revision: u64,
        navigation: &mut Navigation,
    ) {
        self.snapshot.revision = revision;
        self.snapshot.navigation_error = false;
        let result = if runtime.accepts(&generation) {
            self.apply(runtime, action, navigation)
        } else {
            Err(StorageError::SnapshotChanged)
        };
        self.snapshot.feedback = Some(match result {
            Ok(feedback) => feedback,
            Err(e) => Feedback::Error {
                uncertain: matches!(e, StorageError::RuntimeFenced),
                code: e.to_string(),
            },
        });
    }
    fn apply(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        action: Action,
        navigation: &mut Navigation,
    ) -> Result<Feedback> {
        let opening = matches!(action, Action::OpenUnion(_));
        match action {
            Action::Open(id) => {
                open(runtime, navigation, id)?;
                Ok(Feedback::Ready)
            }
            Action::EditQuery(id) => {
                let criteria =
                    runtime.command(|db, _, _| db.collection_repair_draft(id)?.wait())?;
                let mut frame = navigation.current().clone();
                frame.location.scope = Scope::Custom;
                frame.location.request.criteria = criteria;
                frame.location.request.page = 1;
                navigation
                    .push(frame)
                    .map_err(|_| StorageError::InvalidNavigation)?;
                Ok(Feedback::Ready)
            }
            Action::Inspect(id) => {
                let collections::Response::Inspection(inspection) =
                    edit(runtime, collections::Command::Inspect { id })?
                else {
                    return Err(StorageError::CompatibilityInvariant);
                };
                self.snapshot.inspection = Some(inspection);
                Ok(Feedback::Ready)
            }
            Action::Create { name, criteria } => {
                let saved = changed(edit(
                    runtime,
                    collections::Command::Create {
                        name,
                        criteria: criteria.clone(),
                    },
                )?)?;
                let mut frame = navigation.current().clone();
                frame.location.scope = Scope::Collection(
                    CollectionId::new(saved.id).map_err(|_| StorageError::InvalidNavigation)?,
                );
                frame.location.request.criteria = criteria;
                self.snapshot.navigation_error = navigation.replace(frame).is_err();
                Ok(Feedback::Saved(saved.id))
            }
            Action::SaveUnion { name, ids } => {
                let saved = runtime.command(|db, _, _| {
                    committed(
                        db.save_combined_collection(ids, name, Default::default())?
                            .wait()?,
                    )
                })?;
                let Observation::Ready(saved) = saved else {
                    return Err(librairii_core::collections::CollectionError::NeedsAttention.into());
                };
                self.snapshot.navigation_error = open(runtime, navigation, saved.id).is_err();
                Ok(Feedback::Saved(saved.id))
            }
            Action::Rename { id, name } => {
                changed(edit(runtime, collections::Command::Rename { id, name })?)?;
                Ok(Feedback::Saved(id))
            }
            Action::Duplicate { id, name } => {
                let saved = changed(edit(runtime, collections::Command::Duplicate { id, name })?)?;
                self.snapshot.navigation_error = open(runtime, navigation, saved.id).is_err();
                Ok(Feedback::Saved(saved.id))
            }
            Action::Move { id, offset } => {
                if ![-1, 1].contains(&offset) {
                    return Err(librairii_core::collections::CollectionError::InvalidOrder.into());
                }
                let mut ids = runtime.command(|db, _, _| {
                    Ok(db
                        .collections()?
                        .wait()?
                        .into_iter()
                        .map(|c| c.id)
                        .collect::<Vec<_>>())
                })?;
                let index = ids
                    .iter()
                    .position(|i| *i == id)
                    .ok_or(librairii_core::collections::CollectionError::InvalidOrder)?;
                let next = index
                    .checked_add_signed(offset as isize)
                    .filter(|i| *i < ids.len())
                    .ok_or(librairii_core::collections::CollectionError::InvalidOrder)?;
                ids.swap(index, next);
                edit(runtime, collections::Command::Reorder(ids))?;
                Ok(Feedback::Saved(id))
            }
            Action::PlanReplace { id, criteria } => {
                let plan = runtime.command(|db, _, _| {
                    db.collection_replacement_plan(id, criteria.clone())?.wait()
                })?;
                let ticket = Ticket(Arc::new(()));
                self.snapshot.preview = Some(Preview {
                    ticket: ticket.clone(),
                    name: name(plan.before()),
                    replacing: true,
                    shelves: Vec::new(),
                    shelf_count: 0,
                    page: 0,
                    replacement: Some(criteria),
                });
                self.plan = Some((ticket, Plan::Replace(plan)));
                Ok(Feedback::Ready)
            }
            Action::PlanDelete(id) => {
                let id = CollectionId::new(id).map_err(|_| StorageError::InvalidNavigation)?;
                let plan = runtime.command(|db, _, _| {
                    db.linked_collection_deletion_preview(id, "en-GB".into())?
                        .wait()
                })?;
                let ticket = Ticket(Arc::new(()));
                self.snapshot.preview = Some(deletion_preview(&plan, ticket.clone(), 0));
                self.plan = Some((ticket, Plan::Delete(plan)));
                Ok(Feedback::Ready)
            }
            Action::PreviewPage { ticket, page } => {
                let (expected, Plan::Delete(plan)) = self
                    .plan
                    .as_ref()
                    .ok_or(librairii_core::collections::CollectionError::StalePreview)?
                else {
                    return Err(librairii_core::collections::CollectionError::StalePreview.into());
                };
                if *expected != ticket {
                    return Err(librairii_core::collections::CollectionError::StalePreview.into());
                }
                self.snapshot.preview = Some(deletion_preview(plan, ticket, page));
                Ok(Feedback::Ready)
            }
            Action::Confirm(ticket) => {
                let (expected, plan) = self
                    .plan
                    .take()
                    .ok_or(librairii_core::collections::CollectionError::StalePreview)?;
                if expected != ticket {
                    self.plan = Some((expected, plan));
                    return Err(librairii_core::collections::CollectionError::StalePreview.into());
                }
                self.snapshot.preview = None;
                match plan {
                    Plan::Replace(plan) => {
                        let saved = runtime.command(|db, _, _| {
                            committed(db.replace_collection(plan, Default::default())?.wait()?)
                        })?;
                        self.snapshot.inspection = None;
                        if matches!(navigation.current().location.scope,Scope::Collection(id) if id.get()==saved.id)
                        {
                            let opened = open(runtime, navigation, saved.id);
                            self.snapshot.navigation_error = opened.is_err();
                        }
                        Ok(Feedback::Saved(saved.id))
                    }
                    Plan::Delete(plan) => {
                        let stamp = navigation.stamp();
                        let deleted = runtime.command(|db, _, _| {
                            committed(
                                db.delete_linked_collection(
                                    plan,
                                    true,
                                    Vec::new(),
                                    Default::default(),
                                )?
                                .wait()?,
                            )
                        })?;
                        self.snapshot.navigation_error =
                            deleted.apply_navigation(navigation, &stamp).is_err();
                        Ok(Feedback::Deleted)
                    }
                }
            }
            Action::PreviewUnion(ids) | Action::OpenUnion(ids) => {
                self.union_ids = ids;
                self.snapshot.union = None;
                self.snapshot.union_error = false;
                if self.union_ids.is_empty() && !opening {
                    return Ok(Feedback::Ready);
                }
                let preview = match observe_union(runtime, self.union_ids.clone()) {
                    Ok(preview) => preview,
                    Err(error) => {
                        self.snapshot.union_error = true;
                        return Err(error);
                    }
                };
                if opening {
                    let mut request = navigation.current().location.request.clone();
                    request.criteria = preview.criteria.clone();
                    request.page = 1;
                    navigation
                        .push(Frame {
                            location: Location {
                                scope: Scope::Custom,
                                request,
                            },
                            selection: Vec::new(),
                        })
                        .map_err(|_| StorageError::InvalidNavigation)?;
                }
                self.snapshot.union = Some(preview);
                Ok(Feedback::Ready)
            }
        }
    }
}
