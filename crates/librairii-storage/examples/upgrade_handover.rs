//! Task17: actual independent adoption of every retained prefix and bridge source.
mod handover_support;
use handover_support::{Result, files, probe, seal, snapshot};
use librairii_core::{collection_codec::Query, navigation::Presentation};
use librairii_storage::{
    StorageError,
    adoption::{ClosedLibraryInput, ClosedManifest},
    database::{Database, TransactionOutcome},
    native::NativeLibrary,
    profile::{HandoverImport, Profile, ProfileDocument, ProfileLocations},
    settings::{
        Startup, TargetPreview, confirm_adoption, default_legacy_root, inspect_startup,
        prepare_native_profile,
    },
};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
const ID: &str = "a0000000-0000-4000-8000-000000000001";
fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../preservation/corpus")
}
fn seed(root: &Path, prefix: usize) -> Result {
    fs::create_dir_all(root)?;
    let manifest: Value =
        serde_json::from_slice(&fs::read(corpus().join("profiles/library-manifest.json"))?)?;
    for entry in manifest.as_array().ok_or("manifest")? {
        let path = entry["path"].as_str().ok_or("path")?;
        if entry["directory"] == true {
            fs::create_dir(root.join(path))?;
        } else {
            fs::copy(
                corpus().join("libraries/schema018").join(path),
                root.join(path),
            )?;
        }
    }
    if prefix <= 18 {
        fs::copy(
            corpus().join(format!("databases/prefix-{prefix:03}.sqlite3")),
            root.join("db/librairii.sqlite3"),
        )?;
    }
    Ok(())
}
fn profile(root: &Path) -> Result<Profile> {
    Ok(Profile {
        version: 1,
        language: "fr".into(),
        root: root.to_str().ok_or("root UTF8")?.into(),
        library_id: ID.into(),
        complete: true,
        recovering: false,
    })
}
fn copy_tree(source: &Path, target: &Path) -> Result {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(())
}
fn normalize_times(db: &Database<'_>) -> Result {
    // Exact policy of preservation/generator/databases.go::fixTimes, applied
    // only to these disposable synthetic candidate databases.
    let outcome=db.transaction(Default::default(),|tx|{
        let tables=tx.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")?.query_map([],|r|r.get::<_,String>(0))?.collect::<rusqlite::Result<Vec<_>>>()?;
        for name in tables {let name=name.replace('"',"\"\"");let columns=tx.prepare(&format!("PRAGMA table_info(\"{name}\")"))?.query_map([],|r|r.get::<_,String>(1))?.collect::<rusqlite::Result<Vec<_>>>()?;
            for column in columns {if matches!(column.as_str(),"created_at"|"updated_at"|"applied_at"|"added_at"|"accepted_at"|"started_at"|"finished_at") {tx.execute(&format!("UPDATE \"{name}\" SET \"{column}\"='2000-01-01 00:00:00' WHERE \"{column}\" IS NOT NULL"),[])?;}}
        }Ok(())
    })?.wait()?;
    assert!(matches!(outcome, TransactionOutcome::Committed(())));
    Ok(())
}
fn project(db: &Database<'_>) -> Result {
    let ids = db
        .barrier_read(|tx| {
            Ok(tx
                .prepare("SELECT id FROM collections ORDER BY position,id")?
                .query_map([], |r| r.get::<_, i64>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?)
        })?
        .wait()?;
    for id in ids {
        assert!(matches!(
            db.inspect_collection_compatibility(id, Default::default())?
                .wait()?,
            TransactionOutcome::Committed(_)
        ));
    }
    Ok(())
}
fn run_case(prefix: usize, configured: bool, bridge: &Path) -> Result<Value> {
    let roots = librairii_testkit::TestRoots::new()?;
    let locations = ProfileLocations::for_host(
        &roots.profile().join("home"),
        Some(&roots.profile().join("config")),
        Some(&roots.library().join("data")),
    )?;
    let source = if configured {
        roots.library().join("configured Go source")
    } else {
        default_legacy_root(&locations)?
    };
    seed(&source, prefix)?;
    let legacy = locations.legacy.as_ref().ok_or("legacy profile")?;
    fs::create_dir_all(legacy)?;
    if configured || prefix == 19 {
        fs::write(
            legacy.join("settings.json"),
            serde_json::to_vec(&profile(&source)?)?,
        )?;
    }
    if prefix == 19 {
        assert_eq!(probe(bridge, "open", legacy, &source)?["closed"], true);
    }
    let source_before = files(&source)?;
    let legacy_before = files(legacy)?;
    let backup = roots.profile().join("closed backup");
    copy_tree(&source, &backup)?;
    let profile_backup = roots.profile().join("profile backup");
    copy_tree(legacy, &profile_backup)?;
    assert_eq!(files(&backup)?, source_before);
    assert_eq!(files(&profile_backup)?, legacy_before);
    let source_manifest = seal(&source)?;
    let backup_manifest = seal(&backup)?;
    assert_eq!(source_manifest, backup_manifest);
    let startup = inspect_startup(&locations, &Default::default());
    let Startup::AdoptIndependent(input) = startup else {
        return Err(format!("prefix{prefix}, configured{configured}: {startup:?}").into());
    };
    let manifest = input.manifest().original_bytes().to_vec();
    assert!(matches!(
        librairii_storage::adoption::open_legacy_in_place(&source),
        Err(StorageError::LegacyLifetimeExclusionUnproven)
    ));
    let target = roots.library().join("native candidate");
    let store = prepare_native_profile(&locations, &Default::default())?;
    let native = confirm_adoption(
        input,
        TargetPreview::inspect(&target, &[], &Default::default())?,
        &store,
        &locations,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    assert_eq!(files(&source)?, source_before);
    assert_eq!(files(legacy)?, legacy_before);
    for path in source_before.keys() {
        let old = fs::metadata(source.join(path))?;
        let new = fs::metadata(target.join(path))?;
        assert_ne!(
            (old.dev(), old.ino()),
            (new.dev(), new.ino()),
            "source/candidate alias: {path:?}"
        );
    }
    let mut db = Database::open_native(&native)?;
    project(&db)?;
    normalize_times(&db)?;
    let upgraded = snapshot(&db)?;
    if prefix <= 18 {
        let oracle: Value = serde_json::from_slice(&fs::read(
            corpus().join(format!("databases/prefix-{prefix:03}.json")),
        )?)?;
        let mut expected = oracle["afterGoOpen"].clone();
        // Native opening safely settles the frozen pre-move intent: its
        // managed archive still exists byte-exact, so no trash/delete occurs.
        // This is a declared recovery delta, not timestamp/row normalization.
        for intent in expected["tables"]["removal_intents"]
            .as_array()
            .ok_or("removal intents")?
        {
            assert_eq!(intent["id"], "70000000-0000-4000-8000-000000000001");
            assert_eq!(intent["story_id"], 1);
            let path = intent["managed_path"].as_str().ok_or("managed intent")?;
            assert_eq!(fs::read(source.join(path))?, fs::read(target.join(path))?);
        }
        expected["tables"]["removal_intents"] = json!([]);
        if upgraded != expected {
            let out = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/quality/upgrade-snapshot-mismatch.json");
            fs::write(
                &out,
                serde_json::to_vec_pretty(
                    &json!({"prefix":prefix,"actual":upgraded,"expected":expected}),
                )?,
            )?;
            return Err(format!(
                "prefix{prefix}: full snapshot mismatch retained at {}",
                out.display()
            )
            .into());
        }
    }
    let mut query = Query::default();
    use librairii_app::ports::jobs::{JobStore, StoreResult};
    let jobs = librairii_storage::operations_repository::TerminalRepository::new(&db, 1);
    assert!(jobs.claim_session());
    assert!(matches!(jobs.startup(), StoreResult::Known(())));
    jobs.release_session();
    query.name = "new native collection".into();
    assert!(matches!(
        db.edit_collections(
            librairii_storage::collections::Command::Create {
                name: "Native addition".into(),
                criteria: query
            },
            Default::default()
        )?
        .wait()?,
        TransactionOutcome::Committed(_)
    ));
    assert!(matches!(
        db.edit_shelves(
            librairii_storage::shelves::Command::Create {
                name: "Native shelf".into()
            },
            Default::default()
        )?
        .wait()?,
        TransactionOutcome::Committed(_)
    ));
    assert!(matches!(db.transaction(Default::default(),|tx|{tx.execute("UPDATE stories SET embedded_title=COALESCE(embedded_title,'')||' native edit' WHERE id=1",[])?;Ok(())})?.wait()?,TransactionOutcome::Committed(())));
    let post_native = snapshot(&db)?;
    assert_ne!(post_native, upgraded);
    let mut transfer: Value = serde_json::from_slice(include_bytes!(
        "../../librairii-core/tests/fixtures/handover-v1.json"
    ))?;
    transfer["libraryId"] = ID.into();
    transfer["state"]["frames"] = json!([{"hash":"#/library?v=3&page=1","kind":"library"},{"hash":"#/library?v=3&name=bonjour%20%C3%A9&page=2","kind":"custom"}]);
    transfer["state"]["position"] = 1.into();
    assert_eq!(
        store.import_handover(&serde_json::to_vec(&transfer)?, ID.parse()?)?,
        HandoverImport::Imported
    );
    assert_eq!(
        store
            .load_navigation(ID.parse()?, 1)?
            .ok_or("navigation")?
            .presentation(),
        Presentation::SmallCovers
    );
    let mut navigation = store.load_navigation(ID.parse()?, 1)?.ok_or("navigation")?;
    let current = navigation.current().location.clone();
    assert!(navigation.back()?);
    assert!(navigation.forward()?);
    assert_eq!(navigation.current().location, current);
    navigation.set_presentation(Presentation::LargeCovers);
    store.save_navigation(&navigation)?;
    db.close()?;
    drop(db);
    native.finish().map_err(|e| e.to_string())?;
    let reopened = NativeLibrary::reopen(&store, &locations, &Default::default())?;
    let mut db = Database::open_native(&reopened)?;
    assert_eq!(
        snapshot(&db)?,
        post_native,
        "all tables and outcomes after native restart"
    );
    assert_eq!(
        store
            .load_navigation(ID.parse()?, 2)?
            .ok_or("navigation")?
            .presentation(),
        Presentation::LargeCovers
    );
    assert!(
        store
            .load_interface(ID.parse()?)?
            .ok_or("interface")?
            .sync_panel_hidden
    );
    assert_eq!(
        store
            .load_interface(ID.parse()?)?
            .ok_or("interface")?
            .dismissed_operations,
        std::collections::BTreeSet::from(["f2000000-0000-4000-8000-000000000017".into()])
    );
    let mut restored = store.load_navigation(ID.parse()?, 2)?.ok_or("navigation")?;
    assert_eq!(restored.current().location, current);
    assert!(restored.back()?);
    assert!(restored.forward()?);
    assert_eq!(restored.current().location, current);
    db.close()?;
    drop(db);
    reopened.finish().map_err(|e| e.to_string())?;
    let post_files = files(&target)?;
    let native_profile = files(store.directory())?;
    let bridge_reopen = roots.library().join("post-Rust Go compatibility clone");
    copy_tree(&target, &bridge_reopen)?;
    let bridge_profile = roots.profile().join("compatibility profile");
    fs::create_dir(&bridge_profile)?;
    fs::write(
        bridge_profile.join("settings.json"),
        serde_json::to_vec(&profile(&bridge_reopen)?)?,
    )?;
    assert_eq!(
        probe(bridge, "native-refused", &bridge_profile, &bridge_reopen)?["blocked"],
        true
    );
    // Only this disposable compatibility experiment removes its COPIED native
    // control record. It grants no production rollback/rebinding permission.
    fs::remove_file(bridge_reopen.join(".librairii-native-state.json"))?;
    assert_eq!(
        probe(bridge, "open", &bridge_profile, &bridge_reopen)?["closed"],
        true
    );
    let connection = rusqlite::Connection::open_with_flags(
        bridge_reopen.join("db/librairii.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let go_after_native = handover_support::snapshot_connection(&connection)?;
    // Snapshot equality is stronger than compatibility: actual Go startup
    // also refreshes derived search/tag metadata. Retain those deltas rather
    // than misreporting them as unsupported Collection query versions.
    let go_reopen_snapshot_equal = go_after_native == post_native;
    let mut changed_tables = Vec::new();
    for (name, rows) in post_native["tables"].as_object().ok_or("native tables")? {
        if rows != &go_after_native["tables"][name] {
            changed_tables.push(name.clone());
            assert!(matches!(
                name.as_str(),
                "stories" | "tag_definitions" | "tag_values"
            ));
        }
    }
    assert_eq!(post_native["schema"], go_after_native["schema"]);
    for table in ["tag_definitions", "tag_values"] {
        let before = post_native["tables"][table]
            .as_array()
            .ok_or("native tags")?;
        let after = go_after_native["tables"][table]
            .as_array()
            .ok_or("Go tags")?;
        assert!(
            before.iter().all(|row| after.contains(row)),
            "Go lost existing {table}"
        );
    }
    let native_stories = post_native["tables"]["stories"]
        .as_array()
        .ok_or("native stories")?;
    let go_stories = go_after_native["tables"]["stories"]
        .as_array()
        .ok_or("Go stories")?;
    assert_eq!(native_stories.len(), go_stories.len());
    for (before, after) in native_stories.iter().zip(go_stories) {
        for (column, value) in before.as_object().ok_or("story")? {
            if column != "display_name_normalized" {
                assert_eq!(value, &after[column], "Go changed story column {column}");
            }
        }
    }
    let go_reopen_observations = json!({
        "opened_and_closed": true,
        "changed_tables": changed_tables,
        "collections_shelves_memberships_and_outcomes_equal": true,
        "story_fields_except_derived_search_name_equal": true,
        "existing_tags_preserved": true,
        "scope": "disposable compatibility copy only; startup projection deltas remain in full snapshots"
    });
    let native_collection = post_native["tables"]["collections"]
        .as_array()
        .ok_or("collections")?
        .iter()
        .find(|r| r["name"] == "Native addition")
        .ok_or("native addition")?;
    let go_collection = go_after_native["tables"]["collections"]
        .as_array()
        .ok_or("Go collections")?
        .iter()
        .find(|r| r["id"] == native_collection["id"])
        .ok_or("Go retained native addition")?;
    assert_eq!(
        go_collection["query_payload"],
        native_collection["query_payload"]
    );
    assert_eq!(
        go_collection["query_version"],
        native_collection["query_version"]
    );
    drop(connection);
    assert!(
        files(&target)? == post_files,
        "Go compatibility reopening changed active candidate"
    );
    assert!(files(store.directory())? == native_profile);
    // The supported rollback always gets an independent closed pre-adoption
    // snapshot. New native edits remain in the retained candidate, for explicit
    // export/re-import or manual reconciliation; never overwrite with backup.
    let rollback = roots.library().join("rollback clone");
    let input = ClosedLibraryInput::verify(
        &source,
        ProfileDocument::decode(&serde_json::to_vec(&profile(&source)?)?)?,
        ClosedManifest::decode(&manifest)?,
        &Default::default(),
    )?;
    let rollback_copy = input.copy_to_new_root(&rollback, &Default::default())?;
    drop(rollback_copy);
    let rollback_profile = roots.profile().join("rollback profile");
    fs::create_dir(&rollback_profile)?;
    fs::write(
        rollback_profile.join("settings.json"),
        serde_json::to_vec(&profile(&rollback)?)?,
    )?;
    assert_eq!(
        probe(bridge, "open", &rollback_profile, &rollback)?["closed"],
        true
    );
    assert_eq!(
        files(&target)?,
        post_files,
        "rollback changed newer candidate data"
    );
    assert_eq!(
        files(store.directory())?,
        native_profile,
        "rollback changed native profile"
    );
    for (path, bytes) in &source_before {
        if path != Path::new("db/librairii.sqlite3") && path != Path::new(".librairii-library.lock")
        {
            assert_eq!(&fs::read(target.join(path))?, bytes);
        }
    }
    assert_eq!(files(&source)?, source_before);
    assert_eq!(files(legacy)?, legacy_before);
    assert_eq!(files(&backup)?, source_before);
    Ok(
        json!({"prefix":prefix,"current_bridge":prefix==19,"configured_legacy_profile":configured,"source_manifest":source_manifest,"backup_manifest":backup_manifest,"closed_tree_manifest":serde_json::from_slice::<Value>(&manifest)?,"legacy_profile_backup":seal(&profile_backup)?,"upgraded_database":upgraded,"post_native_database":post_native,"go_after_native_on_disposable_copy":go_after_native,"go_reopen_snapshot_equal":go_reopen_snapshot_equal,"go_reopen_observations":go_reopen_observations,"bridge_refuses_native_owned_root":true,"rollback_policy":"independent pre-native snapshot; retain newer candidate and explicitly reconcile edits","candidate_manifest":seal(&target)?,"native_profile_manifest":seal(store.directory())?,"rollback_clone":seal(&rollback)?,"source_and_legacy_unchanged":true,"independent_inodes":true,"restart_all_tables_equal":true,"preferences_retained":true,"in_place_refused":true,"newer_candidate_preserved_during_rollback":true}),
    )
}
fn main() -> Result {
    let bridge = fs::canonicalize(
        std::env::args_os()
            .nth(1)
            .ok_or("explicit bridge application helper")?,
    )?;
    let mut cases = Vec::new();
    for prefix in 1..=19 {
        for configured in [false, true] {
            cases.push(run_case(prefix, configured, &bridge)?);
        }
    }
    println!(
        "{}",
        json!({"format":"librairii.upgrade-handover","version":1,"cases":cases})
    );
    Ok(())
}
