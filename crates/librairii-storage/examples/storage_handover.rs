//! Finite native storage rehearsal, driven with a manifest-only frozen Go build.
//! It does not implement the preference bridge, GUI adoption or live relocation.
#[cfg(unix)]
mod rehearsal {
    use librairii_storage::{
        StorageError,
        adoption::{ClosedLibraryInput, ClosedManifest},
        database::{Database, TransactionOutcome, WorkCancellation},
        native::NativeLibrary,
        platform::hash_stream,
        profile::{NativeProfileStore, Profile, ProfileDocument, ProfileLocations},
    };
    use librairii_testkit::TestRoots;
    use serde_json::{Value, json};
    use std::{
        collections::BTreeMap,
        error::Error,
        fs,
        io::Read,
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        thread,
        time::{Duration, Instant},
    };
    type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
    const MANIFEST: &[u8] =
        include_bytes!("../../../preservation/corpus/profiles/library-manifest.json");

    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn wait(child: &mut Child) -> Result<std::process::ExitStatus> {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait()? {
                if Instant::now() >= deadline {
                    return Err("late Go probe completion".into());
                }
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err("Go probe deadline exceeded".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn probe(helper: &Path, mode: &str, first: &Path, second: &Path) -> Result<Vec<u8>> {
        let mut child = OwnedChild(
            Command::new(helper)
                .env_clear()
                .arg(mode)
                .arg(first)
                .arg(second)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        );
        let status = wait(&mut child.0)?;
        let mut output = Vec::new();
        child
            .0
            .stdout
            .take()
            .ok_or("missing Go stdout")?
            .take(8193)
            .read_to_end(&mut output)?;
        if !status.success() || output.len() > 8192 {
            let mut error = Vec::new();
            child
                .0
                .stderr
                .take()
                .ok_or("missing Go stderr")?
                .take(8192)
                .read_to_end(&mut error)?;
            return Err(format!(
                "Go probe {mode} failed: {status}: {}",
                String::from_utf8_lossy(&error)
            )
            .into());
        }
        Ok(output)
    }
    fn files(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
        let mut result = BTreeMap::new();
        let mut directories = vec![root.to_owned()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                let metadata = fs::symlink_metadata(entry.path())?;
                if metadata.is_dir() {
                    directories.push(entry.path());
                } else {
                    if !metadata.is_file() || metadata.file_type().is_symlink() {
                        return Err("nonregular rehearsal entry".into());
                    }
                    result.insert(
                        entry.path().strip_prefix(root)?.to_owned(),
                        fs::read(entry.path())?,
                    );
                }
            }
        }
        Ok(result)
    }

    pub fn run() -> Result {
        let helper = std::env::args_os()
            .nth(1)
            .ok_or("explicit frozen Go helper required")?;
        let helper = fs::canonicalize(helper)?;
        let roots = TestRoots::new()?;
        let source = roots.library().join("legacy-source");
        fs::create_dir(&source)?;
        let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../preservation/corpus/libraries/schema018");
        let manifest: Value = serde_json::from_slice(MANIFEST)?;
        for entry in manifest.as_array().ok_or("manifest array")? {
            let path = entry["path"].as_str().ok_or("manifest path")?;
            if entry["directory"] == true {
                fs::create_dir(source.join(path))?;
            } else {
                fs::copy(corpus.join(path), source.join(path))?;
            }
        }
        let profile = Profile {
            version: 1,
            language: "fr".into(),
            root: source.to_str().ok_or("source UTF8")?.into(),
            library_id: "a0000000-0000-4000-8000-000000000001".into(),
            complete: true,
            recovering: false,
        };
        let profile_bytes = serde_json::to_vec(&profile)?;
        let cancellation = WorkCancellation::default();
        let input = ClosedLibraryInput::verify(
            &source,
            ProfileDocument::decode(&profile_bytes)?,
            ClosedManifest::decode(MANIFEST)?,
            &cancellation,
        )?;
        let locations = ProfileLocations::for_host(
            &roots.profile().join("home"),
            Some(&roots.profile().join("config")),
            Some(&roots.library().join("data")),
        )?;
        fs::create_dir_all(&locations.data)?;
        fs::create_dir_all(locations.native.parent().ok_or("native parent")?)?;
        let legacy = locations.legacy.as_ref().ok_or("legacy namespace")?;
        fs::create_dir_all(legacy)?;
        fs::write(legacy.join("settings.json"), &profile_bytes)?;
        let target = locations.data.join("candidate");
        let copy = input.copy_to_new_root(&target, &cancellation)?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::bind_independent(copy, &store, &locations, &cancellation)
            .map_err(|e| e.to_string())?;
        assert!(matches!(
            librairii_storage::adoption::open_legacy_in_place(&source),
            Err(StorageError::LegacyLifetimeExclusionUnproven)
        ));

        let active_round = |native: NativeLibrary<'_>| -> Result {
            let mut db = Database::open_native(&native)?;
            // Normal native opening already reconciles the frozen fixture's
            // abandoned intent. Persist a NEW valid pre-move intent after
            // startup, so the legacy launch actually overlaps live recovery
            // data under this candidate's durable Active record.
            let intent = db.transaction(WorkCancellation::default(), |tx| {
                let changed = tx.execute("INSERT INTO removal_intents(id,story_id,managed_path,trash_path) SELECT '70000000-0000-4000-8000-000000000004',story_id,managed_path,'trash/removals/70000000-0000-4000-8000-000000000004/'||substr(managed_path,10) FROM story_archives WHERE story_id=1", [])?;
                if changed != 1 { return Err(StorageError::SnapshotChanged); }
                Ok(())
            })?.wait()?;
            assert!(matches!(intent, TransactionOutcome::Committed(())));
            let (title, intents) = db
                .barrier_read(|tx| {
                    Ok((
                        tx.query_row("SELECT embedded_title FROM stories WHERE id=1", [], |row| {
                            row.get::<_, String>(0)
                        })?,
                        tx.query_row("SELECT count(*) FROM removal_intents", [], |row| {
                            row.get::<_, i64>(0)
                        })?,
                    ))
                })?
                .wait()?;
            assert!(
                intents > 0,
                "fixture must include a live durable removal intent"
            );
            probe(&helper, "blocked", &target, &target)?;
            let candidate = files(&target)?;
            let preferences = files(store.directory())?;
            let source_database = fs::read(source.join("db/librairii.sqlite3"))?;
            let receipt: Value =
                serde_json::from_slice(&probe(&helper, "legacy", legacy, &source)?)?;
            assert_eq!(receipt["root"], source.to_str().ok_or("source UTF8")?);
            assert_eq!(receipt["libraryId"], profile.library_id);
            assert_eq!(receipt["changed"], 1);
            assert_ne!(
                fs::read(source.join("db/librairii.sqlite3"))?,
                source_database,
                "later Go launch must actually mutate its source"
            );
            assert_eq!(
                files(&target)?,
                candidate,
                "legacy startup/write changed candidate bytes or live intent"
            );
            assert_eq!(files(store.directory())?, preferences);
            assert_eq!(fs::read(legacy.join("settings.json"))?, profile_bytes);
            assert_eq!(
                db.barrier_read(|tx| Ok(tx.query_row(
                    "SELECT embedded_title FROM stories WHERE id=1",
                    [],
                    |row| row.get::<_, String>(0)
                )?))?
                .wait()?,
                title
            );
            // The rehearsal created this pre-move intent and performed no
            // custody effect. Settle only that exact owned intent before the
            // real close, which correctly refuses unsettled removal records.
            let settled = db.repair_transaction(|tx| {
                if tx.execute("DELETE FROM removal_intents WHERE id='70000000-0000-4000-8000-000000000004' AND story_id=1", [])? != 1 {
                    return Err(StorageError::SnapshotChanged);
                }
                Ok(())
            })?.wait()?;
            assert!(matches!(settled, TransactionOutcome::Committed(())));
            db.close()?;
            drop(db);
            native.finish().map_err(|e| e.to_string())?;
            Ok(())
        };
        active_round(native)?;
        active_round(NativeLibrary::reopen(&store, &locations, &cancellation)?)?;

        // Reverse language direction, then actual process death/reap. A stale
        // lock-file name never stands in for the kernel ownership observation.
        let ready = roots.profile().join("go-lock-ready");
        let mut child = OwnedChild(
            Command::new(&helper)
                .env_clear()
                .arg("hold")
                .arg(&target)
                .arg(&ready)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ready.exists() {
            if child.0.try_wait()?.is_some() || Instant::now() >= deadline {
                return Err("Go owner did not become ready".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        if Instant::now() >= deadline {
            return Err("late Go readiness".into());
        }
        assert_eq!(fs::read(ready)?, b"go-flock-owned\n");
        assert!(matches!(
            NativeLibrary::reopen(&store, &locations, &cancellation),
            Err(StorageError::LibraryInUse)
        ));
        child.0.kill()?;
        child.0.wait()?;
        let native = NativeLibrary::reopen(&store, &locations, &cancellation)?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;

        let mut archives = 0;
        for entry in manifest.as_array().ok_or("manifest array")? {
            let path = entry["path"].as_str().ok_or("manifest path")?;
            if path.starts_with("archives/") && entry["directory"] != true {
                let size = entry["size"].as_u64().ok_or("archive size")?;
                for root in [&source, &target] {
                    assert_eq!(
                        hash_stream(&mut fs::File::open(root.join(path))?, size, &cancellation)?
                            .checksum()
                            .to_string(),
                        entry["sha256"].as_str().ok_or("archive hash")?
                    );
                }
                archives += 1;
            }
        }
        assert_eq!(archives, 5);
        println!(
            "{}",
            json!({"schema":18,"archiveHashes":archives,"goSeesRustLock":true,"rustSeesGoLock":true,"goCrashReleasesLock":true,"legacySourceOnlyDuringActiveRemovalIntent":true,"legacySourceOnlyAfterNativeReopen":true,"candidateAndProfilesUnchangedByLegacy":true,"inPlaceRefused":true})
        );
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    return rehearsal::run();
    #[cfg(not(unix))]
    Err("storage handover rehearsal requires a qualified Unix host".into())
}
