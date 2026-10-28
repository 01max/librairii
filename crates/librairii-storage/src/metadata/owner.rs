//! Actual filesystem failures live in an operation owner, never an error DTO.
use super::*;
use crate::platform::unix::{
    Custody, LibraryFiles, MetadataStageCleanupFailure, OwnedStage, PublishFailure,
    StageCreateFailure, TempCleanupFailure,
};
use std::sync::{Arc, Mutex};
#[derive(Debug)]
enum RetainedEffect<'db> {
    Creation(Box<StageCreateFailure<'db, 'db>>),
    Publication(Box<PublishFailure<'db, 'db>>),
    Cleanup(Box<TempCleanupFailure<'db, 'db>>),
    MetadataCleanup(Box<MetadataStageCleanupFailure<'db, 'db>>),
    Receipt(Publication<'db, 'db>),
    Directory,
}
#[derive(Debug)]
struct RetainedResources<'db> {
    _files: Arc<LibraryFiles<'db>>,
    _guard: Custody<'db>,
    effect: RetainedEffect<'db>,
}
/// One owner per metadata/artwork operation composition. It borrows the actual
/// Database through every retained file/partial-allocation/guard. Unresolved
/// resources fence the generation; dropping a DTO or cancelling a job cannot
/// drain them. Shutdown must drain this owner before closing the Database;
/// unresolved native generations still cannot finalize as clean.
#[derive(Debug)]
pub struct MetadataStorage<'db, 'library> {
    pub(crate) database: &'db Database<'library>,
    pub(crate) work_cancel: WorkCancellation,
    retained: Mutex<Vec<RetainedResources<'db>>>,
    #[cfg(test)]
    pub(crate) creation_fault: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedKind {
    Creation,
    Publication,
    Cleanup,
    Receipt,
    Directory,
}
impl<'db, 'library> MetadataStorage<'db, 'library> {
    pub fn for_work(database: &'db Database<'library>, work_cancel: WorkCancellation) -> Self {
        Self {
            work_cancel,
            ..Self::new(database)
        }
    }
    pub fn new(database: &'db Database<'library>) -> Self {
        Self {
            database,
            work_cancel: WorkCancellation::default(),
            retained: Mutex::new(Vec::new()),
            #[cfg(test)]
            creation_fault: false,
        }
    }
    pub fn for_job(
        database: &'db Database<'library>,
        job: &librairii_app::native_jobs::Cancellation,
    ) -> Self {
        Self {
            work_cancel: WorkCancellation::for_job(job),
            ..Self::new(database)
        }
    }
    pub fn retained_failures(&self) -> Vec<RetainedKind> {
        let retained = self
            .retained
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retained
            .iter()
            .map(|r| match &r.effect {
                RetainedEffect::Creation(f) => {
                    let _ = f.retained_allocation();
                    RetainedKind::Creation
                }
                RetainedEffect::Publication(f) => {
                    let _ = f.publish_uncertain;
                    RetainedKind::Publication
                }
                RetainedEffect::Cleanup(f) => {
                    let _ = &f.cause;
                    RetainedKind::Cleanup
                }
                RetainedEffect::MetadataCleanup(f) => {
                    let _ = (&f.cause, &f.publication);
                    RetainedKind::Cleanup
                }
                RetainedEffect::Receipt(r) => {
                    let _ = r;
                    RetainedKind::Receipt
                }
                RetainedEffect::Directory => RetainedKind::Directory,
            })
            .collect()
    }
    fn retain(
        &self,
        files: Arc<LibraryFiles<'db>>,
        guard: Custody<'db>,
        effect: RetainedEffect<'db>,
    ) {
        files.fence_metadata();
        self.retained
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(RetainedResources {
                _files: files,
                _guard: guard,
                effect,
            });
    }
    fn retire(
        &self,
        files: Arc<LibraryFiles<'db>>,
        guard: Custody<'db>,
        stage: OwnedStage<'db, 'db>,
    ) {
        if let Err(failure) = stage.retire_owned_temp(&guard) {
            self.retain(files, guard, RetainedEffect::Cleanup(failure));
        }
    }
    pub(crate) fn publish(
        &self,
        bytes: &[u8],
        checksum: librairii_core::Checksum,
        name: String,
        official: Option<librairii_core::Checksum>,
        cancel: &WorkCancellation,
    ) -> Result<MetadataPublication<'db>> {
        if cancel.is_cancelled() {
            return Err(StorageError::Cancelled);
        }
        let name = EntryName::parse(name)?;
        let database: &'db Database<'library> = self.database;
        let mut files = profile_admission(cancel, || database.metadata_files())?;
        let guard = loop {
            if cancel.is_cancelled() {
                return Err(StorageError::Cancelled);
            }
            match files.try_custody() {
                Err(StorageError::CustodyBusy) => {
                    std::thread::sleep(std::time::Duration::from_millis(2))
                }
                other => break other?,
            }
        };
        if let Some(id) = official
            && let Err(error) = files.official_artwork(&guard, id)
        {
            self.retain(Arc::new(files), guard, RetainedEffect::Directory);
            return Err(error);
        }
        let files = Arc::new(files);
        #[cfg(test)]
        let creation = if self.creation_fault {
            LibraryFiles::create_owned_stage_fault(files.clone(), &guard)
        } else {
            LibraryFiles::create_owned_stage(files.clone(), &guard)
        };
        #[cfg(not(test))]
        let creation = LibraryFiles::create_owned_stage(files.clone(), &guard);
        let mut stage = match creation {
            Ok(stage) => stage,
            Err(failure) => {
                if failure.retained_allocation().is_some() {
                    self.retain(files, guard, RetainedEffect::Creation(failure));
                    return Err(StorageError::CustodyRecoveryRequired);
                }
                return Err(StorageError::IntegrityFailed);
            }
        };
        let write = (|| {
            for chunk in bytes.chunks(64 * 1024) {
                if cancel.is_cancelled() {
                    return Err(StorageError::Cancelled);
                }
                stage
                    .write_all(chunk)
                    .map_err(|e| StorageError::io("write metadata payload", e))?;
            }
            let measured = stage.measure(bytes.len() as u64, cancel)?;
            if measured.checksum() != checksum {
                return Err(StorageError::IntegrityFailed);
            }
            Ok(measured)
        })();
        let measured = match write {
            Ok(v) => v,
            Err(error) => {
                self.retire(files, guard, stage);
                return Err(error);
            }
        };
        let mut receipt =
            match stage.publish_no_clobber(&guard, FileArea::Catalog, name, measured, cancel) {
                Ok(v) => v,
                Err(failure) => {
                    if failure.publish_uncertain {
                        self.retain(files, guard, RetainedEffect::Publication(failure));
                    } else {
                        self.retire(files, guard, failure.staged);
                    }
                    return Err(StorageError::CustodyRecoveryRequired);
                }
            };
        // Publication has crossed its filesystem boundary. Cancellation is
        // observed by the next SQL precommit check, never a false lost receipt.
        let verification = WorkCancellation::default();
        let verified = match &mut receipt {
            Publication::NewOwned(f) => f.revalidate(&guard, &verification),
            Publication::ReusedExisting { existing, .. } => {
                existing.revalidate(&guard, &verification)
            }
        };
        if let Err(error) = verified {
            self.retain(files, guard, RetainedEffect::Receipt(receipt));
            return Err(error);
        }
        Ok(MetadataPublication {
            files,
            guard,
            receipt,
        })
    }
    pub(crate) fn acknowledge_files<T>(
        &self,
        publication: MetadataPublication<'db>,
        outcome: Result<TransactionOutcome<T>>,
    ) -> Result<TransactionOutcome<T>> {
        if matches!(
            &outcome,
            Ok(TransactionOutcome::CommitUncertain(_)) | Err(StorageError::WorkerStopped)
        ) {
            self.retain(
                publication.files,
                publication.guard,
                RetainedEffect::Receipt(publication.receipt),
            );
        } else if let Err(failure) = publication
            .receipt
            .finish_metadata_stage(&publication.guard)
        {
            // The SQL outcome remains truthful even if private-stage cleanup
            // fails. Retain its actual handles/custody and fence clean closure.
            self.retain(
                publication.files,
                publication.guard,
                RetainedEffect::MetadataCleanup(failure),
            );
        }
        outcome
    }
}
#[derive(Debug)]
pub(crate) struct MetadataPublication<'db> {
    files: Arc<LibraryFiles<'db>>,
    guard: Custody<'db>,
    receipt: Publication<'db, 'db>,
}
#[cfg(test)]
mod tests {
    use super::*;
    use librairii_app::metadata::{ArtworkAsset, MediaType, normalize};
    use librairii_core::metadata::Locale;
    use librairii_testkit::TestRoots;

    #[test]
    fn known_metadata_acknowledgements_settle_stages_but_uncertainty_retains_receipts()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        for outcome in 0..4 {
            let roots = TestRoots::new()?;
            let library = crate::IsolatedLibrary::initialize_from_override(
                Some(roots.library().as_os_str()),
                "a0000000-0000-4000-8000-000000000001".parse()?,
            )?;
            let mut db = Database::open_isolated(&library)?;
            let store = MetadataStorage::new(&db);
            let bytes = b"exact raw catalog bytes";
            let publication = store.publish(
                bytes,
                librairii_app::metadata::digest(bytes),
                "raw.json".into(),
                None,
                &WorkCancellation::default(),
            )?;
            let acknowledgement = match outcome {
                0 => Ok(TransactionOutcome::Committed(())),
                1 => Ok(TransactionOutcome::ProvenNotCommitted(
                    StorageError::Cancelled,
                )),
                2 => Ok(TransactionOutcome::CommitUncertain(
                    StorageError::WorkerStopped,
                )),
                _ => Err(StorageError::WorkerStopped),
            };
            let result = store.acknowledge_files(publication, acknowledgement);
            assert!(matches!(
                (&result, outcome),
                (Ok(TransactionOutcome::Committed(())), 0)
                    | (
                        Ok(TransactionOutcome::ProvenNotCommitted(
                            StorageError::Cancelled
                        )),
                        1
                    )
                    | (
                        Ok(TransactionOutcome::CommitUncertain(
                            StorageError::WorkerStopped
                        )),
                        2
                    )
                    | (Err(StorageError::WorkerStopped), 3)
            ));
            assert_eq!(
                std::fs::read(roots.library().join("catalog/raw.json"))?,
                bytes
            );
            assert_eq!(
                std::fs::read_dir(roots.library().join("staging"))?.count(),
                usize::from(outcome >= 2)
            );
            assert_eq!(std::fs::read_dir(roots.library().join("trash"))?.count(), 0);
            if outcome >= 2 {
                assert_eq!(store.retained_failures(), vec![RetainedKind::Receipt]);
                assert!(db.metadata_files()?.try_custody().is_err());
            } else {
                assert!(store.retained_failures().is_empty());
                db.metadata_files()?.try_custody()?;
            }
            drop(store);
            db.close()?;
        }
        Ok(())
    }

    #[test]
    fn metadata_cleanup_failure_retains_custody_without_changing_committed_acknowledgement()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let roots = TestRoots::new()?;
        let library = crate::IsolatedLibrary::initialize_from_override(
            Some(roots.library().as_os_str()),
            "a0000000-0000-4000-8000-000000000001".parse()?,
        )?;
        let mut db = Database::open_isolated(&library)?;
        let store = MetadataStorage::new(&db);
        let bytes = b"exact raw catalog bytes";
        let publication = store.publish(
            bytes,
            librairii_app::metadata::digest(bytes),
            "raw.json".into(),
            None,
            &WorkCancellation::default(),
        )?;
        let private = std::fs::read_dir(roots.library().join("staging"))?
            .next()
            .ok_or("stage absent")??
            .path();
        std::fs::rename(&private, roots.library().join("staging/retained-original"))?;
        std::fs::create_dir(&private)?;
        std::fs::write(private.join("foreign"), b"foreign bytes")?;
        assert!(matches!(
            store.acknowledge_files(publication, Ok(TransactionOutcome::Committed(42)))?,
            TransactionOutcome::Committed(42)
        ));
        assert_eq!(store.retained_failures(), vec![RetainedKind::Cleanup]);
        assert_eq!(std::fs::read(private.join("foreign"))?, b"foreign bytes");
        assert_eq!(
            std::fs::read(roots.library().join("catalog/raw.json"))?,
            bytes
        );
        assert!(db.metadata_files()?.try_custody().is_err());
        drop(store);
        db.close()?;
        Ok(())
    }

    #[test]
    fn metadata_and_artwork_partial_creation_keep_original_handles_and_custody()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        const RAW: &[u8] =
            include_bytes!("../../../librairii-app/tests/fixtures/lunii-catalog.json");
        for artwork in [false, true] {
            let roots = TestRoots::new()?;
            let library = crate::IsolatedLibrary::initialize_from_override(
                Some(roots.library().as_os_str()),
                "a0000000-0000-4000-8000-000000000001".parse()?,
            )?;
            let mut db = Database::open_isolated(&library)?;
            let mut store = MetadataStorage::new(&db);
            let id = "a0000000-0000-4000-8000-000000000050".parse()?;
            let normalized = normalize(RAW.to_vec(), "en-GB")?;
            let opaque = normalized.artworks()[0].id;
            let begin = db
                .begin_catalog_sync(
                    id,
                    Locale::parse("en-GB")?,
                    "2026-10-21T12:00:00Z".into(),
                    WorkCancellation::default(),
                )?
                .wait()?;
            assert!(matches!(begin, TransactionOutcome::Committed(())));
            if artwork {
                assert!(matches!(
                    store.stage_catalog(
                        id,
                        normalized.clone(),
                        "2026-10-21T12:00:01Z".into(),
                        WorkCancellation::default()
                    )?,
                    TransactionOutcome::Committed(_)
                ));
                // The actual worker token must interrupt a cache publication
                // waiting on another native custody owner, even while its
                // current-thread HTTP executor cannot poll an async timer.
                let asset = ArtworkAsset::checked(
                    include_bytes!("../../../librairii-adapters/tests/fixtures/artwork/cover.png")
                        .to_vec(),
                    MediaType::Png,
                )?;
                let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let cancellable =
                    MetadataStorage::for_work(&db, WorkCancellation::for_worker(flag.clone()));
                let files = db.metadata_files()?;
                let guard = files.try_custody()?;
                std::thread::scope(
                    |scope| -> std::result::Result<(), Box<dyn std::error::Error>> {
                        let (sent, received) = std::sync::mpsc::sync_channel(1);
                        let cache = &cancellable;
                        let image = &asset;
                        let join = scope.spawn(move || {
                            let result = cache.persist_artwork(
                                opaque,
                                image,
                                "2026-10-21T12:00:02Z",
                                None,
                                None,
                            );
                            let _sent = sent.send(result);
                        });
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        flag.store(true, std::sync::atomic::Ordering::Release);
                        let result = received.recv_timeout(std::time::Duration::from_secs(1));
                        drop(guard);
                        join.join().map_err(|_| "cache worker panicked")?;
                        assert!(
                            matches!(result, Ok(Err(StorageError::Cancelled))),
                            "cache custody wait ignored its actual worker cancellation"
                        );
                        Ok(())
                    },
                )?;
                assert!(cancellable.retained_failures().is_empty());
                assert!(db.read_registered_artwork(opaque)?.cached.is_none());
            }
            store.creation_fault = true;
            if artwork {
                let asset = ArtworkAsset::checked(
                    include_bytes!("../../../librairii-adapters/tests/fixtures/artwork/cover.png")
                        .to_vec(),
                    MediaType::Png,
                )?;
                assert!(
                    store
                        .persist_artwork(opaque, &asset, "2026-10-21T12:00:02Z", None, None)
                        .is_err()
                );
            } else {
                assert!(
                    store
                        .stage_catalog(
                            id,
                            normalized,
                            "2026-10-21T12:00:01Z".into(),
                            WorkCancellation::default()
                        )
                        .is_err()
                );
            }
            assert_eq!(store.retained_failures(), vec![RetainedKind::Creation]);
            let retained = store.retained.lock().map_err(|_| "retained")?;
            let RetainedEffect::Creation(failure) = &retained[0].effect else {
                return Err("creation evidence".into());
            };
            let allocation = failure.retained_allocation().ok_or("actual allocation")?;
            assert!(allocation.payload_acquired());
            assert!(allocation.directory_acquired());
            let before = allocation
                .retained_payload_metadata()?
                .ok_or("actual payload handle")?;
            let path = roots
                .library()
                .join("staging")
                .join(allocation.directory_name())
                .join(allocation.payload_name());
            std::fs::remove_file(&path)?;
            std::fs::write(&path, b"foreign replacement")?;
            let after = allocation
                .retained_payload_metadata()?
                .ok_or("retained payload handle")?;
            assert_eq!(before.len(), after.len());
            assert_eq!(after.len(), 0);
            assert_eq!(std::fs::read(path)?, b"foreign replacement");
            drop(retained);
            assert!(db.metadata_files()?.try_custody().is_err());
            drop(store);
            db.close()?;
        }
        Ok(())
    }
}
