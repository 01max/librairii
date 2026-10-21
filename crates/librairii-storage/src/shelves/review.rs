use super::*;
use crate::database::SnapshotIdentity;
use librairii_core::{browsing::StorySummary, query::QueryRequest, shelves::unique_uuids};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
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
#[derive(Clone, Debug, Default)]
pub struct CreationRequest {
    pub name: String,
    pub manual_uuids: Vec<String>,
    pub collection_ids: Vec<i64>,
}
#[derive(Clone, Debug)]
pub struct CreationPreview {
    pub name: String,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<Warning>,
    pub incomplete: bool,
}
#[derive(Clone, Debug)]
pub struct CreationPlan {
    request: CreationRequest,
    preview: CreationPreview,
    fingerprint: [u8; 32],
    identity: SnapshotIdentity,
    used: Arc<AtomicBool>,
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
fn headings(candidates: &[Candidate]) -> Vec<(&str, &str, bool, &[Source], &[Source])> {
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
    locale: &str,
) -> Result<(CreationPreview, [u8; 32])> {
    let (name, normalized) = normalize_name(&request.name)?;
    let manual = unique_uuids(&request.manual_uuids)?;
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
    for uuid in &manual {
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
        let identity = self.snapshot_identity();
        self.read(move |tx| {
            let (preview, fingerprint) = creation(tx, &request, &locale)?;
            Ok(CreationPlan {
                request,
                preview,
                fingerprint,
                identity,
                used: Arc::new(AtomicBool::new(false)),
                locale,
            })
        })
    }
    pub fn create_shelf_reviewed(
        &self,
        plan: CreationPlan,
        selected: Vec<String>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        self.transaction(cancellation, move |tx| {
            if plan.used.swap(true, Ordering::AcqRel) {
                return Err(ShelfError::StaleReview.into());
            }
            let (fresh, fingerprint) = creation(tx, &plan.request, &plan.locale)?;
            if fingerprint != plan.fingerprint {
                return Err(ShelfError::StaleReview.into());
            }
            let selected = unique_uuids(&selected)?
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
