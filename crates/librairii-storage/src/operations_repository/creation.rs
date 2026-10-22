//! Creation and start transitions on the admitted owned writer.
use super::rows;
use crate::{
    Result, StorageError,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
};
use librairii_app::ports::{
    creation::{InputError, NewOperation},
    operations::OperationSnapshot,
};
use librairii_core::operations::{StoredExportScope, encode_kind, text_present};
use rusqlite::{Transaction, params};
fn invalid() -> StorageError {
    InputError::Invalid.into()
}
fn nonempty(value: &str) -> Option<&str> {
    (!value.is_empty()).then_some(value)
}
fn go_json(value: &impl serde::Serialize) -> Result<String> {
    Ok(serde_json::to_string(value)
        .map_err(|_| invalid())?
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029"))
}
fn source(input: &NewOperation) -> Result<StoredExportScope> {
    match &input.source {
        None => Ok(StoredExportScope::default()),
        Some(s) => Ok(StoredExportScope {
            discriminator: Some(s.source_type.as_persisted().into()),
            group_ids: Some(go_json(&s.group_ids())?),
            group_names: Some(go_json(&s.group_names())?),
        }),
    }
}
pub(super) fn creation_matches(input: &NewOperation, snapshot: &OperationSnapshot) -> Result<bool> {
    let h = &snapshot.operation;
    if h.identity.id != input.id
        || h.kind != input.kind
        || h.export_scope != source(input)?
        || h.destination != input.destination
        || snapshot.items.len() != input.items.len()
        || h.counts.total != i64::try_from(input.items.len()).map_err(|_| invalid())?
        || h.total_bytes != input.validate()?
    {
        return Ok(false);
    }
    Ok(snapshot
        .items
        .iter()
        .zip(&input.items)
        .all(|(stored, new)| {
            stored.identity.resolved_story_id == ((new.story_id != 0).then_some(new.story_id))
                && stored.identity.story_uuid.as_deref() == nonempty(&new.story_uuid)
                && stored.story_title.as_deref() == nonempty(&new.story_title)
                && stored.source_name == new.source_name
                && stored.output_name.as_deref() == nonempty(&new.output_name)
                && stored.archive_relative_path.as_deref() == nonempty(&new.archive_relative_path)
                && stored.archive_sha256.as_deref() == nonempty(&new.archive_sha256)
                && stored.export_skip_code.as_deref() == nonempty(&new.export_skip_code)
                && stored.progress.total == new.total_bytes
        }))
}
pub(super) fn create(
    tx: &Transaction<'_>,
    input: &NewOperation,
    created_at: &str,
) -> Result<OperationSnapshot> {
    let total = input.validate()?;
    if !text_present(created_at) {
        return Err(invalid());
    }
    if let Some(existing) = rows::snapshot(tx, &input.id)? {
        if existing.operation.identity.created_at != created_at
            || !creation_matches(input, &existing)?
        {
            return Err(StorageError::SnapshotChanged);
        }
        return Ok(existing);
    }
    let source = source(input)?;
    tx.execute("INSERT INTO file_operations(id,kind,status,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination,total_items,total_bytes,created_at) VALUES(?,?,'queued',?,?,?,?,?,?,?)",params![input.id,encode_kind(input.kind),source.discriminator,source.group_ids,source.group_names,input.destination,input.items.len() as i64,total,created_at])?;
    for item in &input.items {
        let story = (item.story_id != 0).then_some(item.story_id);
        tx.execute("INSERT INTO file_operation_items(operation_id,story_id,resolved_story_id,story_uuid,story_title,source_name,output_name,archive_relative_path,archive_sha256,export_skip_code,status,total_bytes) VALUES(?,?,?,?,?,?,?,?,?,?,'pending',?)",params![input.id,story,story,nonempty(&item.story_uuid),nonempty(&item.story_title),item.source_name,nonempty(&item.output_name),nonempty(&item.archive_relative_path),nonempty(&item.archive_sha256),nonempty(&item.export_skip_code),item.total_bytes])?;
    }
    rows::snapshot(tx, &input.id)?.ok_or_else(invalid)
}
impl Database<'_> {
    pub fn create_operation(
        &self,
        input: NewOperation,
        created_at: String,
        cancel: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<OperationSnapshot>>> {
        input.validate()?;
        self.transaction(cancel, move |tx| create(tx, &input, &created_at))
    }
    pub fn operation_snapshot(&self, id: String) -> Result<Pending<'_, Option<OperationSnapshot>>> {
        self.read(move |tx| rows::snapshot(tx, &id))
    }
    /// The acknowledgement readback shares the reserved, same-writer lane.
    /// Absence is positive evidence only in this fresh transaction and cannot
    /// erase an earlier unresolved effect owned elsewhere.
    pub fn readback_operation_creation(
        &self,
        input: NewOperation,
        created_at: String,
    ) -> Result<Pending<'_, Option<OperationSnapshot>>> {
        input.validate()?;
        self.barrier_read(move |tx| {
            let Some(snapshot) = rows::snapshot(tx, &input.id)? else {
                return Ok(None);
            };
            if snapshot.operation.identity.created_at != created_at
                || !creation_matches(&input, &snapshot)?
            {
                return Err(StorageError::SnapshotChanged);
            }
            Ok(Some(snapshot))
        })
    }
    pub fn mark_operation_item_running(
        &self,
        id: String,
        item: i64,
        at: String,
        cancel: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<()>>> {
        self.transaction(cancel,move|tx|{
            if tx.execute("UPDATE file_operations SET status='running',started_at=COALESCE(started_at,?) WHERE id=? AND status IN ('queued','running')",params![at,id])?!=1{return Err(invalid());}
            if tx.execute("UPDATE file_operation_items SET status='running' WHERE id=? AND operation_id=? AND status='pending'",params![item,id])?!=1{return Err(invalid());}Ok(())
        })
    }
}
