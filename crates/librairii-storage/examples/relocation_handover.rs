//! Synthetic process-death, permissions, ENOSPC and external-volume rehearsal.
//! Only scripts/verify-native-relocation.py creates the disposable volume and
//! supplies both initially empty fixture directories. No production discovery.
#[cfg(unix)]
mod rehearsal {
    use librairii_app::localization::Language;
    use librairii_storage::{
        database::Database,
        native::{
            CommitStatus, NativeLibrary, RecoveredRoot, RelocationObserver, RelocationStage,
            recover_relocation, retry_relocation,
        },
        platform::hash_stream,
        profile::{NativeProfileStore, ProfileLocations},
        settings::TargetPreview,
    };
    use serde_json::{Value, json};
    use std::{
        error::Error,
        fs::{self, OpenOptions},
        io::Write,
        os::unix::{
            fs::{MetadataExt, PermissionsExt},
            process::ExitStatusExt,
        },
        path::{Path, PathBuf},
        process::{Child, Command, Stdio},
        sync::{
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };
    type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
    const ID: &str = "f1000000-0000-4000-8000-000000000013";
    const MOVE: &str = "f2000000-0000-4000-8000-000000000013";
    const PAYLOAD: &str = "original é archive.pk";
    fn locations(case: &Path) -> Result<ProfileLocations> {
        Ok(ProfileLocations::for_host(
            &case.join("home"),
            Some(&case.join("config")),
            Some(&case.join("data")),
        )?)
    }
    fn setup(case: &Path) -> Result {
        fs::create_dir(case)?;
        let locations = locations(case)?;
        fs::create_dir_all(&locations.data)?;
        fs::create_dir_all(locations.native.parent().ok_or("profile parent")?)?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let source = case.join("source");
        let native = NativeLibrary::initialize_confirmed(
            TargetPreview::inspect(&source, &[], &Default::default())?,
            &store,
            &locations,
            ID.parse()?,
            Language::French,
            &Default::default(),
        )
        .map_err(|e| e.to_string())?;
        let mut database = Database::open_native(&native)?;
        database.close()?;
        drop(database);
        // Synthetic bytes only, deliberately independent of archive decoding.
        fs::write(
            source.join("archives").join(PAYLOAD),
            vec![0x6du8; 1024 * 1024],
        )?;
        fs::write(
            source.join("catalog/snapshot.json"),
            b"synthetic catalog bytes",
        )?;
        fs::create_dir(source.join("trash/removals"))?;
        fs::write(
            source.join("trash/removals/retained-original"),
            b"synthetic retained removal",
        )?;
        fs::write(source.join("notes.txt"), b"additional root data")?;
        native.finish().map_err(|e| e.to_string())?;
        Ok(())
    }
    fn checksum(path: &Path) -> Result<String> {
        Ok(
            hash_stream(&mut fs::File::open(path)?, u64::MAX, &Default::default())?
                .checksum()
                .to_string(),
        )
    }
    struct Paused {
        stage: String,
        ready: PathBuf,
    }
    impl RelocationObserver for Paused {
        fn observe(&self, stage: RelocationStage) {
            if format!("{stage:?}") == self.stage {
                let result = (|| -> std::io::Result<()> {
                    let mut f = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&self.ready)?;
                    f.write_all(format!("{stage:?}").as_bytes())?;
                    f.sync_all()
                })();
                if result.is_err() {
                    std::process::exit(71);
                }
                loop {
                    thread::park();
                } // parent forcibly kills this process
            }
        }
    }
    fn child(case: &Path, target: &Path, stage: String, ready: PathBuf) -> Result {
        let locations = locations(case)?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let mut database = Database::open_native(&native)?;
        database.close()?;
        drop(database);
        let closed = native.close_for_relocation().map_err(|e| e.to_string())?;
        let result = closed.relocate_observed(
            TargetPreview::inspect(target, &[], &Default::default())?,
            &locations,
            MOVE.parse()?,
            &Default::default(),
            &Paused { stage, ready },
        );
        match result {
            Ok(_) => Err("child passed its crash checkpoint".into()),
            Err(e) => Err(e.to_string().into()),
        }
    }
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn crash(case: &Path, target: &Path, stage: &str) -> Result<Value> {
        setup(case)?;
        let source = case.join("source");
        let before = checksum(&source.join("archives").join(PAYLOAD))?;
        let ready = case.join("ready");
        let mut child = OwnedChild(
            Command::new(std::env::current_exe()?)
                .arg("child")
                .arg(case)
                .arg(target)
                .arg(stage)
                .arg(&ready)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()?,
        );
        let deadline = Instant::now() + Duration::from_secs(30);
        while !ready.exists() {
            if let Some(status) = child.0.try_wait()? {
                return Err(format!("child exited before {stage}: {status}").into());
            }
            if Instant::now() > deadline {
                return Err(format!("checkpoint {stage} deadline").into());
            }
            thread::sleep(Duration::from_millis(5));
        }
        child.0.kill()?;
        let status = child.0.wait()?;
        if status.signal() != Some(9) {
            return Err("process was not terminated by SIGKILL".into());
        }
        let locations = locations(case)?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        let recovery = recover_relocation(&store, &locations, &Default::default())?;
        let committed = matches!(stage, "Committed" | "Retired" | "SourceRetired");
        let configured = store.load()?.ok_or("saved profile")?.profile().clone();
        let expected = if committed { target } else { &source };
        if Path::new(&configured.root) != expected || configured.language != "fr" {
            return Err("wrong recovery authority/language".into());
        }
        if let Some(recovery) = &recovery
            && recovery.authoritative
                != if committed {
                    RecoveredRoot::Destination
                } else {
                    RecoveredRoot::Source
                }
        {
            return Err("journal authority disagrees".into());
        }
        if !committed && stage != "Workspace" {
            let outcome = retry_relocation(&store, &locations, &Default::default())
                .map_err(|e| e.to_string())?;
            let mut db = Database::open_native(&outcome.library)?;
            db.close()?;
            drop(db);
            outcome.library.finish().map_err(|e| e.to_string())?;
            if checksum(&target.join("archives").join(PAYLOAD))? != before {
                return Err("retry changed payload".into());
            }
        } else {
            let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
            let mut db = Database::open_native(&native)?;
            db.close()?;
            drop(db);
            native.finish().map_err(|e| e.to_string())?;
            if checksum(&expected.join("archives").join(PAYLOAD))? != before {
                return Err("recovery changed payload".into());
            }
        }
        Ok(
            json!({"stage":stage,"signal":status.signal(),"authority":if committed {"destination"} else {"source"},"payload_sha256":before,"retry_verified":!committed && stage!="Workspace","database_reopened":true,"language":"fr"}),
        )
    }
    struct FailureObserver {
        kind: &'static str,
        volume: PathBuf,
        workspace: PathBuf,
        observed: AtomicBool,
        cause: Mutex<Option<i32>>,
    }
    impl RelocationObserver for FailureObserver {
        fn observe(&self, stage: RelocationStage) {
            if stage != RelocationStage::Journal || self.observed.swap(true, Ordering::AcqRel) {
                return;
            }
            if self.kind == "permissions" {
                if fs::set_permissions(&self.workspace, fs::Permissions::from_mode(0o500)).is_err()
                {
                    std::process::exit(72);
                }
            } else {
                let result = (|| -> std::io::Result<()> {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(self.volume.join("owned-enospc-fill"))?;
                    let buffer = [0xadu8; 64 * 1024];
                    for _ in 0..8192 {
                        file.write_all(&buffer)?;
                    }
                    file.sync_all()
                })();
                if let Err(error) = result
                    && let Ok(mut cause) = self.cause.lock()
                {
                    *cause = error.raw_os_error();
                }
            }
        }
    }
    fn failure(case: &Path, volume: &Path, kind: &'static str) -> Result<Value> {
        setup(case)?;
        let locations = locations(case)?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let source = native.layout().root().to_path_buf();
        let before = checksum(&source.join("archives").join(PAYLOAD))?;
        let mut database = Database::open_native(&native)?;
        database.close()?;
        drop(database);
        let closed = native.close_for_relocation().map_err(|e| e.to_string())?;
        let target = volume.join(format!("{kind}-target"));
        let workspace = volume.join(format!(".librairii-native-move-{MOVE}"));
        let observer = FailureObserver {
            kind,
            volume: volume.to_path_buf(),
            workspace: workspace.clone(),
            observed: AtomicBool::new(false),
            cause: Mutex::new(None),
        };
        let result = closed.relocate_observed(
            TargetPreview::inspect(&target, &[], &Default::default())?,
            &locations,
            MOVE.parse()?,
            &Default::default(),
            &observer,
        );
        if kind == "permissions" {
            fs::set_permissions(&workspace, fs::Permissions::from_mode(0o700))?;
        } else {
            fs::remove_file(volume.join("owned-enospc-fill"))?;
        }
        let failure = result
            .err()
            .ok_or("filesystem fault did not refuse relocation")?;
        if failure.commit_status() != CommitStatus::BeforeCommit {
            return Err("fault crossed root commit".into());
        }
        let actual = match failure.storage_error() {
            librairii_storage::StorageError::Io { source, .. } => source.raw_os_error(),
            _ => None,
        };
        if (kind == "permissions" && actual != Some(13))
            || (kind == "disk-full" && actual != Some(28))
        {
            return Err(
                format!("unexpected real {kind} failure: {failure}; errno={actual:?}").into(),
            );
        }
        if checksum(&source.join("archives").join(PAYLOAD))? != before {
            return Err("fault changed source".into());
        }
        let (native, _) = failure.resume_source().map_err(|e| e.to_string())?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        native.finish().map_err(|e| e.to_string())?;
        if Path::new(&store.load()?.ok_or("profile")?.profile().root) != source {
            return Err("fault retargeted settings".into());
        }
        Ok(
            json!({"failure":kind,"observed_errno":actual,"source_unchanged":true,"source_reopened":true,"before_commit":true,"filler_errno":*observer.cause.lock().map_err(|_|"fault lock")?}),
        )
    }
    fn volume_root(case: &Path, volume: &Path) -> Result<Value> {
        if fs::read_dir(volume)?.next().is_some() {
            return Ok(
                json!({"volume_root":"rejected_nonempty_system_entries","external_folder_test_required":true}),
            );
        }
        setup(case)?;
        let locations = locations(case)?;
        let store = NativeProfileStore::open_native(&locations.native)?;
        let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
        let before = checksum(&native.layout().archives().join(PAYLOAD))?;
        let mut db = Database::open_native(&native)?;
        db.close()?;
        drop(db);
        let closed = native.close_for_relocation().map_err(|e| e.to_string())?;
        let outcome = closed
            .relocate(
                TargetPreview::inspect(volume, &[], &Default::default())?,
                &locations,
                MOVE.parse()?,
                &Default::default(),
            )
            .map_err(|e| e.to_string())?;
        if outcome.library.layout().root() != volume
            || checksum(&volume.join("archives").join(PAYLOAD))? != before
        {
            return Err("volume root was not the exact selected root".into());
        }
        let mut db = Database::open_native(&outcome.library)?;
        db.close()?;
        drop(db);
        outcome.library.finish().map_err(|e| e.to_string())?;
        // Fixture-only cleanup of this initially empty, disposable disk image.
        // The wrapper owns/unmounts the image; no production path is accepted.
        for entry in fs::read_dir(volume)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                fs::remove_dir_all(entry.path())?;
            } else {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(json!({"volume_root":"exact_root_verified","payload_sha256":before}))
    }
    pub fn run() -> Result {
        let args = std::env::args_os().collect::<Vec<_>>();
        if args.get(1).is_some_and(|s| s == "child") {
            if args.len() != 6 {
                return Err("child fixture arguments".into());
            }
            return child(
                Path::new(&args[2]),
                Path::new(&args[3]),
                args[4].to_str().ok_or("stage")?.into(),
                PathBuf::from(&args[5]),
            );
        }
        if args.len() != 3 {
            return Err("explicit empty fixture and disposable volume paths required".into());
        }
        let root = fs::canonicalize(&args[1])?;
        let volume = fs::canonicalize(&args[2])?;
        if fs::read_dir(&root)?.next().is_some()
            || root == volume
            || root.metadata()?.dev() == volume.metadata()?.dev()
        {
            return Err("empty fixture plus actual separate filesystem required".into());
        }
        let source_device = root.metadata()?.dev();
        let destination_device = volume.metadata()?.dev();
        let root_case = volume_root(&root.join("volume-root"), &volume)?;
        let mut crashes = Vec::new();
        for stage in [
            "Workspace",
            "Journal",
            "File",
            "Published",
            "BeforeCommit",
            "Committed",
            "Retired",
            "SourceRetired",
        ] {
            let name = format!("case-{}", stage.to_ascii_lowercase());
            let case = root.join(&name);
            let destination = volume.join(&name);
            fs::create_dir(&destination)?;
            crashes.push(crash(&case, &destination.join("chosen root"), stage)?);
        }
        let mut failures = Vec::new();
        for kind in ["permissions", "disk-full"] {
            let destination = volume.join(format!("{kind}-fixture"));
            fs::create_dir(&destination)?;
            failures.push(failure(
                &root.join(format!("{kind}-case")),
                &destination,
                kind,
            )?);
        }
        println!(
            "{}",
            serde_json::to_string(
                &json!({"format":"librairii.native-relocation-rehearsal","version":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"source_device":source_device,"destination_device":destination_device,"separate_filesystem":true,"volume_root":root_case,"forced_termination":crashes,"filesystem_failures":failures})
            )?
        );
        Ok(())
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        rehearsal::run()
    }
    #[cfg(not(unix))]
    {
        Err("native relocation rehearsal requires a supported Unix host".into())
    }
}
