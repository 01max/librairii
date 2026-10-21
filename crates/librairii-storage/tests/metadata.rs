#![cfg(unix)]
use librairii_app::metadata::normalize;
use librairii_core::{JobId, metadata::Locale};
use librairii_storage::{
    IsolatedLibrary, Result, StorageError,
    database::{Database, TransactionOutcome, WorkCancellation},
    metadata::SnapshotStatus,
};
use librairii_testkit::TestRoots;
const RAW: &[u8] = include_bytes!("../../librairii-app/tests/fixtures/lunii-catalog.json");
fn committed<T>(out: TransactionOutcome<T>) -> Result<T> {
    match out {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e)
        }
    }
}
#[test]
fn staged_raw_is_exact_hashed_reopenable_and_rejected_rows_are_immutable()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let sync: JobId = "a0000000-0000-4000-8000-000000000002".parse()?;
    committed(
        db.begin_catalog_sync(
            sync,
            Locale::parse("en_GB")?,
            "2026-10-21T12:00:00Z".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    let normalized = normalize(RAW.to_vec(), "en-GB")?;
    let hash = normalized.checksum();
    let id = committed(db.stage_catalog(
        sync,
        normalized,
        "2026-10-21T12:00:01Z".into(),
        WorkCancellation::default(),
    )?)?;
    let snapshots = db.catalog_snapshots()?.wait()?;
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].id, id);
    assert_eq!(snapshots[0].hash, hash);
    assert_eq!(snapshots[0].status, SnapshotStatus::Staged);
    assert_eq!(
        std::fs::read(roots.library().join(&snapshots[0].raw_path))?,
        RAW
    );
    committed(db.reject_catalog(id)?.wait()?)?;
    assert!(matches!(
        db.reject_catalog(id)?.wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::Metadata(_))
    ));
    db.close()?;
    let mut reopened = Database::open_isolated(&library)?;
    assert_eq!(
        reopened.catalog_snapshots()?.wait()?[0].status,
        SnapshotStatus::Rejected
    );
    reopened.close()?;
    Ok(())
}
