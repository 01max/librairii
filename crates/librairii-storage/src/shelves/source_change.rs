use super::*;
use crate::database::SnapshotIdentity;
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug)]
pub struct SourceChangePreview {
    pub shelf_id: i64,
    pub shelf_name: String,
    pub sources: Vec<Source>,
    pub detached: Vec<Source>,
    pub(super) affected: Vec<Candidate>,
    pub(super) warnings: Vec<Warning>,
    pub incomplete: bool,
}
#[derive(Clone, Debug)]
pub struct SourceChangePlan {
    id: i64,
    desired: Vec<i64>,
    preview: SourceChangePreview,
    fingerprint: [u8; 32],
    identity: SnapshotIdentity,
    ticket: Arc<tickets::Ticket>,
    locale: String,
}
impl SourceChangePlan {
    pub fn preview(&self) -> &SourceChangePreview {
        &self.preview
    }
}
pub(super) struct ChangeEvaluation {
    pub preview: SourceChangePreview,
    pub desired: Vec<i64>,
    pub fingerprint: [u8; 32],
}
pub(super) fn evaluate(
    tx: &Transaction<'_>,
    id: i64,
    desired: &[i64],
    locale: &str,
) -> Result<ChangeEvaluation> {
    let desired = sources::unique_ids(desired)?;
    let before = review::evaluation(tx, id, locale, None)?;
    let current = sources(tx, id, true)?;
    let mut wanted = sources::potential(tx, &desired)?;
    for source in &mut wanted {
        if let Some(existing) = current
            .iter()
            .find(|s| s.collection_id == source.collection_id)
        {
            *source = existing.clone();
        }
    }
    let after = review::evaluation(tx, id, locale, Some(wanted.clone()))?;
    let detached = current
        .into_iter()
        .filter(|s| !desired.iter().any(|id| Some(*id) == s.collection_id))
        .collect::<Vec<_>>();
    let shelf = read(tx, id)?;
    let mut preview = SourceChangePreview {
        shelf_id: id,
        shelf_name: shelf.name,
        sources: wanted,
        detached,
        affected: Vec::new(),
        warnings: after.preview.warnings,
        incomplete: after.preview.incomplete,
    };
    let coverage = sources::evaluate(tx, preview.sources.clone(), locale)?;
    let members = members(tx, id)?;
    let uuids = members.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>();
    let mut available = BTreeMap::new();
    crate::query::visit_uuids(tx, Default::default(), locale, &uuids, |s| {
        available.insert(s.uuid.clone(), s);
        Ok(())
    })?;
    if !preview.detached.is_empty() {
        for m in members {
            if m.manually_retained || coverage.matching.contains_key(&m.uuid) {
                continue;
            }
            let story = available.remove(&m.uuid);
            preview.affected.push(Candidate {
                uuid: m.uuid,
                title: story
                    .as_ref()
                    .map(|s| s.title.clone())
                    .unwrap_or(m.cached_title),
                story,
                sources: Vec::new(),
                origins: m.origins,
                manual: false,
                reason: if desired.is_empty() {
                    "last_source_detached"
                } else {
                    "source_coverage_lost"
                },
            });
        }
    }
    let headings = preview
        .affected
        .iter()
        .map(|c| (&c.uuid, &c.title, &c.origins, c.story.is_some()))
        .collect::<Vec<_>>();
    let fingerprint = review::digest(&(before.fingerprint, after.fingerprint, &desired, headings));
    Ok(ChangeEvaluation {
        preview,
        desired,
        fingerprint,
    })
}
pub(super) fn validate(
    preview: &SourceChangePreview,
    keep_all: bool,
    decisions: &[Decision],
) -> Result<BTreeMap<String, Action>> {
    if keep_all {
        if !decisions.is_empty() {
            return Err(ShelfError::InvalidInput.into());
        }
        return Ok(preview
            .affected
            .iter()
            .map(|c| (c.uuid.clone(), Action::Keep))
            .collect());
    }
    let known = preview
        .affected
        .iter()
        .map(|c| (c.uuid.as_str(), c))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for d in decisions {
        let candidate = known.get(d.uuid.as_str()).ok_or(ShelfError::InvalidInput)?;
        if !matches!(d.action, Action::Keep | Action::RemoveAndExclude)
            || (d.action == Action::RemoveAndExclude
                && (preview.incomplete || candidate.story.is_none()))
            || result.insert(d.uuid.clone(), d.action).is_some()
        {
            return Err(ShelfError::InvalidInput.into());
        }
    }
    if result.len() != known.len() {
        return Err(ShelfError::InvalidInput.into());
    }
    Ok(result)
}
pub(super) fn apply(
    tx: &Transaction<'_>,
    e: &ChangeEvaluation,
    actions: &BTreeMap<String, Action>,
) -> Result<Shelf> {
    let id = e.preview.shelf_id;
    let mut changed = sources::attach(tx, id, &e.desired)?;
    for source in &e.preview.detached {
        changed|=tx.execute("UPDATE shelf_sources SET active=0,collection_name=? WHERE id=? AND shelf_id=? AND active=1",params![source.name,source.id,id])?!=0;
    }
    for c in &e.preview.affected {
        match actions.get(&c.uuid) {
            Some(Action::Keep) => {
                changed |= review::keep(tx, id, &c.uuid)?;
            }
            Some(Action::RemoveAndExclude) => {
                changed |= curation::remove(tx, id, &c.uuid)?;
                changed |= curation::exclude(tx, id, &c.uuid)?;
            }
            _ => return Err(ShelfError::InvalidInput.into()),
        }
    }
    if e.desired.is_empty() {
        // The resulting manual Shelf retains every survivor, including unavailable UUIDs.
        changed |= tx.execute(
            "UPDATE shelf_stories SET protected=1 WHERE shelf_id=? AND protected=0",
            [id],
        )? != 0;
    }
    bump(tx, id, changed)
}
impl Database<'_> {
    pub fn shelf_source_change_preview(
        &self,
        id: i64,
        desired: Vec<i64>,
        locale: String,
    ) -> Result<Pending<'_, SourceChangePlan>> {
        let identity = self.snapshot_identity();
        let registry = self.shelf_tickets.clone();
        self.read(move |tx| {
            let e = evaluate(tx, id, &desired, &locale)?;
            Ok(SourceChangePlan {
                id,
                desired: e.desired,
                preview: e.preview,
                fingerprint: e.fingerprint,
                identity,
                ticket: tickets::issue(&registry, tickets::Kind::Sources, id)?,
                locale,
            })
        })
    }
    pub fn change_shelf_sources(
        &self,
        plan: SourceChangePlan,
        keep_all: bool,
        decisions: Vec<Decision>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        self.transaction(cancellation, move |tx| {
            plan.ticket.claim(tickets::Kind::Sources, plan.id)?;
            let fresh = evaluate(tx, plan.id, &plan.desired, &plan.locale)?;
            if fresh.fingerprint != plan.fingerprint {
                return Err(ShelfError::StaleReview.into());
            }
            let actions = validate(&fresh.preview, keep_all, &decisions)?;
            apply(tx, &fresh, &actions)
        })
    }
}

impl SourceChangePreview {
    pub fn affected_count(&self) -> usize {
        self.affected.len()
    }
    pub fn affected_page(&self, page: usize) -> &[Candidate] {
        super::rows(&self.affected, page)
    }
    pub fn warning_count(&self) -> usize {
        self.warnings.len()
    }
    pub fn warnings_page(&self, page: usize) -> &[Warning] {
        super::rows(&self.warnings, page)
    }
    pub fn bulk_decisions(&self, action: Action) -> Result<Vec<Decision>> {
        if !matches!(action, Action::Keep | Action::RemoveAndExclude)
            || (action == Action::RemoveAndExclude
                && (self.incomplete || self.affected.iter().any(|c| c.story.is_none())))
        {
            return Err(ShelfError::InvalidInput.into());
        }
        Ok(self
            .affected
            .iter()
            .map(|c| Decision {
                uuid: c.uuid.clone(),
                action,
            })
            .collect())
    }
}
