#![cfg(any(target_os = "macos", target_os = "linux"))]

use librairii_storage::{
    StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest, IndependentCopy},
    database::WorkCancellation,
    native::NativeLibrary,
    platform::lock::LIBRARY_LOCK_FILE,
    profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations, read_legacy},
};
use librairii_testkit::TestRoots;
use std::{error::Error, fs, path::Path};
type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const MANIFEST: &[u8] =
    include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");

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
    let locations = ProfileLocations {
        legacy: Some(roots.profile().join("legacy")),
        native: roots.profile().join("native"),
        data: roots.library().join("native-data"),
    };
    fs::create_dir(&locations.data)?;
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
            | "copied-owner-marker" => assert!(
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
