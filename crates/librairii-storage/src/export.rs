//! Coherent full-scope resolution. Run on database workers, without file I/O.
use crate::{
    Result,
    database::{Database, Pending},
};
use librairii_core::{
    ExportScopeKind, ShelfId,
    browsing::ExportStory,
    collection_codec::Query,
    export::{ExportError, Resolved, Scope, Source},
    query::QueryRequest,
};
use rusqlite::Transaction;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod preparation;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use preparation::Exports;

/// Explicit aggregate retained-data refusal, never truncation or an item cap.
/// This includes row/string capacities and the deduplication keys. Temporary SQL
/// state and the caller's request remain independently owned.
pub const MAX_RESOLVED_BYTES: usize = 64 * 1024 * 1024;

impl Database<'_> {
    pub fn resolve_export(&self, scope: Scope, locale: String) -> Result<Pending<'_, Resolved>> {
        self.read(move |tx| resolve(tx, scope, &locale))
    }
}
fn invalid<T>() -> Result<T> {
    Err(ExportError::InvalidScope.into())
}
fn unique(ids: impl IntoIterator<Item = i64>) -> Result<Vec<i64>> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in ids {
        if id <= 0 || !seen.insert(id) {
            return invalid();
        }
        out.push(id);
    }
    if out.is_empty() {
        return invalid();
    }
    Ok(out)
}
pub(crate) fn row_bytes(s: &ExportStory) -> usize {
    std::mem::size_of::<ExportStory>()
        + 128
        + s.uuid.capacity() * 2
        + s.title.capacity()
        + s.original_filename.capacity()
        + s.detected_format.capacity()
        + s.sha256.capacity()
        + s.managed_relative_path.capacity()
        + s.verification.capacity()
}
struct Rows {
    stories: Vec<ExportStory>,
    seen: BTreeSet<String>,
    bytes: usize,
    overlap: usize,
}
impl Rows {
    fn append(&mut self, story: ExportStory) -> Result<()> {
        if self.seen.contains(&story.uuid) {
            self.overlap += 1;
            return Ok(());
        }
        self.bytes = self
            .bytes
            .checked_add(row_bytes(&story))
            .ok_or(ExportError::ResourceLimit)?;
        if self.bytes > MAX_RESOLVED_BYTES {
            return Err(ExportError::ResourceLimit.into());
        }
        self.seen.insert(story.uuid.clone());
        self.stories.push(story);
        Ok(())
    }
}
fn unavailable(uuid: String, title: String) -> ExportStory {
    ExportStory {
        broken: false,
        id: 0,
        title: if title.is_empty() {
            uuid.clone()
        } else {
            title
        },
        uuid,
        original_filename: String::new(),
        detected_format: String::new(),
        sha256: String::new(),
        byte_size: 0,
        managed_relative_path: String::new(),
        verification: String::new(),
    }
}
fn curated(
    tx: &Transaction<'_>,
    id: ShelfId,
    query: Option<QueryRequest>,
    selected: Option<Vec<String>>,
    locale: &str,
    out: &mut Rows,
) -> Result<String> {
    let shelf = crate::shelves::read(tx, id)?;
    let members = crate::shelves::members(tx, id)?;
    let mut wanted = selected.map(|ids| ids.into_iter().collect::<BTreeSet<_>>());
    if wanted.as_ref().is_some_and(BTreeSet::is_empty) {
        return invalid();
    }
    let mut available = BTreeMap::new();
    // Full query results stream through one ordered cursor; retain only the
    // approved members. Pending proposals never enter this path.
    let approved = members
        .iter()
        .map(|m| m.uuid.as_str())
        .collect::<BTreeSet<_>>();
    let request = query.unwrap_or_default();
    let filtered =
        librairii_core::query::normalize_request(request.clone())?.criteria != Query::default();
    let mut bytes = 0usize;
    crate::query::visit_scope(tx, request, locale, |s| {
        if approved.contains(s.uuid.as_str()) {
            bytes = bytes
                .checked_add(row_bytes(&s))
                .ok_or(ExportError::ResourceLimit)?;
            if bytes > MAX_RESOLVED_BYTES {
                return Err(ExportError::ResourceLimit.into());
            }
            available.insert(s.uuid.clone(), s);
        }
        Ok(())
    })?;
    for member in members {
        if let Some(wanted) = &mut wanted {
            if !wanted.remove(&member.uuid) {
                continue;
            }
        }
        if let Some(story) = available.remove(&member.uuid) {
            out.append(story)?;
        } else if !filtered {
            out.append(unavailable(member.uuid, member.cached_title))?;
        }
    }
    if wanted.is_some_and(|w| !w.is_empty()) {
        return invalid();
    }
    Ok(shelf.name)
}
pub fn resolve(tx: &Transaction<'_>, scope: Scope, locale: &str) -> Result<Resolved> {
    let mut out = Rows {
        stories: Vec::new(),
        seen: BTreeSet::new(),
        bytes: 0,
        overlap: 0,
    };
    let mut source = Source {
        kind: ExportScopeKind::CurrentQuery,
        ids: Vec::new(),
        names: Vec::new(),
    };
    match scope {
        Scope::CurrentQuery(query) => {
            crate::query::visit_scope(tx, query, locale, |s| out.append(s))?;
        }
        Scope::Selection(ids) => {
            source.kind = ExportScopeKind::Selection;
            let ids = ids.into_iter().collect::<Vec<_>>();
            if ids.iter().any(|id| *id <= 0) {
                return invalid();
            }
            let mut seen = BTreeSet::new();
            let ids = ids
                .into_iter()
                .filter(|id| seen.insert(*id))
                .collect::<Vec<_>>();
            let mut found = BTreeMap::new();
            let mut bytes = 0usize;
            crate::query::visit_scope(tx, QueryRequest::default(), locale, |s| {
                if seen.contains(&s.id) {
                    bytes = bytes
                        .checked_add(row_bytes(&s))
                        .ok_or(ExportError::ResourceLimit)?;
                    if bytes > MAX_RESOLVED_BYTES {
                        return Err(ExportError::ResourceLimit.into());
                    }
                    found.insert(s.id, s);
                }
                Ok(())
            })?;
            for id in ids {
                out.append(found.remove(&id).ok_or(ExportError::InvalidScope)?)?;
            }
        }
        Scope::Collections(ids) => {
            source.ids = unique(ids.into_iter().map(|id| id.get()))?;
            source.kind = ExportScopeKind::CollectionUnion;
            for id in &source.ids {
                let c = crate::collections::inspect(tx, *id)?;
                source.names.push(c.collection.name);
                let q = c.query.ok_or(ExportError::InvalidScope)?;
                if c.attention.is_some() {
                    return invalid();
                }
                crate::query::visit_scope(
                    tx,
                    QueryRequest {
                        criteria: q,
                        ..Default::default()
                    },
                    locale,
                    |s| out.append(s),
                )?;
            }
        }
        Scope::Shelves(ids) => {
            source.ids = unique(ids.into_iter().map(|id| id.get()))?;
            source.kind = ExportScopeKind::ShelfUnion;
            for id in &source.ids {
                source.names.push(
                    curated(
                        tx,
                        ShelfId::new(*id).map_err(|_| ExportError::InvalidScope)?,
                        None,
                        None,
                        locale,
                        &mut out,
                    )?
                    .into_bytes(),
                );
            }
        }
        Scope::ShelfResult { shelf, query } => {
            curated(tx, shelf, Some(query), None, locale, &mut out)?;
        }
        Scope::ShelfSelection { shelf, uuids } => {
            source.kind = ExportScopeKind::Selection;
            curated(tx, shelf, None, Some(uuids), locale, &mut out)?;
        }
    }
    Ok(Resolved {
        source,
        stories: out.stories,
        collapsed_overlap: out.overlap,
    })
}
