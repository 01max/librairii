//! On-demand Shelf review authority. Only bounded rows cross the UI boundary;
//! plans and complete explicit decisions stay on the owned controller worker.
use crate::library::Runtime;
use librairii_app::lifecycle::LibraryGeneration;
use librairii_core::{
    CollectionId, ShelfId,
    collection_codec::url::{Location, Scope},
    navigation::{Frame, Navigation},
    query::QueryRequest,
    shelves::{ShelfError, StoryReference, WarningAcknowledgements, WarningKey},
};
use librairii_storage::{
    Result, StorageError,
    database::TransactionOutcome,
    shelves::{self, Decision},
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug)]
pub struct Ticket(Arc<()>);
impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Ticket {}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Section {
    #[default]
    Additions,
    Removals,
    Warnings,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    Updates,
    Sources,
    DeleteCollection,
    DeleteShelf,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Review(ShelfId),
    Sources {
        id: ShelfId,
        sources: Vec<CollectionId>,
    },
    DeleteCollection(CollectionId),
    DeleteShelf(ShelfId),
    Page {
        ticket: Ticket,
        section: Section,
        page: usize,
        shelf: usize,
    },
    Choose {
        ticket: Ticket,
        uuid: String,
        choice: i32,
    },
    Bulk {
        ticket: Ticket,
        choice: i32,
    },
    KeepAll(Ticket),
    Acknowledge(Ticket),
    Apply(Ticket),
    Inspect {
        ticket: Ticket,
        uuid: String,
    },
}
#[derive(Debug)]
pub struct Request {
    pub generation: LibraryGeneration,
    pub action: Action,
}
#[derive(Clone, Debug)]
pub struct Row {
    pub broken: bool,
    pub uuid: String,
    pub title: String,
    pub sources: Vec<String>,
    pub origins: Vec<String>,
    pub reason: &'static str,
    pub unavailable: bool,
    pub can_remove: bool,
    pub choice: i32,
}
#[derive(Clone, Debug)]
pub struct Preview {
    pub ticket: Ticket,
    pub kind: Kind,
    pub name: String,
    pub shelf_id: Option<ShelfId>,
    pub shelf_name: String,
    pub shelf: usize,
    pub shelves: usize,
    pub section: Section,
    pub page: usize,
    pub pages: usize,
    pub rows: Vec<Row>,
    pub additions: usize,
    pub removals: usize,
    pub warnings: usize,
    pub approved: i64,
    pub unavailable: i64,
    pub source_count: usize,
    pub sources: Vec<String>,
    pub detached: Vec<String>,
    pub exclusion_count: usize,
    pub decisions: usize,
    pub approved_additions: usize,
    pub approved_exclusions: usize,
    pub approved_keeps: usize,
    pub approved_removals: usize,
    pub required: usize,
    pub incomplete: bool,
    pub up_to_date: bool,
    pub can_apply: bool,
    pub bulk_removal_available: bool,
    pub mismatch_prompt: bool,
}
#[derive(Clone, Debug)]
pub enum Outcome {
    ShelfSaved { id: ShelfId, changed: bool },
    ShelfDeleted(ShelfId),
    CollectionDeleted(CollectionId),
}
#[derive(Clone, Debug)]
pub enum Feedback {
    Ready,
    Committed(Outcome),
    Error { code: String, uncertain: bool },
}
#[derive(Clone, Debug)]
pub struct StoryPreview {
    pub uuid: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub broken: bool,
    pub unavailable: bool,
    pub thumbnail: Option<librairii_adapters::thumbnails::Thumbnail>,
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub revision: u64,
    pub feedback: Option<Feedback>,
    pub preview: Option<Preview>,
    pub story: Option<StoryPreview>,
    pub navigation_error: bool,
    pub preference_error: bool,
    pub stale: bool,
    pub entry: Option<EntryPrompt>,
    pub entry_error: bool,
    pub warning_preference_error: bool,
}
#[derive(Clone, Debug)]
pub struct EntryPrompt {
    pub ticket: Ticket,
    pub shelf: ShelfId,
}
#[derive(Debug)]
enum Plan {
    Updates(shelves::UpdatePlan),
    Sources(shelves::SourceChangePlan),
    DeleteCollection(shelves::CollectionDeletionPlan),
    DeleteShelf(shelves::CurationPlan),
}
#[derive(Debug)]
struct Review {
    ticket: Ticket,
    plan: Plan,
    decisions: BTreeMap<ShelfId, BTreeMap<String, shelves::Action>>,
    display_flags: BTreeMap<(i64, String), bool>,
    shelf: usize,
    section: Section,
    page: usize,
    approved: i64,
    unavailable: i64,
    exclusions: usize,
    mismatch_prompt: bool,
}
#[derive(Debug, Default)]
pub struct Controller {
    pub snapshot: Snapshot,
    review: Option<Review>,
    entry: Option<(Ticket, ShelfId, WarningKey)>,
}
fn committed<T>(value: TransactionOutcome<T>) -> Result<T> {
    match value {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) => Err(e),
        TransactionOutcome::CommitUncertain(_) => Err(StorageError::RuntimeFenced),
    }
}
fn open(navigation: &mut Navigation, id: ShelfId) -> Result<()> {
    if matches!(navigation.current().location.scope,Scope::Shelf{id:current,..} if id==current) {
        return Ok(());
    }
    navigation
        .push(Frame {
            location: Location {
                scope: Scope::Shelf {
                    id,
                    manual_order: true,
                },
                request: QueryRequest {
                    page_size: navigation.current().location.request.page_size,
                    ..Default::default()
                },
            },
            selection: Vec::new(),
        })
        .map_err(|_| StorageError::InvalidNavigation)
}
fn decision_action(choice: i32, removal: bool) -> Result<shelves::Action> {
    match (choice, removal) {
        (0, _) => Ok(shelves::Action::NotNow),
        (1, false) => Ok(shelves::Action::Add),
        (2, false) => Ok(shelves::Action::Exclude),
        (1, true) => Ok(shelves::Action::Keep),
        (2, true) => Ok(shelves::Action::RemoveAndExclude),
        _ => Err(ShelfError::InvalidInput.into()),
    }
}
fn choice(action: Option<&shelves::Action>) -> i32 {
    match action {
        Some(shelves::Action::Add | shelves::Action::Keep) => 1,
        Some(shelves::Action::Exclude | shelves::Action::RemoveAndExclude) => 2,
        _ => 0,
    }
}
fn choices(decisions: &BTreeMap<String, shelves::Action>) -> Result<Vec<Decision>> {
    decisions
        .iter()
        .map(|(uuid, action)| {
            Ok(Decision {
                uuid: StoryReference::from_stored(uuid)?,
                action: *action,
            })
        })
        .collect()
}
impl Review {
    fn source(&self) -> Option<&shelves::SourceChangePreview> {
        match &self.plan {
            Plan::Sources(p) => Some(p.preview()),
            Plan::DeleteCollection(p) => p.preview().shelf(self.shelf),
            _ => None,
        }
    }
    fn shelf_id(&self) -> Option<ShelfId> {
        match &self.plan {
            Plan::Updates(p) => Some(p.preview().shelf_id),
            Plan::DeleteShelf(p) => Some(p.shelf().id),
            _ => self.source().map(|p| p.shelf_id),
        }
    }
    fn candidates(&self) -> &[shelves::Candidate] {
        match (&self.plan, self.section) {
            (Plan::Updates(p), Section::Additions) => p.preview().additions_page(self.page),
            (Plan::Updates(p), Section::Removals) => p.preview().removals_page(self.page),
            (_, Section::Removals) => self
                .source()
                .map(|p| p.affected_page(self.page))
                .unwrap_or_default(),
            _ => &[],
        }
    }
    fn totals(&self) -> (usize, usize, usize) {
        match &self.plan {
            Plan::Updates(p) => {
                let p = p.preview();
                (p.addition_count(), p.removal_count(), p.warning_count())
            }
            _ => self
                .source()
                .map(|p| (0, p.affected_count(), p.warning_count()))
                .unwrap_or_default(),
        }
    }
    fn count(&self) -> usize {
        let (a, r, w) = self.totals();
        match self.section {
            Section::Additions => a,
            Section::Removals => r,
            Section::Warnings => w,
        }
    }
    fn incomplete(&self) -> bool {
        match &self.plan {
            Plan::Updates(p) => p.preview().incomplete,
            _ => self.source().is_some_and(|p| p.incomplete),
        }
    }
    fn choose(&mut self, uuid: String, choice: i32, available: bool) -> Result<()> {
        let removal = self.section == Section::Removals;
        let action = decision_action(choice, removal)?;
        if removal
            && action == shelves::Action::RemoveAndExclude
            && (self.incomplete() || !available)
        {
            return Err(ShelfError::InvalidInput.into());
        }
        let id = self.shelf_id().ok_or(ShelfError::InvalidInput)?;
        let decisions = self.decisions.entry(id).or_default();
        if action == shelves::Action::NotNow {
            decisions.remove(&uuid);
        } else {
            decisions.insert(uuid, action);
        }
        Ok(())
    }
    fn contains_identity(&self, identity: &librairii_core::browsing::StoryIdentity) -> bool {
        let matches = |c: &shelves::Candidate| {
            c.story
                .as_ref()
                .is_some_and(|s| s.id == identity.id && s.uuid == identity.uuid)
        };
        match &self.plan {
            Plan::Updates(p) => {
                let p = p.preview();
                (0..p.addition_count().div_ceil(25))
                    .any(|i| p.additions_page(i).iter().any(matches))
                    || (0..p.removal_count().div_ceil(25))
                        .any(|i| p.removals_page(i).iter().any(matches))
            }
            Plan::Sources(p) => (0..p.preview().affected_count().div_ceil(25))
                .any(|i| p.preview().affected_page(i).iter().any(matches)),
            Plan::DeleteCollection(p) => (0..p.preview().shelf_count())
                .filter_map(|i| p.preview().shelf(i))
                .any(|s| {
                    (0..s.affected_count().div_ceil(25))
                        .any(|i| s.affected_page(i).iter().any(matches))
                }),
            Plan::DeleteShelf(_) => false,
        }
    }
    fn required(&self) -> usize {
        match &self.plan {
            Plan::Sources(p) => p.preview().affected_count(),
            Plan::DeleteCollection(p) => (0..p.preview().shelf_count())
                .filter_map(|i| p.preview().shelf(i))
                .map(|s| s.affected_count())
                .sum(),
            _ => 0,
        }
    }
    fn preview(&self) -> Preview {
        let (additions, removals, warnings) = self.totals();
        let decisions = self.decisions.values().map(BTreeMap::len).sum();
        let required = self.required();
        let (kind, name) = match &self.plan {
            Plan::Updates(p) => (Kind::Updates, p.preview().shelf_name.clone()),
            Plan::Sources(p) => (Kind::Sources, p.preview().shelf_name.clone()),
            Plan::DeleteCollection(p) => (
                Kind::DeleteCollection,
                String::from_utf8_lossy(&p.preview().collection.name).into_owned(),
            ),
            Plan::DeleteShelf(p) => (Kind::DeleteShelf, p.shelf().name.clone()),
        };
        let warning_rows = match &self.plan {
            Plan::Updates(p) => p.preview().warnings_page(self.page),
            _ => self
                .source()
                .map(|p| p.warnings_page(self.page))
                .unwrap_or_default(),
        };
        let rows = if self.section == Section::Warnings {
            warning_rows
                .iter()
                .map(|w| Row {
                    broken: false,
                    uuid: w.uuid.clone().unwrap_or_default(),
                    title: w
                        .source
                        .as_ref()
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                    sources: Vec::new(),
                    origins: Vec::new(),
                    reason: w.code,
                    unavailable: w.uuid.is_some(),
                    can_remove: false,
                    choice: 0,
                })
                .collect()
        } else {
            self.candidates()
                .iter()
                .map(|c| Row {
                    broken: c.story.as_ref().is_some_and(|s| {
                        self.display_flags
                            .get(&(s.id, s.uuid.clone()))
                            .copied()
                            .unwrap_or(s.broken)
                    }),
                    uuid: c.uuid.clone(),
                    title: c.title.clone(),
                    sources: c.sources.iter().map(|s| s.name.clone()).collect(),
                    origins: c.origins.iter().map(|s| s.name.clone()).collect(),
                    reason: c.reason,
                    unavailable: c.story.is_none(),
                    can_remove: !self.incomplete() && c.story.is_some(),
                    choice: choice(
                        self.shelf_id()
                            .and_then(|id| self.decisions.get(&id))
                            .and_then(|d| d.get(&c.uuid)),
                    ),
                })
                .collect()
        };
        let source_count = match &self.plan {
            Plan::Sources(p) => p.preview().sources.len(),
            Plan::DeleteCollection(_) => self.source().map_or(0, |p| p.sources.len()),
            Plan::DeleteShelf(p) => p.shelf().source_count as usize,
            Plan::Updates(p) => p.preview().source_count,
        };
        let (approved, unavailable, exclusions) = self
            .source()
            .map(|p| (p.approved_count, p.unavailable_count, p.exclusion_count))
            .unwrap_or((self.approved, self.unavailable, self.exclusions));
        Preview {
            ticket: self.ticket.clone(),
            kind,
            name,
            shelf_id: self.shelf_id(),
            shelf_name: self
                .source()
                .map(|p| p.shelf_name.clone())
                .unwrap_or_default(),
            shelf: self.shelf,
            shelves: match &self.plan {
                Plan::DeleteCollection(p) => p.preview().shelf_count(),
                _ => 1,
            },
            section: self.section,
            page: self.page,
            pages: self.count().div_ceil(25).max(1),
            rows,
            additions,
            removals,
            warnings,
            approved,
            unavailable,
            source_count,
            sources: self
                .source()
                .map(|p| p.sources.iter().map(|s| s.name.clone()).collect())
                .unwrap_or_default(),
            detached: self
                .source()
                .map(|p| p.detached.iter().map(|s| s.name.clone()).collect())
                .unwrap_or_default(),
            exclusion_count: exclusions,
            decisions,
            approved_additions: self
                .decisions
                .values()
                .flat_map(BTreeMap::values)
                .filter(|a| **a == shelves::Action::Add)
                .count(),
            approved_exclusions: self
                .decisions
                .values()
                .flat_map(BTreeMap::values)
                .filter(|a| **a == shelves::Action::Exclude)
                .count(),
            approved_keeps: self
                .decisions
                .values()
                .flat_map(BTreeMap::values)
                .filter(|a| **a == shelves::Action::Keep)
                .count(),
            approved_removals: self
                .decisions
                .values()
                .flat_map(BTreeMap::values)
                .filter(|a| **a == shelves::Action::RemoveAndExclude)
                .count(),
            required,
            incomplete: self.incomplete(),
            up_to_date: matches!(&self.plan,Plan::Updates(p) if p.preview().up_to_date),
            can_apply: required == 0 || decisions == required,
            bulk_removal_available: removals > 0
                && !self.incomplete()
                && self.source().is_none_or(|p| {
                    (0..p.affected_count().div_ceil(25))
                        .all(|page| p.affected_page(page).iter().all(|c| c.story.is_some()))
                }),
            mismatch_prompt: self.mismatch_prompt,
        }
    }
}
impl Controller {
    /// Evaluate only on explicit Shelf entry, on the owned worker. This prompt
    /// carries preference authority, never membership approval.
    pub fn enter(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        store: &dyn crate::preferences::Preferences,
        navigation: &Navigation,
    ) -> Result<()> {
        self.entry = None;
        self.snapshot.entry = None;
        self.snapshot.entry_error = false;
        let Scope::Shelf { id, .. } = navigation.current().location.scope else {
            return Ok(());
        };
        let result = (|| -> Result<()> {
            let plan =
                runtime.command(|db, _, _| db.shelf_update_preview(id, "en-GB".into())?.wait())?;
            let mut acknowledged = store
                .load_shelf_warnings(navigation.library())?
                .unwrap_or_else(|| WarningAcknowledgements::new(navigation.library()));
            if let Some(key) = plan.mismatch_key() {
                if acknowledged.needs_prompt_for_members(id, key, &plan.mismatch_members()) {
                    let ticket = Ticket(Arc::new(()));
                    self.snapshot.entry = Some(EntryPrompt {
                        ticket: ticket.clone(),
                        shelf: id,
                    });
                    self.entry = Some((ticket.clone(), id, key));
                    self.review = Some(Review {
                        ticket,
                        approved: plan.preview().approved_count,
                        unavailable: plan.preview().unavailable_count,
                        exclusions: plan.preview().exclusion_count,
                        plan: Plan::Updates(plan),
                        decisions: Default::default(),
                        display_flags: Default::default(),
                        shelf: 0,
                        section: Section::Removals,
                        page: 0,
                        mismatch_prompt: true,
                    });
                    self.snapshot.preview = self.review.as_ref().map(Review::preview);
                    self.snapshot.story = None;
                    self.snapshot.stale = false;
                } else {
                    let previous = acknowledged.clone();
                    acknowledged.acknowledge_members(id, key, &plan.mismatch_members())?;
                    if acknowledged != previous {
                        store.save_shelf_warnings(&acknowledged)?;
                    }
                }
            } else if !plan.preview().incomplete && acknowledged.forget(id) {
                store.save_shelf_warnings(&acknowledged)?;
            }
            Ok(())
        })();
        self.snapshot.entry_error = result.is_err();
        match result {
            Err(StorageError::RuntimeFenced) => Err(StorageError::RuntimeFenced),
            _ => Ok(()),
        }
    }
    pub fn observe_flags(&mut self, changes: &[(librairii_core::browsing::StoryIdentity, bool)]) {
        if let Some(r) = &mut self.review {
            for (identity, value) in changes {
                if r.contains_identity(identity) {
                    r.display_flags
                        .insert((identity.id, identity.uuid.clone()), *value);
                }
                if let Some(story) = &mut self.snapshot.story
                    && story.uuid == identity.uuid
                {
                    story.broken = *value;
                }
            }
            self.snapshot.preview = Some(r.preview());
        }
    }
    pub fn invalidate(&mut self) {
        if self.review.is_some() {
            self.snapshot.stale = true;
        }
    }
    pub fn handle(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        store: &dyn crate::preferences::Preferences,
        request: Request,
        revision: u64,
        navigation: &mut Navigation,
    ) {
        let current = runtime.accepts(&request.generation);
        if current
            && matches!(
                &request.action,
                Action::Review(_)
                    | Action::Sources { .. }
                    | Action::DeleteShelf(_)
                    | Action::DeleteCollection(_)
            )
        {
            self.snapshot.stale = false;
            self.snapshot.story = None;
            self.snapshot.entry_error = false;
        }
        self.snapshot.revision = revision;
        self.snapshot.navigation_error = false;
        self.snapshot.preference_error = false;
        let acknowledging = matches!(&request.action, Action::Acknowledge(_));
        let result = if current {
            self.apply(runtime, store, request.action, navigation)
        } else {
            Err(StorageError::SnapshotChanged)
        };
        if acknowledging {
            self.snapshot.warning_preference_error = result.is_err();
        }
        self.snapshot.feedback = Some(match result {
            Ok(f) => f,
            Err(e) => {
                if matches!(
                    e,
                    StorageError::SnapshotChanged | StorageError::Shelf(ShelfError::StaleReview)
                ) {
                    self.snapshot.stale = true;
                }
                Feedback::Error {
                    code: e.to_string(),
                    uncertain: matches!(e, StorageError::RuntimeFenced),
                }
            }
        });
        self.snapshot.preview = self.review.as_ref().map(Review::preview);
    }
    fn apply(
        &mut self,
        runtime: &Runtime<'_, '_, '_>,
        store: &dyn crate::preferences::Preferences,
        action: Action,
        navigation: &mut Navigation,
    ) -> Result<Feedback> {
        match action {
            Action::Review(id) | Action::DeleteShelf(id) => {
                let deleting = matches!(action, Action::DeleteShelf(_));
                let plan = runtime.command(|db, _, _| {
                    if deleting {
                        Ok(Plan::DeleteShelf(db.shelf_curation_snapshot(id)?.wait()?))
                    } else {
                        Ok(Plan::Updates(
                            db.shelf_update_preview(id, "en-GB".into())?.wait()?,
                        ))
                    }
                })?;
                let (approved, unavailable, exclusions) = match &plan {
                    Plan::Updates(p) => {
                        let p = p.preview();
                        (p.approved_count, p.unavailable_count, p.exclusion_count)
                    }
                    Plan::DeleteShelf(p) => (
                        p.shelf().count,
                        p.shelf().unavailable_count,
                        p.exclusion_count(),
                    ),
                    _ => (0, 0, 0),
                };
                let mismatch_prompt = if let Plan::Updates(p) = &plan {
                    if let Some(key) = p.mismatch_key() {
                        store
                            .load_shelf_warnings(navigation.library())?
                            .unwrap_or_else(|| WarningAcknowledgements::new(navigation.library()))
                            .needs_prompt_for_members(id, key, &p.mismatch_members())
                    } else {
                        false
                    }
                } else {
                    false
                };
                self.review = Some(Review {
                    ticket: Ticket(Arc::new(())),
                    plan,
                    decisions: Default::default(),
                    display_flags: Default::default(),
                    shelf: 0,
                    section: Section::Additions,
                    page: 0,
                    approved,
                    unavailable,
                    exclusions,
                    mismatch_prompt,
                });
                self.snapshot.navigation_error = open(navigation, id).is_err();
            }
            Action::Sources { id, sources } => {
                let plan = runtime.command(|db, _, _| {
                    db.shelf_source_change_preview(id, sources, "en-GB".into())?
                        .wait()
                })?;
                self.review = Some(Review {
                    ticket: Ticket(Arc::new(())),
                    plan: Plan::Sources(plan),
                    decisions: Default::default(),
                    display_flags: Default::default(),
                    shelf: 0,
                    section: Section::Removals,
                    page: 0,
                    approved: 0,
                    unavailable: 0,
                    exclusions: 0,
                    mismatch_prompt: false,
                });
                self.snapshot.navigation_error = open(navigation, id).is_err();
            }
            Action::DeleteCollection(id) => {
                let plan = runtime.command(|db, _, _| {
                    db.linked_collection_deletion_preview(id, "en-GB".into())?
                        .wait()
                })?;
                let first = plan.preview().shelf(0).map(|s| s.shelf_id);
                self.review = Some(Review {
                    ticket: Ticket(Arc::new(())),
                    plan: Plan::DeleteCollection(plan),
                    decisions: Default::default(),
                    display_flags: Default::default(),
                    shelf: 0,
                    section: Section::Removals,
                    page: 0,
                    approved: 0,
                    unavailable: 0,
                    exclusions: 0,
                    mismatch_prompt: false,
                });
                if let Some(id) = first {
                    self.snapshot.navigation_error = open(navigation, id).is_err();
                }
            }
            Action::Page {
                ticket,
                section,
                page,
                shelf,
            } => {
                self.snapshot.story = None;
                let r = self
                    .review
                    .as_mut()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                r.shelf = match &r.plan {
                    Plan::DeleteCollection(p) => {
                        shelf.min(p.preview().shelf_count().saturating_sub(1))
                    }
                    _ => 0,
                };
                r.section = section;
                r.page = page.min(r.count().div_ceil(25).max(1) - 1);
                if let Some(id) = r.shelf_id() {
                    self.snapshot.navigation_error = open(navigation, id).is_err();
                }
            }
            Action::Choose {
                ticket,
                uuid,
                choice,
            } => {
                let r = self
                    .review
                    .as_mut()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                let c = r
                    .candidates()
                    .iter()
                    .find(|c| c.uuid == uuid)
                    .ok_or(ShelfError::StaleReview)?;
                let (uuid, available) = (c.uuid.clone(), c.story.is_some());
                r.choose(uuid, choice, available)?;
            }
            Action::Bulk { ticket, choice } => {
                let r = self
                    .review
                    .as_mut()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                if r.section == Section::Warnings {
                    return Err(ShelfError::InvalidInput.into());
                }
                decision_action(choice, r.section == Section::Removals)?;
                let old_page = r.page;
                let mut targets = Vec::new();
                for page in 0..r.count().div_ceil(25) {
                    r.page = page;
                    targets.extend(
                        r.candidates()
                            .iter()
                            .map(|c| (c.uuid.clone(), c.story.is_some())),
                    );
                }
                r.page = old_page;
                if choice == 2
                    && r.section == Section::Removals
                    && (r.incomplete() || targets.iter().any(|t| !t.1))
                {
                    return Err(ShelfError::InvalidInput.into());
                }
                for (uuid, available) in targets {
                    r.choose(uuid, choice, available)?;
                }
            }
            Action::KeepAll(ticket) => {
                let r = self
                    .review
                    .as_mut()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                let groups = match &r.plan {
                    Plan::Sources(p) => vec![(
                        p.preview().shelf_id,
                        p.preview().bulk_decisions(shelves::Action::Keep)?,
                    )],
                    Plan::DeleteCollection(p) => (0..p.preview().shelf_count())
                        .filter_map(|i| p.preview().shelf(i))
                        .map(|s| Ok((s.shelf_id, s.bulk_decisions(shelves::Action::Keep)?)))
                        .collect::<Result<Vec<_>>>()?,
                    _ => return Err(ShelfError::InvalidInput.into()),
                };
                r.decisions = groups
                    .into_iter()
                    .map(|(id, ds)| {
                        (
                            id,
                            ds.into_iter()
                                .map(|d| (d.uuid.as_stored().to_owned(), d.action))
                                .collect(),
                        )
                    })
                    .collect();
            }
            Action::Acknowledge(ticket) => {
                let r = self
                    .review
                    .as_mut()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                let Plan::Updates(p) = &r.plan else {
                    return Err(ShelfError::InvalidInput.into());
                };
                let key = p.mismatch_key().ok_or(ShelfError::InvalidInput)?;
                let mut acknowledgements = store
                    .load_shelf_warnings(navigation.library())?
                    .unwrap_or_else(|| WarningAcknowledgements::new(navigation.library()));
                acknowledgements.acknowledge_members(
                    p.preview().shelf_id,
                    key,
                    &p.mismatch_members(),
                )?;
                store.save_shelf_warnings(&acknowledgements)?;
                r.mismatch_prompt = false;
                if self.entry.as_ref().is_some_and(|(_, id, entry_key)| {
                    *id == p.preview().shelf_id && *entry_key == key
                }) {
                    self.entry = None;
                    self.snapshot.entry = None;
                }
            }
            Action::Inspect { ticket, uuid } => {
                let r = self
                    .review
                    .as_ref()
                    .filter(|r| r.ticket == ticket)
                    .ok_or(ShelfError::StaleReview)?;
                let c = r
                    .candidates()
                    .iter()
                    .find(|c| c.uuid == uuid)
                    .ok_or(ShelfError::StaleReview)?;
                let mut preview = StoryPreview {
                    uuid: c.uuid.clone(),
                    title: c.title.clone(),
                    author: String::new(),
                    description: String::new(),
                    broken: false,
                    unavailable: c.story.is_none(),
                    thumbnail: None,
                };
                if let Some(expected) = &c.story {
                    let (summary, thumbnail) = runtime.command(|db, _, cache| {
                        let current = db
                            .story_detail(expected.id, "en-GB".into())?
                            .wait()?
                            .ok_or(StorageError::SnapshotChanged)?
                            .summary;
                        if current.uuid != expected.uuid {
                            return Err(StorageError::SnapshotChanged);
                        }
                        let official = current
                            .artwork_id
                            .parse()
                            .ok()
                            .and_then(|id| db.read_registered_artwork(id).ok())
                            .and_then(|a| a.cached);
                        let embedded = db.read_embedded_artwork(current.id).ok().flatten();
                        let thumbnail = match cache.resolve(
                            &cache.generation()?,
                            official.as_ref(),
                            embedded.as_ref(),
                        )? {
                            librairii_adapters::thumbnails::ArtworkModel::Ready {
                                thumbnail,
                                ..
                            } => Some(thumbnail),
                            _ => None,
                        };
                        Ok((current, thumbnail))
                    })?;
                    let mut expected = expected.clone();
                    expected.broken = summary.broken;
                    if expected != summary {
                        self.snapshot.stale = true;
                    }
                    preview.title = summary.title;
                    preview.author = summary.author;
                    preview.description = librairii_ui::description::readable(&summary.description);
                    preview.broken = summary.broken;
                    preview.thumbnail = thumbnail;
                }
                self.snapshot.story = Some(preview);
            }
            Action::Apply(ticket) => {
                let r = self.review.take().ok_or(ShelfError::StaleReview)?;
                if r.ticket != ticket {
                    self.review = Some(r);
                    return Err(ShelfError::StaleReview.into());
                }
                let outcome = match r.plan {
                    Plan::Updates(plan) => {
                        let before = plan.preview().revision;
                        let ds = r
                            .decisions
                            .get(&plan.preview().shelf_id)
                            .map(choices)
                            .transpose()?
                            .unwrap_or_default();
                        let saved = runtime.command(|db, _, _| {
                            committed(
                                db.apply_shelf_updates(plan, ds, Default::default())?
                                    .wait()?,
                            )
                        })?;
                        Outcome::ShelfSaved {
                            id: saved.id,
                            changed: saved.revision != before,
                        }
                    }
                    Plan::Sources(plan) => {
                        let before = plan.preview().revision;
                        let ds = r
                            .decisions
                            .get(&plan.preview().shelf_id)
                            .map(choices)
                            .transpose()?
                            .unwrap_or_default();
                        let saved = runtime.command(|db, _, _| {
                            committed(
                                db.change_shelf_sources(plan, false, ds, Default::default())?
                                    .wait()?,
                            )
                        })?;
                        Outcome::ShelfSaved {
                            id: saved.id,
                            changed: saved.revision != before,
                        }
                    }
                    Plan::DeleteCollection(plan) => {
                        let id = CollectionId::new(plan.preview().collection.id)
                            .map_err(|_| ShelfError::InvalidInput)?;
                        let ds = r
                            .decisions
                            .iter()
                            .map(|(id, ds)| {
                                Ok(shelves::ShelfDecisions {
                                    shelf_id: *id,
                                    decisions: choices(ds)?,
                                })
                            })
                            .collect::<Result<Vec<_>>>()?;
                        let stamp = navigation.stamp();
                        let deleted = runtime.command(|db, _, _| {
                            committed(
                                db.delete_linked_collection(plan, false, ds, Default::default())?
                                    .wait()?,
                            )
                        })?;
                        self.snapshot.navigation_error =
                            deleted.apply_navigation(navigation, &stamp).is_err();
                        Outcome::CollectionDeleted(id)
                    }
                    Plan::DeleteShelf(plan) => {
                        let id = plan.shelf().id;
                        runtime.command(|db, _, _| {
                            committed(db.delete_shelf_reviewed(plan, Default::default())?.wait()?)
                        })?;
                        if matches!(navigation.current().location.scope,Scope::Shelf{id:current,..} if id==current)
                        {
                            self.snapshot.navigation_error = navigation
                                .replace(Frame {
                                    location: Location {
                                        scope: Scope::Library,
                                        request: QueryRequest {
                                            page_size: navigation
                                                .current()
                                                .location
                                                .request
                                                .page_size,
                                            ..Default::default()
                                        },
                                    },
                                    selection: Vec::new(),
                                })
                                .is_err();
                        }
                        self.snapshot.preference_error = (|| -> Result<()> {
                            if let Some(mut acknowledgements) =
                                store.load_shelf_warnings(navigation.library())?
                            {
                                acknowledgements.forget(id);
                                store.save_shelf_warnings(&acknowledgements)?;
                            }
                            Ok(())
                        })()
                        .is_err();
                        Outcome::ShelfDeleted(id)
                    }
                };
                return Ok(Feedback::Committed(outcome));
            }
        }
        Ok(Feedback::Ready)
    }
}
