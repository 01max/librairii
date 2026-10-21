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
#[test]
fn activation_is_atomic_and_failure_cancel_or_invalid_data_preserve_last_good()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use librairii_app::metadata::{RefreshCode, RefreshSession};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let first = RefreshSession::begin(
        &db,
        "a0000000-0000-4000-8000-000000000010".parse()?,
        "en-GB",
        "2026-10-21T12:00:00Z",
    )
    .map_err(|e| format!("{e:?}"))?
    .finish(Ok(RAW.to_vec()), "2026-10-21T12:00:01Z", || false)
    .map_err(|e| format!("{e:?}"))?;
    for (n, payload, cancel, expected) in [
        (
            11,
            Ok(b"invalid".to_vec()),
            false,
            RefreshCode::InvalidCatalog,
        ),
        (
            12,
            Err(librairii_core::metadata::MetadataError::Network),
            false,
            RefreshCode::FetchFailed,
        ),
        (13, Ok(RAW.to_vec()), true, RefreshCode::Cancelled),
    ] {
        let id = format!("a0000000-0000-4000-8000-{n:012}").parse()?;
        let session = RefreshSession::begin(&db, id, "en-GB", "2026-10-21T12:01:00Z")
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            session.finish(payload, "2026-10-21T12:01:01Z", || cancel),
            Err(expected)
        );
        assert_eq!(
            db.catalog_snapshots()?
                .wait()?
                .iter()
                .filter(|s| s.status == SnapshotStatus::Active)
                .map(|s| s.id)
                .collect::<Vec<_>>(),
            vec![first.snapshot]
        );
    }
    // Drift forces Age reprojection failure after a valid snapshot is staged.
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE tag_definitions SET label='Drift' WHERE key='age'",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let id = "a0000000-0000-4000-8000-000000000014".parse()?;
    let session = RefreshSession::begin(&db, id, "en-GB", "2026-10-21T12:02:00Z")
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        session.finish(Ok(RAW.to_vec()), "2026-10-21T12:02:01Z", || false),
        Err(RefreshCode::PersistenceFailed)
    );
    let snapshots = db.catalog_snapshots()?.wait()?;
    assert_eq!(snapshots[0].status, SnapshotStatus::Active);
    assert_eq!(snapshots[1].status, SnapshotStatus::Rejected);
    db.close()?;
    Ok(())
}
