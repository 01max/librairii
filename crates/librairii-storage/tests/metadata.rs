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
#[test]
fn resolved_fields_freshness_and_age_changes_preserve_manual_state_and_curated_members()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use librairii_app::metadata::RefreshSession;
    use librairii_core::{metadata::Freshness, query::QueryRequest, tagging::AgeConfig};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let locale = Locale::parse("en-GB")?;
    assert_eq!(
        db.catalog_status(locale.clone())?.wait()?.state,
        Freshness::NeverSynced
    );
    committed(db.transaction(WorkCancellation::default(),|tx|{
        tx.execute_batch("INSERT INTO stories(id,uuid,embedded_title,embedded_description,embedded_artwork_path,display_name_normalized,is_broken) VALUES(1,'123e4567-e89b-42d3-a456-426614174000','Embedded title','Embedded description','catalog/embedded/test.png','embedded title',1); INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(1,'story.zip','zip',printf('%064d',1),1,'archives/1','valid'); INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,position,is_protected) VALUES(10,'favorite','favorite','Favorite','#405CF5','boolean','user',0,0); INSERT INTO story_tag_assignments(story_id,definition_id,source) VALUES(1,10,'manual'); INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Approved','approved',0); INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(1,'123e4567-e89b-42d3-a456-426614174000',7,1,'Approved title');")?;Ok(())
    })?.wait()?)?;
    committed(
        db.seed_age_facets(AgeConfig::default(), WorkCancellation::default())?
            .wait()?,
    )?;
    committed(
        db.transaction(WorkCancellation::default(), |tx| {
            tx.execute(
                "UPDATE tag_definitions SET color='#405CF5' WHERE key='age'",
                [],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    let mut v: serde_json::Value = serde_json::from_slice(RAW)?;
    let row = v["response"]
        .as_object_mut()
        .ok_or("response")?
        .values_mut()
        .next()
        .ok_or("row")?;
    let infos = row["localized_infos"].as_object_mut().ok_or("infos")?;
    let info = infos.values_mut().next().ok_or("info")?;
    info["description"] = serde_json::json!("");
    info["image"] = serde_json::Value::Null;
    let payload = serde_json::to_vec(&v)?;
    for n in [20, 21] {
        let session = RefreshSession::begin(
            &db,
            format!("a0000000-0000-4000-8000-{n:012}").parse()?,
            "en-GB",
            &format!("2026-10-21T12:{n}:00Z"),
        )
        .map_err(|e| format!("{e:?}"))?;
        let report = session
            .finish(
                Ok(payload.clone()),
                &format!("2026-10-21T12:{n}:01Z"),
                || false,
            )
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(report.matched_stories, 1);
    }
    let snapshots = db.catalog_snapshots()?.wait()?;
    assert_eq!(snapshots[0].status, SnapshotStatus::Superseded);
    assert_eq!(snapshots[1].status, SnapshotStatus::Active);
    let page = db.search(QueryRequest::default(), "en-GB".into())?.wait()?;
    let s = &page.stories[0];
    assert_eq!(s.title, "The Clockwork Mountain");
    assert_eq!(s.description, "Embedded description");
    assert_eq!(s.sources.title, "official");
    assert_eq!(s.sources.description, "embedded");
    assert_eq!(s.artwork_id, "embedded:1");
    assert_eq!(s.sources.artwork, "embedded");
    assert!(s.broken);
    let official = s.official.as_ref().ok_or("official")?;
    assert_eq!(official.publisher, "Fixture Press");
    assert_eq!(official.language, "en-GB");
    // UI language is independent: a different metadata locale falls back locally.
    assert_eq!(
        db.search(QueryRequest::default(), "fr-FR".into())?
            .wait()?
            .stories[0]
            .title,
        "Embedded title"
    );
    let status = db.catalog_status(locale.clone())?.wait()?;
    assert_eq!(status.state, Freshness::Fresh);
    assert_eq!(status.matched_stories, 1);
    let session = RefreshSession::begin(
        &db,
        "a0000000-0000-4000-8000-000000000022".parse()?,
        "en-GB",
        "2026-10-21T12:22:00Z",
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(
        session
            .finish(
                Err(librairii_core::metadata::MetadataError::Network),
                "2026-10-21T12:22:01Z",
                || false
            )
            .is_err()
    );
    assert_eq!(
        db.catalog_status(locale)?.wait()?.state,
        Freshness::StaleCache
    );
    db.read(|tx| {
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM story_tag_assignments WHERE story_id=1 AND source='manual'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            tx.query_row(
                "SELECT count(*) FROM story_tag_assignments WHERE story_id=1 AND source='derived'",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            1
        );
        assert_eq!(
            tx.query_row(
                "SELECT color FROM tag_definitions WHERE key='age'",
                [],
                |r| r.get::<_, String>(0)
            )?,
            "#405CF5"
        );
        assert_eq!(
            tx.query_row(
                "SELECT position FROM shelf_stories WHERE shelf_id=1",
                [],
                |r| r.get::<_, i64>(0)
            )?,
            7
        );
        Ok(())
    })?
    .wait()?;
    db.close()?;
    Ok(())
}
