//! Coherent raw-key History and active/latest reads. No offset pagination.
use super::{rows, snapshot_bytes};
use crate::{
    Result, StorageError,
    database::{Database, Pending},
};
use librairii_app::ports::{
    creation::InputError, history::HistoryPage, operations::OperationSnapshot,
};
use rusqlite::{OptionalExtension, Transaction};
const MAX_VIEW_BYTES: usize = 64 * 1024 * 1024;
fn invalid() -> StorageError {
    InputError::Invalid.into()
}
pub(super) fn page_ids(
    tx: &Transaction<'_>,
    before: &str,
) -> Result<(Vec<String>, Option<String>)> {
    let mut ids = Vec::with_capacity(21);
    if before.is_empty() {
        let mut q =
            tx.prepare("SELECT id FROM file_operations ORDER BY created_at DESC,id DESC LIMIT 21")?;
        for id in q.query_map([], |r| rows::bounded_id(r, 0))? {
            ids.push(id?);
        }
    } else {
        let at = rows::header(tx, before)?
            .ok_or_else(invalid)?
            .identity
            .created_at;
        let mut q=tx.prepare("SELECT id FROM file_operations WHERE (created_at,id)<(?,?) ORDER BY created_at DESC,id DESC LIMIT 21")?;
        for id in q.query_map(rusqlite::params![at, before], |r| rows::bounded_id(r, 0))? {
            ids.push(id?);
        }
    }
    let next_before = if ids.len() > 20 {
        ids.truncate(20);
        ids.last().cloned()
    } else {
        None
    };
    Ok((ids, next_before))
}
fn append(
    tx: &Transaction<'_>,
    id: &str,
    out: &mut Vec<OperationSnapshot>,
    used: &mut usize,
) -> Result<()> {
    let snapshot = rows::snapshot(tx, id)?.ok_or_else(invalid)?;
    *used = used
        .checked_add(snapshot_bytes(&snapshot))
        .ok_or(InputError::ResourceLimit)?;
    out.push(snapshot);
    let spare = (out.capacity() - out.len())
        .checked_mul(std::mem::size_of::<OperationSnapshot>())
        .ok_or(InputError::ResourceLimit)?;
    if used.checked_add(spare).ok_or(InputError::ResourceLimit)? > MAX_VIEW_BYTES {
        return Err(InputError::ResourceLimit.into());
    }
    Ok(())
}
impl Database<'_> {
    /// Explicit diagnostic-safe reader, including during terminal drain. The
    /// actual root borrow and reserved writer transaction remain held throughout.
    pub fn operation_history(&self, before: String) -> Result<Pending<'_, HistoryPage>> {
        if before.capacity() > 256 {
            return Err(invalid());
        }
        self.barrier_read(move |tx| {
            let mut operations = Vec::new();
            let mut used = 0;
            let (ids, next_before) = page_ids(tx, &before)?;
            for id in ids {
                append(tx, &id, &mut operations, &mut used)?;
            }
            Ok(HistoryPage {
                operations,
                next_before,
            })
        })
    }
    /// All active rows in creation order, followed by exactly one latest saved
    /// terminal export. This has no History20 count cap or global re-sort.
    pub fn active_and_latest_operations(&self) -> Result<Pending<'_, Vec<OperationSnapshot>>> {
        self.barrier_read(|tx| {
            let mut operations=Vec::new();let mut used=0;
            let mut q=tx.prepare("SELECT id FROM file_operations WHERE status IN ('queued','running') ORDER BY created_at DESC,id DESC")?;
            for id in q.query_map([],|r|rows::bounded_id(r,0))? {append(tx,&id?,&mut operations,&mut used)?;}
            let latest:Option<String>=tx.query_row("SELECT id FROM file_operations WHERE kind='export' AND status IN ('succeeded','partially_succeeded','failed','cancelled','interrupted') ORDER BY COALESCE(finished_at,created_at) DESC,id DESC LIMIT 1",[],|r|rows::bounded_id(r,0)).optional()?;
            if let Some(id)=latest{append(tx,&id,&mut operations,&mut used)?;}Ok(operations)
        })
    }
}
