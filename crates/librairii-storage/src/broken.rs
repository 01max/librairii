//! Same-writer flag validation and mutation. Cancellation is checked at precommit.
use crate::{
    Result,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
};
use librairii_core::broken::{FlagChange, FlagError, FlagResult, FlagSummary, targets};
use rusqlite::{OptionalExtension, Transaction, params};
impl Database<'_> {
    pub fn set_broken_guarded(
        &self,
        identities: Vec<librairii_core::browsing::StoryIdentity>,
        broken: bool,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<FlagResult>>> {
        self.transaction(cancellation, move |tx| {
            crate::query::require_identities(tx, &identities)?;
            set(
                tx,
                &identities.iter().map(|s| s.id).collect::<Vec<_>>(),
                broken,
            )
        })
    }
    pub fn restore_broken_guarded(
        &self,
        identities: Vec<librairii_core::browsing::StoryIdentity>,
        changes: Vec<FlagChange>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<FlagResult>>> {
        self.transaction(cancellation, move |tx| {
            if !identities
                .iter()
                .map(|s| s.id)
                .eq(changes.iter().map(|c| c.story_id))
            {
                return Err(FlagError::InvalidRequest.into());
            }
            crate::query::require_identities(tx, &identities)?;
            restore(tx, &changes)
        })
    }
    pub fn restore_broken(
        &self,
        changes: Vec<FlagChange>,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<FlagResult>>> {
        self.transaction(cancellation, move |tx| restore(tx, &changes))
    }
    pub fn set_broken(
        &self,
        ids: Vec<i64>,
        broken: bool,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<FlagResult>>> {
        self.transaction(cancellation, move |tx| set(tx, &ids, broken))
    }
    pub fn broken_summary(&self, ids: Vec<i64>) -> Result<Pending<'_, FlagSummary>> {
        self.read(move |tx| summary(tx, &ids))
    }
}
pub(super) fn current(tx: &Transaction<'_>, id: i64) -> Result<bool> {
    tx.query_row("SELECT s.is_broken FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.id=? AND NOT EXISTS(SELECT 1 FROM removal_intents r WHERE r.story_id=s.id)",[id],|r|r.get(0)).optional()?.ok_or(FlagError::StoryUnavailable.into())
}
pub fn summary(tx: &Transaction<'_>, ids: &[i64]) -> Result<FlagSummary> {
    let ids = targets(ids)?;
    let mut flagged = 0;
    for &id in &ids {
        flagged += usize::from(current(tx, id)?);
    }
    Ok(FlagSummary {
        requested_stories: ids.len(),
        flagged_stories: flagged,
    })
}
pub fn set(tx: &Transaction<'_>, ids: &[i64], broken: bool) -> Result<FlagResult> {
    let ids = targets(ids)?;
    let mut changes = vec![];
    for &id in &ids {
        let previous = current(tx, id)?;
        if previous != broken {
            changes.push(FlagChange {
                story_id: id,
                previous,
                current: broken,
            });
        }
    }
    publish(tx, ids.len(), changes)
}
pub fn restore(tx: &Transaction<'_>, changes: &[FlagChange]) -> Result<FlagResult> {
    librairii_core::broken::validate_restore(changes)?;
    let mut restored = Vec::with_capacity(changes.len());
    for c in changes {
        if current(tx, c.story_id)? != c.current {
            return Err(FlagError::RestoreStale.into());
        }
        restored.push(FlagChange {
            story_id: c.story_id,
            previous: c.current,
            current: c.previous,
        });
    }
    publish(tx, changes.len(), restored)
}
pub(super) fn publish(
    tx: &Transaction<'_>,
    requested: usize,
    changes: Vec<FlagChange>,
) -> Result<FlagResult> {
    for c in &changes {
        tx.execute(
            "UPDATE stories SET is_broken=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
            params![c.current, c.story_id],
        )?;
    }
    Ok(FlagResult {
        requested_stories: requested,
        changed_stories: changes.len(),
        changes,
    })
}
