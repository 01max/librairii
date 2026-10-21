use librairii_storage::{StorageError, adoption::ClosedManifest};
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;
const MANIFEST: &[u8] =
    include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");

#[test]
fn closed_manifest_retains_bytes_and_refuses_ambiguous_or_incomplete_inputs() -> TestResult {
    let manifest = ClosedManifest::decode(MANIFEST)?;
    assert_eq!(manifest.original_bytes(), MANIFEST);
    assert_eq!(manifest.entry_count(), 11);
    let base: serde_json::Value = serde_json::from_slice(MANIFEST)?;
    for change in [
        "missing-db",
        "missing-parent",
        "duplicate",
        "escape",
        "unknown",
        "negative-size",
        "bad-hash",
    ] {
        let mut edited = base.clone();
        let entries = edited.as_array_mut().ok_or("array missing")?;
        match change {
            "missing-db" => entries.retain(|e| e["path"] != "db/librairii.sqlite3"),
            "missing-parent" => entries.retain(|e| e["path"] != "db"),
            "duplicate" => entries.push(entries[0].clone()),
            "escape" => entries[0]["path"] = "../.librairii-library".into(),
            "unknown" => entries[0]["unowned"] = "must refuse".into(),
            "negative-size" => entries[0]["size"] = (-1).into(),
            "bad-hash" => entries[0]["sha256"] = "not sha256".into(),
            _ => return Err("unknown synthetic edit".into()),
        }
        assert!(
            matches!(
                ClosedManifest::decode(&serde_json::to_vec(&edited)?),
                Err(StorageError::AdoptionInputInvalid)
            ),
            "{change}"
        );
    }
    let incomplete: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../preservation/corpus/profiles/relocation-incomplete-negative.json"
    ))?;
    assert!(
        matches!(
            ClosedManifest::decode(&serde_json::to_vec(&incomplete["entries"])?),
            Err(StorageError::AdoptionInputInvalid)
        ),
        "marker-only input never becomes a complete independent source by rebinding paths"
    );
    assert!(ClosedManifest::decode(b"[] {}").is_err());
    Ok(())
}

#[test]
fn closed_manifest_refuses_sidecars_before_source_io() -> TestResult {
    for sidecar in ["-wal", "-shm", "-journal"] {
        let mut manifest: serde_json::Value = serde_json::from_slice(MANIFEST)?;
        manifest.as_array_mut().ok_or("array missing")?.push(serde_json::json!({"path":format!("db/librairii.sqlite3{sidecar}"),"size":0,"sha256":"0".repeat(64)}));
        assert!(matches!(
            ClosedManifest::decode(&serde_json::to_vec(&manifest)?),
            Err(StorageError::ClosedSnapshotRequired)
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix {
    use super::*;
    use librairii_storage::{
        adoption::ClosedLibraryInput,
        database::WorkCancellation,
        profile::{Profile, ProfileDocument},
    };
    use librairii_testkit::TestRoots;
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
    };

    fn source(roots: &TestRoots) -> Result<(PathBuf, ProfileDocument), Box<dyn Error>> {
        let root = roots.library().join("closed-source");
        fs::create_dir(&root)?;
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/libraries/schema018");
        let manifest: serde_json::Value = serde_json::from_slice(MANIFEST)?;
        for entry in manifest.as_array().ok_or("array missing")? {
            let name = entry["path"].as_str().ok_or("path missing")?;
            if entry["directory"].as_bool() == Some(true) {
                fs::create_dir(root.join(name))?;
            } else {
                fs::copy(corpus.join(name), root.join(name))?;
            }
        }
        let profile = Profile {
            version: 1,
            language: "fr".into(),
            root: root
                .to_str()
                .ok_or("UTF8 root required for Go profile")?
                .to_owned(),
            library_id: "a0000000-0000-4000-8000-000000000001".into(),
            complete: true,
            recovering: false,
        };
        Ok((
            root,
            ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
        ))
    }
    fn bytes(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, Box<dyn Error>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(parent) = pending.pop() {
            for entry in fs::read_dir(parent)? {
                let entry = entry?;
                let metadata = fs::symlink_metadata(entry.path())?;
                if metadata.is_dir() {
                    pending.push(entry.path());
                } else {
                    files.insert(
                        entry.path().strip_prefix(root)?.to_path_buf(),
                        fs::read(entry.path())?,
                    );
                }
            }
        }
        Ok(files)
    }
    #[test]
    fn verified_input_uses_private_sqlite_probe_and_keeps_every_source_byte() -> TestResult {
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let before = bytes(&root)?;
        let original_profile = profile.original_bytes().to_vec();
        let input = ClosedLibraryInput::verify(
            &root,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        assert_eq!(input.prefix().version(), 18);
        assert_eq!(input.manifest().original_bytes(), MANIFEST);
        assert_eq!(input.profile().original_bytes(), original_profile);
        assert_eq!(input.profile().profile().language, "fr");
        assert_eq!(
            input.identity().original_bytes(),
            before[Path::new(".librairii-library")]
        );
        input.revalidate(&WorkCancellation::default())?;
        assert_eq!(input.root(), fs::canonicalize(&root)?);
        assert_eq!(
            bytes(&root)?,
            before,
            "no source SQLite WAL/SHM/lock/settings/recovery writes"
        );
        assert!(!root.join(".librairii-library.lock").exists());
        assert!(
            matches!(
                librairii_storage::adoption::open_legacy_in_place(&root),
                Err(StorageError::LegacyLifetimeExclusionUnproven)
            ),
            "input verification does not confer writable source authority"
        );
        Ok(())
    }
    #[test]
    fn independent_copy_keeps_raw_bytes_has_no_aliases_and_holds_its_own_lease() -> TestResult {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let before = bytes(&root)?;
        let original_profile = profile.original_bytes().to_vec();
        let input = ClosedLibraryInput::verify(
            &root,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        let target = roots.library().join("independent-inactive Éte # %2F");
        let copy = input.copy_to_new_root(&target, &WorkCancellation::default())?;
        assert_eq!(copy.prefix().version(), 18);
        assert_eq!(copy.root(), fs::canonicalize(&target)?);
        assert_eq!(
            copy.identity().original_bytes(),
            input.identity().original_bytes()
        );
        assert_eq!(copy.source_profile().original_bytes(), original_profile);
        assert_eq!(
            copy.source_profile().profile().root,
            root.to_str().ok_or("root UTF8")?
        );
        let mut copied = bytes(&target)?;
        assert_eq!(
            copied.remove(Path::new(".librairii-library.lock")),
            Some(Vec::new())
        );
        assert_eq!(copied, before);
        assert_eq!(bytes(&root)?, before);
        for path in before.keys() {
            let source = fs::symlink_metadata(root.join(path))?;
            let destination = fs::symlink_metadata(target.join(path))?;
            assert_eq!(destination.nlink(), 1);
            assert_ne!(
                (source.dev(), source.ino()),
                (destination.dev(), destination.ino())
            );
            assert_eq!(destination.permissions().mode() & 0o777, 0o600);
        }
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o700);
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(target.join(".librairii-library.lock"))?;
        assert!(matches!(lock.try_lock(), Err(fs::TryLockError::WouldBlock)));
        copy.revalidate(&WorkCancellation::default())?;
        // Source edits cannot alias the copied immutable or mutable bytes.
        fs::write(root.join("history.json"), b"source-only later activity")?;
        assert_eq!(
            fs::read(target.join("history.json"))?,
            before[Path::new("history.json")]
        );
        copy.revalidate(&WorkCancellation::default())?;
        fs::write(target.join("state.json"), b"candidate changed after proof")?;
        assert!(copy.revalidate(&WorkCancellation::default()).is_err());
        drop(copy);
        lock.try_lock()?;
        assert!(target.join(".librairii-library.lock").exists());
        assert!(!target.join("settings.json").exists());
        assert!(!target.join("recovery").exists());
        Ok(())
    }

    #[test]
    fn copy_refuses_existing_overlapping_changed_and_cancelled_inputs_without_clobber() -> TestResult
    {
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let before = bytes(&root)?;
        let input = ClosedLibraryInput::verify(
            &root,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        let occupied = roots.library().join("occupied");
        fs::create_dir(&occupied)?;
        fs::write(occupied.join("unowned"), b"never replace or purge")?;
        let failed = input
            .copy_to_new_root(&occupied, &WorkCancellation::default())
            .err()
            .ok_or("existing root must refuse")?;
        assert!(failed.retained_root().is_none());
        assert_eq!(failed.attempted_target(), Some(occupied.as_path()));
        assert_eq!(
            fs::read(occupied.join("unowned"))?,
            b"never replace or purge"
        );
        let nested = root.join("candidate");
        let failed = input
            .copy_to_new_root(&nested, &WorkCancellation::default())
            .err()
            .ok_or("source overlap must refuse")?;
        assert!(matches!(failed.storage_error(), StorageError::PathEscape));
        assert!(failed.attempted_target().is_none());
        assert!(!nested.exists());
        let cancelled = WorkCancellation::default();
        cancelled.cancel();
        let target = roots.library().join("new-target");
        let failed = input
            .copy_to_new_root(&target, &cancelled)
            .err()
            .ok_or("cancelled copy must refuse")?;
        assert!(matches!(failed.storage_error(), StorageError::Cancelled));
        assert!(failed.retained_root().is_none());
        assert!(failed.attempted_target().is_none());
        assert!(!target.exists());
        assert_eq!(bytes(&root)?, before);
        fs::write(
            root.join("history.json"),
            b"changed after closed verification",
        )?;
        let failed = input
            .copy_to_new_root(&target, &WorkCancellation::default())
            .err()
            .ok_or("changed input must refuse")?;
        assert!(matches!(
            failed.storage_error(),
            StorageError::AdoptionInputChanged
        ));
        assert!(!target.exists());
        assert_eq!(
            fs::read(root.join("history.json"))?,
            b"changed after closed verification"
        );
        Ok(())
    }
    #[test]
    fn closed_input_revalidation_detects_unlisted_changed_and_missing_entries() -> TestResult {
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let input = ClosedLibraryInput::verify(
            &root,
            profile,
            ClosedManifest::decode(MANIFEST)?,
            &WorkCancellation::default(),
        )?;
        fs::write(root.join("unlisted"), b"preserve this")?;
        assert!(matches!(
            input.revalidate(&WorkCancellation::default()),
            Err(StorageError::AdoptionInputChanged)
        ));
        assert_eq!(fs::read(root.join("unlisted"))?, b"preserve this");
        fs::remove_file(root.join("unlisted"))?;
        let history = fs::read(root.join("history.json"))?;
        fs::write(root.join("history.json"), vec![b'x'; history.len()])?;
        assert!(matches!(
            input.revalidate(&WorkCancellation::default()),
            Err(StorageError::AdoptionInputChanged)
        ));
        fs::write(root.join("history.json"), history)?;
        input.revalidate(&WorkCancellation::default())?;
        let wal = root.join("db/librairii.sqlite3-wal");
        fs::write(&wal, b"unlisted live-source sidecar")?;
        assert!(matches!(
            input.revalidate(&WorkCancellation::default()),
            Err(StorageError::AdoptionInputChanged)
        ));
        assert_eq!(fs::read(&wal)?, b"unlisted live-source sidecar");
        fs::remove_file(wal)?;
        fs::rename(root.join("history.json"), root.join("retained-history"))?;
        assert!(input.revalidate(&WorkCancellation::default()).is_err());
        assert!(root.join("retained-history").exists());
        fs::rename(root.join("retained-history"), root.join("history.json"))?;
        input.revalidate(&WorkCancellation::default())?;
        let retained = root.with_file_name("original-source-retained");
        let before = bytes(&root)?;
        fs::rename(&root, &retained)?;
        fs::create_dir(&root)?;
        fs::write(root.join("unowned"), b"replacement root must survive")?;
        assert!(matches!(
            input.revalidate(&WorkCancellation::default()),
            Err(StorageError::SnapshotChanged)
        ));
        assert_eq!(bytes(&retained)?, before);
        assert_eq!(
            fs::read(root.join("unowned"))?,
            b"replacement root must survive"
        );
        Ok(())
    }
    #[test]
    fn closed_input_refuses_aliases_and_raw_profile_identity_mismatch() -> TestResult {
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let before = bytes(&root)?;
        let mut changed = profile.profile().clone();
        changed.library_id.make_ascii_uppercase();
        assert!(
            matches!(
                ClosedLibraryInput::verify(
                    &root,
                    ProfileDocument::decode(&serde_json::to_vec(&changed)?)?,
                    ClosedManifest::decode(MANIFEST)?,
                    &WorkCancellation::default()
                ),
                Err(StorageError::IdentityMismatch)
            ),
            "typed UUID equality must not erase raw legacy authority spelling"
        );
        assert_eq!(bytes(&root)?, before);
        let outside = roots.library().join("independent-history");
        fs::write(&outside, b"unowned outside bytes")?;
        let target = root.join("history.json");
        fs::remove_file(&target)?;
        std::os::unix::fs::symlink(&outside, &target)?;
        assert!(
            ClosedLibraryInput::verify(
                &root,
                profile.clone(),
                ClosedManifest::decode(MANIFEST)?,
                &WorkCancellation::default()
            )
            .is_err()
        );
        assert_eq!(fs::read(&outside)?, b"unowned outside bytes");
        fs::remove_file(&target)?;
        fs::write(&target, &before[Path::new("history.json")])?;
        fs::hard_link(&target, roots.library().join("history-alias"))?;
        assert!(
            ClosedLibraryInput::verify(
                &root,
                profile,
                ClosedManifest::decode(MANIFEST)?,
                &WorkCancellation::default()
            )
            .is_err()
        );
        assert_eq!(bytes(&root)?, before);
        Ok(())
    }
    #[test]
    fn cancelled_input_verification_does_not_probe_missing_source_or_create_roots() -> TestResult {
        let roots = TestRoots::new()?;
        let (root, profile) = source(&roots)?;
        let before = bytes(&root)?;
        let cancellation = WorkCancellation::default();
        cancellation.cancel();
        let absent = root.join("absent");
        assert!(matches!(
            ClosedLibraryInput::verify(
                &absent,
                profile,
                ClosedManifest::decode(MANIFEST)?,
                &cancellation
            ),
            Err(StorageError::Cancelled)
        ));
        assert!(!absent.exists());
        assert_eq!(bytes(&root)?, before);
        Ok(())
    }
}

#[test]
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn self_consistent_hashes_cannot_qualify_foreign_sqlite_or_repair_it() -> TestResult {
    use librairii_storage::{
        adoption::ClosedLibraryInput,
        database::WorkCancellation,
        platform::hash_stream,
        profile::{Profile, ProfileDocument},
    };
    use librairii_testkit::TestRoots;
    use std::fs;
    // A new synthetic input carries valid independent file hashes, but an
    // unrelated SQLite lineage. Never run a migration to make it recognizable.
    let roots = TestRoots::new()?;
    let root = roots.library().join("foreign");
    fs::create_dir(&root)?;
    fs::create_dir(root.join("db"))?;
    let identity = b"a0000000-0000-4000-8000-000000000001\n";
    fs::write(root.join(".librairii-library"), identity)?;
    let path = root.join("db/librairii.sqlite3");
    let connection = rusqlite::Connection::open(&path)?;
    connection.execute_batch("CREATE TABLE user_data(value TEXT);INSERT INTO user_data VALUES('retain exact foreign bytes');")?;
    connection.close().map_err(|(_, e)| e)?;
    let mut entries = Vec::new();
    for name in [".librairii-library", "db/librairii.sqlite3"] {
        let mut file = fs::File::open(root.join(name))?;
        let size = file.metadata()?.len();
        let digest = hash_stream(&mut file, size, &WorkCancellation::default())?;
        entries.push(
            serde_json::json!({"path":name,"size":size,"sha256":digest.checksum().to_string()}),
        );
    }
    entries.push(serde_json::json!({"path":"db","directory":true,"size":0}));
    let manifest = ClosedManifest::decode(&serde_json::to_vec(&entries)?)?;
    let profile = Profile {
        version: 1,
        language: "en".into(),
        root: root.to_str().ok_or("UTF8 root")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    };
    let before = fs::read(&path)?;
    assert!(matches!(
        ClosedLibraryInput::verify(
            &root,
            ProfileDocument::decode(&serde_json::to_vec(&profile)?)?,
            manifest,
            &WorkCancellation::default()
        ),
        Err(StorageError::ForeignSchema)
    ));
    assert_eq!(fs::read(&path)?, before);
    assert_eq!(fs::read(root.join(".librairii-library"))?, identity);
    assert_eq!(
        fs::read_dir(root.join("db"))?.count(),
        1,
        "source must gain no WAL/SHM/probe/backup"
    );
    assert!(!root.join(".librairii-library.lock").exists());
    Ok(())
}

#[test]
fn closed_manifest_reserves_only_native_root_record_and_exact_atomic_control_prefix() -> TestResult
{
    for name in [
        ".librairii-native-state.json",
        ".librairii-native-state.json.tmp-",
        ".librairii-native-state.json.tmp-foreign",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(MANIFEST)?;
        value
            .as_array_mut()
            .ok_or("manifest")?
            .push(serde_json::json!({"path":name,"size":0,"sha256":"0".repeat(64)}));
        assert!(
            matches!(
                ClosedManifest::decode(&serde_json::to_vec(&value)?),
                Err(StorageError::AdoptionInputInvalid)
            ),
            "{name}"
        );
    }
    for name in [
        ".librairii-native-state.json.other",
        ".librairii-native-state.json.tmpx-",
        "archives/.librairii-native-state.json",
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(MANIFEST)?;
        value
            .as_array_mut()
            .ok_or("manifest")?
            .push(serde_json::json!({"path":name,"size":0,"sha256":"0".repeat(64)}));
        ClosedManifest::decode(&serde_json::to_vec(&value)?)?;
    }
    assert_eq!(ClosedManifest::decode(MANIFEST)?.original_bytes(), MANIFEST);
    Ok(())
}
