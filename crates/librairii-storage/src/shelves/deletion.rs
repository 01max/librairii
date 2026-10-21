use super::*;
use crate::{collections::DeletedCollection, database::SnapshotIdentity};
use librairii_core::collections::Collection;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[derive(Clone, Debug)]
pub struct CollectionDeletionPreview {
    pub collection: Collection,
    shelves: Vec<SourceChangePreview>,
}
#[derive(Clone, Debug)]
pub struct CollectionDeletionPlan {
    id: CollectionId,
    preview: CollectionDeletionPreview,
    fingerprint: [u8; 32],
    identity: SnapshotIdentity,
    ticket: Arc<tickets::Ticket>,
    locale: String,
}
impl CollectionDeletionPlan {
    pub fn preview(&self) -> &CollectionDeletionPreview {
        &self.preview
    }
}
#[derive(Clone, Debug)]
pub struct ShelfDecisions {
    pub shelf_id: ShelfId,
    pub decisions: Vec<Decision>,
}
struct DeletionEvaluation {
    preview: CollectionDeletionPreview,
    changes: Vec<source_change::ChangeEvaluation>,
    fingerprint: [u8; 32],
}
fn evaluate(tx: &Transaction<'_>, id: CollectionId, locale: &str) -> Result<DeletionEvaluation> {
    let collection = crate::collections::read(tx, id.get())?;
    let ids=tx.prepare("SELECT DISTINCT shelf_id FROM shelf_sources WHERE collection_id=? AND active=1 ORDER BY shelf_id")?.query_map([id.get()],|r|shelf_id(r.get(0)?))?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut changes = Vec::new();
    for shelf in ids {
        let desired = sources(tx, shelf, true)?
            .iter()
            .filter_map(|s| s.collection_id.filter(|c| *c != id))
            .collect::<Vec<_>>();
        changes.push(source_change::evaluate(tx, shelf, &desired, locale)?);
    }
    let hashes = changes.iter().map(|e| e.fingerprint).collect::<Vec<_>>();
    let fingerprint = review::digest(&(&collection, hashes));
    let shelves = changes.iter().map(|e| e.preview.clone()).collect();
    Ok(DeletionEvaluation {
        preview: CollectionDeletionPreview {
            collection,
            shelves,
        },
        changes,
        fingerprint,
    })
}
impl Database<'_> {
    pub fn linked_collection_deletion_preview(
        &self,
        id: CollectionId,
        locale: String,
    ) -> Result<Pending<'_, CollectionDeletionPlan>> {
        let identity = self.snapshot_identity();
        let registry = self.shelf_tickets.clone();
        self.writer_read(move |tx| {
            let e = evaluate(tx, id, &locale)?;
            Ok(CollectionDeletionPlan {
                id,
                ticket: tickets::issue(
                    &registry,
                    tickets::Kind::DeleteCollection,
                    id.get(),
                    e.changes
                        .iter()
                        .map(|change| change.preview.shelf_id)
                        .collect(),
                )?,
                preview: e.preview,
                fingerprint: e.fingerprint,
                identity,
                locale,
            })
        })
    }
    pub fn delete_linked_collection(
        &self,
        plan: CollectionDeletionPlan,
        keep_all: bool,
        decisions: Vec<ShelfDecisions>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<DeletedCollection>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        let library = self.library_id();
        self.transaction(cancellation, move |tx| {
            plan.ticket
                .claim(tickets::Kind::DeleteCollection, plan.id.get())?;
            let fresh = evaluate(tx, plan.id, &plan.locale)?;
            if fresh.fingerprint != plan.fingerprint {
                return Err(ShelfError::StaleReview.into());
            }
            if keep_all && !decisions.is_empty() {
                return Err(ShelfError::InvalidInput.into());
            }
            let known = fresh
                .changes
                .iter()
                .map(|e| e.preview.shelf_id)
                .collect::<BTreeSet<_>>();
            let mut groups = BTreeMap::new();
            for d in decisions {
                if !known.contains(&d.shelf_id) || groups.insert(d.shelf_id, d.decisions).is_some()
                {
                    return Err(ShelfError::InvalidInput.into());
                }
            }
            let mut validated = Vec::new();
            for e in &fresh.changes {
                validated.push(source_change::validate(
                    &e.preview,
                    keep_all,
                    groups
                        .get(&e.preview.shelf_id)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                )?);
            }
            for (e, actions) in fresh.changes.iter().zip(&validated) {
                source_change::apply(tx, e, actions)?;
            }
            // The guarded Task 9 participant clears only inactive foreign keys.
            // It still refuses any active link missed by this transaction.
            crate::collections::execute(
                tx,
                crate::collections::Command::Delete { id: plan.id.get() },
            )?;
            Ok(DeletedCollection::after_reviewed_delete(
                plan.id.get(),
                library,
            ))
        })
    }
}

impl CollectionDeletionPreview {
    pub fn shelf_count(&self) -> usize {
        self.shelves.len()
    }
    /// Show one affected Shelf at a time, with its own bounded candidate pages.
    pub fn shelf(&self, index: usize) -> Option<&SourceChangePreview> {
        self.shelves.get(index)
    }
}
