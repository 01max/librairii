//! Owned artwork cache. Network fetches occur before entering this repository.
use crate::{
    Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    platform::unix::{EntryName, FileArea, Publication},
};
use librairii_app::metadata::{
    ArtworkAsset, ArtworkRepository, MAX_ARTWORK_BYTES, MediaType, RegisteredArtwork, utc_time,
};
use librairii_core::{Checksum, metadata::MetadataError};
use rusqlite::{OptionalExtension, params};
use std::io::Write;
#[derive(Debug)]
struct Record {
    url: String,
    path: Option<String>,
    media: Option<String>,
    hash: Option<String>,
    bytes: Option<i64>,
}
impl Database<'_> {
    pub fn read_registered_artwork(&self, id: Checksum) -> Result<RegisteredArtwork> {
        let record=self.read(move|tx|{tx.query_row("SELECT source_url,managed_path,content_type,sha256,byte_size FROM catalog_artworks WHERE id=?",[id.to_string()],|r|Ok(Record{url:r.get(0)?,path:r.get(1)?,media:r.get(2)?,hash:r.get(3)?,bytes:r.get(4)?})).optional()?.ok_or(MetadataError::ArtworkNotFound.into())})?.wait()?;
        let cached = if let Some(path) = record.path {
            let media = MediaType::parse(
                record
                    .media
                    .as_deref()
                    .ok_or(MetadataError::InvalidArtwork)?,
            )?;
            let files = self.metadata_files()?;
            let bytes = files.read_catalog(&path, MAX_ARTWORK_BYTES)?;
            let asset = ArtworkAsset::checked(bytes, media)?;
            if record.hash.as_deref() != Some(asset.checksum.to_string().as_str())
                || record.bytes != Some(asset.bytes.len() as i64)
            {
                return Err(StorageError::IntegrityFailed);
            }
            Some(asset)
        } else {
            None
        };
        Ok(RegisteredArtwork {
            id,
            source_url: record.url,
            cached,
        })
    }
    pub fn persist_artwork(
        &self,
        id: Checksum,
        asset: &ArtworkAsset,
        at: &str,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Result<()> {
        librairii_app::metadata::validate_artwork(&asset.bytes, asset.media)?;
        if librairii_app::metadata::digest(&asset.bytes) != asset.checksum {
            return Err(StorageError::IntegrityFailed);
        }
        // Require registration before any filesystem effect, then revalidate in SQL.
        self.read_registered_artwork(id)?;
        let at = utc_time(at)?;
        let files = self.metadata_files()?;
        let guard = files.try_custody()?;
        let mut stage = files
            .create_stage(&guard)
            .map_err(|_| StorageError::CustodyRecoveryRequired)?;
        stage
            .write_all(&asset.bytes)
            .map_err(|e| StorageError::io("write artwork", e))?;
        let cancel = WorkCancellation::default();
        let measurement = stage.measure(MAX_ARTWORK_BYTES as u64, &cancel)?;
        let name = format!("art-{id}-{}.{}", asset.checksum, asset.media.extension());
        let mut receipt = stage
            .publish_no_clobber(
                &guard,
                FileArea::Catalog,
                EntryName::parse(&name)?,
                measurement,
                &cancel,
            )
            .map_err(|_| StorageError::CustodyRecoveryRequired)?;
        match &mut receipt {
            Publication::NewOwned(file) => file.revalidate(&guard, &cancel)?,
            Publication::ReusedExisting { existing, .. } => existing.revalidate(&guard, &cancel)?,
        }
        let path = format!("catalog/{name}");
        let media = asset.media.as_str();
        let hash = asset.checksum.to_string();
        let size = asset.bytes.len() as i64;
        let outcome=self.repair_transaction(move|tx|{if tx.execute("UPDATE catalog_artworks SET managed_path=?,content_type=?,sha256=?,byte_size=?,etag=?,last_modified=?,cached_at=?,updated_at=? WHERE id=?",params![path,media,hash,size,etag,last_modified,at,at,id.to_string()])?!=1{return Err(MetadataError::ArtworkNotFound.into());}Ok(())})?.wait()?;
        let result = match outcome {
            TransactionOutcome::Committed(()) => Ok(()),
            TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
                Err(e)
            }
        };
        drop(receipt);
        result
    }
}
impl ArtworkRepository for Database<'_> {
    fn registered_artwork(
        &self,
        id: Checksum,
    ) -> std::result::Result<RegisteredArtwork, MetadataError> {
        self.read_registered_artwork(id).map_err(|e| match e {
            StorageError::Metadata(e) => e,
            _ => MetadataError::InvalidArtwork,
        })
    }
    fn cache_artwork(
        &self,
        id: Checksum,
        asset: &ArtworkAsset,
        at: &str,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> std::result::Result<(), MetadataError> {
        self.persist_artwork(id, asset, at, etag, last_modified)
            .map_err(|_| MetadataError::InvalidArtwork)
    }
}
impl Database<'_> {
    /// Opaque local story ID lookup: a UI-supplied pathname is never accepted.
    pub fn read_embedded_artwork(&self, story: i64) -> Result<Option<ArtworkAsset>> {
        if story <= 0 {
            return Err(MetadataError::ArtworkNotFound.into());
        }
        let path = self
            .read(move |tx| {
                Ok(tx
                    .query_row(
                        "SELECT embedded_artwork_path FROM stories WHERE id=?",
                        [story],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .optional()?
                    .flatten())
            })?
            .wait()?;
        let Some(path) = path.filter(|p| !p.is_empty()) else {
            return Ok(None);
        };
        if !path.starts_with("catalog/embedded/") || !path.ends_with(".png") {
            return Err(MetadataError::InvalidArtwork.into());
        }
        let files = self.metadata_files()?;
        let bytes = files.read_catalog(&path, MAX_ARTWORK_BYTES)?;
        Ok(Some(ArtworkAsset::checked(bytes, MediaType::Png)?))
    }
}
