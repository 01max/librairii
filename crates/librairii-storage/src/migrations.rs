//! The existing Go SQL is the persistent format; Rust does not introduce a lineage.
//!
//! Recognition is read-only. A supported prefix is not permission to mutate a
//! library: the database owner must separately retain its library authority,
//! establish a verified backup and provide the worker/transaction lifetime.
use crate::{Result, StorageError};
use librairii_core::collection_codec;
use rusqlite::{Connection, OptionalExtension};

// Only the explicit same-writer Collection inspection below delegates to the
// canonical pure core codec. Schema-only open never rewrites payloads.

/// Storage-local transitional inspection result. Raw legacy text bytes are
/// preserved; this is not a shared domain model or a persistence serialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityCollection {
    pub id: i64,
    pub name: Vec<u8>,
    pub normalized_name: Vec<u8>,
    pub position: i64,
    pub query_version: i64,
    pub query_payload: Vec<u8>,
    pub validity: CollectionValidity,
    pub revision: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionValidity {
    Valid,
    NeedsAttention,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionAttention {
    UnmigratableQuery,
    MissingCriteria,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityInspection {
    pub collection: CompatibilityCollection,
    /// The canonical decoded query, present even if an already-attention
    /// Collection remains unavailable. Missing/unmigratable criteria return no
    /// evaluated query. The canonical core codec supplies this owned wire
    /// value; live reference validation remains in this same transaction.
    pub decoded_query_payload: Option<String>,
    pub attention: Option<CollectionAttention>,
}
impl CompatibilityInspection {
    /// Call AFTER a proven committed inspection acknowledgement. Go Open first
    /// commits Inspect, then reports needs-attention; treating that domain
    /// result as a transaction error would incorrectly roll back its mutation.
    pub const fn is_openable(&self) -> bool {
        matches!(self.collection.validity, CollectionValidity::Valid)
    }
}
impl crate::database::Database<'_> {
    /// Explicit lazy v1–v5 compatibility projection on THIS runtime's same
    /// writer, using its ordinary admission/cancellation/commit-uncertainty
    /// machinery. No migration or projection runs merely from Database::open.
    /// The closure performs bounded DB/codec work only, without I/O callbacks.
    pub fn inspect_collection_compatibility(
        &self,
        id: i64,
        cancellation: crate::database::WorkCancellation,
    ) -> Result<
        crate::database::Pending<'_, crate::database::TransactionOutcome<CompatibilityInspection>>,
    > {
        self.transaction(cancellation, move |tx| inspect_collection(tx, id))
    }
}

fn sql_bytes(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<Vec<u8>> {
    match row.get_ref(index)? {
        rusqlite::types::ValueRef::Text(bytes) | rusqlite::types::ValueRef::Blob(bytes) => {
            Ok(bytes.to_vec())
        }
        _ => Err(rusqlite::Error::InvalidColumnType(
            index,
            "legacy text".into(),
            row.get_ref(index)?.data_type(),
        )),
    }
}
fn read_compatibility_collection(
    tx: &rusqlite::Transaction<'_>,
    id: i64,
) -> Result<CompatibilityCollection> {
    let value = tx.query_row("SELECT id,name,normalized_name,position,query_version,query_payload,validity_state,revision FROM collections WHERE id=?", [id], |row| {
        let validity: String = row.get(6)?;
        let validity = match validity.as_str() {
            "valid" => CollectionValidity::Valid,
            "needs_attention" => CollectionValidity::NeedsAttention,
            _ => return Err(rusqlite::Error::InvalidQuery),
        };
        Ok(CompatibilityCollection { id:row.get(0)?, name:sql_bytes(row,1)?, normalized_name:sql_bytes(row,2)?, position:row.get(3)?, query_version:row.get(4)?, query_payload:sql_bytes(row,5)?, validity, revision:row.get(7)? })
    }).optional()?.ok_or(StorageError::CollectionNotFound)?;
    Ok(value)
}

/// Missing/wrong-kind criteria are a DOMAIN result; statement, conversion or
/// driver failures remain SQL errors and must roll back rather than marking
/// attention. Visit branches first, matching the frozen criteria service.
fn available_criteria(
    tx: &rusqlite::Transaction<'_>,
    query: &collection_codec::Query,
) -> Result<bool> {
    for branch in &query.any_of {
        if !available_criteria(tx, branch)? {
            return Ok(false);
        }
    }
    for filter in &query.boolean_filters {
        if !definition_kind(tx, filter.definition_id, b"boolean")? {
            return Ok(false);
        }
    }
    for filter in &query.choice_filters {
        if !definition_kind(tx, filter.definition_id, b"choice")? {
            return Ok(false);
        }
        for id in &filter.value_ids {
            let owner = tx
                .query_row(
                    "SELECT definition_id FROM tag_values WHERE id=?",
                    [id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if owner != Some(filter.definition_id) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
fn definition_kind(tx: &rusqlite::Transaction<'_>, id: i64, expected: &[u8]) -> Result<bool> {
    let kind = tx
        .query_row("SELECT kind FROM tag_definitions WHERE id=?", [id], |row| {
            sql_bytes(row, 0)
        })
        .optional()?;
    Ok(kind.as_deref() == Some(expected))
}
fn attention_inspection(
    tx: &rusqlite::Transaction<'_>,
    mut collection: CompatibilityCollection,
    reason: CollectionAttention,
) -> Result<CompatibilityInspection> {
    if collection.validity == CollectionValidity::Valid {
        let changed = tx.execute("UPDATE collections SET validity_state='needs_attention',updated_at=CURRENT_TIMESTAMP WHERE id=? AND validity_state='valid'", [collection.id])?;
        if changed != 1 {
            return Err(StorageError::CollectionNotFound);
        }
        collection.validity = CollectionValidity::NeedsAttention;
        // Go's returned int64 bookkeeping; the migration016 SQL trigger alone
        // increments persisted revision. Never issue a second revision UPDATE.
        collection.revision = collection.revision.wrapping_add(1);
    }
    Ok(CompatibilityInspection {
        collection,
        decoded_query_payload: None,
        attention: Some(reason),
    })
}
fn inspect_collection(tx: &rusqlite::Transaction<'_>, id: i64) -> Result<CompatibilityInspection> {
    use collection_codec::CodecError;
    let mut collection = read_compatibility_collection(tx, id)?;
    let legacy_id = tx
        .query_row(
            "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let (payload, _) = match collection_codec::migrate(
        collection.query_version,
        &collection.query_payload,
        legacy_id,
    ) {
        Ok(value) => value,
        Err(CodecError::Invalid | CodecError::UnsupportedVersion) => {
            return attention_inspection(tx, collection, CollectionAttention::UnmigratableQuery);
        }
        Err(CodecError::FrozenData) => return Err(StorageError::CompatibilityInvariant),
    };
    let query = match collection_codec::decode(5, payload.as_bytes(), 0) {
        Ok(value) => value,
        Err(CodecError::Invalid | CodecError::UnsupportedVersion) => {
            return attention_inspection(tx, collection, CollectionAttention::UnmigratableQuery);
        }
        Err(CodecError::FrozenData) => return Err(StorageError::CompatibilityInvariant),
    };
    if !available_criteria(tx, &query)? {
        return attention_inspection(tx, collection, CollectionAttention::MissingCriteria);
    }
    let decoded_query_payload =
        Some(collection_codec::payload(&query).map_err(|_| StorageError::CompatibilityInvariant)?);
    if collection.query_version != 5 || collection.query_payload != payload.as_bytes() {
        tx.execute("UPDATE collections SET query_version=?,query_payload=?,updated_at=CURRENT_TIMESTAMP WHERE id=?", rusqlite::params![5,payload,collection.id])?;
        collection.query_version = 5;
        collection.query_payload = payload.into_bytes();
        collection.revision = collection.revision.wrapping_add(1);
    }
    let attention = (collection.validity == CollectionValidity::NeedsAttention)
        .then_some(CollectionAttention::MissingCriteria);
    Ok(CompatibilityInspection {
        collection,
        decoded_query_payload,
        attention,
    })
}

pub const SCHEMA_PRODUCT: &str = "librairii";
pub const SCHEMA_FAMILY: &str = "wails-go-sqlite";
pub const LATEST_VERSION: u32 = 18;

// Preserve the Go-created ledger spelling for newly initialized databases.
// Historical fixtures use another whitespace spelling with the same columns;
// inspection compares ledger structure rather than rewriting either spelling.
pub const LEDGER_SQL: &str = "\n\t\tCREATE TABLE IF NOT EXISTS schema_migrations (\n\t\t\tversion INTEGER PRIMARY KEY,\n\t\t\tapplied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP\n\t\t)\n\t";

/// An unchanged file from the admitted preservation corpus.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Migration {
    pub version: u32,
    pub filename: &'static str,
    pub sql: &'static str,
}

macro_rules! migration {
    ($version:literal, $filename:literal) => {
        Migration {
            version: $version,
            filename: $filename,
            sql: include_str!(concat!("../migrations/", $filename)),
        }
    };
}

pub const MIGRATIONS: [Migration; 18] = [
    migration!(1, "001_foundation.sql"),
    migration!(2, "002_story_catalog.sql"),
    migration!(3, "003_removal_intents.sql"),
    migration!(4, "004_story_tags.sql"),
    migration!(5, "005_protected_broken_tag.sql"),
    migration!(6, "006_normalized_story_names.sql"),
    migration!(7, "007_official_metadata.sql"),
    migration!(8, "008_catalog_artwork.sql"),
    migration!(9, "009_metadata_operation.sql"),
    migration!(10, "010_official_metadata_query.sql"),
    migration!(11, "011_saved_shelves.sql"),
    migration!(12, "012_export_operations.sql"),
    migration!(13, "013_collection_scale.sql"),
    migration!(14, "014_tag_colors.sql"),
    migration!(15, "015_story_broken_flag.sql"),
    migration!(16, "016_saved_collections.sql"),
    migration!(17, "017_curated_shelves.sql"),
    migration!(18, "018_collection_shelf_exports.sql"),
];

/// Exact contiguous ledger, independent of any SQLite connection or transaction.
/// Zero is reserved for an absent database, never an existing identity-less file.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchemaPrefix(u32);

impl SchemaPrefix {
    /// Inspect one coherent read transaction on a storage-owned connection.
    /// The original database is never migrated, repaired or initialized here.
    /// Callers probing unowned/foreign files must first establish an independent
    /// safe probe; SQLite read-only WAL opens can otherwise touch sidecars.
    /// This SQLite-facing mechanism belongs inside storage adapters, never in
    /// shared application/core ports.
    pub fn inspect(connection: &mut Connection) -> Result<Self> {
        let transaction = connection.transaction()?;
        let expected = Connection::open_in_memory()?;
        expected.execute_batch(LEDGER_SQL)?;
        expected.execute_batch(MIGRATIONS[0].sql)?;
        // Validate physical objects BEFORE selecting their rows. A foreign view
        // or virtual table must not get to execute a query under an identity or
        // ledger name. Ledger whitespace may differ between historical writers;
        // compare the complete known DDL without changing its persisted bytes.
        if table_sql(&transaction, "librairii_schema_identity")?
            != table_sql(&expected, "librairii_schema_identity")?
        {
            return Err(StorageError::ForeignSchema);
        }
        let ledger = table_sql(&transaction, "schema_migrations")?;
        let expected_ledger = table_sql(&expected, "schema_migrations")?;
        let regular_ledger = match (ledger.as_deref(), expected_ledger.as_deref()) {
            (Some(actual), Some(expected)) => actual
                .chars()
                .filter(|c| !c.is_ascii_whitespace())
                .eq(expected.chars().filter(|c| !c.is_ascii_whitespace())),
            _ => false,
        };
        if !regular_ledger {
            return Err(StorageError::MigrationConflict);
        }
        let identity = transaction
            .prepare(
                "SELECT singleton,product,schema_family FROM librairii_schema_identity LIMIT 2",
            )
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .map_err(|_| StorageError::ForeignSchema)?;
        let [(1, product, family)] = identity.as_slice() else {
            return Err(StorageError::ForeignSchema);
        };
        let versions = transaction
            .prepare("SELECT version FROM schema_migrations ORDER BY version LIMIT 19")
            .and_then(|mut statement| {
                statement
                    .query_map([], |row| row.get::<_, i64>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()
            })
            .map_err(|_| StorageError::MigrationConflict)?;
        let prefix = Self::recognize(product, family, &versions)?;
        for migration in MIGRATIONS.iter().take(versions.len()).skip(1) {
            expected.execute_batch(migration.sql)?;
        }
        if schema(&transaction)? != schema(&expected)?
            || ledger_columns(&transaction)? != ledger_columns(&expected)?
        {
            return Err(StorageError::MigrationConflict);
        }
        transaction.commit()?;
        Ok(prefix)
    }

    /// Recognize an existing schema's persisted identity and ordered ledger.
    /// Duplicates, missing versions, future versions and an empty existing
    /// ledger all fail; input strings/rows are never repaired or rewritten.
    pub fn recognize(product: &str, family: &str, versions: &[i64]) -> Result<Self> {
        if product != SCHEMA_PRODUCT || family != SCHEMA_FAMILY {
            return Err(StorageError::ForeignSchema);
        }
        if versions.is_empty() || versions.len() > MIGRATIONS.len() {
            return Err(StorageError::MigrationConflict);
        }
        for (actual, expected) in versions.iter().zip(MIGRATIONS.iter()) {
            if *actual != i64::from(expected.version) {
                return Err(StorageError::MigrationConflict);
            }
        }
        // The last expected version supplies the value without a lossy cast.
        MIGRATIONS
            .get(versions.len() - 1)
            .map(|migration| Self(migration.version))
            .ok_or(StorageError::MigrationConflict)
    }

    pub const fn version(self) -> u32 {
        self.0
    }

    pub fn pending(self) -> impl Iterator<Item = &'static Migration> {
        MIGRATIONS
            .iter()
            .filter(move |migration| migration.version > self.0)
    }
}

fn table_sql(connection: &Connection, name: &str) -> Result<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?",
            [name],
            |row| row.get(0),
        )
        .optional()?)
}

type SchemaEntry = (String, String, String, String);
fn schema(connection: &Connection) -> Result<Vec<SchemaEntry>> {
    let mut statement = connection.prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema WHERE substr(name,1,7) <> 'sqlite_' AND name <> 'schema_migrations' ORDER BY type,name")?;
    Ok(statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}

type LedgerColumn = (i64, String, String, i64, Option<String>, i64);
fn ledger_columns(connection: &Connection) -> Result<Vec<LedgerColumn>> {
    let mut statement = connection.prepare("PRAGMA table_info(schema_migrations)")?;
    Ok(statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?)
}
