//! Raw owned rows; no SQL value escapes its one transaction.
use crate::Result;
use librairii_app::ports::operations::{
    OperationHeader, OperationItem, OperationSnapshot, SiblingCounts, TerminalView,
};
use librairii_core::operations::{
    Counts, ItemState, Progress, RawOutcome, StoredExportScope, StoredItemIdentity,
    StoredOperationIdentity, decode_kind, decode_state,
};
use rusqlite::{OptionalExtension, Row, Transaction, types::Type};

const HEADER: &str = "SELECT id, kind, status, export_source_type,
    export_source_shelf_ids, export_source_shelf_names, export_destination,
    completed_items, total_items, total_bytes, cancel_requested,
    error_code, error_message, created_at, started_at, finished_at
    FROM file_operations WHERE id = ?";
const ITEMS: &str = "SELECT operation_id, id, story_id, resolved_story_id, story_uuid,
    story_title, source_name, output_name, archive_relative_path, archive_sha256,
    export_skip_code, status, outcome_code, outcome_message, completed_bytes, total_bytes
    FROM file_operation_items";

// Bound materialization before extracting any variable-length SQLite text.
// History has a separate aggregate page budget in addition to this per-snapshot
// ceiling. These limits are bytes, not an export count or a stored-row rewrite.
const MAX_ROW_TEXT: i64 = 16 * 1024 * 1024;
const MAX_SNAPSHOT: i64 = 64 * 1024 * 1024;
fn budget(
    tx: &Transaction<'_>,
    table: &str,
    columns: &[&str],
    predicate: &str,
    args: impl rusqlite::Params,
    overhead: usize,
    limit: i64,
) -> Result<()> {
    let text = columns
        .iter()
        .map(|c| format!("COALESCE(length(CAST({c} AS BLOB)),0)"))
        .collect::<Vec<_>>()
        .join("+");
    let (bytes, largest): (i64, i64) = tx.query_row(
        &format!("SELECT COALESCE(SUM({text}+{overhead}),0),COALESCE(MAX({text}+{overhead}),0) FROM {table} WHERE {predicate}"),
        args,
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if bytes > limit || largest > MAX_ROW_TEXT {
        return Err(librairii_app::ports::creation::InputError::ResourceLimit.into());
    }
    Ok(())
}
const HEADER_TEXT: &[&str] = &[
    "id",
    "kind",
    "status",
    "created_at",
    "started_at",
    "finished_at",
    "export_source_type",
    "export_source_shelf_ids",
    "export_source_shelf_names",
    "export_destination",
    "error_code",
    "error_message",
];
const ITEM_TEXT: &[&str] = &[
    "status",
    "operation_id",
    "story_uuid",
    "story_title",
    "source_name",
    "output_name",
    "archive_relative_path",
    "archive_sha256",
    "export_skip_code",
    "outcome_code",
    "outcome_message",
];

fn codec(index: usize, error: librairii_core::operations::OperationCodecError) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

fn header_row(row: &Row<'_>) -> rusqlite::Result<OperationHeader> {
    let kind: String = row.get(1)?;
    let state: String = row.get(2)?;
    Ok(OperationHeader {
        identity: StoredOperationIdentity {
            id: row.get(0)?,
            created_at: row.get(13)?,
            started_at: row.get(14)?,
            finished_at: row.get(15)?,
        },
        kind: decode_kind(&kind).map_err(|e| codec(1, e))?,
        state: decode_state(&state).map_err(|e| codec(2, e))?,
        export_scope: StoredExportScope {
            discriminator: row.get(3)?,
            group_ids: row.get(4)?,
            group_names: row.get(5)?,
        },
        destination: crate::native_path::optional_sql_path(row, 6)?,
        counts: Counts {
            completed: row.get(7)?,
            total: row.get(8)?,
        },
        total_bytes: row.get(9)?,
        cancel_requested: row.get::<_, i64>(10)? == 1,
        outcome: RawOutcome {
            code: row.get(11)?,
            message: row.get(12)?,
        },
    })
}

fn item_row(row: &Row<'_>) -> rusqlite::Result<OperationItem> {
    let state: String = row.get(11)?;
    Ok(OperationItem {
        operation_id: row.get(0)?,
        identity: StoredItemIdentity {
            id: row.get(1)?,
            story_id: row.get(2)?,
            resolved_story_id: row.get(3)?,
            story_uuid: row.get(4)?,
        },
        story_title: row.get(5)?,
        source_name: row.get(6)?,
        output_name: crate::native_path::optional_sql_path(row, 7)?,
        archive_relative_path: crate::native_path::optional_sql_path(row, 8)?,
        archive_sha256: row.get(9)?,
        export_skip_code: row.get(10)?,
        state: ItemState::from_persisted(&state).map_err(|e| codec(11, e))?,
        outcome: RawOutcome {
            code: row.get(12)?,
            message: row.get(13)?,
        },
        progress: Progress {
            completed: row.get(14)?,
            total: row.get(15)?,
        },
    })
}

pub(super) fn header(tx: &Transaction<'_>, id: &str) -> Result<Option<OperationHeader>> {
    budget(
        tx,
        "file_operations",
        HEADER_TEXT,
        "id=?",
        [id],
        std::mem::size_of::<OperationHeader>(),
        MAX_ROW_TEXT,
    )?;
    Ok(tx.query_row(HEADER, [id], header_row).optional()?)
}

pub(super) fn snapshot(tx: &Transaction<'_>, id: &str) -> Result<Option<OperationSnapshot>> {
    let Some(operation) = header(tx, id)? else {
        return Ok(None);
    };
    budget(
        tx,
        "file_operation_items",
        ITEM_TEXT,
        "operation_id=?",
        [id],
        std::mem::size_of::<OperationItem>(),
        MAX_SNAPSHOT - MAX_ROW_TEXT,
    )?;
    let mut statement = tx.prepare(&format!("{ITEMS} WHERE operation_id = ? ORDER BY id"))?;
    let items = statement
        .query_map([id], item_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some(OperationSnapshot { operation, items }))
}
pub(super) fn items_page(
    tx: &Transaction<'_>,
    id: &str,
    offset: usize,
) -> Result<Vec<OperationItem>> {
    let offset = i64::try_from(offset).map_err(|_| {
        crate::StorageError::from(librairii_app::ports::creation::InputError::ResourceLimit)
    })?;
    let predicate = "id IN (SELECT id FROM file_operation_items WHERE operation_id=? ORDER BY id LIMIT 25 OFFSET ?)";
    budget(
        tx,
        "file_operation_items",
        ITEM_TEXT,
        predicate,
        rusqlite::params![id, offset],
        std::mem::size_of::<OperationItem>(),
        MAX_SNAPSHOT - MAX_ROW_TEXT,
    )?;
    let mut q = tx.prepare(&format!("{ITEMS} WHERE {predicate} ORDER BY id"))?;
    Ok(q.query_map(rusqlite::params![id, offset], item_row)?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn view(
    tx: &Transaction<'_>,
    id: &str,
    item: Option<i64>,
) -> Result<Option<TerminalView>> {
    let Some(operation) = header(tx, id)? else {
        return Ok(None);
    };
    if let Some(item) = item {
        budget(
            tx,
            "file_operation_items",
            ITEM_TEXT,
            "operation_id=? AND id=?",
            rusqlite::params![id, item],
            std::mem::size_of::<OperationItem>(),
            MAX_ROW_TEXT,
        )?;
    }
    let item = match item {
        Some(item) => tx
            .query_row(
                &format!("{ITEMS} WHERE operation_id = ? AND id = ?"),
                rusqlite::params![id, item],
                item_row,
            )
            .optional()?,
        None => None,
    };
    let siblings = tx.query_row(
        "SELECT count(*), COALESCE(sum(CASE WHEN status IN
         ('succeeded','skipped','conflicted','failed','cancelled') THEN 1 ELSE 0 END),0)
         FROM file_operation_items WHERE operation_id = ?",
        [id],
        |r| {
            Ok(SiblingCounts {
                items: r.get(0)?,
                terminal: r.get(1)?,
            })
        },
    )?;
    Ok(Some(TerminalView {
        operation,
        item,
        siblings,
    }))
}

// Even selection keys are checked before constructing an owned allocation.
// Frozen IDs have 36 SQLite characters; 256 UTF-8 bytes covers every valid ID.
pub(super) fn bounded_id(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    let value = row.get_ref(index)?.as_str()?;
    if value.len() > 256 {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            index,
            Type::Text,
            Box::new(librairii_app::ports::creation::InputError::ResourceLimit),
        ));
    }
    Ok(value.to_owned())
}

// Aggregation streams arbitrarily many siblings, retaining only one outcome.
// Bound each selected row, without imposing an operation-wide item-count cap.
pub(super) fn terminal_outcomes_budget(tx: &Transaction<'_>, id: &str) -> Result<()> {
    let largest: i64 = tx.query_row("SELECT COALESCE(MAX(length(CAST(status AS BLOB))+COALESCE(length(CAST(outcome_code AS BLOB)),0)+COALESCE(length(CAST(outcome_message AS BLOB)),0)),0) FROM file_operation_items WHERE operation_id=?", [id], |r|r.get(0))?;
    if largest > MAX_ROW_TEXT {
        return Err(librairii_app::ports::creation::InputError::ResourceLimit.into());
    }
    Ok(())
}
