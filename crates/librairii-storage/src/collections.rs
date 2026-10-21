//! Collection commands run on the existing owned SQLite writer, never on a UI thread.
use crate::{
    Result, StorageError,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
    migrations,
};
use librairii_core::{
    collection_codec::{self, Query},
    collections::{Attention, Collection, CollectionError, Inspection, Validity, normalize_name},
};
use rusqlite::{OptionalExtension, Transaction, params};
#[derive(Debug)]
pub enum Command {
    Create { name: String, criteria: Query },
    Rename { id: i64, name: String },
    Duplicate { id: i64, name: String },
    Reorder(Vec<i64>),
    Delete { id: i64 },
    Inspect { id: i64 },
}
#[derive(Debug)]
pub enum Response {
    Collection(Collection),
    Collections(Vec<Collection>),
    Inspection(Inspection),
    NeedsAttention(Inspection),
    Deleted,
}
impl Database<'_> {
    pub fn edit_collections(
        &self,
        command: Command,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Response>>> {
        self.transaction(cancellation, move |tx| execute(tx, command))
    }
    pub fn collections(&self) -> Result<Pending<'_, Vec<Collection>>> {
        self.read(list)
    }
}
fn from_compat(c: migrations::CompatibilityCollection) -> Collection {
    Collection {
        id: c.id,
        name: c.name,
        normalized_name: c.normalized_name,
        position: c.position,
        query_version: c.query_version,
        query_payload: c.query_payload,
        validity: match c.validity {
            migrations::CollectionValidity::Valid => Validity::Valid,
            migrations::CollectionValidity::NeedsAttention => Validity::NeedsAttention,
        },
        revision: c.revision,
    }
}
pub fn read(tx: &Transaction<'_>, id: i64) -> Result<Collection> {
    migrations::read_compatibility_collection(tx, id).map(from_compat)
}
pub fn list(tx: &Transaction<'_>) -> Result<Vec<Collection>> {
    let mut stmt = tx.prepare("SELECT id FROM collections ORDER BY position,id")?;
    let ids = stmt
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ids.into_iter().map(|id| read(tx, id)).collect()
}
pub fn inspect(tx: &Transaction<'_>, id: i64) -> Result<Inspection> {
    let (inspection, query) = migrations::inspect_collection_with_query(tx, id)?;
    Ok(Inspection {
        collection: from_compat(inspection.collection),
        query,
        attention: inspection.attention.map(|a| match a {
            migrations::CollectionAttention::MissingCriteria => Attention::MissingCriteria,
            migrations::CollectionAttention::UnmigratableQuery => Attention::UnmigratableQuery,
        }),
    })
}
fn codec(error: collection_codec::CodecError) -> StorageError {
    match error {
        collection_codec::CodecError::FrozenData => StorageError::CompatibilityInvariant,
        _ => CollectionError::InvalidQuery.into(),
    }
}
fn canonical(tx: &Transaction<'_>, criteria: Query) -> Result<String> {
    let payload = collection_codec::encode(criteria).map_err(codec)?;
    let validated = collection_codec::decode(5, payload.as_bytes(), 0).map_err(codec)?;
    if !migrations::available_criteria(tx, &validated)? {
        return Err(CollectionError::CriteriaUnavailable.into());
    }
    Ok(payload)
}
fn duplicate_name(e: rusqlite::Error) -> StorageError {
    match &e {
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 2067 => {
            CollectionError::DuplicateName.into()
        }
        _ => e.into(),
    }
}
fn insert(
    tx: &Transaction<'_>,
    name: String,
    normalized: String,
    payload: String,
    validity: Validity,
) -> Result<Collection> {
    let position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position),-1)+1 FROM collections",
        [],
        |r| r.get(0),
    )?;
    tx.execute("INSERT INTO collections(name,normalized_name,position,query_version,query_payload,validity_state) VALUES(?,?,?,5,?,?)",params![name,normalized,position,payload,match validity{Validity::Valid=>"valid",Validity::NeedsAttention=>"needs_attention"}]).map_err(duplicate_name)?;
    read(tx, tx.last_insert_rowid())
}
fn reorder(tx: &Transaction<'_>, ids: &[i64]) -> Result<Vec<Collection>> {
    let current = list(tx)?;
    let mut expected = current.iter().map(|c| c.id).collect::<Vec<_>>();
    expected.sort_unstable();
    let mut actual = ids.to_vec();
    actual.sort_unstable();
    if expected != actual || actual.windows(2).any(|w| w[0] == w[1]) {
        return Err(CollectionError::InvalidOrder.into());
    }
    if !ids.is_empty() {
        let max = current.iter().map(|c| c.position).max().unwrap_or(-1);
        let count = i64::try_from(ids.len()).map_err(|_| CollectionError::InvalidOrder)?;
        let offset = max
            .checked_add(count)
            .and_then(|v| v.checked_add(1))
            .ok_or(CollectionError::InvalidOrder)?;
        max.checked_add(offset)
            .ok_or(CollectionError::InvalidOrder)?;
        tx.execute(
            "UPDATE collections SET position=position+?,updated_at=CURRENT_TIMESTAMP",
            [offset],
        )?;
        for (p, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE collections SET position=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",
                params![p as i64, id],
            )?;
        }
    }
    list(tx)
}
pub fn execute(tx: &Transaction<'_>, command: Command) -> Result<Response> {
    match command {
        Command::Create { name, criteria } => {
            let (name, normalized) = normalize_name(&name)?;
            let payload = canonical(tx, criteria)?;
            Ok(Response::Collection(insert(
                tx,
                name,
                normalized,
                payload,
                Validity::Valid,
            )?))
        }
        Command::Rename { id, name } => {
            let (name, normalized) = normalize_name(&name)?;
            read(tx, id)?;
            tx.execute("UPDATE collections SET name=?,normalized_name=?,updated_at=CURRENT_TIMESTAMP WHERE id=?",params![name,normalized,id]).map_err(duplicate_name)?;
            Ok(Response::Collection(read(tx, id)?))
        }
        Command::Reorder(ids) => Ok(Response::Collections(reorder(tx, &ids)?)),
        Command::Inspect { id } => Ok(Response::Inspection(inspect(tx, id)?)),
        Command::Duplicate { id, name } => {
            let (name, normalized) = normalize_name(&name)?;
            let inspection = inspect(tx, id)?;
            if !inspection.is_openable() {
                return Ok(Response::NeedsAttention(inspection));
            }
            let c = inspection.collection;
            let legacy = tx
                .query_row(
                    "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .optional()?
                .unwrap_or(0);
            let (payload, _) = collection_codec::migrate(c.query_version, &c.query_payload, legacy)
                .map_err(codec)?;
            Ok(Response::Collection(insert(
                tx, name, normalized, payload, c.validity,
            )?))
        }
        Command::Delete { id } => {
            read(tx, id)?;
            let active: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM shelf_sources WHERE collection_id=? AND active=1)",
                [id],
                |r| r.get(0),
            )?;
            if active {
                return Err(CollectionError::InUse.into());
            }
            tx.execute(
                "UPDATE shelf_sources SET collection_id=NULL WHERE collection_id=? AND active=0",
                [id],
            )?;
            tx.execute("DELETE FROM collections WHERE id=?", [id])?;
            Ok(Response::Deleted)
        }
    }
}

/// Domain refusal is committed as data so lazy attention/migration updates are
/// not rolled back. Operational failures still fail the enclosing transaction.
#[derive(Debug)]
pub enum Observation<T> {
    Ready(T),
    NeedsAttention(Inspection),
}
#[derive(Debug)]
pub struct Evaluation {
    pub collection: Collection,
    pub criteria: Query,
    pub page: librairii_core::browsing::Page,
}
#[derive(Debug)]
pub struct Summary {
    pub inspection: Inspection,
    pub count: i64,
}
impl Database<'_> {
    pub fn evaluate_collection(
        &self,
        id: i64,
        request: librairii_core::query::QueryRequest,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Observation<Evaluation>>>> {
        self.transaction(cancellation, move |tx| evaluate(tx, id, request, &locale))
    }
    pub fn collection_summaries(
        &self,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Vec<Summary>>>> {
        self.transaction(cancellation, move |tx| summaries(tx, &locale))
    }
}
/// Inspect/migrate and evaluate in one writer snapshot. Request navigation is
/// retained; membership always comes from the saved Collection definition.
pub fn evaluate(
    tx: &Transaction<'_>,
    id: i64,
    mut request: librairii_core::query::QueryRequest,
    locale: &str,
) -> Result<Observation<Evaluation>> {
    let inspection = inspect(tx, id)?;
    if !inspection.is_openable() {
        return Ok(Observation::NeedsAttention(inspection));
    }
    let criteria = inspection
        .query
        .ok_or(StorageError::CompatibilityInvariant)?;
    request.criteria = criteria.clone();
    let page = crate::query::search(tx, request, locale)?;
    Ok(Observation::Ready(Evaluation {
        collection: inspection.collection,
        criteria,
        page,
    }))
}
pub fn summaries(tx: &Transaction<'_>, locale: &str) -> Result<Vec<Summary>> {
    list(tx)?
        .into_iter()
        .map(|c| {
            let inspection = inspect(tx, c.id)?;
            let count = if inspection.is_openable() {
                // Match the source Summaries -> Count -> Open path, including its
                // second saved-query pass (normalization is not idempotent).
                let opened = inspect(tx, c.id)?;
                crate::query::count(
                    tx,
                    librairii_core::query::QueryRequest {
                        criteria: opened.query.ok_or(StorageError::CompatibilityInvariant)?,
                        ..Default::default()
                    },
                    locale,
                )?
            } else {
                0
            };
            Ok(Summary { inspection, count })
        })
        .collect()
}
/// Read-only participant for reviewed Shelf/export snapshots. This deliberately
/// decodes stored bytes directly, without migrating, healing or marking rows.
/// A later mutation must call it again within its own writer transaction.
pub fn source(tx: &Transaction<'_>, id: i64) -> Result<(Collection, Query)> {
    let c = read(tx, id)?;
    if c.validity != Validity::Valid {
        return Err(CollectionError::NeedsAttention.into());
    }
    let legacy = tx
        .query_row(
            "SELECT definition_id FROM legacy_broken_tag WHERE singleton=1",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    let q = collection_codec::decode(c.query_version, &c.query_payload, legacy).map_err(codec)?;
    if !migrations::available_criteria(tx, &q)? {
        return Err(CollectionError::CriteriaUnavailable.into());
    }
    Ok((c, q))
}

#[derive(Debug)]
pub struct SelectedSource {
    pub collection: Collection,
    pub count: i64,
}
#[derive(Debug)]
pub struct SelectionPreview {
    pub sources: Vec<SelectedSource>,
    pub unique_count: i64,
    pub overlap_count: i64,
}
fn validate_selection(ids: &[i64]) -> Result<()> {
    let mut seen = std::collections::BTreeSet::new();
    if ids.is_empty() || ids.iter().any(|id| *id <= 0 || !seen.insert(*id)) {
        return Err(CollectionError::InvalidSelection.into());
    }
    Ok(())
}
fn selected(tx: &Transaction<'_>, ids: &[i64]) -> Result<Observation<Vec<(Collection, Query)>>> {
    validate_selection(ids)?;
    let mut sources = Vec::with_capacity(ids.len());
    for id in ids {
        let inspection = inspect(tx, *id)?;
        if !inspection.is_openable() {
            return Ok(Observation::NeedsAttention(inspection));
        }
        sources.push((
            inspection.collection,
            inspection
                .query
                .ok_or(StorageError::CompatibilityInvariant)?,
        ));
    }
    Ok(Observation::Ready(sources))
}
pub fn preview_selection(
    tx: &Transaction<'_>,
    ids: &[i64],
    locale: &str,
) -> Result<Observation<SelectionPreview>> {
    let selected = match selected(tx, ids)? {
        Observation::Ready(s) => s,
        Observation::NeedsAttention(i) => return Ok(Observation::NeedsAttention(i)),
    };
    let criteria = selected.iter().map(|(_, q)| q.clone()).collect();
    let unique_count = crate::query::count_union(tx, criteria, locale)?;
    let mut sources = Vec::with_capacity(selected.len());
    let mut total = 0_i64;
    for (collection, q) in selected {
        let count = crate::query::count(
            tx,
            librairii_core::query::QueryRequest {
                criteria: q,
                ..Default::default()
            },
            locale,
        )?;
        total = total
            .checked_add(count)
            .ok_or(librairii_core::query::QueryInputError::ResourceOverflow)?;
        sources.push(SelectedSource { collection, count });
    }
    Ok(Observation::Ready(SelectionPreview {
        sources,
        unique_count,
        overlap_count: total - unique_count,
    }))
}
/// Copy current definitions in one transaction, never source Collection IDs.
/// Subsequent edits/deletion of an original cannot change this saved query.
pub fn save_combined(
    tx: &Transaction<'_>,
    ids: &[i64],
    name: &str,
) -> Result<Observation<Collection>> {
    let (name, normalized) = normalize_name(name)?;
    let sources = match selected(tx, ids)? {
        Observation::Ready(s) => s,
        Observation::NeedsAttention(i) => return Ok(Observation::NeedsAttention(i)),
    };
    let mut q = Query::default();
    q.any_of = sources.into_iter().map(|(_, q)| q).collect();
    let payload = canonical(tx, q)?;
    Ok(Observation::Ready(insert(
        tx,
        name,
        normalized,
        payload,
        Validity::Valid,
    )?))
}
impl Database<'_> {
    pub fn collection_selection_preview(
        &self,
        ids: Vec<i64>,
        locale: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Observation<SelectionPreview>>>> {
        self.transaction(cancellation, move |tx| preview_selection(tx, &ids, &locale))
    }
    pub fn save_combined_collection(
        &self,
        ids: Vec<i64>,
        name: String,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Observation<Collection>>>> {
        self.transaction(cancellation, move |tx| save_combined(tx, &ids, &name))
    }
}
