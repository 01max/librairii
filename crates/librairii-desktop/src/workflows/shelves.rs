//! Explicit approved membership and opaque native creation/confirmation plans.
use super::Targets;
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::{
    CollectionId, ShelfId,
    collection_codec::{
        Query,
        url::{Location, Scope},
    },
    navigation::{Frame, Navigation},
    query::QueryRequest,
    shelves::{Shelf, ShelfError, StoryReference},
};
use librairii_storage::{
    Result, StorageError,
    database::TransactionOutcome,
    shelves::{self, Curation},
};
use std::{collections::BTreeSet, sync::Arc};

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
    Inspect(ShelfId),
    Rename {
        id: ShelfId,
        name: String,
    },
    MoveShelf {
        id: ShelfId,
        offset: i32,
    },
    PlanDuplicate {
        id: ShelfId,
        name: String,
    },
    PreviewCreate {
        name: String,
        sources: Vec<CollectionId>,
        manual: Option<Targets>,
    },
    ToggleSeed {
        ticket: Ticket,
        index: usize,
    },
    SeedAll {
        ticket: Ticket,
        selected: bool,
    },
    PreviewPage {
        ticket: Ticket,
        page: usize,
    },
    PreviewWarnings {
        ticket: Ticket,
        warnings: bool,
    },
    AddManual {
        id: ShelfId,
        targets: Targets,
    },
    PlanMembers {
        id: ShelfId,
        uuids: Vec<String>,
        exclude: bool,
    },
    MoveMember {
        id: ShelfId,
        uuid: String,
        offset: i32,
    },
    Exclusions {
        id: ShelfId,
        page: usize,
    },
    AllowSuggestions {
        id: ShelfId,
        uuid: String,
        page: usize,
    },
    Confirm(Ticket),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Create,
    Duplicate,
    Remove,
    RemoveAndExclude,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub index: usize,
    pub title: String,
    pub uuid: String,
    pub sources: Vec<String>,
    pub manual: bool,
    pub selected: bool,
    pub warning: Option<&'static str>,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub ticket: Ticket,
    pub kind: Kind,
    pub name: String,
    pub rows: Vec<Row>,
    pub total: usize,
    pub selected: usize,
    pub warning_count: usize,
    pub warnings: bool,
    pub incomplete: bool,
    pub page: usize,
    pub pages: usize,
    pub source_count: i64,
    pub unavailable_count: i64,
}
#[derive(Clone, Debug)]
pub struct Exclusions {
    pub id: ShelfId,
    pub total: usize,
    pub page: usize,
    pub rows: Vec<(String, String)>,
}
#[derive(Clone, Debug)]
pub enum Feedback {
    Ready,
    Saved(ShelfId),
    Error { code: String, uncertain: bool },
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub feedback: Option<Feedback>,
    pub inspection: Option<Shelf>,
    pub sources: Vec<librairii_core::shelves::Source>,
    pub preview: Option<Preview>,
    pub exclusions: Option<Exclusions>,
    pub refresh_failed: bool,
    pub navigation_error: bool,
    pub focus_member: Option<String>,
}
#[derive(Debug)]
enum Plan {
    Create {
        plan: shelves::CreationPlan,
        selected: BTreeSet<String>,
        page: usize,
        warnings: bool,
    },
    Duplicate {
        plan: shelves::CurationPlan,
        name: String,
    },
    Members {
        plan: shelves::CurationPlan,
        rows: Vec<Row>,
        exclude: bool,
        page: usize,
    },
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    plan: Option<(Ticket, Plan)>,
}
fn committed<T>(value: TransactionOutcome<T>) -> Result<T> {
    match value {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) => Err(e),
        TransactionOutcome::CommitUncertain(_) => Err(StorageError::RuntimeFenced),
    }
}
fn changed(response: shelves::Response) -> Result<Shelf> {
    match response {
        shelves::Response::Shelf(s) => Ok(s),
        _ => Err(StorageError::CompatibilityInvariant),
    }
}
fn open(navigation: &mut Navigation, id: ShelfId) -> Result<()> {
    navigation
        .push(Frame {
            location: Location {
                scope: Scope::Shelf {
                    id,
                    manual_order: true,
                },
                request: QueryRequest {
                    page: 1,
                    page_size: navigation.current().location.request.page_size,
                    ..Default::default()
                },
            },
            selection: Vec::new(),
        })
        .map_err(|_| StorageError::InvalidNavigation)
}
impl Controller {
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        generation: LibraryGeneration,
        action: Action,
        revision: u64,
        navigation: &mut Navigation,
    ) {
        self.snapshot.revision = revision;
        self.snapshot.refresh_failed = false;
        self.snapshot.navigation_error = false;
        self.snapshot.focus_member = None;
        let result = if runtime.accepts(&generation) {
            self.apply(runtime, action, navigation)
        } else {
            Err(StorageError::SnapshotChanged)
        };
        self.snapshot.feedback = Some(match result {
            Ok(feedback) => feedback,
            Err(e) => Feedback::Error {
                code: e.to_string(),
                uncertain: matches!(e, StorageError::RuntimeFenced),
            },
        });
    }
    fn saved(&mut self, shelf: Shelf) -> Feedback {
        let id = shelf.id;
        self.snapshot.inspection = Some(shelf);
        Feedback::Saved(id)
    }
    fn preview(&mut self) {
        let Some((ticket, plan)) = &self.plan else {
            self.snapshot.preview = None;
            return;
        };
        let mut output = Preview {
            ticket: ticket.clone(),
            kind: Kind::Create,
            name: String::new(),
            rows: Vec::new(),
            total: 0,
            selected: 0,
            warning_count: 0,
            warnings: false,
            incomplete: false,
            page: 0,
            pages: 1,
            source_count: 0,
            unavailable_count: 0,
        };
        match plan {
            Plan::Create {
                plan,
                selected,
                page,
                warnings,
            } => {
                let p = plan.preview();
                output.name = p.name.clone();
                output.total = p.candidate_count();
                output.selected = selected.len();
                output.warning_count = p.warning_count();
                output.incomplete = p.incomplete;
                output.warnings = *warnings;
                output.page = *page;
                let count = if *warnings {
                    output.warning_count
                } else {
                    output.total
                };
                output.pages = count.div_ceil(25).max(1);
                output.rows = if *warnings {
                    p.warnings_page(*page)
                        .iter()
                        .enumerate()
                        .map(|(index, w)| Row {
                            index,
                            title: w
                                .source
                                .as_ref()
                                .map(|s| s.name.clone())
                                .unwrap_or_default(),
                            uuid: w.uuid.clone().unwrap_or_default(),
                            sources: Vec::new(),
                            manual: false,
                            selected: false,
                            warning: Some(w.code),
                        })
                        .collect()
                } else {
                    p.candidates_page(*page)
                        .iter()
                        .enumerate()
                        .map(|(index, c)| Row {
                            index,
                            title: c.title.clone(),
                            uuid: c.uuid.clone(),
                            sources: c.sources.iter().map(|s| s.name.clone()).collect(),
                            manual: c.manual,
                            selected: selected.contains(&c.uuid),
                            warning: None,
                        })
                        .collect()
                };
            }
            Plan::Duplicate { plan, name } => {
                let shelf = plan.shelf();
                output.kind = Kind::Duplicate;
                output.name = name.clone();
                output.total = shelf.count as usize;
                output.source_count = shelf.source_count;
                output.unavailable_count = shelf.unavailable_count;
            }
            Plan::Members {
                plan,
                rows,
                exclude,
                page,
            } => {
                let shelf = plan.shelf();
                output.kind = if *exclude {
                    Kind::RemoveAndExclude
                } else {
                    Kind::Remove
                };
                output.name = shelf.name.clone();
                output.total = rows.len();
                output.selected = rows.len();
                output.page = *page;
                output.pages = rows.len().div_ceil(25).max(1);
                output.rows = rows
                    .iter()
                    .skip(page.saturating_mul(25))
                    .take(25)
                    .cloned()
                    .collect();
            }
        }
        self.snapshot.preview = Some(output);
    }
    fn exclusions(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        id: ShelfId,
        page: usize,
    ) -> Result<()> {
        let mut loaded = runtime.command(|db, _, _| db.shelf_exclusions_page(id, page)?.wait())?;
        let clamped = page.min(loaded.total.div_ceil(25).max(1) - 1);
        if clamped != page {
            loaded = runtime.command(|db, _, _| db.shelf_exclusions_page(id, clamped)?.wait())?;
        }
        self.snapshot.exclusions = Some(Exclusions {
            id,
            total: loaded.total,
            page: clamped,
            rows: loaded.rows.into_iter().map(|r| (r.uuid, r.title)).collect(),
        });
        Ok(())
    }
    fn apply(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        action: Action,
        navigation: &mut Navigation,
    ) -> Result<Feedback> {
        match action {
            Action::Inspect(id) => {
                let loaded = runtime.command(|db, _, _| db.inspect_shelf(id)?.wait())?;
                let (inspection, sources) = (loaded.shelf, loaded.sources);
                self.snapshot.inspection = Some(inspection);
                self.snapshot.sources = sources;
                Ok(Feedback::Ready)
            }
            Action::Rename { id, name } => {
                let saved = runtime.command(|db, _, _| {
                    committed(
                        db.edit_shelves(shelves::Command::Rename { id, name }, Default::default())?
                            .wait()?,
                    )
                })?;
                Ok(self.saved(changed(saved)?))
            }
            Action::MoveShelf { id, offset } => {
                if ![-1, 1].contains(&offset) {
                    return Err(ShelfError::InvalidInput.into());
                }
                let mut ids = runtime.command(|db, _, _| {
                    Ok(db
                        .shelves()?
                        .wait()?
                        .into_iter()
                        .map(|s| s.id)
                        .collect::<Vec<_>>())
                })?;
                let index = ids
                    .iter()
                    .position(|current| *current == id)
                    .ok_or(ShelfError::NotFound)?;
                let next = index
                    .checked_add_signed(offset as isize)
                    .filter(|i| *i < ids.len())
                    .ok_or(ShelfError::InvalidInput)?;
                ids.swap(index, next);
                runtime.command(|db, _, _| {
                    committed(
                        db.edit_shelves(shelves::Command::Reorder(ids), Default::default())?
                            .wait()?,
                    )
                })?;
                Ok(Feedback::Saved(id))
            }
            Action::PlanDuplicate { id, name } => {
                librairii_core::shelves::normalize_name(&name)?;
                let plan = runtime.command(|db, _, _| db.shelf_curation_snapshot(id)?.wait())?;
                self.plan = Some((Ticket(Arc::new(())), Plan::Duplicate { plan, name }));
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::PreviewCreate {
                name,
                sources,
                manual,
            } => {
                if manual
                    .as_ref()
                    .is_some_and(|t| !runtime.accepts(&t.generation))
                {
                    return Err(StorageError::SnapshotChanged);
                }
                let targets = manual.map(|t| t.stories).unwrap_or_default();
                let plan = runtime.command(|db, _, _| {
                    db.shelf_creation_preview_for_stories(name, sources, targets, "en-GB".into())?
                        .wait()
                })?;
                self.plan = Some((
                    Ticket(Arc::new(())),
                    Plan::Create {
                        plan,
                        selected: BTreeSet::new(),
                        page: 0,
                        warnings: false,
                    },
                ));
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::ToggleSeed { ticket, index } => {
                let Some((
                    expected,
                    Plan::Create {
                        plan,
                        selected,
                        page,
                        warnings: false,
                    },
                )) = &mut self.plan
                else {
                    return Err(ShelfError::StaleReview.into());
                };
                if *expected != ticket {
                    return Err(ShelfError::StaleReview.into());
                }
                let candidate = plan
                    .preview()
                    .candidates_page(*page)
                    .get(index)
                    .ok_or(ShelfError::InvalidInput)?;
                if !selected.remove(&candidate.uuid) {
                    selected.insert(candidate.uuid.clone());
                }
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::SeedAll {
                ticket,
                selected: select,
            } => {
                let Some((expected, Plan::Create { plan, selected, .. })) = &mut self.plan else {
                    return Err(ShelfError::StaleReview.into());
                };
                if *expected != ticket {
                    return Err(ShelfError::StaleReview.into());
                }
                selected.clear();
                if select {
                    selected.extend(
                        plan.preview()
                            .all_uuids()?
                            .into_iter()
                            .map(|r| r.as_stored().to_owned()),
                    );
                }
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::PreviewPage {
                ticket,
                page: requested,
            } => {
                let Some((expected, plan)) = &mut self.plan else {
                    return Err(ShelfError::StaleReview.into());
                };
                if *expected != ticket {
                    return Err(ShelfError::StaleReview.into());
                }
                match plan {
                    Plan::Create {
                        plan,
                        page,
                        warnings,
                        ..
                    } => {
                        let n = if *warnings {
                            plan.preview().warning_count()
                        } else {
                            plan.preview().candidate_count()
                        };
                        *page = requested.min(n.div_ceil(25).max(1) - 1);
                    }
                    Plan::Members { rows, page, .. } => {
                        *page = requested.min(rows.len().div_ceil(25).max(1) - 1)
                    }
                    Plan::Duplicate { .. } => return Err(ShelfError::InvalidInput.into()),
                };
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::PreviewWarnings {
                ticket,
                warnings: show,
            } => {
                let Some((expected, Plan::Create { warnings, page, .. })) = &mut self.plan else {
                    return Err(ShelfError::StaleReview.into());
                };
                if *expected != ticket {
                    return Err(ShelfError::StaleReview.into());
                }
                *warnings = show;
                *page = 0;
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::AddManual { id, targets } => {
                if !runtime.accepts(&targets.generation) {
                    return Err(StorageError::SnapshotChanged);
                }
                let saved = runtime.command(|db, _, _| {
                    committed(
                        db.add_shelf_stories_guarded(
                            id,
                            targets.stories,
                            "en-GB".into(),
                            Default::default(),
                        )?
                        .wait()?,
                    )
                })?;
                Ok(self.saved(saved))
            }
            Action::PlanMembers { id, uuids, exclude } => {
                let plan = runtime.command(|db, _, _| db.shelf_curation_snapshot(id)?.wait())?;
                let wanted = uuids.into_iter().collect::<BTreeSet<_>>();
                if wanted.is_empty() {
                    return Err(ShelfError::InvalidInput.into());
                }
                let rows = plan
                    .members()
                    .iter()
                    .filter(|m| wanted.contains(&m.uuid))
                    .cloned()
                    .enumerate()
                    .map(|(index, m)| Row {
                        index,
                        title: m.cached_title,
                        uuid: m.uuid,
                        sources: m.origins.into_iter().map(|s| s.name).collect(),
                        manual: m.manually_retained,
                        selected: true,
                        warning: None,
                    })
                    .collect::<Vec<_>>();
                if rows.len() != wanted.len() {
                    return Err(ShelfError::StaleReview.into());
                }
                self.plan = Some((
                    Ticket(Arc::new(())),
                    Plan::Members {
                        plan,
                        rows,
                        exclude,
                        page: 0,
                    },
                ));
                self.preview();
                Ok(Feedback::Ready)
            }
            Action::MoveMember { id, uuid, offset } => {
                if !matches!(navigation.current().location.scope,Scope::Shelf{id:current,manual_order:true} if current==id)
                    || navigation.current().location.request.criteria != Query::default()
                    || ![-1, 1].contains(&offset)
                {
                    return Err(ShelfError::InvalidInput.into());
                }
                let size = librairii_core::query::normalize_request(
                    navigation.current().location.request.clone(),
                )?
                .page_size
                .clamp(1, 25) as usize;
                let plan = runtime.command(|db, _, _| db.shelf_curation_snapshot(id)?.wait())?;
                let mut members = plan.members().to_vec();
                let index = members
                    .iter()
                    .position(|m| m.uuid == uuid)
                    .ok_or(ShelfError::NotFound)?;
                let next = index
                    .checked_add_signed(offset as isize)
                    .filter(|i| *i < members.len())
                    .ok_or(ShelfError::InvalidInput)?;
                members.swap(index, next);
                let refs = members
                    .iter()
                    .map(|m| m.reference())
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let saved = runtime.command(|db, _, _| {
                    committed(
                        db.curate_shelf_reviewed(
                            plan,
                            Curation::Reorder(refs),
                            "en-GB".into(),
                            Default::default(),
                        )?
                        .wait()?,
                    )
                })?;
                let mut frame = navigation.current().clone();
                frame.location.request.page = (next / size + 1) as i64;
                self.snapshot.navigation_error = navigation.replace(frame).is_err();
                self.snapshot.focus_member = Some(uuid);
                Ok(self.saved(saved))
            }
            Action::Exclusions { id, page } => {
                self.exclusions(runtime, id, page)?;
                Ok(Feedback::Ready)
            }
            Action::AllowSuggestions { id, uuid, page } => {
                let saved = runtime.command(|db, _, _| {
                    committed(
                        db.curate_shelf(
                            id,
                            Curation::ClearExclusion(StoryReference::from_stored(&uuid)?),
                            "en-GB".into(),
                            Default::default(),
                        )?
                        .wait()?,
                    )
                })?;
                self.snapshot.refresh_failed = self.exclusions(runtime, id, page).is_err();
                Ok(self.saved(saved))
            }
            Action::Confirm(ticket) => {
                let (expected, plan) = self.plan.take().ok_or(ShelfError::StaleReview)?;
                if expected != ticket {
                    self.plan = Some((expected, plan));
                    return Err(ShelfError::StaleReview.into());
                }
                self.snapshot.preview = None;
                match plan {
                    Plan::Create { plan, selected, .. } => {
                        let selected = selected
                            .iter()
                            .map(|u| StoryReference::from_stored(u))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        let saved = runtime.command(|db, _, _| {
                            committed(
                                db.create_shelf_reviewed(plan, selected, Default::default())?
                                    .wait()?,
                            )
                        })?;
                        self.snapshot.navigation_error = open(navigation, saved.id).is_err();
                        Ok(self.saved(saved))
                    }
                    Plan::Duplicate { plan, name } => {
                        let saved = runtime.command(|db, _, _| {
                            committed(
                                db.duplicate_shelf_reviewed(plan, name, Default::default())?
                                    .wait()?,
                            )
                        })?;
                        let saved = changed(saved)?;
                        self.snapshot.navigation_error = open(navigation, saved.id).is_err();
                        Ok(self.saved(saved))
                    }
                    Plan::Members {
                        plan,
                        rows,
                        exclude,
                        ..
                    } => {
                        let refs = rows
                            .iter()
                            .map(|r| StoryReference::from_stored(&r.uuid))
                            .collect::<std::result::Result<Vec<_>, _>>()?;
                        let action = if exclude {
                            Curation::RemoveAndExclude(refs)
                        } else {
                            Curation::Remove(refs)
                        };
                        let saved = runtime.command(|db, _, _| {
                            committed(
                                db.curate_shelf_reviewed(
                                    plan,
                                    action,
                                    "en-GB".into(),
                                    Default::default(),
                                )?
                                .wait()?,
                            )
                        })?;
                        Ok(self.saved(saved))
                    }
                }
            }
        }
    }
}
