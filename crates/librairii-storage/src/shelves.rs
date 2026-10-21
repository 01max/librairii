//! Curated Shelf participants share the existing owned writer and transaction.
use crate::{
    Result, StorageError,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
};
use librairii_core::shelves::{Member, Shelf, ShelfError, Source, StoryReference, normalize_name};
use librairii_core::{CollectionId, ShelfId, StoryUuid};
use rusqlite::{OptionalExtension, Transaction, params};

/// Shelf and Collection identities cannot cross command scopes.
/// ```compile_fail
/// use librairii_core::CollectionId;
/// use librairii_storage::shelves::Command;
/// fn wrong(id: CollectionId) { let _ = Command::Delete { id }; }
/// ```
#[derive(Debug)]
pub enum Command {
    Create { name: String },
    Rename { id: ShelfId, name: String },
    Duplicate { id: ShelfId, name: String },
    Reorder(Vec<ShelfId>),
    Delete { id: ShelfId },
}
#[derive(Debug)]
pub enum Response {
    Shelf(Shelf),
    Shelves(Vec<Shelf>),
    Deleted,
}
impl Database<'_> {
    pub fn edit_shelves(
        &self,
        command: Command,
        cancellation: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<Response>>> {
        self.transaction(cancellation, move |tx| execute(tx, command))
    }
    pub fn shelves(&self) -> Result<Pending<'_, Vec<Shelf>>> {
        self.read(list)
    }
    pub fn shelf_members(&self, id: ShelfId) -> Result<Pending<'_, Vec<Member>>> {
        self.read(move |tx| members(tx, id))
    }
}
const SELECT: &str = "SELECT s.id,s.name,s.position,s.revision,(SELECT COUNT(*) FROM shelf_stories m WHERE m.shelf_id=s.id),(SELECT COUNT(*) FROM shelf_stories m JOIN stories st ON st.uuid=m.story_uuid WHERE m.shelf_id=s.id AND st.is_broken=1),(SELECT COUNT(*) FROM shelf_stories m WHERE m.shelf_id=s.id AND NOT EXISTS(SELECT 1 FROM stories st JOIN story_archives a ON a.story_id=st.id WHERE st.uuid=m.story_uuid)),(SELECT COUNT(*) FROM shelf_sources src WHERE src.shelf_id=s.id AND src.active=1) FROM shelves s";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Shelf> {
    Ok(Shelf {
        id: shelf_id(r.get(0)?)?,
        name: r.get(1)?,
        position: r.get(2)?,
        revision: r.get(3)?,
        count: r.get(4)?,
        broken_count: r.get(5)?,
        unavailable_count: r.get(6)?,
        source_count: r.get(7)?,
    })
}
pub fn read(tx: &Transaction<'_>, id: ShelfId) -> Result<Shelf> {
    Ok(tx
        .query_row(&format!("{SELECT} WHERE s.id=?"), [id.get()], row)
        .optional()?
        .ok_or(ShelfError::NotFound)?)
}
pub fn list(tx: &Transaction<'_>) -> Result<Vec<Shelf>> {
    Ok(tx
        .prepare(&format!("{SELECT} ORDER BY s.position,s.id"))?
        .query_map([], row)?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn sources(tx: &Transaction<'_>, id: ShelfId, active_only: bool) -> Result<Vec<Source>> {
    let mut stmt=tx.prepare("SELECT src.id,src.collection_id,CASE WHEN src.active=1 THEN COALESCE(c.name,src.collection_name) ELSE src.collection_name END,src.active FROM shelf_sources src LEFT JOIN collections c ON c.id=src.collection_id WHERE src.shelf_id=? AND (?=0 OR src.active=1) ORDER BY src.id")?;
    Ok(stmt
        .query_map(params![id.get(), active_only], |r| {
            Ok(Source {
                id: r.get(0)?,
                collection_id: r.get::<_, Option<i64>>(1)?.map(collection_id).transpose()?,
                name: r.get(2)?,
                active: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}
pub fn members(tx: &Transaction<'_>, id: ShelfId) -> Result<Vec<Member>> {
    read(tx, id)?;
    let mut stmt=tx.prepare("SELECT story_uuid,position,protected,cached_title FROM shelf_stories WHERE shelf_id=? ORDER BY position,story_uuid")?;
    let mut result = stmt
        .query_map([id.get()], |r| {
            Ok(Member {
                uuid: r.get(0)?,
                position: r.get(1)?,
                manually_retained: r.get(2)?,
                cached_title: r.get(3)?,
                origins: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut stmt=tx.prepare("SELECT src.id,src.collection_id,src.collection_name,src.active FROM shelf_story_origins o JOIN shelf_sources src ON src.id=o.source_id AND src.shelf_id=o.shelf_id WHERE o.shelf_id=? AND o.story_uuid=? ORDER BY src.id")?;
    for m in &mut result {
        m.origins = stmt
            .query_map(params![id.get(), m.uuid], |r| {
                Ok(Source {
                    id: r.get(0)?,
                    collection_id: r.get::<_, Option<i64>>(1)?.map(collection_id).transpose()?,
                    name: r.get(2)?,
                    active: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
    }
    Ok(result)
}
fn duplicate_name(e: rusqlite::Error) -> StorageError {
    match &e {
        rusqlite::Error::SqliteFailure(code, _) if code.extended_code == 2067 => {
            ShelfError::DuplicateName.into()
        }
        _ => e.into(),
    }
}
fn insert(tx: &Transaction<'_>, name: &str) -> Result<Shelf> {
    let (name, normalized) = normalize_name(name)?;
    tx.execute("INSERT INTO shelves(name,normalized_name,position) VALUES(?,?,(SELECT COALESCE(MAX(position),-1)+1 FROM shelves))",params![name,normalized]).map_err(duplicate_name)?;
    read(
        tx,
        ShelfId::new(tx.last_insert_rowid()).map_err(|_| ShelfError::InvalidInput)?,
    )
}
fn bump(tx: &Transaction<'_>, id: ShelfId, changed: bool) -> Result<Shelf> {
    if changed {
        tx.execute(
            "UPDATE shelves SET revision=revision+1,updated_at=CURRENT_TIMESTAMP WHERE id=?",
            [id.get()],
        )?;
    }
    read(tx, id)
}
pub fn execute(tx: &Transaction<'_>, command: Command) -> Result<Response> {
    match command {
        Command::Create { name } => Ok(Response::Shelf(insert(tx, &name)?)),
        Command::Rename { id, name } => {
            read(tx, id)?;
            let (name, normalized) = normalize_name(&name)?;
            let changed = tx
                .execute(
                    "UPDATE shelves SET name=?,normalized_name=? WHERE id=? AND name!=?",
                    params![name, normalized, id.get(), name],
                )
                .map_err(duplicate_name)?;
            Ok(Response::Shelf(bump(tx, id, changed != 0)?))
        }
        Command::Delete { id } => {
            read(tx, id)?;
            tx.execute("DELETE FROM shelves WHERE id=?", [id.get()])?;
            Ok(Response::Deleted)
        }
        Command::Reorder(ids) => {
            let current = list(tx)?;
            let expected: std::collections::BTreeSet<_> = current.iter().map(|s| s.id).collect();
            if ids.len() != expected.len()
                || ids
                    .iter()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    != expected
            {
                return Err(ShelfError::InvalidInput.into());
            }
            let offset = current
                .iter()
                .map(|s| s.position)
                .max()
                .unwrap_or(-1)
                .checked_add(1)
                .ok_or(ShelfError::InvalidInput)?;
            for s in &current {
                s.position
                    .checked_add(offset)
                    .ok_or(ShelfError::InvalidInput)?;
            }
            tx.execute("UPDATE shelves SET position=position+?", [offset])?;
            for (p, id) in ids.iter().enumerate() {
                tx.execute(
                    "UPDATE shelves SET position=? WHERE id=?",
                    params![p as i64, id.get()],
                )?;
            }
            Ok(Response::Shelves(list(tx)?))
        }
        Command::Duplicate { id, name } => {
            read(tx, id)?;
            let copy = insert(tx, &name)?;
            tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title,added_at) SELECT ?,story_uuid,position,protected,cached_title,added_at FROM shelf_stories WHERE shelf_id=?",params![copy.id.get(),id.get()])?;
            tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid,created_at) SELECT ?,story_uuid,created_at FROM shelf_exclusions WHERE shelf_id=?",params![copy.id.get(),id.get()])?;
            for source in sources(tx, id, false)? {
                tx.execute("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name,active,created_at) SELECT ?,collection_id,collection_name,active,created_at FROM shelf_sources WHERE id=?",params![copy.id.get(),source.id])?;
                let new_id = tx.last_insert_rowid();
                tx.execute("INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id,accepted_at) SELECT ?,story_uuid,?,accepted_at FROM shelf_story_origins WHERE shelf_id=? AND source_id=?",params![copy.id.get(),new_id,id.get(),source.id])?;
            }
            Ok(Response::Shelf(read(tx, copy.id)?))
        }
    }
}

mod curation;
pub use curation::{Curation, curate, exclusions};
mod browse;
pub use browse::{MemberPage, ResolvedMember, page};
mod sources;
pub use sources::{SourceEvaluation, SourceState};
mod review;
pub use review::{Action, Decision, UpdatePlan, UpdatePreview};
pub use review::{Candidate, CreationPlan, CreationPreview, CreationRequest, Warning};

pub(crate) mod tickets;

mod source_change;
pub use source_change::{SourceChangePlan, SourceChangePreview};

mod deletion;
pub use deletion::{CollectionDeletionPlan, CollectionDeletionPreview, ShelfDecisions};

/// Page indices are zero based. Overflow/out-of-range pages are empty.
fn rows<T>(values: &[T], page: usize) -> &[T] {
    let start = page.saturating_mul(25).min(values.len());
    &values[start..start.saturating_add(25).min(values.len())]
}
pub use curation::{Exclusion, ExclusionPage};

fn shelf_id(raw: i64) -> rusqlite::Result<ShelfId> {
    ShelfId::new(raw).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn collection_id(raw: i64) -> rusqlite::Result<CollectionId> {
    CollectionId::new(raw).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn uuid_strings(values: &[StoryUuid]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .iter()
        .filter(|u| seen.insert(**u))
        .map(ToString::to_string)
        .collect()
}

fn reference_strings(values: &[StoryReference]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    values
        .iter()
        .filter(|u| seen.insert(u.as_stored()))
        .map(|u| u.as_stored().to_owned())
        .collect()
}
