use super::*;
use librairii_core::query::QueryRequest;
use std::collections::BTreeMap;
#[derive(Debug)]
pub enum Curation {
    Add(Vec<StoryUuid>),
    Remove(Vec<StoryReference>),
    RemoveAndExclude(Vec<StoryReference>),
    Exclude(Vec<StoryReference>),
    ClearExclusion(StoryReference),
    Reorder(Vec<StoryReference>),
}
impl Database<'_> {
    /// Add selected available stories with transaction-local ID/UUID checks.
    /// Existing legacy spellings remain distinct from canonical aliases.
    pub fn add_shelf_stories_guarded(
        &self,
        id: ShelfId,
        targets: Vec<librairii_core::browsing::StoryIdentity>,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        self.transaction(cancellation, move |tx| {
            crate::query::require_identities(tx, &targets)?;
            read(tx, id)?;
            let mut seen = std::collections::BTreeSet::new();
            let ids = targets
                .into_iter()
                .map(|s| s.uuid)
                .filter(|uuid| seen.insert(uuid.clone()))
                .collect::<Vec<_>>();
            for uuid in &ids {
                StoryReference::from_stored(uuid)?;
            }
            let available = titles(tx, &ids, &locale)?;
            if available.len() != ids.len() {
                return Err(ShelfError::NotFound.into());
            }
            let mut changed = false;
            for uuid in ids {
                changed |= add(tx, id, &uuid, &available[&uuid], true)?;
            }
            bump(tx, id, changed)
        })
    }
    pub fn curate_shelf(
        &self,
        id: ShelfId,
        action: Curation,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        self.transaction(cancellation, move |tx| curate(tx, id, action, &locale))
    }
    /// Explicit member actions recheck the Shelf version observed by their
    /// confirmation/order view. Broken flags do not change that version.
    pub fn curate_shelf_reviewed(
        &self,
        plan: CurationPlan,
        action: Curation,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Shelf>>> {
        if self.snapshot_identity() != plan.identity {
            return Err(ShelfError::StaleReview.into());
        }
        self.transaction(cancellation, move |tx| {
            let id = plan.shelf.id;
            plan.ticket.claim(tickets::Kind::Curation, id.get())?;
            if read(tx, id)?.revision != plan.shelf.revision {
                return Err(ShelfError::StaleReview.into());
            }
            curate(tx, id, action, &locale)
        })
    }
}
pub fn exclusions(tx: &Transaction<'_>, id: ShelfId) -> Result<Vec<String>> {
    Ok(tx
        .prepare("SELECT story_uuid FROM shelf_exclusions WHERE shelf_id=? ORDER BY story_uuid")?
        .query_map([id.get()], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
pub(super) fn titles(
    tx: &Transaction<'_>,
    uuids: &[String],
    locale: &str,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    crate::query::visit_uuids(tx, QueryRequest::default(), locale, uuids, |s| {
        result.insert(s.uuid, s.title);
        Ok(())
    })?;
    Ok(result)
}
pub(super) fn add(
    tx: &Transaction<'_>,
    id: ShelfId,
    uuid: &str,
    title: &str,
    manual: bool,
) -> Result<bool> {
    let inserted=tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,(SELECT COALESCE(MAX(position),-1)+1 FROM shelf_stories WHERE shelf_id=?),?,?) ON CONFLICT(shelf_id,story_uuid) DO UPDATE SET protected=1 WHERE excluded.protected=1 AND shelf_stories.protected=0",params![id.get(),uuid,id.get(),manual,title])?;
    let cleared = tx.execute(
        "DELETE FROM shelf_exclusions WHERE shelf_id=? AND story_uuid=?",
        params![id.get(), uuid],
    )?;
    Ok(inserted + cleared != 0)
}
pub(super) fn exclude(tx: &Transaction<'_>, id: ShelfId, uuid: &str) -> Result<bool> {
    Ok(tx.execute(
        "INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?) ON CONFLICT DO NOTHING",
        params![id.get(), uuid],
    )? != 0)
}
pub(super) fn remove(tx: &Transaction<'_>, id: ShelfId, uuid: &str) -> Result<bool> {
    Ok(tx.execute(
        "DELETE FROM shelf_stories WHERE shelf_id=? AND story_uuid=?",
        params![id.get(), uuid],
    )? != 0)
}
pub fn curate(tx: &Transaction<'_>, id: ShelfId, action: Curation, locale: &str) -> Result<Shelf> {
    read(tx, id)?;
    let mut changed = false;
    let removals = match &action {
        Curation::Remove(v) => Some((v, false, true)),
        Curation::RemoveAndExclude(v) => Some((v, true, true)),
        Curation::Exclude(v) => Some((v, true, false)),
        _ => None,
    };
    if let Some((values, excluding, removing)) = removals {
        let ids = reference_strings(values);
        if removing {
            let known: std::collections::BTreeSet<_> = members(tx, id)?
                .into_iter()
                .map(|m| m.uuid)
                .chain(exclusions(tx, id)?)
                .collect();
            if ids.iter().any(|u| !known.contains(u)) {
                return Err(ShelfError::NotFound.into());
            }
        }
        for uuid in ids {
            if removing {
                changed |= remove(tx, id, &uuid)?;
            }
            if excluding {
                changed |= exclude(tx, id, &uuid)?;
            }
        }
        return bump(tx, id, changed);
    }
    match action {
        Curation::Add(values) => {
            let ids = uuid_strings(&values);
            let available = titles(tx, &ids, locale)?;
            if available.len() != ids.len() {
                return Err(ShelfError::NotFound.into());
            }
            for uuid in ids {
                changed |= add(tx, id, &uuid, &available[&uuid], true)?;
            }
        }
        Curation::Remove(values)
        | Curation::RemoveAndExclude(values)
        | Curation::Exclude(values) => {
            // These variants were validated and handled together above.
            let _ = values;
            return Err(ShelfError::InvalidInput.into());
        }
        Curation::ClearExclusion(value) => {
            let value = value.as_stored();
            changed = tx.execute(
                "DELETE FROM shelf_exclusions WHERE shelf_id=? AND story_uuid=?",
                params![id.get(), value],
            )? != 0;
        }
        Curation::Reorder(values) => {
            let ids = reference_strings(&values);
            let current = members(tx, id)?;
            let old: Vec<_> = current.iter().map(|m| m.uuid.clone()).collect();
            if ids.len() != values.len()
                || ids.iter().collect::<std::collections::BTreeSet<_>>()
                    != old.iter().collect::<std::collections::BTreeSet<_>>()
            {
                return Err(ShelfError::InvalidInput.into());
            }
            if ids != old {
                let offset = current
                    .iter()
                    .map(|m| m.position)
                    .max()
                    .unwrap_or(-1)
                    .checked_add(1)
                    .ok_or(ShelfError::InvalidInput)?;
                for m in &current {
                    m.position
                        .checked_add(offset)
                        .ok_or(ShelfError::InvalidInput)?;
                }
                tx.execute(
                    "UPDATE shelf_stories SET position=position+? WHERE shelf_id=?",
                    params![offset, id.get()],
                )?;
                for (p, uuid) in ids.iter().enumerate() {
                    tx.execute(
                        "UPDATE shelf_stories SET position=? WHERE shelf_id=? AND story_uuid=?",
                        params![p as i64, id.get(), uuid],
                    )?;
                }
                changed = true;
            }
        }
    }
    bump(tx, id, changed)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Exclusion {
    pub uuid: String,
    pub title: String,
}
#[derive(Debug)]
pub struct ExclusionPage {
    pub total: usize,
    pub rows: Vec<Exclusion>,
}
impl Database<'_> {
    pub fn shelf_exclusions_page(
        &self,
        id: ShelfId,
        page: usize,
    ) -> Result<Pending<'_, ExclusionPage>> {
        self.read(move|tx|{
            read(tx,id)?;
            let total=tx.query_row("SELECT COUNT(*) FROM shelf_exclusions WHERE shelf_id=?",[id.get()],|r|r.get::<_,i64>(0))? as usize;
            let offset=page.checked_mul(25).and_then(|n|i64::try_from(n).ok()).unwrap_or(i64::MAX);
            let rows=tx.prepare("SELECT e.story_uuid,COALESCE(NULLIF(s.embedded_title,''),e.story_uuid) FROM shelf_exclusions e LEFT JOIN stories s ON s.uuid=e.story_uuid WHERE e.shelf_id=? ORDER BY e.story_uuid LIMIT 25 OFFSET ?")?.query_map(params![id.get(),offset],|r|Ok(Exclusion{uuid:r.get(0)?,title:r.get(1)?}))?.collect::<rusqlite::Result<_>>()?;
            Ok(ExclusionPage{total,rows})
        })
    }
}
