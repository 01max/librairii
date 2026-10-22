#![cfg(any(target_os = "macos", target_os = "linux"))]

#[path = "support/runtime.rs"]
mod runtime_support;

use librairii_storage::{
    StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::{Database, TransactionOutcome, WorkCancellation},
    native::NativeLibrary,
    platform::lock::LIBRARY_LOCK_FILE,
    profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations, read_legacy},
};
use librairii_testkit::TestRoots;
use std::{
    error::Error,
    fs,
    io::{Read, Seek, Write},
    path::Path,
};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const MANIFEST: &[u8] =
    include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");

#[test]
fn retired_export_temporaries_keep_descriptor_usage_bounded() -> TestResult {
    // Run alone so unrelated test threads cannot perturb process-wide FD counts.
    const CHILD: &str = "LIBRAIRII_EXPORT_DESCRIPTOR_PROBE";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "retired_export_temporaries_keep_descriptor_usage_bounded",
            ])
            .env(CHILD, "1")
            .output()?;
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return Ok(());
    }
    use librairii_app::ports::creation::{NewItem, NewOperation};
    use librairii_core::{
        OperationKind,
        operations::{ExportSource, ExportSourceType},
    };
    use librairii_storage::platform::unix::LibraryFiles;
    #[cfg(target_os = "macos")]
    let descriptors = Path::new("/dev/fd");
    #[cfg(target_os = "linux")]
    let descriptors = Path::new("/proc/self/fd");
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let destination = roots.profile().join("exports");
    fs::create_dir(&destination)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    let operation = committed_operation(
        db.create_operation(
            NewOperation {
                id: "a0000000-0000-4000-8000-000000000092".into(),
                kind: OperationKind::Export,
                source: Some(ExportSource {
                    source_type: ExportSourceType::Selection,
                    collection_ids: Vec::new(),
                    collection_names: Vec::new(),
                    shelf_ids: Vec::new(),
                    shelf_names: Vec::new(),
                }),
                destination: Some(destination.to_str().ok_or("UTF8 destination")?.into()),
                items: (1..=16)
                    .map(|n| NewItem {
                        source_name: format!("export-{n}"),
                        story_title: "Gone".into(),
                        story_uuid: format!("c0000000-0000-4000-8000-{n:012}"),
                        export_skip_code: "story_unavailable".into(),
                        ..Default::default()
                    })
                    .collect(),
            },
            "2026-10-22T00:00:00Z".into(),
            Default::default(),
        )?
        .wait()?,
    )?;
    for item in &operation.items {
        committed_operation(
            db.mark_operation_item_running(
                operation.operation.identity.id.clone(),
                item.identity.id,
                "2026-10-22T00:00:01Z".into(),
                Default::default(),
            )?
            .wait()?,
        )?;
    }
    {
        let files = LibraryFiles::open_native(&db)?;
        let guard = files.try_custody()?;
        let before = fs::read_dir(descriptors)?.count();
        for item in &operation.items {
            files
                .create_export_temporary(
                    &guard,
                    operation.operation.identity.id.clone(),
                    item.identity.id,
                )
                .map_err(|f| f.to_string())?
                .retire(&guard)
                .map_err(|f| f.to_string())?;
            assert_eq!(fs::read_dir(destination.as_path())?.count(), 0);
            assert!(
                native_state(&root)?["temporaries"]
                    .as_array()
                    .is_none_or(Vec::is_empty)
            );
            let after = fs::read_dir(descriptors)?.count();
            assert!(
                after <= before,
                "retired export leaked descriptors: {before} -> {after}"
            );
        }
    }
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_owned_read_source_retains_database_and_reads_after_stop_or_uncertainty() -> TestResult {
    use librairii_storage::platform::{
        hash_stream,
        unix::{EntryName, FileArea, LibraryFiles},
    };
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let entry = fs::read_dir(root.join("archives"))?
        .next()
        .ok_or("owned copied archive absent")??;
    let name = EntryName::parse(entry.file_name())?;
    let original = fs::read(entry.path())?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    {
        let files = LibraryFiles::open_native(&db)?;
        let mut source = files.open_read_source(FileArea::Archives, name.clone())?;
        let initial = source.initial();
        let cancel = WorkCancellation::default();
        let before = hash_stream(&mut source, initial.bytes(), &cancel)?;
        db.stop_ordinary()?;
        assert!(matches!(
            files.open_read_source(FileArea::Archives, name.clone()),
            Err(StorageError::IntakeClosed)
        ));
        source.rewind()?;
        assert_eq!(hash_stream(&mut source, initial.bytes(), &cancel)?, before);
        assert!(matches!(db.repair_transaction(|tx| {
            tx.execute_batch("CREATE TEMP TABLE read_source_parent(id INTEGER PRIMARY KEY); CREATE TEMP TABLE read_source_child(parent INTEGER REFERENCES read_source_parent(id) DEFERRABLE INITIALLY DEFERRED); INSERT INTO read_source_child VALUES (999);")?;
            Ok(())
        })?.wait()?, TransactionOutcome::CommitUncertain(_)));
        assert!(matches!(
            files.open_read_source(FileArea::Archives, name),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        assert_eq!(source.revalidate()?, initial);
        source.rewind()?;
        assert_eq!(hash_stream(&mut source, initial.bytes(), &cancel)?, before);
        assert_eq!(
            db.barrier_read(|tx| Ok(tx.query_row(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='read_source_child'",
                [],
                |row| row.get::<_, i64>(0)
            )?))?
            .wait()?,
            0
        );
    }
    db.close()?;
    drop(db);
    assert_eq!(fs::read(entry.path())?, original);
    let retained = native
        .finish()
        .err()
        .ok_or("read observation cleared fence")?;
    require_lock_held(retained.retained_library().layout().root())?;
    assert_eq!(native_state(&root)?["state"], "active");
    Ok(())
}

#[test]
fn native_files_retain_same_database_custody_and_drain_after_intake_stop() -> TestResult {
    use librairii_storage::platform::unix::{EntryName, FileArea, LibraryFiles, Publication};
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let source = Path::new(&copy.source_profile().profile().root).to_owned();
    let source_database = fs::read(source.join("db/librairii.sqlite3"))?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    {
        // This actual guard outlives its original Files instance and still
        // retains the same Database borrow and generation-wide exclusion.
        let guard = {
            let files = LibraryFiles::open_native(&db)?;
            files.try_custody()?
        };
        let files = LibraryFiles::open_native(&db)?;
        let peer = LibraryFiles::open_native(&db)?;
        assert!(matches!(peer.try_custody(), Err(StorageError::CustodyBusy)));
        let creation = files.create_stage(&guard);
        assert!(creation.is_ok(), "successful-stage fixture: {creation:?}");
        let Ok(mut stage) = creation else {
            return Err("successful-stage fixture failed".into());
        };
        stage.write_all(b"owned native payload")?;
        let digest = stage.measure(20, &WorkCancellation::default())?;
        let publication = stage
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                EntryName::parse("owned Éte # %2F.zip")?,
                digest,
                &WorkCancellation::default(),
            )
            .map_err(|failure| failure.cause)?;
        let Publication::NewOwned(mut committed_file) = publication else {
            return Err("new destination was not owned".into());
        };
        committed_file.revalidate(&guard, &WorkCancellation::default())?;
        let creation = peer.create_stage(&guard);
        assert!(creation.is_ok(), "successful-stage fixture: {creation:?}");
        let Ok(mut other) = creation else {
            return Err("successful-stage fixture failed".into());
        };
        other.write_all(b"owned native payload")?;
        let reused = other
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                EntryName::parse("owned Éte # %2F.zip")?,
                digest,
                &WorkCancellation::default(),
            )
            .map_err(|failure| failure.cause)?;
        let Publication::ReusedExisting {
            mut existing,
            staged,
        } = reused
        else {
            return Err("exact reuse acquired new ownership".into());
        };
        existing.revalidate(&guard, &WorkCancellation::default())?;
        assert_eq!(existing.identity(), committed_file.identity());
        let creation = files.create_stage(&guard);
        assert!(creation.is_ok(), "successful-stage fixture: {creation:?}");
        let Ok(mut cancelled_stage) = creation else {
            return Err("successful-stage fixture failed".into());
        };
        cancelled_stage.write_all(b"cancel keeps ownership")?;
        let measured = cancelled_stage.measure(22, &WorkCancellation::default())?;
        let cancelled = WorkCancellation::default();
        cancelled.cancel();
        let failure = cancelled_stage
            .publish_no_clobber(
                &guard,
                FileArea::Archives,
                EntryName::parse("cancelled.zip")?,
                measured,
                &cancelled,
            )
            .err()
            .ok_or("cancelled publication admitted")?;
        assert!(!failure.publish_uncertain);
        assert!(matches!(failure.cause, StorageError::Cancelled));
        assert!(!root.join("archives/cancelled.zip").exists());
        db.stop_ordinary()?;
        assert!(matches!(
            LibraryFiles::open_native(&db),
            Err(StorageError::IntakeClosed)
        ));
        assert!(matches!(
            peer.try_custody(),
            Err(StorageError::IntakeClosed)
        ));
        // Already-owned guarded resolution and SAME-writer readback survive
        // intake stop. Retiring the separate stage never removes reused bytes.
        staged
            .retire_owned_temp(&guard)
            .map_err(|failure| failure.cause)?;
        failure
            .staged
            .retire_owned_temp(&guard)
            .map_err(|failure| failure.cause)?;
        committed_file.revalidate(&guard, &WorkCancellation::default())?;
        assert_eq!(
            db.barrier_read(|tx| Ok(tx.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?))?
                .wait()?,
            1
        );
        require_lock_held(&root)?;
    }
    db.close()?;
    drop(db);
    assert_eq!(
        fs::read(root.join("archives/owned Éte # %2F.zip"))?,
        b"owned native payload"
    );
    assert_eq!(
        fs::read(source.join("db/librairii.sqlite3"))?,
        source_database
    );
    require_lock_held(&root)?;
    drop(native);

    let path = roots.library().join("isolated-only");
    let isolated = librairii_storage::IsolatedLibrary::initialize_from_override(
        Some(path.as_os_str()),
        "b0000000-0000-4000-8000-000000000001".parse()?,
    )?;
    let mut development = Database::open_isolated(&isolated)?;
    assert!(matches!(
        LibraryFiles::open_native(&development),
        Err(StorageError::IdentityMismatch)
    ));
    development.close()?;
    Ok(())
}

#[test]
fn native_files_keep_owned_bytes_and_writer_repair_when_commit_becomes_uncertain() -> TestResult {
    use librairii_storage::platform::unix::LibraryFiles;
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    {
        let files = LibraryFiles::open_native(&db)?;
        let guard = files.try_custody()?;
        let creation = files.create_stage(&guard);
        assert!(creation.is_ok(), "successful-stage fixture: {creation:?}");
        let Ok(mut stage) = creation else {
            return Err("successful-stage fixture failed".into());
        };
        stage.write_all(b"needs repair")?;
        let outcome = db.transaction(WorkCancellation::default(), |tx| {
            tx.execute_batch("CREATE TEMP TABLE files_parent(id INTEGER PRIMARY KEY); CREATE TEMP TABLE files_child(parent INTEGER REFERENCES files_parent(id) DEFERRABLE INITIALLY DEFERRED); INSERT INTO files_child VALUES (999);")?;
            Ok(())
        })?.wait()?;
        assert!(matches!(outcome, TransactionOutcome::CommitUncertain(_)));
        assert!(stage.write_all(b"not admitted").is_err());
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        let mut actual = [0; 12];
        stage.inspector().seek(std::io::SeekFrom::Start(0))?;
        stage.inspector().read_exact(&mut actual)?;
        assert_eq!(&actual, b"needs repair");
        assert_eq!(
            db.barrier_read(|tx| Ok(tx.query_row(
                "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='files_child'",
                [],
                |row| row.get::<_, i64>(0)
            )?))?
            .wait()?,
            0
        );
        assert!(matches!(
            db.repair_transaction(|_| Ok(()))?.wait()?,
            TransactionOutcome::Committed(())
        ));
        // Neither inspection/readback nor actual guard/owner drops clear the
        // native generation fence or authorize destructive temp compensation.
    }
    let directories: Vec<_> =
        fs::read_dir(root.join("staging"))?.collect::<std::io::Result<_>>()?;
    assert_eq!(directories.len(), 1);
    assert_eq!(
        fs::read(directories[0].path().join("payload"))?,
        b"needs repair"
    );
    db.close()?;
    drop(db);
    assert!(Database::open_native(&native).is_err());
    require_lock_held(&root)?;
    Ok(())
}

#[test]
fn native_files_refuse_substituted_directory_and_preserve_db_only_readback() -> TestResult {
    use librairii_storage::platform::unix::LibraryFiles;
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    {
        let files = LibraryFiles::open_native(&db)?;
        let guard = files.try_custody()?;
        fs::rename(root.join("archives"), root.join("retained-archives"))?;
        fs::create_dir(root.join("archives"))?;
        fs::write(root.join("archives/foreign"), b"untouched foreign bytes")?;
        let retained_creation = files.create_stage(&guard);
        assert!(retained_creation.is_err());
        assert!(matches!(
            files.try_custody(),
            Err(StorageError::CustodyRecoveryRequired)
        ));
        assert_eq!(
            db.barrier_read(|tx| Ok(tx.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?))?
                .wait()?,
            1
        );
        assert_eq!(
            fs::read(root.join("archives/foreign"))?,
            b"untouched foreign bytes"
        );
        assert_eq!(fs::read_dir(root.join("staging"))?.count(), 0);
    }
    db.close()?;
    drop(db);
    assert!(Database::open_native(&native).is_err());
    require_lock_held(&root)?;
    Ok(())
}

fn prepare(roots: &TestRoots, recovering: bool) -> TestResult<(IndependentCopy, ProfileLocations)> {
    prepare_with_recovery_file(roots, recovering, None)
}

fn prepare_with_recovery_file(
    roots: &TestRoots,
    recovering: bool,
    recovery_file: Option<&str>,
) -> TestResult<(IndependentCopy, ProfileLocations)> {
    let source = roots.library().join("legacy-source");
    fs::create_dir(&source)?;
    let corpus =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus/libraries/schema018");
    let mut manifest: serde_json::Value = serde_json::from_slice(MANIFEST)?;
    for entry in manifest.as_array().ok_or("manifest array")? {
        let path = entry["path"].as_str().ok_or("manifest path")?;
        if entry["directory"].as_bool() == Some(true) {
            fs::create_dir(source.join(path))?;
        } else {
            fs::copy(corpus.join(path), source.join(path))?;
        }
    }
    if let Some(name) = recovery_file {
        let bytes = b"unverified copied recovery is not cleanup authority";
        fs::write(source.join(name), bytes)?;
        let digest = librairii_storage::platform::hash_stream(
            &mut bytes.as_slice(),
            bytes.len() as u64,
            &WorkCancellation::default(),
        )?;
        manifest
            .as_array_mut()
            .ok_or("manifest array")?
            .push(serde_json::json!({
                "path": name, "size": bytes.len(), "sha256": digest.checksum().to_string(),
            }));
    }
    let profile = Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("source UTF8")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering,
    };
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
        ClosedManifest::decode(&serde_json::to_vec(&manifest)?)?,
        &WorkCancellation::default(),
    )?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    fs::create_dir_all(&locations.data)?;
    fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy context")?;
    fs::create_dir_all(legacy)?;
    fs::write(legacy.join("settings.json"), serde_json::to_vec(&profile)?)?;
    let copy = input.copy_to_new_root(
        &locations.data.join("owned Éte # %2F"),
        &WorkCancellation::default(),
    )?;
    Ok((copy, locations))
}

fn require_lock_held(root: &Path) -> TestResult {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    assert!(matches!(file.try_lock(), Err(fs::TryLockError::WouldBlock)));
    Ok(())
}

#[test]
fn native_binding_retains_real_copy_and_both_leases_without_sqlite_activation() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let source = Path::new(&copy.source_profile().profile().root).to_owned();
    let before_db = fs::read(copy.root().join("db/librairii.sqlite3"))?;
    let before_marker = fs::read(copy.root().join(".librairii-library"))?;
    let root = copy.root().to_owned();
    let source_settings = copy.source_profile().original_bytes().to_vec();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    native.revalidate(&WorkCancellation::default())?;
    assert_eq!(native.layout().root(), root);
    assert_eq!(native.identity().original_bytes(), before_marker);
    let bound = store.load()?.ok_or("bound profile missing")?;
    assert_eq!(bound.profile().root, root.to_str().ok_or("root UTF8")?);
    assert_eq!(
        bound.profile().library_id,
        "a0000000-0000-4000-8000-000000000001"
    );
    assert_eq!(bound.profile().language, "fr");
    assert!(bound.profile().complete);
    assert!(!bound.profile().recovering);
    require_lock_held(&root)?;
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(StorageError::ProfileInUse)
    ));
    assert_eq!(fs::read(root.join("db/librairii.sqlite3"))?, before_db);
    assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, before_db);
    assert_eq!(fs::read(root.join(".librairii-library"))?, before_marker);
    assert!(!root.join("db/librairii.sqlite3-wal").exists());
    assert!(!root.join("db/librairii.sqlite3-shm").exists());
    assert_eq!(
        source_settings,
        serde_json::to_vec(&Profile {
            root: source.to_str().ok_or("source UTF8")?.into(),
            ..bound.profile().clone()
        })?
    );
    drop(native);
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    lock.try_lock()?;
    // The native profile namespace is retained by its separate real owner.
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(StorageError::ProfileInUse)
    ));
    drop(store);
    assert!(
        NativeProfileStore::open_native(&locations.native)?
            .load()?
            .is_some()
    );
    Ok(())
}

#[test]
fn active_binding_excludes_root_raw_id_and_recovery_changes_but_allows_language() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    store.update(|p| {
        p.language = "en".into();
        Ok(())
    })?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    assert_eq!(store.load()?.ok_or("profile")?.profile().language, "en");
    let before = fs::read(locations.native.join("settings.json"))?;
    for change in ["root", "id", "complete", "recovering"] {
        let result = store.update(|p| {
            match change {
                "root" => p.root.push_str("/replacement"),
                "id" => p.library_id = p.library_id.to_ascii_uppercase(),
                "complete" => p.complete = false,
                "recovering" => p.recovering = true,
                _ => return Err(StorageError::InvalidProfile),
            }
            Ok(())
        });
        assert!(
            matches!(result, Err(StorageError::ProfileBindingActive)),
            "{change}"
        );
        assert_eq!(fs::read(locations.native.join("settings.json"))?, before);
        native.revalidate(&WorkCancellation::default())?;
    }
    std::thread::scope(|scope| -> TestResult {
        let update = scope.spawn(|| {
            store.update(|p| {
                p.language = "fr".into();
                Ok(())
            })
        });
        let observation = native.revalidate(&WorkCancellation::default());
        assert!(matches!(
            observation,
            Ok(()) | Err(StorageError::ProfileBusy)
        ));
        match update.join().map_err(|_| "language updater panicked")? {
            Ok(_) => (),
            Err(StorageError::ProfileBusy) => {
                store.update(|p| {
                    p.language = "fr".into();
                    Ok(())
                })?;
            }
            Err(error) => return Err(error.into()),
        }
        Ok(())
    })?;
    native.revalidate(&WorkCancellation::default())?;
    assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
    drop(native);
    assert!(matches!(
        store.update(|p| {
            p.complete = false;
            Ok(())
        }),
        Err(StorageError::ProfileRestartRequired)
    ));
    Ok(())
}

#[test]
fn binding_refuses_unrebound_recovery_or_wrong_namespace_and_retains_copy() -> TestResult {
    for scenario in [
        "source-recovering",
        "journal",
        "native-recovering",
        "wrong-native",
        "legacy-overlap",
        "wrong-data",
        "cancelled",
        "copied-journal",
        "copied-owner-marker",
        "source-journal",
        "go-discoverable-native",
    ] {
        let roots = TestRoots::new()?;
        let recovery_file = match scenario {
            "copied-journal" => Some("relocation.json"),
            "copied-owner-marker" => Some(".librairii-move-owner"),
            _ => None,
        };
        let (copy, mut locations) =
            prepare_with_recovery_file(&roots, scenario == "source-recovering", recovery_file)?;
        let root = copy.root().to_owned();
        let before_db = fs::read(root.join("db/librairii.sqlite3"))?;
        if scenario == "go-discoverable-native" {
            let go_name = locations
                .legacy
                .as_ref()
                .and_then(|path| path.file_name())
                .ok_or("Go namespace")?;
            locations.native = roots.profile().join("other-config").join(go_name);
            fs::create_dir_all(locations.native.parent().ok_or("config parent")?)?;
        }
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        match scenario {
            "journal" => fs::write(
                locations.native.join("relocation.json"),
                b"unrebound foreign bytes",
            )?,
            "native-recovering" => {
                // Foreign profile DATA is seeded before first cache load;
                // public update must no longer publish root/recovery authority.
                fs::write(
                    locations.native.join("settings.json"),
                    serde_json::to_vec(&Profile {
                        root: root.to_str().ok_or(StorageError::InvalidProfile)?.into(),
                        library_id: copy.identity().spelling().into(),
                        recovering: true,
                        ..Profile::setup()
                    })?,
                )?;
            }
            "wrong-native" => {
                locations.native = roots.profile().to_owned();
            }
            "legacy-overlap" => {
                locations.legacy = Some(locations.data.clone());
            }
            "wrong-data" => {
                locations.data = roots.profile().to_owned();
            }
            "source-journal" => fs::write(
                locations
                    .legacy
                    .as_ref()
                    .ok_or("legacy namespace")?
                    .join("relocation.json"),
                b"legacy authority stays untouched",
            )?,
            _ => (),
        }
        let cancellation = WorkCancellation::default();
        if scenario == "cancelled" {
            cancellation.cancel();
        }
        let failed = NativeLibrary::bind_independent(copy, &store, &locations, &cancellation)
            .err()
            .ok_or("unsafe binding accepted")?;
        match scenario {
            "source-recovering"
            | "native-recovering"
            | "journal"
            | "copied-journal"
            | "copied-owner-marker"
            | "source-journal" => assert!(
                matches!(
                    failed.storage_error(),
                    StorageError::RecoveryBindingRequired
                ),
                "{scenario}: {failed}"
            ),
            "cancelled" => assert!(matches!(failed.storage_error(), StorageError::Cancelled)),
            _ => assert!(matches!(failed.storage_error(), StorageError::PathEscape)),
        }
        assert_eq!(failed.retained_copy().root(), root, "{scenario}");
        failed
            .retained_copy()
            .revalidate(&WorkCancellation::default())?;
        assert_eq!(failed.retained_profile().directory(), store.directory());
        require_lock_held(&root)?;
        assert_eq!(fs::read(root.join("db/librairii.sqlite3"))?, before_db);
        if scenario == "journal" {
            assert_eq!(
                fs::read(store.directory().join("relocation.json"))?,
                b"unrebound foreign bytes"
            );
        }
        if scenario == "source-journal" {
            assert_eq!(
                fs::read(
                    locations
                        .legacy
                        .as_ref()
                        .ok_or("legacy namespace")?
                        .join("relocation.json")
                )?,
                b"legacy authority stays untouched"
            );
        }
        if scenario != "native-recovering" {
            assert!(read_legacy(store.directory())?.is_none(), "{scenario}");
        }
        drop(failed);
    }
    Ok(())
}

#[test]
fn changed_copy_or_profile_fences_binding_until_real_namespace_restart() -> TestResult {
    for scenario in ["copy", "profile", "root-substitution", "journal"] {
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|failure| failure.to_string())?;
        let cancelled = WorkCancellation::default();
        cancelled.cancel();
        assert!(matches!(
            native.revalidate(&cancelled),
            Err(StorageError::Cancelled)
        ));
        native.revalidate(&WorkCancellation::default())?;
        if scenario == "copy" {
            fs::write(root.join("unknown-recovery-file"), b"retain changed bytes")?;
        } else if scenario == "profile" {
            let mut profile = store.load()?.ok_or("profile")?.profile().clone();
            profile.library_id = profile.library_id.to_ascii_uppercase();
            fs::write(
                locations.native.join("settings.json"),
                serde_json::to_vec(&profile)?,
            )?;
        } else if scenario == "root-substitution" {
            fs::rename(&root, locations.data.join("retained-actual-copy"))?;
            fs::create_dir(&root)?;
            fs::write(root.join("foreign"), b"substituted root stays untouched")?;
        } else {
            fs::write(
                locations.native.join("relocation.json"),
                b"unverified recovery stays opaque",
            )?;
        }
        assert!(native.revalidate(&WorkCancellation::default()).is_err());
        drop(native);
        // Drop cannot clear the unresolved generation; no SQL-runtime reset or
        // receipt/path can turn changed authority into a successful binding.
        assert!(matches!(
            store.update(|p| {
                p.language = "en".into();
                Ok(())
            }),
            Err(StorageError::ProfileRestartRequired)
        ));
        assert!(NativeProfileStore::open_native(&locations.native).is_err());
        drop(store);
        let reopened = NativeProfileStore::open_native(&locations.native)?;
        assert!(reopened.load()?.is_some());
        if scenario == "copy" {
            assert_eq!(
                fs::read(root.join("unknown-recovery-file"))?,
                b"retain changed bytes"
            );
        }
        if scenario == "root-substitution" {
            assert_eq!(
                fs::read(root.join("foreign"))?,
                b"substituted root stays untouched"
            );
            assert_eq!(fs::read_dir(&root)?.count(), 1);
        }
        if scenario == "journal" {
            assert_eq!(
                fs::read(locations.native.join("relocation.json"))?,
                b"unverified recovery stays opaque"
            );
        }
    }
    Ok(())
}

#[test]
fn concurrent_independent_bindings_keep_one_generation_and_the_losing_copy_owned() -> TestResult {
    let roots_a = TestRoots::new()?;
    let roots_b = TestRoots::new()?;
    let (copy_a, locations_a) = prepare(&roots_a, false)?;
    let (copy_b, mut locations_b) = prepare(&roots_b, false)?;
    locations_b.native.clone_from(&locations_a.native);
    let root_a = copy_a.root().to_owned();
    let root_b = copy_b.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations_a.native)?;
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| -> TestResult {
        let left = scope.spawn(|| {
            barrier.wait();
            NativeLibrary::bind_independent(
                copy_a,
                &store,
                &locations_a,
                &WorkCancellation::default(),
            )
        });
        let right = scope.spawn(|| {
            barrier.wait();
            NativeLibrary::bind_independent(
                copy_b,
                &store,
                &locations_b,
                &WorkCancellation::default(),
            )
        });
        let a = left.join().map_err(|_| "binding A panicked")?;
        let b = right.join().map_err(|_| "binding B panicked")?;
        let (native, failed) = match (a, b) {
            (Ok(native), Err(failed)) | (Err(failed), Ok(native)) => (native, failed),
            _ => return Err("exactly one binding must own this profile generation".into()),
        };
        assert!(matches!(
            failed.storage_error(),
            StorageError::ProfileBindingActive | StorageError::ProfileBusy
        ));
        assert_ne!(native.layout().root(), failed.retained_copy().root());
        native.revalidate(&WorkCancellation::default())?;
        failed
            .retained_copy()
            .revalidate(&WorkCancellation::default())?;
        require_lock_held(&root_a)?;
        require_lock_held(&root_b)?;
        assert_eq!(
            store.load()?.ok_or("profile")?.profile().root,
            native.layout().root().to_str().ok_or("root UTF8")?
        );
        Ok(())
    })?;
    Ok(())
}

#[test]
fn native_sqlite_uses_same_five_workers_and_releases_runtime_only_after_real_close() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let marker = fs::read(root.join(".librairii-library"))?;
    let source = Path::new(&copy.source_profile().profile().root).to_owned();
    let source_db = fs::read(source.join("db/librairii.sqlite3"))?;
    let archives: Vec<_> = fs::read_dir(root.join("archives"))?
        .map(|entry| {
            let entry = entry?;
            Ok((entry.file_name(), fs::read(entry.path())?))
        })
        .collect::<std::io::Result<_>>()?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(db.diagnostics().len(), 5);
    assert_eq!(
        db.diagnostics()
            .iter()
            .filter(|entry| entry.query_only == 0)
            .count(),
        1
    );
    assert_eq!(
        db.diagnostics()
            .iter()
            .filter(|entry| entry.query_only == 1)
            .count(),
        4
    );
    for entry in db.diagnostics() {
        assert_eq!(entry.journal_mode, "wal");
        assert_eq!(entry.foreign_keys, 1);
        assert_eq!(entry.busy_timeout_ms, 5000);
    }
    assert!(matches!(
        Database::open_native(&native),
        Err(StorageError::RuntimeInUse)
    ));
    let outcome = db.transaction(WorkCancellation::default(), |tx| {
        tx.execute_batch("CREATE TEMP TABLE native_probe(value TEXT); INSERT INTO native_probe VALUES ('owned');")?;
        Ok(())
    })?.wait()?;
    assert!(matches!(outcome, TransactionOutcome::Committed(())));
    assert_eq!(
        db.barrier_read(
            |tx| Ok(tx.query_row("SELECT value FROM native_probe", [], |row| row
                .get::<_, String>(0))?)
        )?
        .wait()?,
        "owned"
    );
    assert_eq!(
        db.read(|tx| Ok(
            tx.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))?
        ))?
        .wait()?,
        18
    );
    native.revalidate(&WorkCancellation::default())?;
    db.stop_ordinary()?;
    assert!(matches!(
        db.read(|_| Ok(())),
        Err(StorageError::IntakeClosed)
    ));
    assert!(matches!(
        db.repair_transaction(|_| Ok(()))?.wait()?,
        TransactionOutcome::Committed(())
    ));
    db.close()?;
    drop(db);
    require_lock_held(&root)?;
    let mut reopened = Database::open_native(&native)?;
    reopened.close()?;
    drop(reopened);
    assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, source_db);
    assert_eq!(fs::read(root.join(".librairii-library"))?, marker);
    for (name, bytes) in archives {
        assert_eq!(fs::read(root.join("archives").join(name))?, bytes);
    }
    drop(native);
    assert!(matches!(
        store.update(|p| {
            p.complete = false;
            Ok(())
        }),
        Err(StorageError::ProfileRestartRequired)
    ));
    Ok(())
}

#[test]
fn native_commit_uncertainty_retains_repair_and_fence_across_database_close_and_drop() -> TestResult
{
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    let outcome = db.transaction(WorkCancellation::default(), |tx| {
        tx.execute_batch("CREATE TEMP TABLE native_parent(id INTEGER PRIMARY KEY); CREATE TEMP TABLE native_child(parent INTEGER REFERENCES native_parent(id) DEFERRABLE INITIALLY DEFERRED); INSERT INTO native_child VALUES (999);")?;
        Ok(())
    })?.wait()?;
    assert!(matches!(
        outcome,
        TransactionOutcome::CommitUncertain(StorageError::Sqlite(_))
    ));
    assert!(
        db.transaction(WorkCancellation::default(), |_| Ok(()))
            .is_err()
    );
    // One actual writer-barrier transaction observes rollback of the failed
    // CREATE/INSERT; observation is not a fence-reset or generic retry grant.
    assert_eq!(
        db.barrier_read(|tx| Ok(tx.query_row(
            "SELECT COUNT(*) FROM sqlite_temp_master WHERE name='native_child'",
            [],
            |row| row.get::<_, i64>(0)
        )?))?
        .wait()?,
        0
    );
    assert!(matches!(db.repair_transaction(|tx| {
        tx.execute_batch("CREATE TEMP TABLE native_repair(value TEXT); INSERT INTO native_repair VALUES ('reported');")?;
        Ok(())
    })?.wait()?, TransactionOutcome::Committed(())));
    assert_eq!(
        db.barrier_read(|tx| Ok(tx
            .query_row("SELECT value FROM native_repair", [], |row| row
                .get::<_, String>(0))?))?
            .wait()?,
        "reported"
    );
    db.close()?;
    drop(db);
    assert!(Database::open_native(&native).is_err());
    require_lock_held(&root)?;
    drop(native);
    assert!(matches!(
        store.update(|_| Ok(())),
        Err(StorageError::ProfileRestartRequired)
    ));
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(StorageError::ProfileInUse)
    ));
    Ok(())
}

#[test]
#[expect(
    clippy::panic,
    reason = "inject a real owned SQLite worker panic to prove failed close retains native exclusion"
)]
fn native_worker_failure_joins_and_never_acknowledges_closed_replacement_generation() -> TestResult
{
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    let outcome = db
        .transaction::<()>(WorkCancellation::default(), |_| {
            panic!("injected actual writer exit");
        })?
        .wait();
    assert!(matches!(outcome, Err(StorageError::WorkerStopped)));
    assert!(matches!(db.close(), Err(StorageError::WorkerStopped)));
    assert!(matches!(db.close(), Err(StorageError::RuntimeFenced)));
    drop(db);
    assert!(Database::open_native(&native).is_err());
    require_lock_held(&root)?;
    let failure = native
        .finish()
        .err()
        .ok_or("failed DB close minted Closed")?;
    assert_eq!(native_state(&root)?["state"], "active");
    require_lock_held(&root)?;
    drop(failure);
    assert!(matches!(
        store.update(|_| Ok(())),
        Err(StorageError::ProfileRestartRequired)
    ));
    Ok(())
}

#[test]
fn native_database_directory_substitution_is_refused_before_sqlite_mutates_foreign_bytes()
-> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    fs::rename(root.join("db"), root.join("retained-original-db"))?;
    fs::create_dir(root.join("db"))?;
    fs::write(
        root.join("db/librairii.sqlite3"),
        b"foreign file is not SQLite authority",
    )?;
    assert!(Database::open_native(&native).is_err());
    assert_eq!(
        fs::read(root.join("db/librairii.sqlite3"))?,
        b"foreign file is not SQLite authority"
    );
    assert_eq!(fs::read_dir(root.join("db"))?.count(), 1);
    require_lock_held(&root)?;
    drop(native);
    assert!(matches!(
        store.update(|_| Ok(())),
        Err(StorageError::ProfileRestartRequired)
    ));
    Ok(())
}

#[test]
fn poisoned_native_profile_fences_queued_work_but_retains_actual_commit_and_repair() -> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|failure| failure.to_string())?;
    let mut db = Database::open_native(&native)?;
    let (started_tx, started_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    // Bounded test-only scheduling seam; product DB closures remain DB-only.
    let first = db.transaction(WorkCancellation::default(), move |tx| {
        tx.execute_batch("CREATE TEMP TABLE prior_commit(value TEXT); INSERT INTO prior_commit VALUES ('real committed fact');")?;
        started_tx.send(()).map_err(|_| StorageError::WorkerStopped)?;
        release_rx.recv().map_err(|_| StorageError::WorkerStopped)?;
        Ok("committed identity fact".to_owned())
    })?;
    started_rx.recv()?;
    let ran = Arc::new(AtomicBool::new(false));
    let observe = ran.clone();
    let queued = db.transaction(WorkCancellation::default(), move |_| {
        observe.store(true, Ordering::Release);
        Ok(())
    })?;
    let mut changed = store.load()?.ok_or("profile")?.profile().clone();
    changed.library_id = changed.library_id.to_ascii_uppercase();
    fs::write(
        locations.native.join("settings.json"),
        serde_json::to_vec(&changed)?,
    )?;
    assert!(matches!(
        store.update(|_| Ok(())),
        Err(StorageError::ProfileChanged)
    ));
    release_tx.send(())?;
    assert!(
        matches!(first.wait()?, TransactionOutcome::Committed(value) if value == "committed identity fact")
    );
    assert!(matches!(
        queued.wait()?,
        TransactionOutcome::ProvenNotCommitted(StorageError::RuntimeFenced)
    ));
    assert!(!ran.load(Ordering::Acquire));
    assert!(
        db.transaction(WorkCancellation::default(), |_| Ok(()))
            .is_err()
    );
    db.stop_ordinary()?;
    assert_eq!(
        db.barrier_read(
            |tx| Ok(tx.query_row("SELECT value FROM prior_commit", [], |row| row
                .get::<_, String>(0))?)
        )?
        .wait()?,
        "real committed fact"
    );
    assert!(matches!(
        db.repair_transaction(|_| Ok(()))?.wait()?,
        TransactionOutcome::Committed(())
    ));
    db.close()?;
    drop(db);
    assert!(Database::open_native(&native).is_err());
    drop(native);
    assert!(matches!(
        store.load(),
        Err(StorageError::ProfileRestartRequired)
    ));
    Ok(())
}

const NATIVE_STATE: &str = ".librairii-native-state.json";
fn native_state(root: &Path) -> TestResult<serde_json::Value> {
    Ok(serde_json::from_slice(&fs::read(root.join(NATIVE_STATE))?)?)
}

#[test]
fn native_clean_close_reopens_next_epoch_without_rematching_historical_database_hash() -> TestResult
{
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let original_db = fs::read(root.join("db/librairii.sqlite3"))?;
    let raw_id = copy.identity().spelling().to_owned();
    let source = copy.source_profile().profile().root.clone();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    assert_eq!(native_state(&root)?["state"], "imported_closed");
    assert_eq!(native_state(&root)?["epoch"], 0);
    let mut db = Database::open_native(&native)?;
    assert_eq!(native_state(&root)?["state"], "active");
    assert_eq!(native_state(&root)?["epoch"], 1);
    assert_eq!(native_state(&root)?["library_id_raw"], raw_id);
    assert!(matches!(db.transaction(WorkCancellation::default(), |tx| {
        tx.execute("UPDATE stories SET embedded_description='native persisted Éte' WHERE id=(SELECT MIN(id) FROM stories)", [])?;
        Ok(())
    })?.wait()?, TransactionOutcome::Committed(())));
    db.close()?;
    drop(db);
    require_lock_held(&root)?;
    native.finish().map_err(|f| f.to_string())?;
    let closed = native_state(&root)?;
    assert_eq!(closed["state"], "closed");
    assert_eq!(closed["epoch"], 1);
    assert_ne!(fs::read(root.join("db/librairii.sqlite3"))?, original_db);
    assert_eq!(
        fs::read(Path::new(&source).join("db/librairii.sqlite3"))?,
        original_db
    );
    let native = NativeLibrary::reopen(&store, &locations, &WorkCancellation::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(native_state(&root)?["epoch"], 2);
    assert_eq!(native_state(&root)?["state"], "active");
    assert_eq!(
        db.read(|tx| Ok(tx.query_row(
            "SELECT embedded_description FROM stories ORDER BY id LIMIT 1",
            [],
            |r| r.get::<_, String>(0)
        )?))?
        .wait()?,
        "native persisted Éte"
    );
    assert_eq!(native.identity().spelling(), raw_id);
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    assert_eq!(native_state(&root)?["epoch"], 2);
    assert_eq!(native_state(&root)?["state"], "closed");
    assert!(matches!(
        store.update(|p| {
            p.root.push_str("/other");
            Ok(())
        }),
        Err(StorageError::ProfileBindingActive)
    ));
    Ok(())
}

#[test]
fn native_same_owner_database_reopen_revalidates_active_record_before_sqlite() -> TestResult {
    let mut admitted = Vec::new();
    for phase in ["closed", "corrupt"] {
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        let before = fs::read(root.join("db/librairii.sqlite3"))?;
        let mut changed = native_state(&root)?;
        assert_eq!(changed["state"], "active");
        assert_eq!(changed["epoch"], 1);
        let bytes = if phase == "closed" {
            changed["state"] = serde_json::json!("closed");
            serde_json::to_vec(&changed)?
        } else {
            b"corrupt retained native state".to_vec()
        };
        fs::write(root.join(NATIVE_STATE), &bytes)?;

        match Database::open_native(&native) {
            Ok(mut unexpected) => {
                // RED evidence counts actual initialized connection owners,
                // then explicitly drains them before finishing this case.
                admitted.push((phase, unexpected.diagnostics().len()));
                unexpected.close()?;
            }
            Err(_) => {
                // No SQLite open/sidecar or ordinary callback is needed to
                // notice the retained record no longer matches Active bytes.
                for suffix in ["-wal", "-shm", "-journal"] {
                    assert!(!root.join(format!("db/librairii.sqlite3{suffix}")).exists());
                }
                assert!(store.update(|_| Ok(())).is_err());
            }
        }
        assert_eq!(fs::read(root.join(NATIVE_STATE))?, bytes);
        assert_eq!(fs::read(root.join("db/librairii.sqlite3"))?, before);
        require_lock_held(&root)?;
        let retained = native
            .finish()
            .err()
            .ok_or("tampered record became Closed")?;
        assert_eq!(fs::read(root.join(NATIVE_STATE))?, bytes);
        require_lock_held(retained.retained_library().layout().root())?;
    }
    assert!(
        admitted.is_empty(),
        "tampered record admitted SQLite owners: {admitted:?}"
    );
    Ok(())
}

#[test]
fn native_active_abandonment_and_unsettled_trash_never_mint_clean_restart() -> TestResult {
    for artifact in [None, Some("staging"), Some("trash")] {
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|f| f.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        if let Some(area) = artifact {
            fs::write(
                root.join(area).join("unsettled-owner"),
                b"retain recovery bytes",
            )?;
            let failure = native.finish().err().ok_or("unsettled root closed")?;
            require_lock_held(&root)?;
            assert_eq!(
                fs::read(root.join(area).join("unsettled-owner"))?,
                b"retain recovery bytes"
            );
            assert_eq!(native_state(&root)?["state"], "active");
            assert!(matches!(
                store.update(|_| Ok(())),
                Err(StorageError::ProfileBusy)
            ));
            drop(failure);
        } else {
            drop(native);
        }
        assert_eq!(native_state(&root)?["state"], "active");
        assert!(store.update(|_| Ok(())).is_err());
        drop(store);
        let store = NativeProfileStore::open_native(&locations.native)?;
        assert!(matches!(
            NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
            Err(StorageError::RecoveryBindingRequired)
        ));
        assert_eq!(native_state(&root)?["state"], "active");
    }
    Ok(())
}

#[test]
fn native_finalizer_busy_retains_owner_until_admitted_profile_update_really_settles() -> TestResult
{
    use std::sync::mpsc;
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    let (entered, observed) = mpsc::sync_channel(1);
    let (resume, wait) = mpsc::sync_channel(1);
    std::thread::scope(|scope| -> TestResult {
        let store = &store;
        let update = scope.spawn(move || {
            store.update(|p| {
                entered.send(()).map_err(|_| StorageError::WorkerStopped)?;
                wait.recv().map_err(|_| StorageError::WorkerStopped)?;
                p.language = "fr".into();
                Ok(())
            })
        });
        observed.recv()?;
        let failure = native
            .finish()
            .err()
            .ok_or("inflight update was not owned")?;
        assert!(matches!(failure.storage_error(), StorageError::ProfileBusy));
        require_lock_held(&root)?;
        assert_eq!(native_state(&root)?["state"], "active");
        resume.send(())?;
        update.join().map_err(|_| "updater panic")??;
        failure.into_library().finish().map_err(|f| f.to_string())?;
        Ok(())
    })?;
    assert_eq!(native_state(&root)?["state"], "closed");
    assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
    Ok(())
}

#[test]
fn native_imported_orphan_never_publishes_profile_authority_or_retargets() -> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let profile = fs::read(locations.native.join("settings.json"))?;
    drop(native);
    drop(store);
    let before = fs::read(root.join(NATIVE_STATE))?;
    fs::write(
        locations.native.join("settings.json"),
        serde_json::to_vec(&Profile::setup())?,
    )?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    assert!(matches!(
        NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
        Err(StorageError::RecoveryBindingRequired)
    ));
    assert!(matches!(
        store.update(|p| {
            p.root = root.to_str().ok_or(StorageError::InvalidProfile)?.into();
            p.library_id = "a0000000-0000-4000-8000-000000000001".into();
            p.complete = true;
            Ok(())
        }),
        Err(StorageError::ProfileBindingActive)
    ));
    assert_eq!(fs::read(root.join(NATIVE_STATE))?, before);
    drop(store);
    fs::write(locations.native.join("settings.json"), profile)?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    let previous = store
        .load()?
        .ok_or("restored profile missing")?
        .profile()
        .clone();
    let changed = store.update(|p| {
        p.language = if p.language == "en" { "fr" } else { "en" }.into();
        Ok(())
    })?;
    assert_ne!(changed.language, previous.language);
    assert_eq!(
        (
            &changed.root,
            &changed.library_id,
            changed.complete,
            changed.recovering
        ),
        (
            &previous.root,
            &previous.library_id,
            previous.complete,
            previous.recovering
        )
    );
    assert_eq!(fs::read(root.join(NATIVE_STATE))?, before);
    let native = NativeLibrary::reopen(&store, &locations, &WorkCancellation::default())?;
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_restart_refuses_corrupt_records_controls_sidecars_and_marker_aliases_without_cleanup()
-> TestResult {
    for scenario in [
        "duplicate",
        "unknown",
        "epoch",
        "hash",
        "root",
        "temp",
        "journal",
        "sidecar",
        "marker-link",
        "record-link",
        "overflow",
        "raw-id",
        "profile-hash",
        "foreign-db",
        "managed-link",
        "wal",
        "shm",
    ] {
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|f| f.to_string())?;
        if scenario == "overflow" {
            let mut db = Database::open_native(&native)?;
            db.close()?;
            drop(db);
            native.finish().map_err(|f| f.to_string())?;
        } else {
            drop(native);
        }
        drop(store);
        let mut record = native_state(&root)?;
        match scenario {
            "duplicate" => {
                let bytes = fs::read_to_string(root.join(NATIVE_STATE))?;
                fs::write(
                    root.join(NATIVE_STATE),
                    format!("{{\"version\":1,{}", &bytes[1..]),
                )?;
            }
            "unknown" => {
                record["extra"] = true.into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "epoch" => {
                record["epoch"] = 1.into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "hash" => {
                record["library_marker_sha256"] = "F".repeat(64).into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "root" => {
                record["library_root"] = locations.data.to_str().ok_or("data")?.into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "temp" => fs::write(
                root.join(".librairii-native-state.json.tmp-foreign"),
                b"not cleanup authority",
            )?,
            "journal" => fs::write(root.join("relocation.json"), b"not replay authority")?,
            "sidecar" => fs::write(
                root.join("db/librairii.sqlite3-journal"),
                b"foreign journal",
            )?,
            "marker-link" => fs::hard_link(
                root.join(".librairii-library"),
                roots.profile().join("marker-alias"),
            )?,
            "record-link" => fs::hard_link(
                root.join(NATIVE_STATE),
                roots.profile().join("record-alias"),
            )?,
            "raw-id" => {
                record["library_id_raw"] = record["library_id_raw"]
                    .as_str()
                    .ok_or("raw id")?
                    .to_ascii_uppercase()
                    .into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "profile-hash" => {
                record["profile_marker_sha256"] = "0".repeat(64).into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            "foreign-db" => fs::write(
                root.join("db/librairii.sqlite3"),
                b"foreign schema bytes remain untouched",
            )?,
            "managed-link" => {
                fs::rename(
                    root.join("archives"),
                    roots.profile().join("retained-archives"),
                )?;
                std::os::unix::fs::symlink(roots.profile(), root.join("archives"))?;
            }
            "wal" => fs::write(root.join("db/librairii.sqlite3-wal"), b"foreign wal")?,
            "shm" => fs::write(root.join("db/librairii.sqlite3-shm"), b"foreign shm")?,
            "overflow" => {
                record["state"] = "closed".into();
                record["epoch"] = u64::MAX.into();
                fs::write(root.join(NATIVE_STATE), serde_json::to_vec(&record)?)?;
            }
            _ => return Err("scenario".into()),
        }
        let before = fs::read(root.join(NATIVE_STATE))?;
        let before_db = fs::read(root.join("db/librairii.sqlite3"))?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        if scenario == "overflow" {
            let native = NativeLibrary::reopen(&store, &locations, &WorkCancellation::default())?;
            assert!(Database::open_native(&native).is_err());
        } else {
            assert!(
                NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()).is_err(),
                "{scenario}"
            );
        }
        assert_eq!(fs::read(root.join(NATIVE_STATE))?, before, "{scenario}");
        assert_eq!(
            fs::read(root.join("db/librairii.sqlite3"))?,
            before_db,
            "{scenario}"
        );
        if scenario != "overflow" {
            assert!(!root.join("staging").exists());
            assert!(!root.join("catalog").exists());
        } else {
            assert_eq!(fs::read_dir(root.join("staging"))?.count(), 0);
            assert_eq!(fs::read_dir(root.join("trash"))?.count(), 0);
        }
        if scenario == "temp" {
            assert_eq!(
                fs::read(root.join(".librairii-native-state.json.tmp-foreign"))?,
                b"not cleanup authority"
            );
        }
    }
    Ok(())
}

#[test]
fn native_unacknowledged_admitted_commit_cannot_be_hidden_by_database_close() -> TestResult {
    use std::sync::mpsc;
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    let (entered, observed) = mpsc::sync_channel(1);
    let (resume, wait) = mpsc::sync_channel(1);
    let pending = db.transaction(WorkCancellation::default(), move |tx| {
        tx.execute("UPDATE stories SET embedded_description='committed without receiver' WHERE id=(SELECT MIN(id) FROM stories)", [])?;
        entered.send(()).map_err(|_| StorageError::WorkerStopped)?;
        wait.recv().map_err(|_| StorageError::WorkerStopped)?;
        Ok(())
    })?;
    observed.recv()?;
    drop(pending); // No cancellation or rollback fiction.
    resume.send(())?;
    assert_eq!(
        db.barrier_read(|tx| Ok(tx.query_row(
            "SELECT embedded_description FROM stories ORDER BY id LIMIT 1",
            [],
            |r| r.get::<_, String>(0)
        )?))?
        .wait()?,
        "committed without receiver"
    );
    db.close()?;
    drop(db);
    let failure = native
        .finish()
        .err()
        .ok_or("lost acknowledgment minted Closed")?;
    require_lock_held(&root)?;
    assert_eq!(native_state(&root)?["state"], "active");
    assert!(
        failure
            .retained_library()
            .revalidate(&WorkCancellation::default())
            .is_err()
    );
    assert!(store.update(|_| Ok(())).is_err());
    Ok(())
}

#[test]
fn native_language_update_busy_before_first_open_keeps_record_imported_and_retryable() -> TestResult
{
    use std::sync::mpsc;
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let before = fs::read(root.join(NATIVE_STATE))?;
    let (entered, observed) = mpsc::sync_channel(1);
    let (resume, wait) = mpsc::sync_channel(1);
    std::thread::scope(|scope| -> TestResult {
        let store = &store;
        let updater = scope.spawn(move || {
            store.update(|p| {
                entered.send(()).map_err(|_| StorageError::WorkerStopped)?;
                wait.recv().map_err(|_| StorageError::WorkerStopped)?;
                p.language = "fr".into();
                Ok(())
            })
        });
        observed.recv()?;
        let attempt = Database::open_native(&native);
        let unchanged =
            fs::read(root.join(NATIVE_STATE))? == before && !root.join("staging").exists();
        resume.send(())?;
        updater.join().map_err(|_| "updater panic")??;
        assert!(matches!(attempt, Err(StorageError::ProfileBusy)));
        assert!(unchanged);
        Ok(())
    })?;
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_language_update_busy_refuses_database_admission_without_poisoning_generation()
-> TestResult {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    let ran = Arc::new(AtomicBool::new(false));
    let (entered, observed) = mpsc::sync_channel(1);
    let (resume, wait) = mpsc::sync_channel(1);
    std::thread::scope(|scope| -> TestResult {
        let store = &store;
        let updater = scope.spawn(move || {
            store.update(|p| {
                entered.send(()).map_err(|_| StorageError::WorkerStopped)?;
                wait.recv().map_err(|_| StorageError::WorkerStopped)?;
                p.language = "fr".into();
                Ok(())
            })
        });
        observed.recv()?;
        let read_ran = ran.clone();
        let read = db.read(move |_| {
            read_ran.store(true, Ordering::Release);
            Ok(())
        });
        let write_ran = ran.clone();
        let write = db.transaction(WorkCancellation::default(), move |_| {
            write_ran.store(true, Ordering::Release);
            Ok(())
        });
        resume.send(())?;
        updater.join().map_err(|_| "updater panic")??;
        assert!(matches!(read, Err(StorageError::ProfileBusy)));
        assert!(matches!(write, Err(StorageError::ProfileBusy)));
        assert!(!ran.load(Ordering::Acquire));
        Ok(())
    })?;
    assert_eq!(
        db.read(|tx| Ok(tx.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?))?
            .wait()?,
        1
    );
    assert!(matches!(
        db.transaction(WorkCancellation::default(), |_| Ok(()))?
            .wait()?,
        TransactionOutcome::Committed(())
    ));
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_forgotten_pending_retains_owned_slot_and_cannot_acknowledge_clean_database_close()
-> TestResult {
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    let pending = db.barrier_read(|_| Ok(()))?;
    // Explicitly exercise safe Rust's destructor bypass. No live root/DB owner
    // is leaked: the forgotten bounded result slot only owns its admission Arc.
    std::mem::forget(pending);
    db.barrier_read(|_| Ok(()))?.wait()?;
    assert!(matches!(db.close(), Err(StorageError::RuntimeFenced)));
    drop(db);
    let failure = native
        .finish()
        .err()
        .ok_or("unacknowledged slot minted Closed")?;
    require_lock_held(&root)?;
    assert_eq!(native_state(&root)?["state"], "active");
    assert!(store.update(|_| Ok(())).is_err());
    drop(failure);
    Ok(())
}

#[test]
fn native_actual_process_crash_keeps_active_and_releases_only_os_locks() -> TestResult {
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    const CHILD: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_FIXTURE";
    const SOURCE: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_SOURCE";
    const NAMESPACE: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_NAMESPACE";
    const DATA: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_DATA";
    const LEGACY: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_LEGACY";
    const READY: &str = "LIBRAIRII_STORAGE_NATIVE_CRASH_READY";
    if std::env::var_os(CHILD).is_some() {
        // These are explicit private test input locations, never authority.
        // The child independently verifies and owns a NEW copy through the
        // actual storage route; no parent receipt/path cast crosses processes.
        let value = |name| -> TestResult<std::path::PathBuf> {
            Ok(std::env::var_os(name)
                .ok_or("missing child fixture input")?
                .into())
        };
        let locations = ProfileLocations {
            native: value(NAMESPACE)?,
            data: value(DATA)?,
            legacy: Some(value(LEGACY)?),
        };
        let source = value(SOURCE)?;
        let profile = ProfileDocument::decode(&fs::read(
            locations
                .legacy
                .as_ref()
                .ok_or("legacy input")?
                .join("settings.json"),
        )?)?;
        let input = ClosedLibraryInput::verify(
            &source,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        let copy = input.copy_to_new_root(
            &locations.data.join("actual-process-owned"),
            &WorkCancellation::default(),
        )?;
        let root = copy.root().to_owned();
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native =
            NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
                .map_err(|f| f.to_string())?;
        let db = Database::open_native(&native)?;
        assert_eq!(db.diagnostics().len(), 5);
        assert_eq!(native_state(&root)?["state"], "active");
        assert_eq!(native_state(&root)?["epoch"], 1);
        let mut ready = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(value(READY)?)?;
        ready.write_all(b"active-five-owned-connections")?;
        ready.sync_all()?;
        // The parent forcibly kills this real process. No Rust Drop/finalizer,
        // successful close or synthetic clean flag can execute on that path.
        loop {
            std::thread::park();
            std::hint::black_box((&db, &native, &store));
        }
    }

    struct ProcessOwner(Child);
    impl Drop for ProcessOwner {
        fn drop(&mut self) {
            // Test-host fallback only. Explicit success/failure below always
            // performs real wait; this never certifies library settlement.
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let roots = TestRoots::new()?;
    let (original_copy, locations) = prepare(&roots, false)?;
    let source = Path::new(&original_copy.source_profile().profile().root);
    let source_db = fs::read(source.join("db/librairii.sqlite3"))?;
    let legacy = locations.legacy.as_ref().ok_or("legacy context")?;
    let legacy_profile = fs::read(legacy.join("settings.json"))?;
    let ready = roots.profile().join("native-child-ready");
    let output_path = roots.profile().join("native-child-output");
    let output = fs::File::create(&output_path)?;
    let mut child = ProcessOwner(
        Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "native_actual_process_crash_keeps_active_and_releases_only_os_locks",
                "--nocapture",
            ])
            .env(CHILD, "owned-child")
            .env(SOURCE, source)
            .env(NAMESPACE, &locations.native)
            .env(DATA, &locations.data)
            .env(LEGACY, legacy)
            .env(READY, &ready)
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::from(output))
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if ready.exists() && fs::read(&ready)? == b"active-five-owned-connections" {
            break;
        }
        if let Some(status) = child.0.try_wait()? {
            return Err(format!(
                "native child exited {status}: {}",
                fs::read_to_string(&output_path)?
            )
            .into());
        }
        if Instant::now() >= deadline {
            // Expiration fails this test and the owning fallback kills/WAITS;
            // it cannot fabricate settled/closed product state or a pass.
            return Err("native child did not reach actual Active runtime".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let root = locations.data.join("actual-process-owned");
    let active = fs::read(root.join(NATIVE_STATE))?;
    assert_eq!(native_state(&root)?["state"], "active");
    require_lock_held(&root)?;
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(StorageError::ProfileInUse)
    ));
    child.0.kill()?;
    assert!(!child.0.wait()?.success());
    // Actual process death releases kernel locks, NOT the persisted-generation
    // recovery fence. The probe is dropped before testing checked reopen.
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(LIBRARY_LOCK_FILE))?;
    lock.try_lock()?;
    drop(lock);
    let store = NativeProfileStore::open_native(&locations.native)?;
    assert!(matches!(
        NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
        Err(StorageError::RecoveryBindingRequired)
    ));
    // Independent UI preferences remain writable after actual process death.
    // This must not grant a new library binding or clear persisted Active.
    let before = store
        .load()?
        .ok_or("crashed profile missing")?
        .profile()
        .clone();
    let changed = store.update(|profile| {
        profile.language = if profile.language == "en" { "fr" } else { "en" }.into();
        Ok(())
    })?;
    assert_ne!(changed.language, before.language);
    assert_eq!(
        (
            &changed.root,
            &changed.library_id,
            changed.complete,
            changed.recovering
        ),
        (
            &before.root,
            &before.library_id,
            before.complete,
            before.recovering
        )
    );
    for field in ["root", "raw_id", "complete", "recovering"] {
        assert!(
            matches!(
                store.update(|profile| {
                    match field {
                        "root" => profile.root.push_str("-unverified"),
                        "raw_id" => profile.library_id.make_ascii_uppercase(),
                        "complete" => profile.complete = !profile.complete,
                        _ => profile.recovering = !profile.recovering,
                    }
                    Ok(())
                }),
                Err(StorageError::ProfileBindingActive)
            ),
            "{field}"
        );
    }
    assert_eq!(
        store.load()?.ok_or("updated profile missing")?.profile(),
        &changed
    );
    assert!(matches!(
        NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()),
        Err(StorageError::RecoveryBindingRequired)
    ));
    assert_eq!(fs::read(root.join(NATIVE_STATE))?, active);
    assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, source_db);
    assert_eq!(fs::read(legacy.join("settings.json"))?, legacy_profile);
    for entry in fs::read_dir(source.join("archives"))? {
        let entry = entry?;
        assert_eq!(
            fs::read(root.join("archives").join(entry.file_name()))?,
            fs::read(entry.path())?
        );
    }
    Ok(())
}

#[test]
fn native_closed_root_relocation_refuses_unverified_retarget_without_rewriting_data() -> TestResult
{
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let old_root = copy.root().to_owned();
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    let settings = fs::read(locations.native.join("settings.json"))?;
    let record = fs::read(old_root.join(NATIVE_STATE))?;
    let marker = fs::read(old_root.join(".librairii-library"))?;
    let moved = locations.data.join("unverified-relocation");
    fs::rename(&old_root, &moved)?;
    assert!(NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()).is_err());
    assert!(matches!(
        store.update(|p| {
            p.root = moved.to_str().ok_or(StorageError::InvalidProfile)?.into();
            Ok(())
        }),
        Err(StorageError::ProfileBindingActive)
    ));
    assert_eq!(fs::read(locations.native.join("settings.json"))?, settings);
    assert_eq!(fs::read(moved.join(NATIVE_STATE))?, record);
    assert_eq!(fs::read(moved.join(".librairii-library"))?, marker);
    assert!(!old_root.exists());
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn native_non_utf8_copy_root_is_retained_without_lossy_record_or_profile_publication() -> TestResult
{
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let roots = TestRoots::new()?;
    let (first_copy, locations) = prepare(&roots, false)?;
    let source = Path::new(&first_copy.source_profile().profile().root);
    let source_before = fs::read(source.join("db/librairii.sqlite3"))?;
    let input = ClosedLibraryInput::verify(
        source,
        first_copy.source_profile().clone(),
        ClosedManifest::decode(MANIFEST)?,
        &WorkCancellation::default(),
    )?;
    let raw = OsString::from_vec(b"owned-invalid-utf8-\xff".to_vec());
    let path = locations.data.join(&raw);
    let copy = input.copy_to_new_root(&path, &WorkCancellation::default())?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let failure =
        NativeLibrary::bind_independent(copy, &store, &locations, &WorkCancellation::default())
            .err()
            .ok_or("lossy root was published")?;
    assert_eq!(
        failure.retained_copy().root().file_name(),
        Some(raw.as_os_str())
    );
    assert!(failure.retained_profile().load()?.is_none());
    assert!(!path.join(NATIVE_STATE).exists());
    require_lock_held(&path)?;
    assert_eq!(
        fs::read(source.join("db/librairii.sqlite3"))?,
        source_before
    );
    Ok(())
}

#[test]
fn metadata_refresh_and_artwork_settle_native_close_and_offline_reopen() -> TestResult {
    use librairii_app::metadata::{ArtworkAsset, MediaType, RefreshSession, normalize};
    use librairii_storage::metadata::MetadataStorage;
    const RAW: &[u8] = include_bytes!("../../librairii-app/tests/fixtures/lunii-catalog.json");
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &profile, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    let metadata = MetadataStorage::new(&db);
    let report = RefreshSession::begin(
        &metadata,
        "a0000000-0000-4000-8000-000000000091".parse()?,
        "en-GB",
        "2026-10-21T14:00:00Z",
    )
    .map_err(|e| format!("{e:?}"))?
    .finish(Ok(RAW.to_vec()), "2026-10-21T14:00:01Z", || false)
    .map_err(|e| format!("{e:?}"))?;
    let id = normalize(RAW.to_vec(), "en-GB")?.artworks()[0].id;
    let asset = ArtworkAsset::checked(
        include_bytes!("../../librairii-adapters/tests/fixtures/artwork/cover.png").to_vec(),
        MediaType::Png,
    )?;
    // Exercise both newly published and reused existing artwork bytes.
    for _ in 0..2 {
        metadata.persist_artwork(id, &asset, "2026-10-21T14:00:02Z", None, None)?;
    }
    assert!(metadata.retained_failures().is_empty());
    drop(metadata);
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    assert_eq!(fs::read_dir(root.join("staging"))?.count(), 0);
    assert_eq!(fs::read_dir(root.join("trash"))?.count(), 0);
    drop(profile);
    let profile = NativeProfileStore::open_native(&locations.native)?;
    let native = NativeLibrary::reopen(&profile, &locations, &WorkCancellation::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(
        db.catalog_snapshots()?.wait()?.last().ok_or("snapshot")?.id,
        report.snapshot
    );
    assert_eq!(db.read_registered_artwork(id)?.cached, Some(asset));
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_job_session_borrows_actual_database_and_file_owner_until_durable_settlement() -> TestResult
{
    use librairii_app::{
        native_jobs::*,
        ports::{
            creation::{NewItem, NewOperation},
            jobs::WorkItem,
        },
    };
    use librairii_core::{
        OperationKind, OperationState,
        operations::{ItemState, RawOutcome},
    };
    use librairii_storage::{
        operations_repository::TerminalRepository,
        platform::unix::{EntryName, FileArea, LibraryFiles, OwnedReadSource},
    };
    use std::{
        thread,
        time::{Duration, Instant},
    };
    struct Source<'files, 'library> {
        files: &'files LibraryFiles<'library>,
        name: EntryName,
    }
    impl<'files, 'library> Executor for Source<'files, 'library> {
        type Resources = OwnedReadSource<'files, 'library>;
        fn execute(
            &self,
            _: &WorkItem,
            _: &Cancellation,
            _: &ProgressSink,
        ) -> Execution<Self::Resources> {
            let deadline = Instant::now() + Duration::from_secs(10);
            let source = loop {
                match self
                    .files
                    .open_read_source(FileArea::Archives, self.name.clone())
                {
                    // No source owner was admitted. Retain this work item and
                    // retry after concurrent profile validation releases its slot.
                    Err(StorageError::ProfileBusy) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    result => break result,
                }
            };
            match source {
                Ok(source) => Execution {
                    boundary: Boundary::BeforeCommit,
                    report: Report {
                        state: ItemState::Skipped,
                        story_id: 0,
                        completed_bytes: 0,
                        outcome: RawOutcome::default(),
                    },
                    resources: Some(source),
                },
                Err(error) => Execution {
                    boundary: Boundary::BeforeCommit,
                    report: Report {
                        state: ItemState::Failed,
                        story_id: 0,
                        completed_bytes: 0,
                        outcome: RawOutcome {
                            code: Some("source_unavailable".into()),
                            message: Some(error.to_string()),
                        },
                    },
                    resources: None,
                },
            }
        }
        fn compensate(&self, _: &mut Self::Resources) -> bool {
            true
        }
        fn settle(
            &self,
            source: Self::Resources,
            durable: &DurableReport<'_>,
        ) -> Result<(), Self::Resources> {
            if durable.command().state == ItemState::Skipped && source.revalidate().is_ok() {
                Ok(())
            } else {
                Err(source)
            }
        }
    }
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let name = EntryName::parse(
        fs::read_dir(copy.root().join("archives"))?
            .next()
            .ok_or("archive")??
            .file_name(),
    )?;
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &profile, &locations, &WorkCancellation::default())
            .map_err(|f| f.to_string())?;
    let mut db = Database::open_native(&native)?;
    {
        let files = LibraryFiles::open_native(&db)?;
        let source = Source {
            files: &files,
            name,
        };
        let store = TerminalRepository::new(&db, 1);
        let request = NewOperation {
            id: "a0000000-0000-4000-8000-000000000099".into(),
            kind: OperationKind::Import,
            source: None,
            destination: None,
            items: vec![NewItem {
                source_name: "original.zip".into(),
                ..Default::default()
            }],
        };
        let id = request.id.clone();
        let start = Start::new(request).map_err(|_| "start")?;
        with_runtime(
            &store,
            &source,
            Configuration::default(),
            |handle| -> TestResult {
                runtime_support::offer(&handle, start)?;
                let end = Instant::now() + Duration::from_secs(10);
                loop {
                    if let Some(admission) = handle.take_admission()
                        && !admission.accepted
                    {
                        return Err("native start rejected".into());
                    }
                    // Native ownership revalidation may briefly reserve the
                    // profile while the coordinator reads it. Busy/queue refusal
                    // precedes this read's admission; retain the request and retry.
                    let snapshot = match db.operation_snapshot(id.clone()).and_then(|p| p.wait()) {
                        Ok(snapshot) => snapshot,
                        Err(
                            librairii_storage::StorageError::ProfileBusy
                            | librairii_storage::StorageError::QueueFull,
                        ) => None,
                        Err(error) => return Err(error.into()),
                    };
                    if let Some(snapshot) = snapshot {
                        if snapshot.operation.state == OperationState::Succeeded {
                            break;
                        }
                        if librairii_core::operations::is_terminal(snapshot.operation.state) {
                            return Err(format!(
                                "native terminal {:?}: {:?}, items: {:?}",
                                snapshot.operation.state,
                                snapshot.operation.outcome,
                                snapshot.items
                            )
                            .into());
                        }
                    }
                    if Instant::now() >= end {
                        return Err("native operation timed out".into());
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        )??;
    }
    db.close()?;
    drop(db);
    native.finish().map_err(|f| f.to_string())?;
    Ok(())
}

#[test]
fn native_registered_temporaries_recover_after_process_death_and_preserve_final_files() -> TestResult
{
    use librairii_app::ports::{
        creation::{NewItem, NewOperation},
        jobs::{JobStore, StoreResult},
        operations::*,
    };
    use librairii_core::{
        OperationKind,
        operations::{ExportSource, ExportSourceType, ItemState, RawOutcome},
    };
    use librairii_storage::{
        operations_repository::TerminalRepository, platform::unix::LibraryFiles,
    };
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    const CONTEXT: &str = "LIBRAIRII_NATIVE_OPERATION_CRASH_CONTEXT";
    if let Some(context) = std::env::var_os(CONTEXT) {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(context)?)?;
        let path = |key: &str| -> TestResult<std::path::PathBuf> {
            if key == "destination" && value[key].is_null() {
                use std::os::unix::ffi::OsStringExt;
                let bytes: Vec<u8> = serde_json::from_value(value["destination_bytes"].clone())?;
                return Ok(std::ffi::OsString::from_vec(bytes).into());
            }
            Ok(value[key].as_str().ok_or("fixture path")?.into())
        };
        let locations = ProfileLocations {
            native: path("native")?,
            data: path("data")?,
            legacy: Some(path("legacy")?),
        };
        let profile = ProfileDocument::decode(&fs::read(path("legacy")?.join("settings.json"))?)?;
        let verified = ClosedLibraryInput::verify(
            &path("source")?,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        let copy = verified.copy_to_new_root(
            &locations.data.join("job-recovery-child"),
            &WorkCancellation::default(),
        )?;
        let root = copy.root().to_owned();
        let profile = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(
            copy,
            &profile,
            &locations,
            &WorkCancellation::default(),
        )
        .map_err(|f| f.to_string())?;
        operation_crash_checkpoint(&value, "bound", &root, &native)?;
        let mut db = Database::open_native(&native)?;
        let repository = TerminalRepository::new(&db, 1);
        assert!(matches!(repository.startup(), StoreResult::Known(())));
        operation_crash_checkpoint(&value, "opened", &root, &(&db, &native))?;
        let import = NewOperation {
            id: "a0000000-0000-4000-8000-000000000081".into(),
            kind: OperationKind::Import,
            source: None,
            destination: None,
            items: vec![
                NewItem {
                    source_name: "partial.zip".into(),
                    total_bytes: 1024,
                    ..Default::default()
                },
                NewItem {
                    source_name: "saved.zip".into(),
                    ..Default::default()
                },
            ],
        };
        let import = committed_operation(
            db.create_operation(import, "2026-10-22T00:00:00Z".into(), Default::default())?
                .wait()?,
        )?;
        operation_crash_checkpoint(&value, "created", &root, &(&db, &native))?;
        committed_operation(
            db.mark_operation_item_running(
                import.operation.identity.id.clone(),
                import.items[0].identity.id,
                "2026-10-22T00:00:01Z".into(),
                Default::default(),
            )?
            .wait()?,
        )?;
        operation_crash_checkpoint(&value, "running", &root, &(&db, &native))?;
        assert!(matches!(
            repository.save_progress(import.items[0].identity.id, 256, 1024),
            StoreResult::Known(())
        ));
        operation_crash_checkpoint(&value, "progress", &root, &(&db, &native))?;
        let saved = CompleteItemCommand {
            generation: 1,
            operation: import.operation.immutable_scope(),
            item: import.items[1].immutable_scope(),
            story_id: 0,
            state: ItemState::Skipped,
            completed_bytes: 0,
            outcome: RawOutcome {
                code: Some("future.outcome".into()),
                message: Some("Original saved outcome".into()),
            },
        };
        assert!(matches!(
            repository.complete_item(&saved).disposition,
            AttemptDisposition::Committed { .. }
        ));
        operation_crash_checkpoint(&value, "item_saved", &root, &(&db, &native))?;
        if matches!(
            value["phase"].as_str(),
            Some("finished" | "sql_closed" | "native_closed")
        ) {
            let mut completed = saved.clone();
            completed.item = import.items[0].immutable_scope();
            completed.state = ItemState::Succeeded;
            completed.completed_bytes = 1024;
            completed.outcome = RawOutcome::default();
            assert!(matches!(
                repository.complete_item(&completed).disposition,
                AttemptDisposition::Committed { .. }
            ));
            let finish = FinishCommand {
                generation: 1,
                operation: import.operation.immutable_scope(),
                state: librairii_core::OperationState::Succeeded,
                outcome: RawOutcome::default(),
                finished_at: "2026-10-22T00:00:04Z".into(),
            };
            assert!(matches!(
                repository.finish(&finish).disposition,
                AttemptDisposition::Committed { .. }
            ));
            operation_crash_checkpoint(&value, "finished", &root, &(&db, &native))?;
            db.close()?;
            drop(db);
            operation_crash_checkpoint(&value, "sql_closed", &root, &native)?;
            native.finish().map_err(|f| f.to_string())?;
            operation_crash_checkpoint(&value, "native_closed", &root, &profile)?;
            return Err("missing close checkpoint".into());
        }
        let export = NewOperation {
            id: "a0000000-0000-4000-8000-000000000082".into(),
            kind: OperationKind::Export,
            source: Some(ExportSource {
                source_type: ExportSourceType::Selection,
                collection_ids: Vec::new(),
                collection_names: Vec::new(),
                shelf_ids: Vec::new(),
                shelf_names: Vec::new(),
            }),
            destination: Some(path("destination")?.into()),
            items: (1..=2)
                .map(|n| NewItem {
                    source_name: format!("export-{n}"),
                    output_name: if n == 2 {
                        "completed.zip"
                    } else {
                        "partial.zip"
                    }
                    .into(),
                    story_title: "Gone".into(),
                    story_uuid: format!("c0000000-0000-4000-8000-{n:012}"),
                    export_skip_code: "story_unavailable".into(),
                    ..Default::default()
                })
                .collect(),
        };
        let export = committed_operation(
            db.create_operation(export, "2026-10-22T00:00:02Z".into(), Default::default())?
                .wait()?,
        )?;
        operation_crash_checkpoint(&value, "export_created", &root, &(&db, &native))?;
        for item in &export.items {
            committed_operation(
                db.mark_operation_item_running(
                    export.operation.identity.id.clone(),
                    item.identity.id,
                    "2026-10-22T00:00:03Z".into(),
                    Default::default(),
                )?
                .wait()?,
            )?;
        }
        operation_crash_checkpoint(&value, "export_running", &root, &(&db, &native))?;
        let files = LibraryFiles::open_native(&db)?;
        let custody = files.try_custody()?;
        let mut stage = files
            .create_stage(&custody)
            .map_err(|f| f.cause().to_string())?;
        stage.register_operation(
            &custody,
            import.operation.identity.id.clone(),
            import.items[0].identity.id,
        )?;
        operation_crash_checkpoint(
            &value,
            "stage_registered",
            &root,
            &(&db, &native, &files, &custody, &stage),
        )?;
        stage.write_all(b"partial import")?;
        operation_crash_checkpoint(
            &value,
            "stage_written",
            &root,
            &(&db, &native, &files, &custody, &stage),
        )?;
        let mut partial = files
            .create_export_temporary(
                &custody,
                export.operation.identity.id.clone(),
                export.items[0].identity.id,
            )
            .map_err(|f| f.to_string())?;
        operation_crash_checkpoint(
            &value,
            "export_registered",
            &root,
            &(&db, &native, &files, &custody, &stage, &partial),
        )?;
        partial.write_all(b"partial export")?;
        partial.flush_durable()?;
        operation_crash_checkpoint(
            &value,
            "export_written",
            &root,
            &(&db, &native, &files, &custody, &stage, &partial),
        )?;
        let mut complete = files
            .create_export_temporary(
                &custody,
                export.operation.identity.id.clone(),
                export.items[1].identity.id,
            )
            .map_err(|f| f.to_string())?;
        complete.write_all(b"completed export")?;
        complete.flush_durable()?;
        // The actual registered publisher commits the final name before the
        // child is killed, with no final-name cleanup authority in its registry.
        complete.publish_registered(&custody, "completed.zip")?;
        operation_crash_checkpoint(
            &value,
            "export_published",
            &root,
            &(&db, &native, &files, &custody, &stage, &partial, &complete),
        )?;
        let ready = serde_json::json!({"root":root,"partial":partial.workspace_name().to_str().ok_or("partial name")?,"complete":complete.workspace_name().to_str().ok_or("complete name")?});
        let mut signal = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path("ready")?.with_extension("writing"))?;
        signal.write_all(&serde_json::to_vec(&ready)?)?;
        signal.sync_all()?;
        fs::rename(path("ready")?.with_extension("writing"), path("ready")?)?;
        loop {
            std::thread::park();
            std::hint::black_box((&db, &native, &files, &custody, &stage, &partial, &complete));
        }
    }
    struct Process(Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    for mode in ["owned", "symlink", "hardlink", "nonutf8"] {
        // APFS refuses invalid UTF-8 directory components. Linux exercises the
        // actual filesystem case; all native targets exercise the byte codec.
        if mode == "nonutf8" && !cfg!(target_os = "linux") {
            continue;
        }
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let source = Path::new(&copy.source_profile().profile().root);
        let source_db = fs::read(source.join("db/librairii.sqlite3"))?;
        let destination = if mode == "nonutf8" {
            use std::os::unix::ffi::OsStringExt;
            roots
                .profile()
                .join(std::ffi::OsString::from_vec(b"exports-\xff".to_vec()))
        } else {
            roots.profile().join("exports")
        };
        fs::create_dir(&destination)?;
        let foreign = destination.join("foreign.txt");
        fs::write(&foreign, b"unrelated file")?;
        fs::write(
            destination.join(".librairii-export-unregistered.tmp"),
            b"unregistered regular file",
        )?;
        let context = roots.profile().join("recovery-context.json");
        let ready = roots.profile().join("recovery-ready.json");
        fs::write(
            &context,
            serde_json::to_vec(
                &serde_json::json!({"native":locations.native,"data":locations.data,"legacy":locations.legacy,"source":source,"destination":destination.to_str(),"destination_bytes":destination.as_os_str().as_encoded_bytes(),"ready":ready}),
            )?,
        )?;
        let output_path = roots.profile().join("child-output");
        let output = fs::File::create(&output_path)?;
        let mut child=Process(Command::new(std::env::current_exe()?).args(["--exact","native_registered_temporaries_recover_after_process_death_and_preserve_final_files","--nocapture"]).env(CONTEXT,&context).stdout(Stdio::from(output.try_clone()?)).stderr(Stdio::from(output)).spawn()?);
        let end = Instant::now() + Duration::from_secs(30);
        while !ready.exists() {
            if let Some(status) = child.0.try_wait()? {
                return Err(format!(
                    "registered child {status}: {}",
                    fs::read_to_string(&output_path)?
                )
                .into());
            }
            if Instant::now() >= end {
                return Err("registered child timed out".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let ready: serde_json::Value = serde_json::from_slice(&fs::read(&ready)?)?;
        let root = Path::new(ready["root"].as_str().ok_or("root")?);
        child.0.kill()?;
        child.0.wait()?;
        let partial = destination.join(ready["partial"].as_str().ok_or("partial")?);
        if matches!(mode, "symlink" | "hardlink") {
            fs::remove_file(partial.join("payload"))?;
            if mode == "symlink" {
                std::os::unix::fs::symlink(&foreign, partial.join("payload"))?;
            } else {
                fs::hard_link(&foreign, partial.join("payload"))?;
            }
        }
        let store = NativeProfileStore::open_native(&locations.native)?;
        assert!(NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()).is_err());
        let recovery =
            NativeLibrary::recover_interrupted(&store, &locations, &WorkCancellation::default())?;
        match recovery.open() {
            Ok(mut db) => {
                assert!(matches!(mode, "owned" | "nonutf8"));
                let import = db
                    .operation_snapshot("a0000000-0000-4000-8000-000000000081".into())?
                    .wait()?
                    .ok_or("import history")?;
                assert_eq!(
                    import.operation.state,
                    librairii_core::OperationState::Interrupted
                );
                assert_eq!(import.items[1].state, ItemState::Skipped);
                assert_eq!(
                    import.items[1].outcome.code.as_deref(),
                    Some("future.outcome")
                );
                assert!(fs::read_dir(root.join("staging"))?.next().is_none());
                assert!(!partial.exists());
                db.close()?;
                drop(db);
            }
            Err(_) => {
                assert!(matches!(mode, "symlink" | "hardlink"));
                assert!(fs::symlink_metadata(partial.join("payload")).is_ok());
            }
        }
        assert_eq!(
            fs::read(destination.join("completed.zip"))?,
            b"completed export"
        );
        assert_eq!(fs::read(&foreign)?, b"unrelated file");
        assert_eq!(
            fs::read(destination.join(".librairii-export-unregistered.tmp"))?,
            b"unregistered regular file"
        );
        assert_eq!(fs::read(source.join("db/librairii.sqlite3"))?, source_db);
        if matches!(mode, "owned" | "nonutf8") {
            recovery.finish().map_err(|f| f.to_string())?;
            assert_eq!(native_state(root)?["state"], "closed");
        } else {
            assert!(recovery.finish().is_err());
            assert_eq!(native_state(root)?["state"], "active");
        }
    }
    Ok(())
}
fn committed_operation<T>(value: TransactionOutcome<T>) -> TestResult<T> {
    match value {
        TransactionOutcome::Committed(v) => Ok(v),
        TransactionOutcome::ProvenNotCommitted(e) | TransactionOutcome::CommitUncertain(e) => {
            Err(e.into())
        }
    }
}

// A ready signal is emitted only while the actual owners at the named boundary
// remain borrowed on the child stack. The parent kills and reaps that process.
fn operation_crash_checkpoint(
    context: &serde_json::Value,
    phase: &str,
    root: &Path,
    owners: &impl Sized,
) -> TestResult {
    if context["phase"].as_str() != Some(phase) {
        return Ok(());
    }
    let ready = Path::new(context["ready"].as_str().ok_or("ready path")?);
    let mut signal = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(ready.with_extension("writing"))?;
    signal.write_all(&serde_json::to_vec(
        &serde_json::json!({"root":root,"phase":phase}),
    )?)?;
    signal.sync_all()?;
    fs::rename(ready.with_extension("writing"), ready)?;
    loop {
        std::thread::park();
        std::hint::black_box(owners);
    }
}

#[test]
fn native_operation_crash_transition_matrix_reconciles_history_and_progress() -> TestResult {
    use librairii_app::ports::jobs::{JobStore, StoreResult};
    use librairii_core::{OperationState, operations::ItemState};
    use librairii_storage::operations_repository::TerminalRepository;
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct Process(Child);
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let phases = [
        "bound",
        "opened",
        "created",
        "running",
        "progress",
        "item_saved",
        "finished",
        "sql_closed",
        "native_closed",
        "export_created",
        "export_running",
        "stage_registered",
        "stage_written",
        "export_registered",
        "export_written",
        "export_published",
    ];
    for phase in phases {
        let roots = TestRoots::new()?;
        let (copy, locations) = prepare(&roots, false)?;
        let source = Path::new(&copy.source_profile().profile().root);
        let original = fs::read(source.join("db/librairii.sqlite3"))?;
        let destination = roots.profile().join("exports");
        fs::create_dir(&destination)?;
        fs::write(destination.join("foreign.txt"), b"unchanged")?;
        let context = roots.profile().join("context.json");
        let ready = roots.profile().join("ready.json");
        fs::write(
            &context,
            serde_json::to_vec(
                &serde_json::json!({"native":locations.native,"data":locations.data,"legacy":locations.legacy,"source":source,"destination":destination,"ready":ready,"phase":phase}),
            )?,
        )?;
        let output_path = roots.profile().join("child-output");
        let output = fs::File::create(&output_path)?;
        let mut child = Process(Command::new(std::env::current_exe()?).args(["--exact", "native_registered_temporaries_recover_after_process_death_and_preserve_final_files", "--nocapture"]).env("LIBRAIRII_NATIVE_OPERATION_CRASH_CONTEXT", &context).stdout(Stdio::from(output.try_clone()?)).stderr(Stdio::from(output)).spawn()?);
        let end = Instant::now() + Duration::from_secs(30);
        while !ready.exists() {
            if let Some(status) = child.0.try_wait()? {
                return Err(format!(
                    "{phase}: child {status}: {}",
                    fs::read_to_string(&output_path)?
                )
                .into());
            }
            if Instant::now() >= end {
                return Err(format!("{phase}: child timed out").into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let signal: serde_json::Value = serde_json::from_slice(&fs::read(&ready)?)?;
        assert_eq!(signal["phase"], phase);
        let root = Path::new(signal["root"].as_str().ok_or("root")?);
        child.0.kill()?;
        child.0.wait()?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        let inspect = |db: &Database<'_>| -> TestResult {
            let repository = TerminalRepository::new(db, 2);
            assert!(
                matches!(repository.startup(), StoreResult::Known(())),
                "{phase}"
            );
            let saved = db
                .operation_snapshot("a0000000-0000-4000-8000-000000000081".into())?
                .wait()?;
            if matches!(phase, "bound" | "opened") {
                assert!(saved.is_none(), "{phase}");
            } else {
                let saved = saved.ok_or("missing import history")?;
                let terminal = matches!(phase, "finished" | "sql_closed" | "native_closed");
                assert_eq!(
                    saved.operation.state,
                    if terminal {
                        OperationState::Succeeded
                    } else {
                        OperationState::Interrupted
                    },
                    "{phase}"
                );
                if !matches!(phase, "created" | "running" | "progress") {
                    assert_eq!(saved.items[1].state, ItemState::Skipped, "{phase}");
                    assert_eq!(
                        saved.items[1].outcome.code.as_deref(),
                        Some("future.outcome"),
                        "{phase}"
                    );
                }
                let bytes = if terminal {
                    1024
                } else if matches!(phase, "created" | "running") {
                    0
                } else {
                    256
                };
                assert_eq!(saved.items[0].progress.completed, bytes, "{phase}");
                let history = db.operation_history(Default::default())?.wait()?;
                assert!(
                    history
                        .operations
                        .iter()
                        .any(|o| o.operation.identity.id == saved.operation.identity.id),
                    "{phase}"
                );
            }
            let export = db
                .operation_snapshot("a0000000-0000-4000-8000-000000000082".into())?
                .wait()?;
            if let Some(export) = export {
                assert_eq!(
                    export.operation.state,
                    OperationState::Interrupted,
                    "{phase}"
                );
            }
            assert!(
                fs::read_dir(root.join("staging"))?.next().is_none(),
                "{phase}"
            );
            Ok(())
        };
        if matches!(phase, "bound" | "native_closed") {
            let native = NativeLibrary::reopen(&store, &locations, &WorkCancellation::default())?;
            let mut db = Database::open_native(&native)?;
            inspect(&db)?;
            db.close()?;
            drop(db);
            native.finish().map_err(|f| f.to_string())?;
        } else {
            assert!(
                NativeLibrary::reopen(&store, &locations, &WorkCancellation::default()).is_err(),
                "{phase}"
            );
            let recovery = NativeLibrary::recover_interrupted(
                &store,
                &locations,
                &WorkCancellation::default(),
            )?;
            let mut db = recovery.open()?;
            inspect(&db)?;
            db.close()?;
            drop(db);
            recovery.finish().map_err(|f| f.to_string())?;
        }
        assert_eq!(native_state(root)?["state"], "closed", "{phase}");
        assert_eq!(
            fs::read(source.join("db/librairii.sqlite3"))?,
            original,
            "{phase}"
        );
        assert_eq!(fs::read(destination.join("foreign.txt"))?, b"unchanged");
        assert_eq!(
            destination.join("completed.zip").exists(),
            phase == "export_published"
        );
        assert_eq!(
            fs::read_dir(&destination)?.count(),
            if phase == "export_published" { 2 } else { 1 },
            "{phase}"
        );
    }
    Ok(())
}

#[test]
fn native_prepared_export_runs_durable_reports_and_preserves_racing_destinations() -> TestResult {
    use librairii_app::native_jobs::Configuration;
    use librairii_core::{OperationState, export::Scope};
    use librairii_storage::export::Exports;
    use sha2::{Digest, Sha256};
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native =
        NativeLibrary::bind_independent(copy, &profile, &locations, &WorkCancellation::default())
            .map_err(|e| e.to_string())?;
    let mut db = Database::open_native(&native)?;
    let bytes = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/archives/clockwork-forest.zip"),
    )?;
    fs::write(root.join("archives/native-export"), &bytes)?;
    let hash = hex_digest(&bytes);
    let size = bytes.len() as i64;
    committed_operation(db.transaction(Default::default(),move|tx|{
        tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(10001,'e0000000-0000-4000-8000-000000010001','Native export','native export')",[])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(10001,'native.zip','zip',?,?,'archives/native-export','valid')",rusqlite::params![hash,size])?;Ok(())
    })?.wait()?)?;
    fn hex_digest(bytes: &[u8]) -> String {
        librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string()
    }
    let unsafe_bytes = b"synthetic managed bytes for a skipped unsafe filename";
    fs::write(root.join("archives/unsafe-export"), unsafe_bytes)?;
    let unsafe_hash = hex_digest(unsafe_bytes);
    committed_operation(db.transaction(Default::default(),move|tx|{
        tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(10002,'e0000000-0000-4000-8000-000000010002','Unsafe stored name','unsafe stored name')",[])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(10002,'../unsafe.zip','zip',?,?,'archives/unsafe-export','valid')",rusqlite::params![unsafe_hash,unsafe_bytes.len() as i64])?;Ok(())
    })?.wait()?)?;
    let mut criteria = librairii_core::collection_codec::Query::default();
    criteria.name = "Native export".into();
    committed_operation(
        db.edit_collections(crate_collection_create(criteria), Default::default())?
            .wait()?,
    )?;
    let collection_id = db
        .barrier_read(|tx| {
            Ok(tx.query_row(
                "SELECT id FROM collections WHERE normalized_name='raw export'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })?
        .wait()?;
    committed_operation(
        db.transaction(Default::default(), move |tx| {
            tx.execute(
                "UPDATE collections SET name=CAST(x'52e1806177' AS TEXT) WHERE id=?",
                [collection_id],
            )?;
            Ok(())
        })?
        .wait()?,
    )?;
    fn crate_collection_create(
        criteria: librairii_core::collection_codec::Query,
    ) -> librairii_storage::collections::Command {
        librairii_storage::collections::Command::Create {
            name: "Raw export".into(),
            criteria,
        }
    }
    committed_operation(db.transaction(Default::default(),|tx|{
        tx.execute("INSERT INTO shelves(id,name,normalized_name,position) VALUES(999,'Native refined Shelf','native refined shelf',999)",[])?;
        tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,protected,cached_title) VALUES(999,'e0000000-0000-4000-8000-000000010001',0,0,'Native export')",[])?;Ok(())
    })?.wait()?)?;
    let mut cases = vec![
        (false, false, false, false, false),
        (true, false, false, false, false),
        (false, true, false, false, false),
        (false, false, false, true, false),
        (false, false, false, false, true),
    ];
    if cfg!(target_os = "linux") {
        cases.push((false, false, true, false, false));
    }
    for (n, (race, collection, raw_path, mixed, refined)) in cases.into_iter().enumerate() {
        let destination = if raw_path {
            use std::os::unix::ffi::OsStringExt;
            roots
                .profile()
                .join(std::ffi::OsString::from_vec(b"native-export-\xff".to_vec()))
        } else {
            roots.profile().join(format!("native-export-{n}"))
        };
        fs::create_dir(&destination)?;
        let mut exports = Exports::new(&db);
        let preview = exports.prepare(
            if refined {
                Scope::ShelfResult {
                    shelf: librairii_core::ShelfId::new(999)?,
                    query: librairii_core::query::QueryRequest {
                        criteria: {
                            let mut query = librairii_core::collection_codec::Query::default();
                            query.name = "Native export".into();
                            query
                        },
                        ..Default::default()
                    },
                }
            } else if collection {
                Scope::Collections(vec![librairii_core::CollectionId::new(collection_id)?])
            } else {
                Scope::Selection(if mixed {
                    vec![10001, 10002]
                } else {
                    vec![10001]
                })
            },
            &destination,
            "fr-FR".into(),
            &Default::default(),
        )?;
        if collection {
            assert_eq!(preview.source.names, vec![b"R\xe1\x80aw".to_vec()]);
        }
        let preparation = preview.preparation.ok_or("preparation")?;
        if race {
            fs::write(destination.join("native.zip"), b"racing foreign file")?;
        }
        let id = format!("a0000000-0000-4000-8000-{:012}", 1000 + n);
        exports.run_prepared(
            &preparation,
            id.clone(),
            Configuration::default(),
            |handle, start| -> TestResult {
                runtime_support::offer(&handle, start)?;
                let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
                loop {
                    if let Some(admission) = handle.take_admission()
                        && !admission.accepted
                    {
                        return Err(format!("durable export admission rejected in case {n}").into());
                    }
                    match db.operation_snapshot(id.clone()).and_then(|p| p.wait()) {
                        Ok(Some(s))
                            if librairii_core::operations::is_terminal(s.operation.state) =>
                        {
                            assert_eq!(
                                s.operation.state,
                                if race {
                                    OperationState::Failed
                                } else if mixed {
                                    OperationState::PartiallySucceeded
                                } else {
                                    OperationState::Succeeded
                                }
                            );
                            assert_eq!(
                                s.items[0].outcome.code.as_deref(),
                                Some(if race {
                                    "filename_conflict"
                                } else {
                                    "exported"
                                })
                            );
                            break;
                        }
                        Err(StorageError::ProfileBusy | StorageError::QueueFull) => {}
                        Err(e) => return Err(e.into()),
                        _ => {}
                    }
                    if std::time::Instant::now() >= end {
                        return Err("durable export timed out".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
                Ok(())
            },
        )??;
        assert_eq!(
            fs::read(destination.join("native.zip"))?,
            if race {
                b"racing foreign file".to_vec()
            } else {
                bytes.clone()
            }
        );
        assert_eq!(fs::read_dir(&destination)?.count(), 1);
        let saved = db.operation_snapshot(id.clone())?.wait()?.ok_or("saved")?;
        assert_eq!(
            saved.operation.destination.as_ref().map(|p| p.as_os_str()),
            Some(destination.as_os_str())
        );
        if !race {
            assert_eq!(
                db.export_destination(id)?.path_for_reveal()?.as_os_str(),
                destination.as_os_str()
            );
        }
        if mixed {
            assert_eq!(
                saved.items[1].outcome.code.as_deref(),
                Some("unsupported_extension")
            );
            assert_eq!(saved.items[1].source_name, "../unsafe.zip");
            assert_eq!(saved.items[1].output_name.as_deref(), Some("../unsafe.zip"));
        }
        if refined {
            assert_eq!(
                saved.operation.export_scope.discriminator.as_deref(),
                Some("curated_shelf")
            );
            assert_eq!(
                saved.operation.export_scope.group_ids.as_deref(),
                Some("[999]")
            );
            assert_eq!(
                saved.operation.export_scope.group_names.as_deref(),
                Some("[\"Native refined Shelf\"]")
            );
        }
        if collection {
            assert_eq!(
                saved.operation.export_scope.group_names.as_deref(),
                Some("[\"R��aw\"]")
            );
        }
    }
    assert_eq!(fs::read(root.join("archives/native-export"))?, bytes);
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn native_export_cancel_during_second_copy_keeps_completed_file_and_retires_partial() -> TestResult
{
    use librairii_app::native_jobs::Configuration;
    use librairii_core::{OperationState, export::Scope, operations::ItemState};
    use librairii_storage::export::Exports;
    use sha2::{Digest, Sha256};
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native = NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
        .map_err(|e| e.to_string())?;
    let mut db = Database::open_native(&native)?;
    let complete = b"small completed synthetic copy".to_vec();
    let large = vec![97u8; 8 * 1024 * 1024];
    for (id, name, bytes) in [
        (11001, "completed.zip", &complete),
        (11002, "cancelled.zip", &large),
    ] {
        let hash = librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string();
        let size = bytes.len() as i64;
        let path = format!("archives/cancel-{id}");
        fs::write(root.join(&path), bytes)?;
        committed_operation(db.transaction(Default::default(),move|tx|{
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(?,?,'Synthetic stream','synthetic stream')",rusqlite::params![id,format!("e0000000-0000-4000-8000-{id:012}")])?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?,?,'zip',?,?,?,'valid')",rusqlite::params![id,name,hash,size,path])?;Ok(())
        })?.wait()?)?;
    }
    let destination = roots.profile().join("cancel-output");
    fs::create_dir(&destination)?;
    let mut exports = Exports::new(&db);
    let preparation = exports
        .prepare(
            Scope::Selection(vec![11001, 11002]),
            &destination,
            "fr-FR".into(),
            &Default::default(),
        )?
        .preparation
        .ok_or("preparation")?;
    let id = "a0000000-0000-4000-8000-000000011099".to_string();
    let mut cancelled = false;
    exports.run_prepared(
        &preparation,
        id.clone(),
        Configuration::new(1).ok_or("workers")?,
        |handle, start| -> TestResult {
            runtime_support::offer(&handle, start)?;
            let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                if !cancelled
                    && handle
                        .take_progress()
                        .iter()
                        .any(|p| p.total == large.len() as i64 && p.completed > 0)
                    && handle.cancel(id.clone()).is_ok()
                {
                    cancelled = true;
                }
                match db.operation_snapshot(id.clone()).and_then(|p| p.wait()) {
                    Ok(Some(s)) if librairii_core::operations::is_terminal(s.operation.state) => {
                        assert_eq!(
                            s.operation.state,
                            OperationState::PartiallySucceeded,
                            "{:?}",
                            s.items
                        );
                        assert_eq!(s.items[0].state, ItemState::Succeeded);
                        assert_eq!(s.items[1].state, ItemState::Cancelled);
                        break;
                    }
                    Err(StorageError::ProfileBusy | StorageError::QueueFull) => {}
                    Err(e) => return Err(e.into()),
                    _ => {}
                }
                if std::time::Instant::now() >= end {
                    return Err("cancelled export timed out".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Ok(())
        },
    )??;
    assert!(cancelled);
    assert_eq!(fs::read(destination.join("completed.zip"))?, complete);
    assert_eq!(fs::read_dir(&destination)?.count(), 1);
    assert_eq!(fs::read(root.join("archives/cancel-11002"))?, large);
    drop(exports);
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn native_export_rechecks_broken_after_waiting_for_actual_custody() -> TestResult {
    use librairii_app::native_jobs::Configuration;
    use librairii_core::{export::Scope, operations::ItemState};
    use librairii_storage::{export::Exports, platform::unix::LibraryFiles};
    use sha2::{Digest, Sha256};
    use std::os::unix::fs::MetadataExt;
    use std::time::{Duration, Instant};
    let roots = TestRoots::new()?;
    let (copy, locations) = prepare(&roots, false)?;
    let root = copy.root().to_owned();
    let profile = NativeProfileStore::create_fresh(&locations.native)?;
    let native = NativeLibrary::bind_independent(copy, &profile, &locations, &Default::default())
        .map_err(|e| e.to_string())?;
    let mut db = Database::open_native(&native)?;
    let bytes = b"synthetic passthrough bytes behind the custody guard";
    let managed = root.join("archives/custody-wait.zip");
    fs::write(&managed, bytes)?;
    let identity = fs::metadata(&managed)?;
    let hash = librairii_core::Checksum::from_bytes(Sha256::digest(bytes).into()).to_string();
    committed_operation(db.transaction(Default::default(),move|tx|{
        tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(120099,'e0000000-0000-4000-8000-000000120099','Custody wait','custody wait')",[])?;
        tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(120099,'custody-wait.zip','zip',?,?,'archives/custody-wait.zip','valid')",rusqlite::params![hash,bytes.len() as i64])?;Ok(())
    })?.wait()?)?;
    let destination = roots.profile().join("exports-after-wait");
    fs::create_dir(&destination)?;
    {
        let files = LibraryFiles::open_native(&db)?;
        let mut exports = Exports::new(&db);
        let preview = exports.prepare(
            Scope::Selection(vec![120099]),
            &destination,
            "en".into(),
            &Default::default(),
        )?;
        let id = "a0000000-0000-4000-8000-000000120099".to_owned();
        exports.run_prepared(
            preview.preparation.as_ref().ok_or("preparation")?,
            id.clone(),
            Configuration::default(),
            |handle, start| -> TestResult {
                let startup_deadline = Instant::now() + Duration::from_secs(10);
                while handle.status().starting {
                    if Instant::now() > startup_deadline {
                        return Err("startup did not settle".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                let guard = files.try_custody()?;
                runtime_support::offer(&handle, start)?;
                #[cfg(target_os = "macos")]
                let descriptors = Path::new("/dev/fd");
                #[cfg(target_os = "linux")]
                let descriptors = Path::new("/proc/self/fd");
                let deadline = Instant::now() + Duration::from_secs(10);
                // Source acquisition occurs after the first eligibility check. Its
                // actual descriptor proves execution reached the custody wait;
                // this is not a sleep or an observation of scheduler state alone.
                loop {
                    let acquired = fs::read_dir(descriptors)?
                        .filter_map(std::result::Result::ok)
                        .any(|entry| {
                            rustix::fs::open(
                                entry.path(),
                                rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NONBLOCK,
                                rustix::fs::Mode::empty(),
                            )
                            .ok()
                            .map(fs::File::from)
                            .and_then(|f| f.metadata().ok())
                            .is_some_and(|m| m.dev() == identity.dev() && m.ino() == identity.ino())
                        });
                    if acquired {
                        break;
                    }
                    if Instant::now() > deadline {
                        return Err("worker never acquired its source before waiting".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                committed_operation(
                    db.set_broken(vec![120099], true, Default::default())?
                        .wait()?,
                )?;
                drop(guard);
                loop {
                    match db.operation_snapshot(id.clone()).and_then(|p| p.wait()) {
                        Ok(Some(saved))
                            if librairii_core::operations::is_terminal(saved.operation.state) =>
                        {
                            assert_eq!(saved.items[0].state, ItemState::Skipped);
                            assert_eq!(
                                saved.items[0].outcome.code.as_deref(),
                                Some("story_marked_broken")
                            );
                            break;
                        }
                        Err(StorageError::ProfileBusy | StorageError::QueueFull) => {}
                        Err(e) => return Err(e.into()),
                        _ => {}
                    }
                    if Instant::now() > deadline {
                        return Err("queued Broken export did not settle".into());
                    }
                    std::thread::sleep(Duration::from_millis(2));
                }
                Ok(())
            },
        )??;
        assert_eq!(fs::read_dir(&destination)?.count(), 0);
        assert_eq!(fs::read(&managed)?, bytes);
    }
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    Ok(())
}
