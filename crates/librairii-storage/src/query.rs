//! Frozen query semantics on a single caller-owned SQLite snapshot.
//!
//! SQL work stays on the existing database workers. Values are bound parameters;
//! only fixed predicates/order clauses are composed into SQL. A result is data,
//! never authority to export or mutate a story.
use crate::{
    Result, StorageError,
    database::{Database, Pending},
};
use librairii_core::{
    browsing::{DisplaySources, ExportStory, OfficialInfo, Page, StorySummary},
    collection_codec::Query,
    query::{QueryInputError, QueryRequest, normalize_request},
};
use rusqlite::{OptionalExtension, Transaction, params, params_from_iter, types::Value};

const FROM: &str = " FROM stories s JOIN story_archives a ON a.story_id=s.id";
// Exact matching/sorting expression from frozen story_query.go. In particular,
// name matching uses raw UUID spelling; official display lookup canonicalizes it.
const DISPLAY_NAME: &str = "COALESCE(NULLIF((SELECT m.title_normalized FROM official_story_metadata m JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE m.story_uuid=s.uuid AND c.status='active' AND c.locale=? ORDER BY c.activated_at DESC,c.id DESC LIMIT 1),''),s.display_name_normalized)";
const RECORD: &str = "SELECT s.id,s.uuid,COALESCE(s.embedded_title,''),COALESCE(s.embedded_description,''),COALESCE(s.embedded_artwork_path,''),s.created_at,a.original_filename,a.detected_format,a.sha256,a.byte_size,a.managed_path,a.validation_state,s.is_broken";

fn invalid() -> StorageError {
    QueryInputError::InvalidQuery.into()
}
fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}
pub(crate) fn require_identities(
    tx: &Transaction<'_>,
    identities: &[librairii_core::browsing::StoryIdentity],
) -> Result<()> {
    if identities.is_empty() || identities.len() > 1000 {
        return Err(StorageError::SnapshotChanged);
    }
    for story in identities {
        if story.id <= 0 || !tx.query_row("SELECT EXISTS(SELECT 1 FROM stories WHERE id=? AND uuid=? AND NOT EXISTS(SELECT 1 FROM removal_intents WHERE story_id=?))", params![story.id, story.uuid, story.id], |row| row.get::<_, bool>(0))? {
            return Err(StorageError::SnapshotChanged);
        }
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct StoryDetail {
    pub summary: StorySummary,
    pub archive: ExportStory,
}

impl Database<'_> {
    /// One coherent observation, with lossless native filename bytes retained.
    pub fn story_detail(
        &self,
        id: i64,
        locale: String,
    ) -> Result<Pending<'_, Option<StoryDetail>>> {
        self.read(move |tx| {
            let record = tx
                .query_row(&format!("{RECORD}{FROM} WHERE s.id=?"), [id], Record::read)
                .optional()?;
            record
                .map(|record| {
                    let summary = summary(tx, &record, &locale)?;
                    let archive = ExportStory {
                        broken: record.broken,
                        id: record.id,
                        uuid: record.uuid,
                        title: summary.title.clone(),
                        original_filename: String::from_utf8(record.filename.clone())
                            .unwrap_or_default(),
                        original_filename_bytes: record.filename,
                        detected_format: record.format,
                        sha256: record.checksum,
                        byte_size: record.bytes,
                        managed_relative_path: String::from_utf8(record.path.clone())
                            .unwrap_or_default(),
                        managed_relative_path_bytes: record.path,
                        verification: summary.compatibility.clone(),
                    };
                    Ok(StoryDetail { summary, archive })
                })
                .transpose()
        })
    }
    /// Facet choices observe active metadata independently of the display locale.
    pub fn available_languages(&self) -> Result<Pending<'_, Vec<String>>> {
        self.read(|tx| {
            let mut statement=tx.prepare("SELECT DISTINCT m.language FROM official_story_metadata m JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE c.status='active' AND m.language<>'' ORDER BY m.language")?;
            let rows=statement.query_map([],|row|row.get::<_,String>(0))?.collect::<std::result::Result<Vec<_>,_>>()?;
            Ok(rows)
        })
    }
    pub fn search(&self, request: QueryRequest, locale: String) -> Result<Pending<'_, Page>> {
        self.read(move |tx| search(tx, request, &locale))
    }
    pub fn count(&self, request: QueryRequest, locale: String) -> Result<Pending<'_, i64>> {
        self.read(move |tx| count(tx, request, &locale))
    }
}

/// Validate against this exact transaction, including ignored Boolean filters
/// and value ownership in every branch. Missing references are never omitted.
fn validate_references(tx: &Transaction<'_>, query: &Query) -> Result<()> {
    for branch in &query.any_of {
        validate_references(tx, branch)?;
    }
    for filter in &query.boolean_filters {
        let kind: Option<String> = tx
            .query_row(
                "SELECT kind FROM tag_definitions WHERE id=?",
                [filter.definition_id],
                |r| r.get(0),
            )
            .optional()?;
        if kind.as_deref() != Some("boolean") {
            return Err(invalid());
        }
    }
    for filter in &query.choice_filters {
        let kind: Option<String> = tx
            .query_row(
                "SELECT kind FROM tag_definitions WHERE id=?",
                [filter.definition_id],
                |r| r.get(0),
            )
            .optional()?;
        if kind.as_deref() != Some("choice") {
            return Err(invalid());
        }
        for value in &filter.value_ids {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM tag_values WHERE id=? AND definition_id=?)",
                params![value, filter.definition_id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

fn list(values: impl serde::Serialize) -> Result<Value> {
    serde_json::to_string(&values)
        .map(Value::Text)
        .map_err(|_| invalid())
}

fn predicate(q: &Query, locale: &str, args: &mut Vec<Value>) -> Result<String> {
    let mut parts = Vec::new();
    if !q.name.is_empty() {
        parts.push(format!("instr({DISPLAY_NAME},?)>0"));
        args.extend([text(locale), text(&q.name)]);
    }
    if !q.languages().is_empty() {
        parts.push("EXISTS(SELECT 1 FROM official_story_metadata m JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE m.story_uuid=s.uuid AND c.status='active' AND m.language IN (SELECT value FROM json_each(?)))".into());
        args.push(list(q.languages())?);
    }
    if !q.compatibilities().is_empty() {
        parts.push("a.validation_state IN (SELECT value FROM json_each(?))".into());
        let states: Vec<&str> = q
            .compatibilities()
            .iter()
            .map(|v| match v.as_str() {
                "compatible" => "valid",
                "missing" => "missing",
                _ => "invalid",
            })
            .collect();
        args.push(list(states)?);
    }
    for f in &q.boolean_filters {
        if f.state == "ignored" {
            continue;
        }
        let prefix = if f.state == "false" { "NOT " } else { "" };
        parts.push(format!("{prefix}EXISTS(SELECT 1 FROM story_tag_assignments t WHERE t.story_id=s.id AND t.definition_id=? AND t.value_id IS NULL)"));
        args.push(Value::Integer(f.definition_id));
    }
    for f in &q.choice_filters {
        parts.push("EXISTS(SELECT 1 FROM story_tag_assignments t WHERE t.story_id=s.id AND t.definition_id=? AND t.value_id IS NOT NULL AND t.value_id IN (SELECT CAST(value AS INTEGER) FROM json_each(?)))".into());
        args.extend([Value::Integer(f.definition_id), list(&f.value_ids)?]);
    }
    if !q.any_of.is_empty() {
        let mut branches = Vec::new();
        for branch in &q.any_of {
            let p = predicate(branch, locale, args)?;
            branches.push(format!("({})", if p.is_empty() { "1" } else { &p }));
        }
        parts.push(format!("({})", branches.join(" OR ")));
    }
    Ok(parts.join(" AND "))
}

fn matching(
    tx: &Transaction<'_>,
    request: QueryRequest,
    locale: &str,
) -> Result<(QueryRequest, String, Vec<Value>)> {
    let request = normalize_request(request)?;
    validate_references(tx, &request.criteria)?;
    let mut args = Vec::new();
    let predicate = predicate(&request.criteria, locale, &mut args)?;
    let from = if predicate.is_empty() {
        FROM.to_owned()
    } else {
        format!("{FROM} WHERE {predicate}")
    };
    Ok((request, from, args))
}

fn ordering(request: &QueryRequest, locale: &str, args: &mut Vec<Value>) -> String {
    if request.sort == "imported_desc" {
        " ORDER BY s.created_at DESC,s.id DESC".into()
    } else {
        args.push(text(locale));
        format!(" ORDER BY {DISPLAY_NAME},s.uuid,s.id")
    }
}

pub fn count(tx: &Transaction<'_>, request: QueryRequest, locale: &str) -> Result<i64> {
    let (_, from, args) = matching(tx, request, locale)?;
    Ok(tx.query_row(
        &format!("SELECT COUNT(*){from}"),
        params_from_iter(args),
        |r| r.get(0),
    )?)
}

pub fn search(tx: &Transaction<'_>, request: QueryRequest, locale: &str) -> Result<Page> {
    let (request, from, mut args) = matching(tx, request, locale)?;
    let total: i64 = tx.query_row(
        &format!("SELECT COUNT(*){from}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;
    let total_pages = total / request.page_size + i64::from(total % request.page_size != 0);
    let mut stories = Vec::new();
    // Huge valid page numbers are empty pages, not integer-wrap offsets.
    if request.page <= total_pages {
        let order = ordering(&request, locale, &mut args);
        args.extend([
            Value::Integer(request.page_size),
            Value::Integer((request.page - 1) * request.page_size),
        ]);
        let mut statement = tx.prepare(&format!("{RECORD}{from}{order} LIMIT ? OFFSET ?"))?;
        let mut rows = statement.query(params_from_iter(args))?;
        while let Some(row) = rows.next()? {
            stories.push(summary(tx, &Record::read(row)?, locale)?);
        }
    }
    Ok(Page {
        stories,
        page: request.page,
        page_size: request.page_size,
        total_items: total,
        total_pages,
        sort: request.sort,
    })
}

#[derive(Debug)]
struct Record {
    id: i64,
    uuid: String,
    title: String,
    description: String,
    artwork: String,
    created: String,
    filename: Vec<u8>,
    checksum: String,
    path: Vec<u8>,
    format: String,
    bytes: i64,
    validation: String,
    broken: bool,
}
impl Record {
    fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            uuid: r.get(1)?,
            title: r.get(2)?,
            description: r.get(3)?,
            artwork: r.get(4)?,
            created: r.get(5)?,
            filename: r.get_ref(6)?.as_bytes()?.to_vec(),
            checksum: r.get(8)?,
            path: r.get_ref(10)?.as_bytes()?.to_vec(),
            format: r.get(7)?,
            bytes: r.get(9)?,
            validation: r.get(11)?,
            broken: r.get(12)?,
        })
    }
}

fn resolved(official: &str, embedded: &str, fallback: String) -> (String, String) {
    if !official.trim().is_empty() {
        (official.trim().into(), "official".into())
    } else if !embedded.trim().is_empty() {
        (embedded.trim().into(), "embedded".into())
    } else {
        (fallback, "fallback".into())
    }
}

fn summary(tx: &Transaction<'_>, r: &Record, locale: &str) -> Result<StorySummary> {
    let uuid = r
        .uuid
        .parse::<librairii_core::StoryUuid>()
        .map_err(|_| invalid())?
        .to_string();
    let official=tx.query_row("SELECT COALESCE(m.title,''),COALESCE(m.description,''),COALESCE(m.author,''),COALESCE(m.artwork_id,''),m.locale,COALESCE(m.publisher,''),COALESCE(m.language,''),m.duration_seconds,m.minimum_age,m.maximum_age,m.provenance,COALESCE(m.source_record_id,''),COALESCE(m.source_updated_at,''),c.fetched_at,c.activated_at FROM official_story_metadata m JOIN catalog_snapshots c ON c.id=m.snapshot_id WHERE m.story_uuid=? AND c.locale=? AND c.status='active'",params![uuid,locale],|row| {
        Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?,row.get::<_,String>(3)?,OfficialInfo {locale:row.get(4)?,publisher:row.get(5)?,language:row.get(6)?,duration_seconds:row.get(7)?,minimum_age:row.get(8)?,maximum_age:row.get(9)?,provenance:row.get(10)?,source_record_id:row.get(11)?,source_updated_at:row.get(12)?,fetched_at:row.get(13)?,activated_at:row.get(14)?}))
    }).optional()?;
    let (ot, od, oa, oi, info) = official.map_or_else(
        || {
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                None,
            )
        },
        |(t, d, a, i, info)| (t, d, a, i, Some(info)),
    );
    let (title, ts) = resolved(&ot, &r.title, format!("Story {}", r.uuid));
    let (description, ds) = resolved(&od, &r.description, String::new());
    let (author, aus) = resolved(&oa, "", String::new());
    let (artwork_id, ars) = if !oi.trim().is_empty() {
        (oi.trim().to_owned(), "official".into())
    } else if !r.artwork.is_empty() {
        (format!("embedded:{}", r.id), "embedded".into())
    } else {
        (String::new(), "fallback".into())
    };
    let (compatibility, reason) = match r.validation.as_str() {
        "valid" => ("compatible", ""),
        "missing" => ("missing", "Managed archive bytes are missing."),
        _ => ("invalid", "Managed archive verification failed."),
    };
    Ok(StorySummary {
        broken: r.broken,
        id: r.id,
        uuid: r.uuid.clone(),
        title,
        description,
        author,
        artwork_id,
        sources: DisplaySources {
            title: ts,
            description: ds,
            author: aus,
            artwork: ars,
        },
        detected_format: r.format.clone(),
        compatibility: compatibility.into(),
        compatibility_reason: reason.into(),
        byte_size: r.bytes,
        imported_at: r.created.clone(),
        official: info,
    })
}

/// Query-plan diagnostics for the same validated predicate and order as search.
pub fn explain_search(
    tx: &Transaction<'_>,
    request: QueryRequest,
    locale: &str,
) -> Result<Vec<String>> {
    let (request, from, mut args) = matching(tx, request, locale)?;
    let order = ordering(&request, locale, &mut args);
    let mut statement = tx.prepare(&format!(
        "EXPLAIN QUERY PLAN {RECORD}{from}{order} LIMIT 100"
    ))?;
    Ok(statement
        .query_map(params_from_iter(args), |r| r.get(3))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Current canonical membership after version migration and same-snapshot live
/// validation. This is an observation; later mutations must validate again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedMembership {
    pub version: i64,
    pub payload: String,
    pub criteria: Query,
}

pub fn saved_membership(
    tx: &Transaction<'_>,
    version: i64,
    payload: &[u8],
    legacy_broken_id: i64,
) -> Result<SavedMembership> {
    use librairii_core::collection_codec::{self, CodecError};
    let (payload, criteria) = collection_codec::migrate(version, payload, legacy_broken_id)
        .map_err(|e| match e {
            CodecError::FrozenData => StorageError::CompatibilityInvariant,
            _ => invalid(),
        })?;
    validate_references(tx, &criteria)?;
    Ok(SavedMembership {
        version: 5,
        payload,
        criteria,
    })
}

/// Resolve the complete ordered result in this one SQLite transaction, ignoring
/// navigation limits. The callback must do bounded synchronous, DB-only work;
/// it must not queue work or perform filesystem/network I/O under the snapshot.
///
/// Storage retains one row at a time, without building an uncapped full-scope
/// Vec or replaying page offsets. The caller owns any retained output bytes and
/// must account for them. A callback error stops immediately; no complete result
/// or mutation authority is returned for partial consumption.
pub fn visit_scope(
    tx: &Transaction<'_>,
    request: QueryRequest,
    locale: &str,
    mut consume: impl FnMut(ExportStory) -> Result<()>,
) -> Result<u64> {
    let (request, from, mut args) = matching(tx, request, locale)?;
    let order = ordering(&request, locale, &mut args);
    let mut statement = tx.prepare(&format!("{RECORD}{from}{order}"))?;
    let mut rows = statement.query(params_from_iter(args))?;
    let mut count = 0_u64;
    while let Some(row) = rows.next()? {
        let record = Record::read(row)?;
        let display = summary(tx, &record, locale)?;
        consume(ExportStory {
            broken: record.broken,
            id: record.id,
            uuid: record.uuid,
            title: display.title,
            original_filename: String::from_utf8(record.filename.clone()).unwrap_or_default(),
            original_filename_bytes: record.filename,
            detected_format: record.format,
            sha256: record.checksum,
            byte_size: record.bytes,
            managed_relative_path: String::from_utf8(record.path.clone()).unwrap_or_default(),
            managed_relative_path_bytes: record.path,
            verification: display.compatibility,
        })?;
        count = count
            .checked_add(1)
            .ok_or(QueryInputError::ResourceOverflow)?;
    }
    Ok(count)
}

/// Count a union without replaying pages or retaining every matching story ID.
/// Each source gets exactly one runtime pass and live validation in this same
/// transaction. One ordered cursor per source keeps only its next ID in a heap;
/// neither compound SELECT limits nor aggregate SQL parameter limits cap the
/// selection. Retained SQL/bindings/cursors scale with selected definitions,
/// never with their complete story membership.
pub fn count_union(tx: &Transaction<'_>, criteria: Vec<Query>, locale: &str) -> Result<i64> {
    use std::{cmp::Reverse, collections::BinaryHeap};
    if criteria.is_empty() {
        return Err(invalid());
    }
    let mut queries = Vec::with_capacity(criteria.len());
    for q in criteria {
        let (_, from, args) = matching(
            tx,
            QueryRequest {
                criteria: q,
                ..Default::default()
            },
            locale,
        )?;
        queries.push((format!("SELECT s.id{from} ORDER BY s.id"), args));
    }
    let mut statements = queries
        .iter()
        .map(|(sql, _)| tx.prepare(sql))
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut cursors = statements
        .iter_mut()
        .zip(&queries)
        .map(|(stmt, (_, args))| stmt.query(params_from_iter(args)))
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut next = BinaryHeap::new();
    for (source, cursor) in cursors.iter_mut().enumerate() {
        if let Some(row) = cursor.next()? {
            next.push(Reverse((row.get::<_, i64>(0)?, source)));
        }
    }
    let mut previous = None;
    let mut count = 0_i64;
    while let Some(Reverse((id, source))) = next.pop() {
        if previous != Some(id) {
            count = count
                .checked_add(1)
                .ok_or(QueryInputError::ResourceOverflow)?;
            previous = Some(id);
        }
        if let Some(row) = cursors[source].next()? {
            next.push(Reverse((row.get::<_, i64>(0)?, source)));
        }
    }
    Ok(count)
}

/// Resolve only the requested UUID scope using the frozen query/display/sort
/// path. One JSON binding avoids parameter ceilings for large curated Shelves.
/// The caller owns retained rows; no navigation or membership is persisted.
pub fn visit_uuids(
    tx: &Transaction<'_>,
    request: QueryRequest,
    locale: &str,
    uuids: &[String],
    mut consume: impl FnMut(StorySummary) -> Result<()>,
) -> Result<()> {
    let (request, mut from, mut args) = matching(tx, request, locale)?;
    from.push_str(if from == FROM {
        " WHERE s.uuid IN (SELECT value FROM json_each(?))"
    } else {
        " AND s.uuid IN (SELECT value FROM json_each(?))"
    });
    args.push(list(uuids)?);
    let order = ordering(&request, locale, &mut args);
    let mut stmt = tx.prepare(&format!("{RECORD}{from}{order}"))?;
    let mut rows = stmt.query(params_from_iter(args))?;
    while let Some(row) = rows.next()? {
        consume(summary(tx, &Record::read(row)?, locale)?)?;
    }
    Ok(())
}
