//! Bounded native operation observations, each from one writer snapshot.
use super::rows;
use crate::{
    Result, StorageError,
    database::{Database, Pending},
};
use librairii_app::ports::{
    creation::InputError,
    operations::{OperationHeader, OperationItem},
};
use rusqlite::{OptionalExtension, Transaction};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct OperationCard {
    pub operation: OperationHeader,
    pub completed_bytes: i64,
    pub total_bytes: i64,
    pub succeeded: i64,
    pub skipped: i64,
    pub conflicted: i64,
    pub failed: i64,
    pub cancelled: i64,
    pub interrupted: i64,
    pub running: Vec<(i64, i64)>,
}
#[derive(Clone, Debug)]
pub struct OperationItems {
    pub card: OperationCard,
    pub page: usize,
    pub pages: usize,
    pub items: Vec<OperationItem>,
}
#[derive(Clone, Debug)]
pub struct OperationHistory {
    pub cards: Vec<OperationCard>,
    pub next_before: Option<String>,
}
fn invalid() -> StorageError {
    InputError::Invalid.into()
}
pub(super) fn card(tx: &Transaction<'_>, id: &str) -> Result<OperationCard> {
    let operation = rows::header(tx, id)?.ok_or_else(invalid)?;
    let total_bytes = tx.query_row(
        "SELECT COALESCE(SUM(total_bytes),0) FROM file_operation_items WHERE operation_id=?",
        [id],
        |r| r.get(0),
    )?;
    let values=tx.query_row("SELECT COALESCE(SUM(completed_bytes),0),COALESCE(SUM(status='succeeded'),0),COALESCE(SUM(status='skipped'),0),COALESCE(SUM(status='conflicted'),0),COALESCE(SUM(status='failed'),0),COALESCE(SUM(status='cancelled'),0),COALESCE(SUM(status='interrupted'),0) FROM file_operation_items WHERE operation_id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)))?;
    let mut q=tx.prepare("SELECT id,completed_bytes FROM file_operation_items WHERE operation_id=? AND status='running' ORDER BY id LIMIT 9")?;
    let running = q
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if running.len() > 8 {
        return Err(InputError::ResourceLimit.into());
    }
    Ok(OperationCard {
        operation,
        completed_bytes: values.0,
        total_bytes,
        succeeded: values.1,
        skipped: values.2,
        conflicted: values.3,
        failed: values.4,
        cancelled: values.5,
        interrupted: values.6,
        running,
    })
}
fn read_cards(tx: &Transaction<'_>, ids: Vec<String>) -> Result<Vec<OperationCard>> {
    let mut result = Vec::with_capacity(ids.len());
    let mut used = 0usize;
    for id in ids {
        let value = card(tx, &id)?;
        used = used
            .checked_add(super::snapshot_bytes(
                &librairii_app::ports::operations::OperationSnapshot {
                    operation: value.operation.clone(),
                    items: Vec::new(),
                },
            ))
            .ok_or(InputError::ResourceLimit)?;
        if used > 64 * 1024 * 1024 {
            return Err(InputError::ResourceLimit.into());
        }
        result.push(value);
    }
    Ok(result)
}
impl Database<'_> {
    /// Reobserve exactly the displayed History identities without moving its
    /// creation cursor, inserting new rows or retaining item payloads.
    pub fn native_history_cards(
        &self,
        ids: Vec<String>,
    ) -> Result<Pending<'_, Vec<OperationCard>>> {
        if ids.len() > 20 || ids.iter().any(|id| id.is_empty() || id.len() > 256) {
            return Err(invalid());
        }
        self.barrier_read(move |tx| read_cards(tx, ids))
    }
    /// The same raw creation-time/ID cursor as History, with bounded card
    /// observations instead of materializing every historical item's payload.
    pub fn native_operation_history(
        &self,
        before: String,
    ) -> Result<Pending<'_, OperationHistory>> {
        if before.capacity() > 256 {
            return Err(invalid());
        }
        self.barrier_read(move |tx| {
            let (ids, next_before) = super::history::page_ids(tx, &before)?;
            let cards = read_cards(tx, ids)?;
            Ok(OperationHistory { cards, next_before })
        })
    }
    /// Active work plus at most eight explicitly retained recent IDs and the
    /// latest import. This does not change the historical export-feed contract.
    pub fn native_operation_cards(
        &self,
        recent: Vec<String>,
    ) -> Result<Pending<'_, Vec<OperationCard>>> {
        if recent.len() > 8 || recent.iter().any(|s| s.is_empty() || s.len() > 256) {
            return Err(invalid());
        }
        self.barrier_read(move|tx|{
            let mut ids=Vec::new();let mut seen=BTreeSet::new();
            let mut q=tx.prepare("SELECT id FROM file_operations WHERE status IN ('queued','running') ORDER BY created_at DESC,id DESC LIMIT 17")?;
            for id in q.query_map([],|r|rows::bounded_id(r,0))? {let id=id?;seen.insert(id.clone());ids.push(id);}
            if ids.len()>16{return Err(InputError::ResourceLimit.into());}
            for id in recent {if seen.insert(id.clone()) && rows::header(tx,&id)?.is_some(){ids.push(id);}}
            let mut latest=tx.prepare("SELECT id FROM file_operations WHERE kind='import' AND status NOT IN ('queued','running') ORDER BY COALESCE(finished_at,created_at) DESC,id DESC LIMIT 1")?;
            for id in latest.query_map([],|r|rows::bounded_id(r,0))? {let id=id?;if seen.insert(id.clone()){ids.push(id);}}
            let latest_export:Option<String>=tx.query_row("SELECT id FROM file_operations WHERE kind='export' AND status NOT IN ('queued','running') ORDER BY COALESCE(finished_at,created_at) DESC,id DESC LIMIT 1",[],|r|rows::bounded_id(r,0)).optional()?;
            if let Some(id)=latest_export && seen.insert(id.clone()){ids.push(id);}
            read_cards(tx,ids)
        })
    }
    pub fn native_operation_items(
        &self,
        id: String,
        page: usize,
    ) -> Result<Pending<'_, OperationItems>> {
        if id.is_empty() || id.len() > 256 {
            return Err(invalid());
        }
        self.barrier_read(move |tx| {
            let card = card(tx, &id)?;
            let total: i64 = tx.query_row(
                "SELECT count(*) FROM file_operation_items WHERE operation_id=?",
                [&id],
                |r| r.get(0),
            )?;
            let total = usize::try_from(total).map_err(|_| InputError::ResourceLimit)?;
            let pages = total.div_ceil(25).max(1);
            let page = page.min(pages - 1);
            let items = rows::items_page(tx, &id, page * 25)?;
            Ok(OperationItems {
                card,
                page,
                pages,
                items,
            })
        })
    }
}
