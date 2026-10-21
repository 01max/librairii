//! Explicit catalog persistence. Synchronous entry points run on the caller's
//! blocking worker; HTTP and filesystem work never execute in SQL callbacks.
use crate::{
    Result, StorageError,
    database::{Database, Pending, TransactionOutcome, WorkCancellation},
    platform::unix::{EntryName, FileArea, Publication},
};
use librairii_app::metadata::{NormalizedCatalog, utc_time};
use librairii_core::{
    CatalogSnapshotId, JobId,
    metadata::{Locale, MetadataError, title_key},
};
use rusqlite::{Transaction, params};
use std::io::Write;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub id: CatalogSnapshotId,
    pub sync: JobId,
    pub locale: Locale,
    pub raw_path: String,
    pub hash: librairii_core::Checksum,
    pub bytes: u64,
    pub status: SnapshotStatus,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotStatus {
    Staged,
    Active,
    Superseded,
    Rejected,
}
fn invalid() -> StorageError {
    MetadataError::InvalidTransition.into()
}
impl Database<'_> {
    pub fn begin_catalog_sync(
        &self,
        id: JobId,
        locale: Locale,
        started_at: String,
        cancel: WorkCancellation,
    ) -> Result<Pending<'_, TransactionOutcome<()>>> {
        let started_at = utc_time(&started_at)?;
        self.transaction(cancel, move |tx| {
            let busy: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM catalog_syncs WHERE status='running')",
                [],
                |r| r.get(0),
            )?;
            if busy {
                return Err(MetadataError::Busy.into());
            }
            tx.execute(
                "INSERT INTO catalog_syncs(id,locale,status,started_at) VALUES(?,?,'running',?)",
                params![id.to_string(), locale.as_str(), started_at],
            )?;
            Ok(())
        })
    }
    /// Publish the exact validated response using retained descriptor custody,
    /// then retain its receipt until the owned SQL acknowledgement is observed.
    /// Failed/uncertain SQL leaves a recoverable raw file; never compensates by
    /// deleting a pathname whose database reference may have committed.
    pub fn stage_catalog(
        &self,
        id: JobId,
        catalog: NormalizedCatalog,
        fetched_at: String,
        cancel: WorkCancellation,
    ) -> Result<TransactionOutcome<CatalogSnapshotId>> {
        let fetched_at = utc_time(&fetched_at)?;
        let files = self.metadata_files()?;
        let guard = files.try_custody()?;
        let mut stage = files.create_stage(&guard).map_err(|e| {
            StorageError::io(
                "create catalog stage",
                std::io::Error::other(e.cause().code()),
            )
        })?;
        for chunk in catalog.raw().chunks(64 * 1024) {
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            stage
                .write_all(chunk)
                .map_err(|e| StorageError::io("write catalog stage", e))?;
        }
        let measurement =
            stage.measure(librairii_app::metadata::MAX_CATALOG_BYTES as u64, &cancel)?;
        if measurement.checksum() != catalog.checksum() {
            return Err(StorageError::IntegrityFailed);
        }
        let name = format!("raw-{id}.json");
        let mut receipt = stage
            .publish_no_clobber(
                &guard,
                FileArea::Catalog,
                EntryName::parse(&name)?,
                measurement,
                &cancel,
            )
            .map_err(|e| {
                StorageError::io("publish catalog", std::io::Error::other(e.cause.code()))
            })?;
        match &mut receipt {
            Publication::NewOwned(file) => file.revalidate(&guard, &cancel)?,
            Publication::ReusedExisting { existing, .. } => existing.revalidate(&guard, &cancel)?,
        }
        let path = format!("catalog/{name}");
        let result = self
            .transaction(cancel, move |tx| {
                stage_rows(tx, id, &catalog, &path, &fetched_at)
            })?
            .wait();
        // Receipt, custody and Database loan stay live through acknowledgement.
        drop(receipt);
        result
    }
    pub fn catalog_snapshots(&self) -> Result<Pending<'_, Vec<Snapshot>>> {
        self.read(|tx|{
        let mut stmt=tx.prepare("SELECT id,sync_id,locale,raw_path,raw_sha256,byte_size,status FROM catalog_snapshots ORDER BY id")?;
        let data=stmt.query_map([],|r|Ok((r.get::<_,i64>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,String>(6)?)))?.collect::<rusqlite::Result<Vec<_>>>()?;
        data.into_iter().map(|(id,sync,locale,raw_path,hash,bytes,status)|Ok(Snapshot{id:CatalogSnapshotId::new(id).map_err(|_|invalid())?,sync:sync.parse().map_err(|_|invalid())?,locale:Locale::parse(&locale)?,raw_path,hash:hash.parse().map_err(|_|invalid())?,bytes:bytes.try_into().map_err(|_|invalid())?,status:match status.as_str(){"staged"=>SnapshotStatus::Staged,"active"=>SnapshotStatus::Active,"superseded"=>SnapshotStatus::Superseded,"rejected"=>SnapshotStatus::Rejected,_=>return Err(invalid())}})).collect()
    })
    }
    pub fn reject_catalog(
        &self,
        id: CatalogSnapshotId,
    ) -> Result<Pending<'_, TransactionOutcome<()>>> {
        self.repair_transaction(move |tx| {
            if tx.execute(
                "UPDATE catalog_snapshots SET status='rejected' WHERE id=? AND status='staged'",
                [id.get()],
            )? != 1
            {
                return Err(invalid());
            }
            Ok(())
        })
    }
}
fn stage_rows(
    tx: &Transaction<'_>,
    id: JobId,
    catalog: &NormalizedCatalog,
    path: &str,
    fetched_at: &str,
) -> Result<CatalogSnapshotId> {
    let running: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalog_syncs WHERE id=? AND locale=? AND status='running')",
        params![id.to_string(), catalog.locale().as_str()],
        |r| r.get(0),
    )?;
    if !running {
        return Err(invalid());
    }
    tx.execute("INSERT INTO catalog_snapshots(sync_id,locale,raw_path,raw_sha256,byte_size,record_count,status,fetched_at) VALUES(?,?,?,?,?,?,'staged',?)",params![id.to_string(),catalog.locale().as_str(),path,catalog.checksum().to_string(),catalog.raw().len() as i64,catalog.records().len() as i64,fetched_at])?;
    let snapshot = CatalogSnapshotId::new(tx.last_insert_rowid()).map_err(|_| invalid())?;
    for art in catalog.artworks() {
        tx.execute(
            "INSERT INTO catalog_artworks(id,source_url) VALUES(?,?) ON CONFLICT(id) DO NOTHING",
            params![art.id.to_string(), art.source_url],
        )?;
        let url: String = tx.query_row(
            "SELECT source_url FROM catalog_artworks WHERE id=?",
            [art.id.to_string()],
            |r| r.get(0),
        )?;
        if url != art.source_url {
            return Err(StorageError::IntegrityFailed);
        }
    }
    for r in catalog.records() {
        tx.execute("INSERT INTO official_story_metadata(snapshot_id,story_uuid,locale,title,title_normalized,description,author,publisher,language,duration_seconds,minimum_age,maximum_age,artwork_id,provenance,source_record_id,source_updated_at) VALUES(?,?,?,?,?,NULLIF(?,''),NULLIF(?,''),NULLIF(?,''),?,?,?,?,?,'lunii_catalog',?,?)",params![snapshot.get(),r.uuid.to_string(),catalog.locale().as_str(),r.title,title_key(&r.title)?,r.description,r.author,r.publisher,r.language.as_str(),r.duration_seconds,r.minimum_age,r.maximum_age,r.artwork_id.map(|v|v.to_string()),r.source_record_id,r.source_updated_at])?;
    }
    Ok(snapshot)
}
