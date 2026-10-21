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
#[test]
fn registered_artwork_cache_is_hashed_offline_and_refuses_corruption_or_aliases()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use librairii_app::metadata::{ArtworkAsset, MediaType};
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let image = include_bytes!("../../librairii-adapters/tests/fixtures/artwork/cover.png");
    let asset = ArtworkAsset::checked(image.to_vec(), MediaType::Png)?;
    let catalog = normalize(RAW.to_vec(), "en-GB")?;
    let id = catalog.artworks()[0].id;
    assert!(
        db.persist_artwork(id, &asset, "2026-10-21T12:00:00Z", None, None)
            .is_err()
    );
    let sync = "a0000000-0000-4000-8000-000000000030".parse()?;
    committed(
        db.begin_catalog_sync(
            sync,
            Locale::parse("en-GB")?,
            "2026-10-21T12:00:00Z".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    committed(db.stage_catalog(
        sync,
        catalog,
        "2026-10-21T12:00:01Z".into(),
        WorkCancellation::default(),
    )?)?;
    assert!(db.read_registered_artwork(id)?.cached.is_none());
    db.persist_artwork(
        id,
        &asset,
        "2026-10-21T12:00:02Z",
        Some("fixture-tag".into()),
        None,
    )?;
    assert_eq!(db.read_registered_artwork(id)?.cached, Some(asset.clone()));
    let path = db
        .read(move |tx| {
            Ok(tx.query_row(
                "SELECT managed_path FROM catalog_artworks WHERE id=?",
                [id.to_string()],
                |r| r.get::<_, String>(0),
            )?)
        })?
        .wait()?;
    db.close()?;
    let mut reopened = Database::open_isolated(&library)?;
    assert_eq!(reopened.read_registered_artwork(id)?.cached, Some(asset));
    std::fs::write(roots.library().join(&path), b"corruption")?;
    assert!(reopened.read_registered_artwork(id).is_err());
    std::fs::remove_file(roots.library().join(&path))?;
    let outside = roots
        .library()
        .parent()
        .ok_or("parent")?
        .join("outside-cover.png");
    std::fs::write(&outside, image)?;
    std::os::unix::fs::symlink(&outside, roots.library().join(path))?;
    assert!(reopened.read_registered_artwork(id).is_err());
    assert_eq!(std::fs::read(outside)?, image);
    reopened.close()?;
    Ok(())
}
#[test]
fn embedded_legacy_cover_reads_are_bounded_and_do_not_accept_arbitrary_paths()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let dir = roots
        .library()
        .join("catalog/embedded/123e4567-e89b-42d3-a456-426614174000");
    std::fs::create_dir_all(&dir)?;
    let image = include_bytes!("../../librairii-adapters/tests/fixtures/artwork/cover.png");
    let hash = librairii_app::metadata::digest(image);
    std::fs::write(dir.join(format!("{hash}.png")), image)?;
    let path = format!("catalog/embedded/123e4567-e89b-42d3-a456-426614174000/{hash}.png");
    committed(db.transaction(WorkCancellation::default(),move|tx|{tx.execute("INSERT INTO stories(id,uuid,display_name_normalized,embedded_artwork_path) VALUES(1,'123e4567-e89b-42d3-a456-426614174000','test',?)",[path])?;Ok(())})?.wait()?)?;
    assert_eq!(
        db.read_embedded_artwork(1)?
            .ok_or("embedded")?
            .bytes
            .as_ref(),
        image
    );
    committed(db.transaction(WorkCancellation::default(),|tx|{tx.execute("UPDATE stories SET embedded_artwork_path='catalog/official/elsewhere.png' WHERE id=1",[])?;Ok(())})?.wait()?)?;
    assert!(db.read_embedded_artwork(1).is_err());
    assert!(db.read_embedded_artwork(999)?.is_none());
    db.close()?;
    Ok(())
}
#[test]
fn frozen_go_catalog_normalization_and_artwork_url_cases_match()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use serde_json::json;
    let inputs: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/metadata/cases.json"))?;
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/metadata/reference.json"))?;
    assert_eq!(
        inputs.as_array().ok_or("inputs")?.len(),
        reference["cases"].as_array().ok_or("cases")?.len()
    );
    for (input, expected) in inputs
        .as_array()
        .ok_or("inputs")?
        .iter()
        .zip(reference["cases"].as_array().ok_or("cases")?)
    {
        let name = input["name"].as_str().ok_or("name")?;
        assert_eq!(input["name"], expected["name"]);
        let hex = input["payload"].as_str().ok_or("payload")?;
        let bytes = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair)?, 16).map_err(Into::into))
            .collect::<std::result::Result<Vec<u8>, Box<dyn std::error::Error>>>()?;
        let result = normalize(bytes, input["locale"].as_str().ok_or("locale")?);
        if input.get("hardening").is_some() {
            assert_eq!(
                expected["valid"], true,
                "{name} must observe a deliberate difference"
            );
            assert!(result.is_err(), "{name}");
            continue;
        }
        assert_eq!(
            result.is_ok(),
            expected["valid"].as_bool().ok_or("valid")?,
            "{name}"
        );
        if let Ok(catalog) = result {
            let stories:Vec<_>=catalog.records().iter().map(|r|json!({"uuid":r.uuid.to_string(),"title":r.title,"description":r.description,"author":r.author,"publisher":r.publisher,"language":r.language.as_str(),"duration":r.duration_seconds,"minimumAge":r.minimum_age,"maximumAge":r.maximum_age,"artworkId":r.artwork_id.map(|id|id.to_string()).unwrap_or_default(),"provenance":"lunii_catalog","sourceId":r.source_record_id,"updatedAt":r.source_updated_at})).collect();
            let artworks: Vec<_> = catalog
                .artworks()
                .iter()
                .map(|a| json!({"ID":a.id.to_string(),"SourceURL":a.source_url}))
                .collect();
            assert_eq!(json!(stories), expected["stories"], "{name}");
            assert_eq!(json!(artworks), expected["artworks"], "{name}");
        }
    }
    for p in reference["paths"].as_array().ok_or("paths")? {
        let result = librairii_app::metadata::artwork_url(p["path"].as_str().ok_or("path")?);
        assert_eq!(result.is_ok(), p["valid"].as_bool().ok_or("valid")?);
        if let Ok(url) = result {
            assert_eq!(url, p["url"]);
        }
    }
    Ok(())
}
#[test]
fn frozen_go_artwork_validation_bytes_hash_and_legacy_layout_match()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use librairii_app::metadata::{ArtworkAsset, MediaType};
    let inputs: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/metadata/artwork-cases.json"))?;
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/metadata/artwork-reference.json"))?;
    assert_eq!(
        inputs.as_array().ok_or("inputs")?.len(),
        expected.as_array().ok_or("expected")?.len()
    );
    for (input, expected) in inputs
        .as_array()
        .ok_or("inputs")?
        .iter()
        .zip(expected.as_array().ok_or("expected")?)
    {
        assert_eq!(input["name"], expected["name"]);
        let hex = input["payload"].as_str().ok_or("hex")?;
        let mut bytes = hex
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p)?, 16).map_err(Into::into))
            .collect::<std::result::Result<Vec<u8>, Box<dyn std::error::Error>>>()?;
        if let Some(repeat) = input["repeat"].as_u64().filter(|n| *n > 0) {
            bytes = vec![0; repeat as usize];
        }
        let media = MediaType::parse(input["media"].as_str().ok_or("media")?)?;
        let result = ArtworkAsset::checked(bytes, media);
        assert_eq!(
            result.is_ok(),
            expected["valid"].as_bool().ok_or("valid")?,
            "{}",
            input["name"]
        );
        if let Ok(asset) = result {
            assert_eq!(asset.checksum.to_string(), expected["hash"]);
            assert_eq!(asset.bytes.len(), expected["size"]);
            assert_eq!(asset.media.as_str(), expected["media"]);
        }
    }
    let roots = TestRoots::new()?;
    let library = IsolatedLibrary::initialize_from_override(
        Some(roots.library().as_os_str()),
        "a0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut db = Database::open_isolated(&library)?;
    let normalized = normalize(RAW.to_vec(), "en-GB")?;
    let id = normalized.artworks()[0].id;
    let sync = "a0000000-0000-4000-8000-000000000031".parse()?;
    committed(
        db.begin_catalog_sync(
            sync,
            Locale::parse("en-GB")?,
            "2026-10-21T12:00:00Z".into(),
            WorkCancellation::default(),
        )?
        .wait()?,
    )?;
    committed(db.stage_catalog(
        sync,
        normalized,
        "2026-10-21T12:00:01Z".into(),
        WorkCancellation::default(),
    )?)?;
    let image = include_bytes!("../../librairii-adapters/tests/fixtures/artwork/cover.png");
    let asset = ArtworkAsset::checked(image.to_vec(), MediaType::Png)?;
    db.persist_artwork(id, &asset, "2026-10-21T12:00:02Z", None, None)?;
    let path = db
        .read(move |tx| {
            Ok(tx.query_row(
                "SELECT managed_path FROM catalog_artworks WHERE id=?",
                [id.to_string()],
                |r| r.get::<_, String>(0),
            )?)
        })?
        .wait()?;
    assert_eq!(path, expected[0]["path"]);
    assert_eq!(db.read_registered_artwork(id)?.cached, Some(asset));
    db.close()?;
    Ok(())
}
