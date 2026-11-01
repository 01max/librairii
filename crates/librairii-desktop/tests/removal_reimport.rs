#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::{
    importing::OperationState,
    localization::Language,
    native_jobs::{Configuration, with_runtime},
};
use librairii_core::{
    CollectionId,
    tagging::{CreateDefinition, Kind},
};
use librairii_desktop::imports::{ImportExecutor, PrepareError, PreparedImport};
use librairii_storage::{
    Result, StorageError, collections,
    database::{Database, TransactionOutcome, WorkCancellation},
    native::NativeLibrary,
    operations_repository::TerminalRepository,
    platform::unix::LibraryFiles,
    profile::{NativeProfileStore, ProfileLocations},
    removal::Removals,
    settings::TargetPreview,
    shelves::{self, Curation},
    tagging::{Command, Response},
};
use librairii_testkit::TestRoots;
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
type TestResult<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn committed<T>(outcome: TransactionOutcome<T>) -> Result<T> {
    match outcome {
        TransactionOutcome::Committed(value) => Ok(value),
        TransactionOutcome::ProvenNotCommitted(error)
        | TransactionOutcome::CommitUncertain(error) => Err(error),
    }
}

fn import(db: &Database<'_>, path: &Path, id: &str) -> TestResult {
    let executor = ImportExecutor::new(db);
    let store = TerminalRepository::new(db, 1);
    with_runtime(
        &store,
        &executor,
        Configuration::default(),
        |handle| -> TestResult {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut prepared = PreparedImport::new(id.parse()?, vec![path.to_path_buf()])?;
            loop {
                match executor.offer(&handle, prepared) {
                    Ok(()) => break,
                    Err((PrepareError::Busy, returned)) if Instant::now() < deadline => {
                        prepared = returned
                    }
                    Err((error, _)) => return Err(error.into()),
                }
                thread::sleep(Duration::from_millis(2));
            }
            loop {
                if let Some(snapshot) = db.operation_snapshot(id.into())?.wait()?
                    && !matches!(
                        snapshot.operation.state,
                        OperationState::Queued | OperationState::Running
                    )
                {
                    assert_eq!(snapshot.operation.state, OperationState::Succeeded);
                    assert_eq!(snapshot.items.len(), 1);
                    assert_eq!(snapshot.items[0].outcome.code.as_deref(), Some("imported"));
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err("import did not settle".into());
                }
                thread::sleep(Duration::from_millis(2));
            }
        },
    )??;
    Ok(())
}

#[test]
fn actual_removed_archive_reimport_reconnects_shelf_state_without_manual_tags_after_reopen()
-> TestResult {
    let roots = TestRoots::new()?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("profile parent")?)?;
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(&roots.library().join("native"), &[], &Default::default())?,
        &profile,
        &locations,
        "a0000000-0000-4000-8000-000000000030".parse()?,
        Language::French,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let source = roots.profile().join("Original Été.ZIP");
    fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/archives/clockwork-forest-studio.zip"),
        &source,
    )?;
    let bytes = fs::read(&source)?;
    let mut db = Database::open_native(&native)?;
    import(&db, &source, "a0000000-0000-4000-8000-000000000031")?;
    let original = db
        .search(Default::default(), "fr-FR".into())?
        .wait()?
        .stories
        .into_iter()
        .next()
        .ok_or("imported story")?;
    let old_id = original.id;
    let uuid = original.uuid.clone();
    let Response::Definition(tag) = committed(
        db.edit_tags(
            Command::CreateDefinition(CreateDefinition {
                key: "manual-recovery".into(),
                label: "Manual recovery".into(),
                color: "#405CF5".into(),
                kind: Kind::Boolean,
            }),
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("tag definition".into());
    };
    committed(
        db.edit_tags(
            Command::Boolean {
                story_ids: vec![old_id],
                definition_id: tag.id,
                assigned: true,
            },
            Default::default(),
        )?
        .wait()?,
    )?;
    let collections::Response::Collection(collection) = committed(
        db.edit_collections(
            collections::Command::Create {
                name: "Original source".into(),
                criteria: Default::default(),
            },
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("collection".into());
    };
    let shelves::Response::Shelf(shelf) = committed(
        db.edit_shelves(
            shelves::Command::Create {
                name: "Approved recovery".into(),
            },
            Default::default(),
        )?
        .wait()?,
    )?
    else {
        return Err("shelf".into());
    };
    let shelf_id = shelf.id;
    committed(
        db.attach_shelf_sources(
            shelf_id,
            vec![CollectionId::new(collection.id)?],
            Default::default(),
        )?
        .wait()?,
    )?;
    let plan = db.shelf_update_preview(shelf_id, "fr-FR".into())?.wait()?;
    committed(
        db.apply_shelf_updates(
            plan,
            vec![shelves::Decision {
                uuid: uuid.parse::<librairii_core::StoryUuid>()?.into(),
                action: shelves::Action::Add,
            }],
            Default::default(),
        )?
        .wait()?,
    )?;
    committed(
        db.curate_shelf(
            shelf_id,
            Curation::Add(vec![uuid.parse()?]),
            "fr-FR".into(),
            Default::default(),
        )?
        .wait()?,
    )?;
    let missing = "90000000-0000-4000-8000-000000000098";
    let excluded = "90000000-0000-4000-8000-000000000099";
    committed(db.transaction(Default::default(), move |tx| {
        tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(?,?,9,1,'Unavailable approved member')", rusqlite::params![shelf_id.get(), missing])?;
        tx.execute("INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)", rusqlite::params![shelf_id.get(), excluded])?;
        // A later unrelated record prevents SQLite recycling the removed numeric ID.
        tx.execute("INSERT INTO stories(id,uuid,display_name_normalized) VALUES(?,?,'unrelated')", rusqlite::params![old_id+100, "90000000-0000-4000-8000-000000000097"])?;
        Ok(())
    })?.wait()?)?;
    let before = db.shelf_members(shelf_id)?.wait()?;
    assert_eq!(before.len(), 2);
    assert_eq!(before[0].uuid, uuid);
    assert!(before[0].manually_retained);
    assert_eq!(before[0].origins.len(), 1);
    let exclusions = db
        .read(move |tx| shelves::exclusions(tx, shelf_id))?
        .wait()?;
    let manual_count = |database: &Database<'_>| {
        database
            .read(|tx| {
                Ok(tx.query_row(
                    "SELECT count(*) FROM story_tag_assignments WHERE source='manual'",
                    [],
                    |r| r.get::<_, i64>(0),
                )?)
            })?
            .wait()
    };
    assert_eq!(manual_count(&db)?, 1);
    {
        let files = LibraryFiles::open_native(&db)?;
        let mut removals = Removals::new(&db, &files)?;
        assert_eq!(
            removals.remove(old_id, &WorkCancellation::default())?.uuid,
            uuid
        );
    }
    assert_eq!(manual_count(&db)?, 0);
    assert_eq!(db.shelf_members(shelf_id)?.wait()?, before);
    let page = db
        .shelf_page(shelf_id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert!(page.members[0].story.is_none());
    let recovery = roots.profile().join("manual recovery");
    fs::create_dir(&recovery)?;
    let recovered = recovery.join("Original Été.ZIP");
    let mut copied = false;
    for entry in fs::read_dir(native.layout().root().join("trash/removals"))? {
        let archived = entry?.path().join("Original Été.ZIP");
        if archived.is_file() {
            assert_eq!(fs::read(&archived)?, bytes);
            fs::copy(archived, &recovered)?;
            copied = true;
        }
    }
    assert!(
        copied,
        "manual recovery must use the actual removed archive"
    );
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    let native = NativeLibrary::reopen(&profile, &locations, &Default::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(manual_count(&db)?, 0);
    assert_eq!(db.shelf_members(shelf_id)?.wait()?, before);
    import(&db, &recovered, "a0000000-0000-4000-8000-000000000032")?;
    let uuid_for_read = uuid.clone();
    let new_id = db
        .read(move |tx| {
            Ok(tx.query_row(
                "SELECT id FROM stories WHERE uuid=?",
                [uuid_for_read],
                |r| r.get::<_, i64>(0),
            )?)
        })?
        .wait()?;
    assert_ne!(new_id, old_id);
    assert_eq!(manual_count(&db)?, 0);
    assert_eq!(db.shelf_members(shelf_id)?.wait()?, before);
    assert_eq!(
        db.read(move |tx| shelves::exclusions(tx, shelf_id))?
            .wait()?,
        exclusions
    );
    let page = db
        .shelf_page(shelf_id, Default::default(), "fr-FR".into())?
        .wait()?;
    assert_eq!(page.members[0].story.as_ref().map(|s| s.id), Some(new_id));
    assert!(page.members[1].story.is_none());
    assert_eq!(fs::read(&source)?, bytes);
    assert_eq!(fs::read(&recovered)?, bytes);
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    let native = NativeLibrary::reopen(&profile, &locations, &Default::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(manual_count(&db)?, 0);
    assert_eq!(db.shelf_members(shelf_id)?.wait()?, before);
    assert_eq!(
        db.read(move |tx| shelves::exclusions(tx, shelf_id))?
            .wait()?,
        exclusions
    );
    assert_eq!(
        db.shelf_page(shelf_id, Default::default(), "fr-FR".into())?
            .wait()?
            .members[0]
            .story
            .as_ref()
            .map(|s| s.id),
        Some(new_id)
    );
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    Ok(())
}
