#![cfg(any(target_os = "macos", target_os = "linux"))]

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
        let mut stage = files.create_stage(&guard)?;
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
        let mut other = peer.create_stage(&guard)?;
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
        let mut cancelled_stage = files.create_stage(&guard)?;
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
        let mut stage = files.create_stage(&guard)?;
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
        assert!(files.create_stage(&guard).is_err());
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
        native.revalidate(&WorkCancellation::default())?;
        update.join().map_err(|_| "language updater panicked")??;
        Ok(())
    })?;
    native.revalidate(&WorkCancellation::default())?;
    assert_eq!(store.load()?.ok_or("profile")?.profile().language, "fr");
    drop(native);
    store.update(|p| {
        p.complete = false;
        Ok(())
    })?;
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
                store.update(|p| {
                    p.root = root.to_str().ok_or(StorageError::InvalidProfile)?.into();
                    p.library_id = copy.identity().spelling().into();
                    p.recovering = true;
                    Ok(())
                })?;
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
            StorageError::ProfileBindingActive
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
    store.update(|p| {
        p.complete = false;
        Ok(())
    })?;
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
    drop(native);
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
