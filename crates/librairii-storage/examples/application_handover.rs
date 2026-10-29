//! Real current/frozen Go application lifecycles overlapping owned Rust journals.
mod handover_support;
use handover_support::{OwnedChild, Result, files, probe, seal, snapshot};
use librairii_app::ports::jobs::{JobStore, StoreResult};
use librairii_storage::{
    StorageError,
    database::{Database, TransactionOutcome},
    native::{NativeLibrary, RelocationObserver, RelocationStage},
    operations_repository::TerminalRepository,
    profile::{NativeProfileStore, Profile, ProfileLocations},
    settings::{Startup, TargetPreview, confirm_adoption, inspect_startup, prepare_native_profile},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
const ID: &str = "a0000000-0000-4000-8000-000000000001";
const REMOVAL: &str = "70000000-0000-4000-8000-000000000017";
const EXPORT: &str = "71000000-0000-4000-8000-000000000017";
fn locations(case: &Path) -> Result<ProfileLocations> {
    Ok(ProfileLocations::for_host(
        &case.join("home"),
        Some(&case.join("config")),
        Some(&case.join("data")),
    )?)
}
fn go_profile(directory: &Path, root: &Path) -> Result {
    fs::create_dir_all(directory)?;
    fs::write(
        directory.join("settings.json"),
        serde_json::to_vec(&Profile {
            version: 1,
            language: "fr".into(),
            root: root.to_str().ok_or("root")?.into(),
            library_id: ID.into(),
            complete: true,
            recovering: false,
        })?,
    )?;
    Ok(())
}
fn seed(case: &Path, prefix: Option<usize>) -> Result {
    let l = locations(case)?;
    let source = case.join("legacy");
    fs::create_dir_all(&source)?;
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus");
    let manifest: Value =
        serde_json::from_slice(&fs::read(corpus.join("profiles/library-manifest.json"))?)?;
    for e in manifest.as_array().ok_or("manifest")? {
        let path = e["path"].as_str().ok_or("path")?;
        if e["directory"] == true {
            fs::create_dir(source.join(path))?;
        } else {
            fs::copy(
                corpus.join("libraries/schema018").join(path),
                source.join(path),
            )?;
        }
    }
    if let Some(prefix) = prefix {
        fs::copy(
            corpus.join(format!("databases/prefix-{prefix:03}.sqlite3")),
            source.join("db/librairii.sqlite3"),
        )?;
    }
    go_profile(l.legacy.as_ref().ok_or("legacy")?, &source)?;
    let Startup::AdoptIndependent(input) = inspect_startup(&l, &Default::default()) else {
        return Err("adoption not offered".into());
    };
    let store = prepare_native_profile(&l, &Default::default())?;
    let native = confirm_adoption(
        input,
        TargetPreview::inspect(&case.join("candidate"), &[], &Default::default())?,
        &store,
        &l,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    if prefix.is_some() {
        drop(native);
        return Ok(());
    }
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    Ok(())
}
fn ready(path: &Path) -> Result {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)?;
    file.write_all(b"native-journals-live\n")?;
    file.sync_all()?;
    Ok(())
}
fn child(case: &Path, receipt: &Path) -> Result {
    let l = locations(case)?;
    let store = NativeProfileStore::open_native(&l.native)?;
    let native = NativeLibrary::reopen(&store, &l, &Default::default())?;
    let db = Database::open_native(&native)?;
    let repository = TerminalRepository::new(&db, 1);
    assert!(repository.claim_session());
    assert!(matches!(repository.startup(), StoreResult::Known(())));
    let external = case.join("external exports");
    fs::create_dir(&external)?;
    fs::write(external.join("foreign.partial"), b"unowned export bytes")?;
    let destination = external.to_str().ok_or("destination")?.to_owned();
    assert!(matches!(db.transaction(Default::default(),move|tx|{
        tx.execute("INSERT INTO removal_intents(id,story_id,managed_path,trash_path) SELECT ?1,story_id,managed_path,'trash/removals/'||?1||'/archive.zip' FROM story_archives WHERE story_id=1",[REMOVAL])?;
        tx.execute("INSERT INTO file_operations(id,kind,status,total_items,total_bytes,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination) VALUES(?1,'export','running',1,1292,'selection','[]','[]',?2)",rusqlite::params![EXPORT,destination])?;
        tx.execute("INSERT INTO file_operation_items(operation_id,story_id,source_name,status,total_bytes,resolved_story_id,story_uuid,story_title,output_name,archive_relative_path,archive_sha256) SELECT ?1,s.id,a.original_filename,'running',a.byte_size,s.id,s.uuid,s.embedded_title,'exported.zip',a.managed_path,a.sha256 FROM stories s JOIN story_archives a ON a.story_id=s.id WHERE s.id=1",[EXPORT])?;
        Ok(())
    })?.wait()?,TransactionOutcome::Committed(())));
    ready(receipt)?;
    loop {
        thread::park();
    }
}
fn protected(case: &Path) -> Result<Value> {
    let l = locations(case)?;
    let legacy = l.legacy.as_ref().ok_or("legacy")?.strip_prefix(case)?;
    let filtered = files(case)?
        .into_iter()
        .filter(|(p, _)| !p.starts_with("legacy") && !p.starts_with(legacy))
        .map(|(p, b)| {
            (
                p,
                b.len(),
                librairii_core::Checksum::from_bytes(sha2::Sha256::digest(&b).into()).to_string(),
            )
        })
        .collect::<Vec<_>>();
    Ok(json!(filtered))
}
use sha2::Digest;
fn overlap(
    case: &Path,
    frozen: &Path,
    bridge: &Path,
    bridge_profiles: &Path,
    label: &str,
) -> Result<Value> {
    let l = locations(case)?;
    let source = case.join("legacy");
    let before = protected(case)?;
    let legacy = l.legacy.as_ref().ok_or("legacy")?;
    let legacy_settings = fs::read(legacy.join("settings.json"))?;
    let old_source = fs::read(source.join("db/librairii.sqlite3"))?;
    let result = probe(frozen, "mutate", legacy, &source)?;
    assert_eq!(result["root"], source.to_str().ok_or("root")?);
    assert_eq!(result["closed"], true);
    assert_ne!(
        fs::read(source.join("db/librairii.sqlite3"))?,
        old_source,
        "unmodified application must really mutate its own source"
    );
    assert_eq!(
        protected(case)?,
        before,
        "frozen application changed candidate/profile/journal bytes"
    );
    assert_eq!(fs::read(legacy.join("settings.json"))?, legacy_settings);
    let mut refused = Vec::new();
    for name in ["candidate", "moved"] {
        let root = case.join(name);
        if root.join(".librairii-library").exists() {
            let profile = bridge_profiles.join(format!("{label}-{name}"));
            go_profile(&profile, &root)?;
            assert_eq!(probe(bridge, "blocked", &profile, &root)?["blocked"], true);
            refused.push(name);
        }
    }
    assert!(
        !refused.is_empty(),
        "no participating second launch attempted"
    );
    assert_eq!(protected(case)?, before);
    Ok(
        json!({"stage":label,"frozen_application":result,"candidate_and_profile_and_journal_unchanged":true,"bridge_refused_before_recovery":refused,"protected_manifest":before}),
    )
}
struct Observer<'a> {
    case: &'a Path,
    frozen: &'a Path,
    bridge: &'a Path,
    profiles: &'a Path,
    observations: Mutex<Vec<std::result::Result<Value, String>>>,
}
impl RelocationObserver for Observer<'_> {
    fn observe(&self, stage: RelocationStage) {
        let result = overlap(
            self.case,
            self.frozen,
            self.bridge,
            self.profiles,
            &format!("{stage:?}"),
        )
        .map_err(|error| format!("application overlap {stage:?}: {error}"));
        if let Ok(mut entries) = self.observations.lock() {
            entries.push(result);
        }
    }
}
fn migration(roots: &librairii_testkit::TestRoots, frozen: &Path, bridge: &Path) -> Result<Value> {
    let case = roots.library().join("interrupted migration");
    fs::create_dir(&case)?;
    seed(&case, Some(1))?;
    let l = locations(&case)?;
    let candidate = case.join("candidate");
    let source = case.join("legacy");
    // Only this closed disposable candidate changes journal mode to install
    // the deterministic SQLite read barrier; the legacy input stays exact.
    let fixture = rusqlite::Connection::open(candidate.join("db/librairii.sqlite3"))?;
    fixture.execute_batch("PRAGMA journal_mode=DELETE;")?;
    drop(fixture);
    let original_database = fs::read(candidate.join("db/librairii.sqlite3"))?;
    let source_backup = seal(&source)?;
    // A real readonly SQLite transaction holds the initial journal-mode switch.
    // The child has acquired native ownership and published Active, but cannot
    // commit schema changes. SIGKILL therefore exercises a deterministic
    // interrupted upgrade before its first migration, never a timing race.
    let blocker = rusqlite::Connection::open_with_flags(
        candidate.join("db/librairii.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    blocker.execute_batch("BEGIN; SELECT version FROM schema_migrations LIMIT 1;")?;
    let mut process = OwnedChild(
        Command::new(std::env::current_exe()?)
            .arg("migration-child")
            .arg(&case)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let state: Value =
            serde_json::from_slice(&fs::read(candidate.join(".librairii-native-state.json"))?)?;
        let no_pending_control = fs::read_dir(&candidate)?.all(|entry| {
            entry.is_ok_and(|e| {
                !e.file_name()
                    .to_string_lossy()
                    .starts_with(".librairii-native-state.json.tmp-")
            })
        });
        let directories_ready = ["db", "archives", "catalog", "staging", "trash", "logs"]
            .iter()
            .all(|d| candidate.join(d).is_dir());
        if state["state"] == "active" && no_pending_control && directories_ready {
            break;
        }
        if process.0.try_wait()?.is_some() || Instant::now() >= deadline {
            return Err("upgrade child did not acquire native ownership".into());
        }
        thread::sleep(Duration::from_millis(2));
    }
    assert!(
        Command::new("/bin/kill")
            .arg("-STOP")
            .arg(process.0.id().to_string())
            .status()?
            .success()
    );
    let before = protected(&case)?;
    assert_eq!(protected(&case)?, before, "upgrade child did not stop");
    let bridge_profile = roots.profile().join("migration bridge");
    go_profile(&bridge_profile, &candidate)?;
    assert_eq!(
        probe(bridge, "blocked", &bridge_profile, &candidate)?["blocked"],
        true
    );
    let legacy = l.legacy.as_ref().ok_or("legacy")?;
    assert_eq!(probe(frozen, "open", legacy, &source)?["closed"], true);
    assert_eq!(
        protected(&case)?,
        before,
        "later original Go changed the interrupted candidate"
    );
    assert!(
        process.0.try_wait()?.is_none(),
        "migration passed the SQLite blocker"
    );
    process.0.kill()?;
    assert_eq!(process.0.wait()?.signal(), Some(9));
    assert_eq!(
        fs::read(candidate.join("db/librairii.sqlite3"))?,
        original_database
    );
    blocker.execute_batch("ROLLBACK")?;
    drop(blocker);
    let store = NativeProfileStore::open_native(&l.native)?;
    let recovered = NativeLibrary::recover_interrupted(&store, &l, &Default::default())?;
    let mut db = recovered.open()?;
    let snapshot = snapshot(&db)?;
    assert_eq!(
        snapshot["tables"]["schema_migrations"]
            .as_array()
            .ok_or("ledger")?
            .len(),
        18
    );
    assert_eq!(snapshot["integrity"], "ok");
    assert_eq!(snapshot["foreignKeyViolations"], json!([]));
    db.close()?;
    drop(db);
    recovered.finish().map_err(|e| e.to_string())?;
    Ok(
        json!({"boundary":"native Active before first migration commit; SQLite readonly blocker","sigkill":9,"candidate_database_unchanged_before_recovery":true,"bridge_refused_before_recovery":true,"unmodified_go_opened_and_upgraded_source_only":true,"candidate_and_native_journal_unchanged_by_go":true,"closed_source_backup_manifest":source_backup,"after_recovery":snapshot,"candidate_manifest":seal(&candidate)?}),
    )
}
fn main() -> Result {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|a| a == "child") {
        return child(
            Path::new(args.get(1).ok_or("case")?),
            Path::new(args.get(2).ok_or("receipt")?),
        );
    }
    if args.first().is_some_and(|a| a == "migration-child") {
        let case = Path::new(args.get(1).ok_or("case")?);
        let l = locations(case)?;
        let store = NativeProfileStore::open_native(&l.native)?;
        let owner = NativeLibrary::reopen(&store, &l, &Default::default())?;
        let mut db = Database::open_native(&owner)?;
        db.close()?;
        drop(db);
        owner.finish().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let frozen = fs::canonicalize(args.first().ok_or("frozen application helper")?)?;
    let bridge = fs::canonicalize(args.get(1).ok_or("bridge application helper")?)?;
    let roots = librairii_testkit::TestRoots::new()?;
    let case = roots.library().join("application handover");
    fs::create_dir(&case)?;
    seed(&case, None).map_err(|e| format!("seed/adopt: {e}"))?;
    let receipt = roots.profile().join("ready");
    let mut process = OwnedChild(
        Command::new(std::env::current_exe()?)
            .arg("child")
            .arg(&case)
            .arg(&receipt)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while !receipt.exists() {
        if process.0.try_wait()?.is_some() || Instant::now() >= deadline {
            return Err("native journal child did not become ready".into());
        }
        thread::sleep(Duration::from_millis(5));
    }
    let l = locations(&case)?;
    assert!(matches!(
        NativeProfileStore::open_native(&l.native),
        Err(StorageError::ProfileInUse)
    ));
    let live = overlap(
        &case,
        &frozen,
        &bridge,
        roots.profile(),
        "removal-and-export-before-effects",
    )?;
    process.0.kill()?;
    assert_eq!(process.0.wait()?.signal(), Some(9));
    let store = NativeProfileStore::open_native(&l.native)
        .map_err(|e| format!("post-crash profile: {e}"))?;
    let recovered = NativeLibrary::recover_interrupted(&store, &l, &Default::default())
        .map_err(|e| format!("post-crash owner: {e}"))?;
    let mut db = recovered
        .open()
        .map_err(|e| format!("post-crash database/recovery: {e}"))?;
    let repository = TerminalRepository::new(&db, 2);
    assert!(repository.claim_session());
    assert!(matches!(repository.startup(), StoreResult::Known(())));
    repository.release_session();
    let after = snapshot(&db)?;
    assert!(
        !after["tables"]["removal_intents"]
            .as_array()
            .ok_or("intents")?
            .iter()
            .any(|r| r["id"] == REMOVAL)
    );
    let operation = after["tables"]["file_operations"]
        .as_array()
        .ok_or("operations")?
        .iter()
        .find(|r| r["id"] == EXPORT)
        .ok_or("recovered export")?;
    assert_eq!(operation["status"], "interrupted");
    assert_eq!(
        fs::read(case.join("external exports/foreign.partial"))?,
        b"unowned export bytes"
    );
    let restarted = overlap(&case, &frozen, &bridge, roots.profile(), "native-restarted")?;
    db.close()?;
    drop(db);
    recovered.finish().map_err(|e| e.to_string())?;
    let native = NativeLibrary::reopen(&store, &l, &Default::default())?;
    let mut db = Database::open_native(&native)?;
    db.close()?;
    drop(db);
    let observer = Observer {
        case: &case,
        frozen: &frozen,
        bridge: &bridge,
        profiles: roots.profile(),
        observations: Mutex::new(Vec::new()),
    };
    let moved = native
        .close_for_relocation()
        .map_err(|e| e.to_string())?
        .relocate_observed(
            TargetPreview::inspect(&case.join("moved"), &[], &Default::default())?,
            &l,
            "72000000-0000-4000-8000-000000000017".parse()?,
            &Default::default(),
            &observer,
        )
        .map_err(|e| e.to_string())?;
    let mut db = Database::open_native(&moved.library)?;
    assert_eq!(snapshot(&db)?, after);
    db.close()?;
    drop(db);
    moved.library.finish().map_err(|e| e.to_string())?;
    let native = NativeLibrary::reopen(&store, &l, &Default::default())?;
    let mut db = Database::open_native(&native)?;
    assert_eq!(snapshot(&db)?, after);
    let after_move = overlap(
        &case,
        &frozen,
        &bridge,
        roots.profile(),
        "native-restarted-after-move",
    )?;
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    let stages = observer
        .observations
        .into_inner()
        .map_err(|_| "observations")?
        .into_iter()
        .collect::<std::result::Result<Vec<_>, String>>()?;
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
        assert!(
            stages.iter().any(|r| r["stage"] == stage),
            "missing relocation overlap {stage}"
        );
    }
    let migration = migration(&roots, &frozen, &bridge)?;
    println!(
        "{}",
        json!({"format":"librairii.application-handover","version":1,"migration":migration,"live":live,"sigkill":9,"native_second_launch_refused_before_recovery":true,"recovered_export":operation,"restarted":restarted,"relocation_stages":stages,"after_move":after_move,"final_native_profile":seal(store.directory())?,"final_database":after,"foreign_export_preserved":true})
    );
    Ok(())
}
