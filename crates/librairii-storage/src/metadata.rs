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

use librairii_app::metadata::{CatalogRepository, RefreshCode, RefreshReport, RepositoryFailure};
fn acknowledge<T>(
    result: Result<Pending<'_, TransactionOutcome<T>>>,
) -> std::result::Result<T, RepositoryFailure> {
    let pending = result.map_err(|_| RepositoryFailure::Proven)?;
    match pending.wait() {
        Ok(TransactionOutcome::Committed(v)) => Ok(v),
        Ok(TransactionOutcome::ProvenNotCommitted(_)) => Err(RepositoryFailure::Proven),
        Ok(TransactionOutcome::CommitUncertain(_)) | Err(_) => Err(RepositoryFailure::Uncertain),
    }
}
impl CatalogRepository for Database<'_> {
    fn begin(
        &self,
        id: JobId,
        locale: Locale,
        at: &str,
    ) -> std::result::Result<(), RepositoryFailure> {
        acknowledge(self.begin_catalog_sync(id, locale, at.into(), WorkCancellation::default()))
    }
    fn stage(
        &self,
        id: JobId,
        catalog: NormalizedCatalog,
        at: &str,
    ) -> std::result::Result<CatalogSnapshotId, RepositoryFailure> {
        match self.stage_catalog(id, catalog, at.into(), WorkCancellation::default()) {
            Ok(TransactionOutcome::Committed(v)) => Ok(v),
            Ok(TransactionOutcome::ProvenNotCommitted(_)) => Err(RepositoryFailure::Proven),
            Err(StorageError::WorkerStopped) | Ok(TransactionOutcome::CommitUncertain(_)) => {
                Err(RepositoryFailure::Uncertain)
            }
            Err(_) => Err(RepositoryFailure::Proven),
        }
    }
    fn activate(
        &self,
        id: CatalogSnapshotId,
        at: &str,
    ) -> std::result::Result<RefreshReport, RepositoryFailure> {
        acknowledge(self.activate_catalog(id, at.into()))
    }
    fn fail(
        &self,
        id: JobId,
        snapshot: Option<CatalogSnapshotId>,
        code: RefreshCode,
        at: &str,
    ) -> std::result::Result<(), RepositoryFailure> {
        acknowledge(self.finish_catalog_failure(id, snapshot, code, at.into()))
    }
}
impl Database<'_> {
    /// The caller has crossed the final cancellation check. No work token is
    /// admitted here: activation, Age and success are a single transaction.
    pub fn activate_catalog(
        &self,
        id: CatalogSnapshotId,
        at: String,
    ) -> Result<Pending<'_, TransactionOutcome<RefreshReport>>> {
        let at = utc_time(&at)?;
        self.repair_transaction(move|tx|{
            let (sync,locale):(String,String)=tx.query_row("SELECT c.sync_id,c.locale FROM catalog_snapshots c JOIN catalog_syncs s ON s.id=c.sync_id WHERE c.id=? AND c.status='staged' AND s.status='running'",[id.get()],|r|Ok((r.get(0)?,r.get(1)?)))?;
            let projection=crate::tagging::project_age_snapshot(tx,id.get(),&librairii_core::tagging::AgeConfig::default())?;
            tx.execute("UPDATE catalog_snapshots SET status='superseded' WHERE locale=? AND status='active'",[&locale])?;
            if tx.execute("UPDATE catalog_snapshots SET status='active',activated_at=? WHERE id=? AND status='staged'",params![at,id.get()])?!=1{return Err(invalid());}
            if tx.execute("UPDATE catalog_syncs SET status='succeeded',matched_story_count=?,finished_at=?,error_code=NULL,error_message=NULL WHERE id=? AND status='running'",params![projection.matched_stories as i64,at,sync])?!=1{return Err(invalid());}
            Ok(RefreshReport{sync:sync.parse().map_err(|_|invalid())?,snapshot:id,matched_stories:projection.matched_stories as i64,activated_at:at})
        })
    }
    pub fn finish_catalog_failure(
        &self,
        id: JobId,
        snapshot: Option<CatalogSnapshotId>,
        code: RefreshCode,
        at: String,
    ) -> Result<Pending<'_, TransactionOutcome<()>>> {
        let at = utc_time(&at)?;
        self.repair_transaction(move|tx|{
            if let Some(snapshot)=snapshot && tx.execute("UPDATE catalog_snapshots SET status='rejected' WHERE id=? AND sync_id=? AND status='staged'",params![snapshot.get(),id.to_string()])?!=1{return Err(invalid());}
            let status=if code==RefreshCode::Cancelled{"cancelled"}else{"failed"};
            if tx.execute("UPDATE catalog_syncs SET status=?,error_code=?,error_message=?,finished_at=? WHERE id=? AND status='running'",params![status,code.as_str(),code.as_str(),at,id.to_string()])?!=1{return Err(invalid());}Ok(())
        })
    }
}
impl Database<'_> {
    pub fn catalog_status(
        &self,
        locale: Locale,
    ) -> Result<Pending<'_, librairii_core::metadata::CatalogStatus>> {
        self.read(move|tx|{
        use librairii_core::metadata::{CatalogStatus,Freshness};use rusqlite::OptionalExtension;
        let active=tx.query_row("SELECT c.fetched_at,c.activated_at,s.matched_story_count FROM catalog_snapshots c JOIN catalog_syncs s ON s.id=c.sync_id WHERE c.locale=? AND c.status='active'",[locale.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).optional()?;
        let latest=tx.query_row("SELECT status,started_at,error_code FROM catalog_syncs WHERE locale=? ORDER BY started_at DESC,id DESC LIMIT 1",[locale.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?))).optional()?;
        let mut status=CatalogStatus{state:Freshness::NeverSynced,locale,matched_stories:0,fetched_at:None,activated_at:None,last_attempt_status:None,last_attempt_at:None,error_code:None};
        if let Some((fetched,activated,count))=active{status.state=Freshness::Fresh;status.fetched_at=Some(fetched);status.activated_at=Some(activated);status.matched_stories=count;}
        if let Some((state,at,error))=latest{if status.state==Freshness::Fresh && matches!(state.as_str(),"failed"|"cancelled"){status.state=Freshness::StaleCache;}status.last_attempt_status=Some(state);status.last_attempt_at=Some(at);status.error_code=error;}
        Ok(status)
    })
    }
}
