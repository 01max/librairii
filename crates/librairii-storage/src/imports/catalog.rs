use super::*;
use rusqlite::{
    OptionalExtension, Transaction, params,
    types::{ToSql, ToSqlOutput, ValueRef},
};

/// SQLite TEXT can contain native non-UTF-8 filename bytes. Binding them as
/// TEXT retains the legacy cell type and byte identity; a lossy String cannot.
#[derive(Clone, Debug)]
pub(super) struct RawText(pub Vec<u8>);
impl ToSql for RawText {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(&self.0)))
    }
}
#[derive(Clone, Debug)]
pub(super) struct Candidate {
    pub metadata: Inspection,
    pub filename: Vec<u8>,
    pub managed: Vec<u8>,
    pub checksum: librairii_core::Checksum,
    pub bytes: i64,
    pub artwork: String,
}
#[derive(Clone, Debug)]
pub struct ImportedStory {
    pub story_id: i64,
    pub archive_id: i64,
    pub uuid: String,
    pub checksum: librairii_core::Checksum,
}
#[derive(Clone, Debug)]
pub enum Decision {
    Imported(ImportedStory),
    Duplicate(ImportedStory),
    Conflict(ImportedStory),
}
impl Decision {
    pub fn story(&self) -> &ImportedStory {
        match self {
            Self::Imported(s) | Self::Duplicate(s) | Self::Conflict(s) => s,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Imported(_) => "imported",
            Self::Duplicate(_) => "duplicate_checksum",
            Self::Conflict(_) => "uuid_conflict",
        }
    }
}
fn lookup(tx: &Transaction<'_>, predicate: &str, value: &str) -> Result<Option<ImportedStory>> {
    let row = tx.query_row(&format!("SELECT s.id,a.id,s.uuid,a.sha256 FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE {predicate}=?"),[value],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?))).optional()?;
    row.map(|(story_id, archive_id, uuid, checksum)| {
        Ok(ImportedStory {
            story_id,
            archive_id,
            uuid,
            checksum: checksum
                .parse()
                .map_err(|_| StorageError::IntegrityFailed)?,
        })
    })
    .transpose()
}
pub(super) fn by_hash(
    tx: &Transaction<'_>,
    hash: librairii_core::Checksum,
) -> Result<Option<ImportedStory>> {
    lookup(tx, "a.sha256", &hash.to_string())
}
pub(super) fn before_publish(tx: &Transaction<'_>, c: &Candidate) -> Result<Option<Decision>> {
    if let Some(s) = by_hash(tx, c.checksum)? {
        return Ok(Some(Decision::Duplicate(s)));
    }
    Ok(lookup(tx, "s.uuid", &c.metadata.identity.canonical())?.map(Decision::Conflict))
}
pub(super) fn create(tx: &Transaction<'_>, c: &Candidate) -> Result<Decision> {
    if let Some(out) = before_publish(tx, c)? {
        return Ok(out);
    }
    let uuid = c.metadata.identity.canonical();
    let title = if c.metadata.text.title.is_empty() {
        format!("Story {uuid}")
    } else {
        c.metadata.text.title.clone()
    };
    let normalized = librairii_core::metadata::title_key(&title)?;
    tx.execute("INSERT INTO stories(uuid,embedded_title,embedded_description,embedded_artwork_path,display_name_normalized) VALUES(?,NULLIF(?,''),NULLIF(?,''),NULLIF(?,''),?)",params![uuid,c.metadata.text.title,c.metadata.text.description,c.artwork,normalized])?;
    let story_id = tx.last_insert_rowid();
    tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path) VALUES(?,?,?,?,?,?)",params![story_id,RawText(c.filename.clone()),c.metadata.format.as_str(),c.checksum.to_string(),c.bytes,RawText(c.managed.clone())])?;
    let archive_id = tx.last_insert_rowid();
    crate::tagging::after_story_create_age(
        tx,
        story_id,
        &librairii_core::tagging::AgeConfig::default(),
    )?;
    Ok(Decision::Imported(ImportedStory {
        story_id,
        archive_id,
        uuid,
        checksum: c.checksum,
    }))
}
pub(super) fn absent(tx: &Transaction<'_>, c: &Candidate) -> Result<bool> {
    let archive: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM story_archives WHERE managed_path=?)",
        [RawText(c.managed.clone())],
        |r| r.get(0),
    )?;
    let artwork: bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM stories WHERE embedded_artwork_path=? AND embedded_artwork_path<>'')",[&c.artwork],|r|r.get(0))?;
    Ok(!archive && !artwork)
}
pub(super) fn readback(tx: &Transaction<'_>, c: &Candidate) -> Result<Option<ImportedStory>> {
    let Some(row) = by_hash(tx, c.checksum)? else {
        return Ok(None);
    };
    let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.id=? AND s.uuid=? AND COALESCE(s.embedded_title,'')=? AND COALESCE(s.embedded_description,'')=? AND COALESCE(s.embedded_artwork_path,'')=? AND a.original_filename=? AND a.detected_format=? AND a.sha256=? AND a.byte_size=? AND a.managed_path=? AND a.validation_state='valid')",params![row.story_id,c.metadata.identity.canonical(),c.metadata.text.title,c.metadata.text.description,c.artwork,RawText(c.filename.clone()),c.metadata.format.as_str(),c.checksum.to_string(),c.bytes,RawText(c.managed.clone())],|r|r.get(0))?;
    if !exact {
        return Err(StorageError::SnapshotChanged);
    }
    Ok(Some(row))
}
