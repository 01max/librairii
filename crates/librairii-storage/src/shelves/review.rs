use super::*;
use crate::database::SnapshotIdentity;
use librairii_core::{browsing::StorySummary, query::QueryRequest};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub uuid: String,
    pub title: String,
    pub story: Option<StorySummary>,
    pub sources: Vec<Source>,
    pub origins: Vec<Source>,
    pub manual: bool,
    pub reason: &'static str,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Warning {
    pub source: Option<Source>,
    pub uuid: Option<String>,
    pub code: &'static str,
}
/// Native commands accept typed UUID identities after canonical text admission.
/// ```compile_fail
/// use librairii_storage::shelves::CreationRequest;
/// let _ = CreationRequest { manual_uuids: vec!["arbitrary text".to_owned()], ..Default::default() };
/// ```
#[derive(Clone, Debug, Default)]
pub struct CreationRequest {
    pub name: String,
    pub manual_uuids: Vec<StoryUuid>,
    pub collection_ids: Vec<CollectionId>,
}
#[derive(Clone, Debug)]
pub struct CreationPreview {
    pub name: String,
    pub(super) candidates: Vec<Candidate>,
    pub(super) warnings: Vec<Warning>,
    pub incomplete: bool,
}
#[derive(Clone, Debug)]
pub struct CreationPlan {
    request: CreationRequest,
    manual_exact: Vec<String>,
    preview: CreationPreview,
    fingerprint: [u8; 32],
    identity: SnapshotIdentity,
    ticket: Arc<super::tickets::Ticket>,
    locale: String,
}
impl CreationPlan {
    pub fn preview(&self) -> &CreationPreview {
        &self.preview
    }
}
// Fingerprints are process-local, never a durable/wire format. Hash only these
// explicit semantic inputs; runtime identity separately prevents cross-open use.
pub(super) fn digest(value: &impl std::fmt::Debug) -> [u8; 32] {
    Sha256::digest(format!("{value:?}").as_bytes()).into()
}
type Heading<'a> = (&'a str, &'a str, bool, &'a [Source], &'a [Source]);
fn headings(candidates: &[Candidate]) -> Vec<Heading<'_>> {
    candidates
        .iter()
        .map(|c| {
            (
                c.uuid.as_str(),
                c.title.as_str(),
                c.manual,
                c.sources.as_slice(),
                c.origins.as_slice(),
            )
        })
        .collect()
}
fn warnings(e: &SourceEvaluation) -> Vec<Warning> {
    e.sources
        .iter()
        .filter_map(|s| {
            s.warning.map(|code| Warning {
                source: Some(s.source.clone()),
                uuid: None,
                code,
            })
        })
        .collect()
}
fn stories(tx: &Transaction<'_>, ids: &[String], locale: &str) -> Result<Vec<StorySummary>> {
    let mut result = Vec::new();
    crate::query::visit_uuids(tx, QueryRequest::default(), locale, ids, |s| {
        result.push(s);
        Ok(())
    })?;
    Ok(result)
}
fn creation(
    tx: &Transaction<'_>,
    request: &CreationRequest,
    manual: &[String],
    locale: &str,
) -> Result<(CreationPreview, [u8; 32])> {
    let (name, normalized) = normalize_name(&request.name)?;
    let evaluated =
        sources::evaluate(tx, sources::potential(tx, &request.collection_ids)?, locale)?;
    let ids: Vec<_> = manual
        .iter()
        .cloned()
        .chain(evaluated.matching.keys().cloned())
        .collect();
    let ordered = stories(tx, &ids, locale)?;
    let mut by_uuid = ordered
        .iter()
        .map(|s| (s.uuid.clone(), s.clone()))
        .collect::<BTreeMap<_, _>>();
    if manual.iter().any(|u| !by_uuid.contains_key(u)) {
        return Err(ShelfError::NotFound.into());
    }
    let mut candidates = Vec::new();
    for uuid in manual {
        if let Some(story) = by_uuid.remove(uuid) {
            candidates.push(Candidate {
                uuid: uuid.clone(),
                title: story.title.clone(),
                story: Some(story),
                sources: evaluated.matching.get(uuid).cloned().unwrap_or_default(),
                origins: Vec::new(),
                manual: true,
                reason: "initial_membership",
            });
        }
    }
    for story in ordered {
        if by_uuid.remove(&story.uuid).is_some() {
            candidates.push(Candidate {
                sources: evaluated
                    .matching
                    .get(&story.uuid)
                    .cloned()
                    .unwrap_or_default(),
                uuid: story.uuid.clone(),
                title: story.title.clone(),
                story: Some(story),
                origins: Vec::new(),
                manual: false,
                reason: "initial_membership",
            });
        }
    }
    let fingerprint = digest(&(
        &name,
        normalized,
        &manual,
        &evaluated,
        headings(&candidates),
    ));
    Ok((
        CreationPreview {
            name,
            candidates,
            warnings: warnings(&evaluated),
            incomplete: evaluated.incomplete,
        },
        fingerprint,
    ))
}
impl Database<'_> {
    pub fn shelf_creation_preview(
        &self,
        request: CreationRequest,
        locale: String,
    ) -> Result<Pending<'_, CreationPlan>> {
        let manual = uuid_strings(&request.manual_uuids);
        self.creation_preview(request, manual, None, locale)
    }
    /// Native selections retain the exact stored UUID spelling. The numeric
    /// row and UUID are checked together in the same preview observation.
    pub fn shelf_creation_preview_for_stories(
        &self,
        name: String,
        collection_ids: Vec<CollectionId>,
        targets: Vec<librairii_core::browsing::StoryIdentity>,
        locale: String,
    ) -> Result<Pending<'_, CreationPlan>> {
        let manual = targets.iter().map(|s| s.uuid.clone()).collect();
        self.creation_preview(
            CreationRequest {
                name,
                collection_ids,
                ..Default::default()
            },
            manual,
            Some(targets),
            locale,
        )
    }
    fn creation_preview(
        &self,
        request: CreationRequest,
        manual_exact: Vec<String>,
        targets: Option<Vec<librairii_core::browsing::StoryIdentity>>,
        locale: String,
    ) -> Result<Pending<'_, CreationPlan>> {
        let identity = self.snapshot_identity();
        let registry = self.shelf_tickets.clone();
        self.read(move |tx| {
            if let Some(targets) = targets.filter(|targets| !targets.is_empty()) {
                crate::query::require_identities(tx, &targets)?;
            }
            for uuid in &manual_exact {
                StoryReference::from_stored(uuid)?;
            }
            let (preview, fingerprint) = creation(tx, &request, &manual_exact, &locale)?;
            Ok(CreationPlan {
                request,
                manual_exact,
                preview,
                fingerprint,
                identity,
                ticket: super::tickets::issue(&registry, super::tickets::Kind::Create, 0, vec![])?,
                locale,
            })
        })
    }
    pub fn create_shelf_reviewed(
        &self,
        plan: CreationPlan,
        selected: Vec<StoryReference>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        self.transaction(cancellation, move |tx| {
            plan.ticket.claim(super::tickets::Kind::Create, 0)?;
            let (fresh, fingerprint) =
                creation(tx, &plan.request, &plan.manual_exact, &plan.locale)?;
            if fingerprint != plan.fingerprint {
                return Err(ShelfError::StaleReview.into());
            }
            let selected = reference_strings(&selected)
                .into_iter()
                .collect::<BTreeSet<_>>();
            let known = fresh
                .candidates
                .iter()
                .map(|c| c.uuid.clone())
                .collect::<BTreeSet<_>>();
            if !selected.is_subset(&known) {
                return Err(ShelfError::InvalidInput.into());
            }
            let shelf = insert(tx, &fresh.name)?;
            sources::attach(tx, shelf.id, &plan.request.collection_ids)?;
            let attached = sources(tx, shelf.id, true)?;
            for c in fresh.candidates {
                if selected.contains(&c.uuid) {
                    curation::add(tx, shelf.id, &c.uuid, &c.title, c.manual)?;
                    let accepted = attached
                        .iter()
                        .filter(|s| c.sources.iter().any(|p| p.collection_id == s.collection_id))
                        .cloned()
                        .collect::<Vec<_>>();
                    sources::origins(tx, shelf.id, &c.uuid, &accepted)?;
                }
            }
            read(tx, shelf.id)
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Add,
    Exclude,
    Keep,
    RemoveAndExclude,
    NotNow,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decision {
    pub uuid: StoryReference,
    pub action: Action,
}
#[derive(Clone, Debug)]
pub struct UpdatePreview {
    pub shelf_id: ShelfId,
    pub revision: i64,
    pub(super) additions: Vec<Candidate>,
    pub(super) removals: Vec<Candidate>,
    pub(super) warnings: Vec<Warning>,
    pub incomplete: bool,
    pub up_to_date: bool,
}
#[derive(Clone, Debug)]
pub struct UpdatePlan {
    id: ShelfId,
    preview: UpdatePreview,
    fingerprint: [u8; 32],
    identity: SnapshotIdentity,
    ticket: Arc<super::tickets::Ticket>,
    locale: String,
}
impl UpdatePlan {
    pub fn mismatch_key(&self) -> Option<librairii_core::shelves::WarningKey> {
        (!self.preview.removals.is_empty())
            .then(|| librairii_core::shelves::WarningKey::from_snapshot(self.fingerprint))
    }
    pub fn preview(&self) -> &UpdatePreview {
        &self.preview
    }
}
pub(super) struct Evaluation {
    pub preview: UpdatePreview,
    pub fingerprint: [u8; 32],
}
pub(super) fn evaluation(
    tx: &Transaction<'_>,
    id: ShelfId,
    locale: &str,
    override_sources: Option<Vec<Source>>,
) -> Result<Evaluation> {
    let shelf = read(tx, id)?;
    let members = members(tx, id)?;
    let exclusions = curation::exclusions(tx, id)?;
    let evaluated = sources::evaluate(
        tx,
        match override_sources {
            Some(s) => s,
            None => sources(tx, id, true)?,
        },
        locale,
    )?;
    let ids: Vec<_> = members
        .iter()
        .map(|m| m.uuid.clone())
        .chain(evaluated.matching.keys().cloned())
        .collect();
    let stories = stories(tx, &ids, locale)?;
    let available = stories
        .iter()
        .map(|s| (s.uuid.clone(), s))
        .collect::<BTreeMap<_, _>>();
    let approved = members
        .iter()
        .map(|m| m.uuid.as_str())
        .collect::<BTreeSet<_>>();
    let excluded = exclusions.iter().collect::<BTreeSet<_>>();
    let mut preview = UpdatePreview {
        shelf_id: id,
        revision: shelf.revision,
        additions: Vec::new(),
        removals: Vec::new(),
        warnings: warnings(&evaluated),
        incomplete: evaluated.incomplete,
        up_to_date: false,
    };
    for story in &stories {
        if approved.contains(story.uuid.as_str()) || excluded.contains(&story.uuid) {
            continue;
        }
        if let Some(matching) = evaluated.matching.get(&story.uuid) {
            preview.additions.push(Candidate {
                uuid: story.uuid.clone(),
                title: story.title.clone(),
                story: Some(story.clone()),
                sources: matching.clone(),
                origins: Vec::new(),
                manual: false,
                reason: "source_match",
            });
        }
    }
    for member in &members {
        let Some(story) = available.get(&member.uuid) else {
            preview.warnings.push(Warning {
                source: None,
                uuid: Some(member.uuid.clone()),
                code: "story_unavailable",
            });
            continue;
        };
        if !evaluated.incomplete
            && !evaluated.sources.is_empty()
            && !member.manually_retained
            && !evaluated.matching.contains_key(&member.uuid)
        {
            preview.removals.push(Candidate {
                uuid: member.uuid.clone(),
                title: story.title.clone(),
                story: Some((*story).clone()),
                sources: Vec::new(),
                origins: member.origins.clone(),
                manual: false,
                reason: "no_source_matches",
            });
        }
    }
    preview.up_to_date =
        !preview.incomplete && preview.additions.is_empty() && preview.removals.is_empty();
    let mut state = shelf.clone();
    state.broken_count = 0;
    let available_headings = available
        .iter()
        .map(|(u, s)| (u.as_str(), s.title.as_str()))
        .collect::<Vec<_>>();
    let fingerprint = digest(&(
        &state,
        &members,
        &exclusions,
        &evaluated,
        available_headings,
        headings(&preview.additions),
        headings(&preview.removals),
    ));
    Ok(Evaluation {
        preview,
        fingerprint,
    })
}
fn actions(preview: &UpdatePreview, decisions: &[Decision]) -> Result<BTreeMap<String, Action>> {
    let additions = preview
        .additions
        .iter()
        .map(|c| c.uuid.as_str())
        .collect::<BTreeSet<_>>();
    let removals = preview
        .removals
        .iter()
        .map(|c| c.uuid.as_str())
        .collect::<BTreeSet<_>>();
    let mut result = BTreeMap::new();
    for d in decisions {
        let allowed = if additions.contains(d.uuid.as_stored()) {
            matches!(d.action, Action::Add | Action::Exclude | Action::NotNow)
        } else if removals.contains(d.uuid.as_stored()) {
            matches!(
                d.action,
                Action::Keep | Action::RemoveAndExclude | Action::NotNow
            )
        } else {
            false
        };
        if !allowed
            || result
                .insert(d.uuid.as_stored().to_owned(), d.action)
                .is_some()
        {
            return Err(ShelfError::InvalidInput.into());
        }
    }
    Ok(result)
}
pub(super) fn keep(tx: &Transaction<'_>, id: ShelfId, uuid: &str) -> Result<bool> {
    Ok(tx.execute(
        "UPDATE shelf_stories SET protected=1 WHERE shelf_id=? AND story_uuid=? AND protected=0",
        params![id.get(), uuid],
    )? != 0)
}
impl Database<'_> {
    pub fn shelf_update_preview(
        &self,
        id: ShelfId,
        locale: String,
    ) -> Result<Pending<'_, UpdatePlan>> {
        let identity = self.snapshot_identity();
        let registry = self.shelf_tickets.clone();
        self.writer_read(move |tx| {
            let e = evaluation(tx, id, &locale, None)?;
            Ok(UpdatePlan {
                id,
                preview: e.preview,
                fingerprint: e.fingerprint,
                identity,
                ticket: super::tickets::issue(
                    &registry,
                    super::tickets::Kind::Update,
                    id.get(),
                    vec![id],
                )?,
                locale,
            })
        })
    }
    pub fn apply_shelf_updates(
        &self,
        plan: UpdatePlan,
        decisions: Vec<Decision>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        self.transaction(cancellation, move |tx| {
            plan.ticket
                .claim(super::tickets::Kind::Update, plan.id.get())?;
            let fresh = evaluation(tx, plan.id, &plan.locale, None)?;
            if fresh.fingerprint != plan.fingerprint {
                return Err(ShelfError::StaleReview.into());
            }
            let actions = actions(&fresh.preview, &decisions)?;
            let mut changed = false;
            for c in fresh
                .preview
                .additions
                .iter()
                .chain(&fresh.preview.removals)
            {
                match actions.get(&c.uuid).copied().unwrap_or(Action::NotNow) {
                    Action::Add => {
                        changed |= curation::add(tx, plan.id, &c.uuid, &c.title, false)?;
                        changed |= sources::origins(tx, plan.id, &c.uuid, &c.sources)?;
                    }
                    Action::Exclude => {
                        changed |= curation::exclude(tx, plan.id, &c.uuid)?;
                    }
                    Action::Keep => {
                        changed |= keep(tx, plan.id, &c.uuid)?;
                    }
                    Action::RemoveAndExclude => {
                        changed |= curation::remove(tx, plan.id, &c.uuid)?;
                        changed |= curation::exclude(tx, plan.id, &c.uuid)?;
                    }
                    Action::NotNow => {}
                }
            }
            bump(tx, plan.id, changed)
        })
    }
}

impl CreationPreview {
    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }
    pub fn candidates_page(&self, page: usize) -> &[Candidate] {
        super::rows(&self.candidates, page)
    }
    /// Confirm the exact previewed rows, including preserved legacy spelling.
    pub fn all_uuids(&self) -> Result<Vec<StoryReference>> {
        self.candidates.iter().map(Candidate::reference).collect()
    }
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
    pub fn warnings_page(&self, page: usize) -> &[Warning] {
        super::rows(&self.warnings, page)
    }
}
impl UpdatePreview {
    pub fn addition_count(&self) -> usize {
        self.additions.len()
    }
    pub fn removal_count(&self) -> usize {
        self.removals.len()
    }
    pub fn additions_page(&self, page: usize) -> &[Candidate] {
        super::rows(&self.additions, page)
    }
    pub fn removals_page(&self, page: usize) -> &[Candidate] {
        super::rows(&self.removals, page)
    }
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
    pub fn warnings_page(&self, page: usize) -> &[Warning] {
        super::rows(&self.warnings, page)
    }
    /// Called only after an explicit bulk choice; previews default to Not now.
    pub fn bulk_decisions(&self, action: Action) -> Result<Vec<Decision>> {
        let candidates: Box<dyn Iterator<Item = &Candidate>> = match action {
            Action::Add | Action::Exclude => Box::new(self.additions.iter()),
            Action::Keep | Action::RemoveAndExclude => Box::new(self.removals.iter()),
            Action::NotNow => Box::new(self.additions.iter().chain(&self.removals)),
        };
        candidates
            .map(|c| {
                Ok(Decision {
                    uuid: StoryReference::from_stored(&c.uuid)?,
                    action,
                })
            })
            .collect()
    }
}

impl Candidate {
    pub fn reference(&self) -> Result<StoryReference> {
        Ok(StoryReference::from_stored(&self.uuid)?)
    }
}
