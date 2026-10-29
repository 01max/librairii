#![cfg(any(target_os = "macos", target_os = "linux"))]
use librairii_app::localization::Language;
use librairii_core::navigation::Presentation;
use librairii_desktop::{
    presentation::{Command, Snapshot, Worker},
    startup::StartupEnvironment,
};
use librairii_storage::{
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
};
use librairii_testkit::TestRoots;
use std::{
    ffi::OsString,
    fs,
    time::{Duration, Instant},
};
type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Environment {
    home: OsString,
    config: OsString,
    data: OsString,
}
impl StartupEnvironment for Environment {
    fn variable(&self, name: &str) -> Option<OsString> {
        match name {
            "HOME" => Some(self.home.clone()),
            "XDG_CONFIG_HOME" => Some(self.config.clone()),
            "XDG_DATA_HOME" => Some(self.data.clone()),
            _ => None,
        }
    }
}
fn receive(worker: &Worker) -> Result<Snapshot, Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(snapshot) = worker.take_snapshot()
            && snapshot.phase != librairii_desktop::presentation::Phase::Opening
        {
            return Ok(snapshot);
        }
        if worker.finished() || Instant::now() > deadline {
            return Err("native worker did not publish a snapshot".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn isolated_export_preparation_retains_usable_library_after_empty_and_refused_previews()
-> TestResult {
    use librairii_desktop::{presentation::Phase, workflows::exports::Action};
    struct OverrideEnvironment(OsString);
    impl StartupEnvironment for OverrideEnvironment {
        fn variable(&self, name: &str) -> Option<OsString> {
            assert_eq!(name, "LIBRAIRII_DATA_ROOT");
            Some(self.0.clone())
        }
    }
    let roots = TestRoots::new()?;
    let destination = roots.library().join("export destination");
    fs::create_dir(&destination)?;
    let mut worker = Worker::start_with(OverrideEnvironment(
        roots.library().join("isolated library").into_os_string(),
    ))?;
    let initial = receive(&worker)?;
    assert_eq!(initial.phase, Phase::Library);
    let generation = initial.generation.ok_or("generation")?;
    worker.offer(Command::Exports {
        generation: generation.clone(),
        action: Action::Open(Vec::new()),
    })?;
    receive(&worker)?;
    for path in [
        destination.clone(),
        roots.library().join("missing destination"),
        destination,
    ] {
        let valid = path.is_dir();
        worker.offer(Command::Exports {
            generation: generation.clone(),
            action: Action::Destination(path),
        })?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let settled = loop {
            let snapshot = receive(&worker)?;
            if snapshot.phase == Phase::Recovery || !snapshot.workflows.exports.preparing {
                break snapshot;
            }
            if Instant::now() >= deadline {
                return Err("export preparation did not settle".into());
            }
        };
        if settled.phase != Phase::Library {
            worker.finish()?;
            return Err(format!(
                "healthy isolated export forced recovery: {:?}",
                settled.workflows.exports.error
            )
            .into());
        }
        let exports = &settled.workflows.exports;
        assert!(!exports.recovery_required);
        if valid {
            assert!(exports.error.is_none());
            let preview = exports.preview.as_ref().ok_or("empty preview missing")?;
            assert_eq!(preview.ready, 0);
            assert_eq!(
                preview.issue,
                Some(librairii_core::export::Issue::EmptyScope)
            );
        } else {
            assert!(exports.error.is_some());
            assert!(exports.preview.is_none());
        }
    }
    worker.offer(Command::Language(Language::French))?;
    let after = receive(&worker)?;
    worker.finish()?;
    assert_eq!(after.phase, Phase::Library);
    assert_eq!(after.language, Language::French);
    assert_eq!(fs::read_dir(roots.profile())?.count(), 0);
    Ok(())
}
fn library_worker() -> Result<
    (
        TestRoots,
        ProfileLocations,
        librairii_core::LibraryId,
        Worker,
    ),
    Box<dyn std::error::Error>,
> {
    let roots = TestRoots::new()?;
    let home = roots.profile().join("home");
    let config = roots.profile().join("config");
    let data = roots.library().join("data");
    let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
    fs::create_dir_all(locations.native.parent().ok_or("parent")?)?;
    fs::create_dir_all(&locations.data)?;
    let store = NativeProfileStore::create_fresh(&locations.native)?;
    let native = NativeLibrary::initialize_confirmed(
        TargetPreview::inspect(&roots.library().join("native"), &[], &Default::default())?,
        &store,
        &locations,
        "e0000000-0000-4000-8000-000000000014".parse()?,
        Language::French,
        &Default::default(),
    )
    .map_err(|e| e.to_string())?;
    let id = native.identity().typed();
    let mut database = librairii_storage::database::Database::open_native(&native)?;
    let created = database
        .edit_collections(
            librairii_storage::collections::Command::Create {
                name: "My collection".into(),
                criteria: {
                    let mut query = librairii_core::collection_codec::Query::default();
                    query.name = "saved membership".into();
                    query
                },
            },
            Default::default(),
        )?
        .wait()?;
    assert!(matches!(
        created,
        librairii_storage::database::TransactionOutcome::Committed(_)
    ));
    let created = database
        .edit_shelves(
            librairii_storage::shelves::Command::Create {
                name: "My shelf".into(),
            },
            Default::default(),
        )?
        .wait()?;
    assert!(matches!(
        created,
        librairii_storage::database::TransactionOutcome::Committed(_)
    ));
    database.close()?;
    drop(database);
    native.finish().map_err(|e| e.to_string())?;
    drop(store);
    let worker = Worker::start_with(Environment {
        home: home.into_os_string(),
        config: config.into_os_string(),
        data: data.into_os_string(),
    })?;
    Ok((roots, locations, id, worker))
}
#[test]
fn preference_transfer_runs_on_the_native_worker_and_never_replays_or_accepts_stale_commands()
-> TestResult {
    use librairii_desktop::maintenance::{Action, Notice};
    let (roots, locations, id, mut worker) = library_worker()?;
    let initial = receive(&worker)?;
    let generation = initial.generation.ok_or("generation")?;
    let mut document: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../librairii-core/tests/fixtures/handover-v1.json"
    ))?;
    document["libraryId"] = id.to_string().into();
    document["state"]["frames"] =
        serde_json::json!([{"hash":"#/library?v=3&page=1","kind":"library"}]);
    document["state"]["position"] = 0.into();
    let selected = roots.profile().join("selected.json");
    fs::write(&selected, serde_json::to_vec(&document)?)?;
    worker.offer(Command::Maintenance {
        generation: None,
        action: Action::ImportPreferences(selected.clone()),
    })?;
    assert_eq!(
        receive(&worker)?.workflows.settings.notice,
        Some(Notice::PreferencesRejected)
    );
    assert!(!locations.native.join("handover.json").exists());
    worker.offer(Command::Maintenance {
        generation: Some(generation.clone()),
        action: Action::ImportPreferences(selected.clone()),
    })?;
    let imported = receive(&worker)?;
    assert_eq!(
        imported.workflows.settings.notice,
        Some(Notice::PreferencesImported)
    );
    assert_eq!(
        imported.navigation.ok_or("navigation")?.presentation(),
        Presentation::SmallCovers
    );
    assert!(imported.workflows.operations.sync_hidden);
    let receipt = fs::read(locations.native.join("handover.json"))?;
    worker.offer(Command::Presentation(Presentation::LargeCovers))?;
    receive(&worker)?;
    worker.offer(Command::Maintenance {
        generation: Some(generation),
        action: Action::ImportPreferences(selected.clone()),
    })?;
    let repeated = receive(&worker)?;
    assert_eq!(
        repeated.workflows.settings.notice,
        Some(Notice::PreferencesAlreadyImported)
    );
    assert_eq!(
        repeated.navigation.ok_or("navigation")?.presentation(),
        Presentation::LargeCovers
    );
    assert_eq!(fs::read(locations.native.join("handover.json"))?, receipt);
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    assert_eq!(
        store
            .load_navigation(id, 2)?
            .ok_or("navigation")?
            .presentation(),
        Presentation::LargeCovers
    );
    assert!(
        store
            .load_interface(id)?
            .ok_or("interface")?
            .sync_panel_hidden
    );
    Ok(())
}

#[test]
fn real_native_library_restores_and_persists_each_mode_before_joined_shutdown() -> TestResult {
    let (_roots, locations, id, mut worker) = library_worker()?;
    let initial = receive(&worker)?;
    assert_eq!(initial.language, Language::French);
    assert_eq!(initial.page.ok_or("page")?.total, 0);
    for mode in [
        Presentation::SmallCovers,
        Presentation::LargeCovers,
        Presentation::List,
    ] {
        worker.offer(Command::Presentation(mode))?;
        assert_eq!(
            receive(&worker)?
                .navigation
                .ok_or("navigation")?
                .presentation(),
            mode
        );
        let saved = fs::read(locations.native.join("navigation.json"))?;
        assert_eq!(
            librairii_core::navigation::Navigation::decode(&saved, id, 2)?.presentation(),
            mode
        );
    }
    worker.request_shutdown();
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    let reopened = NativeLibrary::reopen(&store, &locations, &Default::default())?;
    let mut database = librairii_storage::database::Database::open_native(&reopened)?;
    database.close()?;
    drop(database);
    reopened.finish().map_err(|e| e.to_string())?;
    Ok(())
}

#[test]
fn native_history_reopens_typed_groups_and_discards_superseded_search_snapshots() -> TestResult {
    use librairii_core::{
        collection_codec::url::Scope, navigation::Navigation, query::QueryRequest,
    };
    let (_roots, locations, id, mut worker) = library_worker()?;
    let initial = receive(&worker)?;
    assert_eq!(initial.groups.len(), 2);
    let mut request = QueryRequest::default();
    request.criteria.name = "Étoiles".into();
    worker.offer(Command::Request(request.clone()))?;
    let first = receive(&worker)?.navigation.ok_or("first")?;
    request.criteria.name = "Forêt".into();
    worker.offer(Command::Request(request))?;
    receive(&worker)?;
    worker.offer(Command::Back)?;
    let back = receive(&worker)?.navigation.ok_or("back")?;
    assert_eq!(back.current(), first.current());
    assert!(back.can_forward());
    worker.offer(Command::Presentation(Presentation::SmallCovers))?;
    receive(&worker)?;
    worker.offer(Command::Forward)?;
    let forward = receive(&worker)?.navigation.ok_or("forward")?;
    assert_eq!(forward.current().location.request.criteria.name, "Forêt");
    assert_eq!(forward.presentation(), Presentation::SmallCovers);
    let collection = Scope::Collection(librairii_core::CollectionId::new(1)?);
    let shelf = Scope::Shelf {
        id: librairii_core::ShelfId::new(1)?,
        manual_order: true,
    };
    for scope in [collection, shelf.clone()] {
        worker.offer(Command::Navigate(scope.clone()))?;
        let opened = receive(&worker)?.navigation.ok_or("scope")?;
        assert_eq!(opened.current().location.scope, scope);
        if matches!(scope, Scope::Collection(_)) {
            assert_eq!(
                opened.current().location.request.criteria.name,
                "saved membership"
            );
            let mut edited = opened.current().location.request.clone();
            edited.criteria.name = "Edited membership".into();
            worker.offer(Command::Request(edited))?;
            let edited = receive(&worker)?.navigation.ok_or("edited Collection")?;
            assert_eq!(edited.current().location.scope, scope);
            assert_eq!(
                edited.current().location.request.criteria.name,
                "Edited membership"
            );
            worker.offer(Command::Back)?;
            assert_eq!(
                receive(&worker)?
                    .navigation
                    .ok_or("saved membership")?
                    .current()
                    .location
                    .request
                    .criteria
                    .name,
                "saved membership"
            );
        }
    }
    for index in 0..100 {
        let mut request = QueryRequest::default();
        request.criteria.name = format!("query {index}");
        worker.offer(Command::Request(request))?;
    }
    let latest = receive(&worker)?;
    assert_eq!(latest.revision, worker.revision());
    assert_eq!(
        latest
            .navigation
            .ok_or("latest")?
            .current()
            .location
            .request
            .criteria
            .name,
        "query 99"
    );
    worker.finish()?;
    let saved = Navigation::decode(&fs::read(locations.native.join("navigation.json"))?, id, 5)?;
    assert_eq!(saved.current().location.scope, shelf);
    assert_eq!(saved.current().location.request.criteria.name, "query 99");
    assert_eq!(saved.presentation(), Presentation::SmallCovers);
    Ok(())
}

#[test]
fn corrupt_navigation_reaches_recovery_without_touching_library_and_language_remains_independent()
-> TestResult {
    use librairii_desktop::presentation::Phase;
    let (roots, locations, _id, mut worker) = library_worker()?;
    receive(&worker)?;
    worker.finish()?;
    let profile = fs::read(locations.native.join("settings.json"))?;
    let database = roots.library().join("native/db/librairii.sqlite3");
    let original = fs::read(&database)?;
    let navigation = locations.native.join("navigation.json");
    fs::write(&navigation, b"future or corrupt native navigation")?;
    let mut worker = Worker::start_with(Environment {
        home: roots.profile().join("home").into_os_string(),
        config: roots.profile().join("config").into_os_string(),
        data: roots.library().join("data").into_os_string(),
    })?;
    let recovery = receive(&worker)?;
    assert_eq!(recovery.phase, Phase::Recovery);
    assert!(recovery.navigation.is_none() && recovery.page.is_none());
    assert_eq!(
        fs::read(&navigation)?,
        b"future or corrupt native navigation"
    );
    assert_eq!(fs::read(locations.native.join("settings.json"))?, profile);
    assert_eq!(fs::read(&database)?, original);
    assert!(worker.offer(Command::Request(Default::default())).is_err());
    worker.offer(Command::Language(Language::English))?;
    assert_eq!(receive(&worker)?.language, Language::English);
    worker.finish()?;
    assert_eq!(
        fs::read(&navigation)?,
        b"future or corrupt native navigation"
    );
    assert_eq!(fs::read(&database)?, original);
    let store = NativeProfileStore::open_native(&locations.native)?;
    assert_eq!(store.load()?.ok_or("profile")?.profile().language, "en");
    Ok(())
}
#[test]
fn first_run_inspection_is_readonly_and_explicit_language_never_initializes_a_library() -> TestResult
{
    use librairii_desktop::presentation::Phase;
    let roots = TestRoots::new()?;
    let home = roots.profile().join("new-home");
    let config = roots.profile().join("new-config");
    let data = roots.library().join("new-data");
    let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
    let mut worker = Worker::start_with(Environment {
        home: home.into_os_string(),
        config: config.into_os_string(),
        data: data.into_os_string(),
    })?;
    assert_eq!(receive(&worker)?.phase, Phase::Setup);
    assert!(!locations.native.exists());
    assert!(!locations.data.exists());
    worker.offer(Command::Language(Language::French))?;
    let snapshot = receive(&worker)?;
    assert_eq!(snapshot.phase, Phase::Setup);
    assert_eq!(snapshot.language, Language::French);
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    let saved = store.load()?.ok_or("saved")?;
    assert_eq!(saved.profile().language, "fr");
    assert!(!saved.profile().complete);
    assert!(saved.profile().root.is_empty());
    assert!(!locations.data.join("library").exists());
    Ok(())
}

#[test]
fn saved_shelf_warning_preferences_are_recognized_as_native_state() -> TestResult {
    let (roots, locations, id, mut worker) = library_worker()?;
    receive(&worker)?;
    worker.offer(Command::Presentation(Presentation::SmallCovers))?;
    receive(&worker)?;
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    store.save_shelf_warnings(&librairii_core::shelves::WarningAcknowledgements::new(id))?;
    drop(store);
    let mut worker = Worker::start_with(Environment {
        home: roots.profile().join("home").into_os_string(),
        config: roots.profile().join("config").into_os_string(),
        data: roots.library().join("data").into_os_string(),
    })?;
    let snapshot = receive(&worker)?;
    assert_eq!(
        snapshot.phase,
        librairii_desktop::presentation::Phase::Library
    );
    assert!(snapshot.page.is_some());
    worker.finish()?;
    Ok(())
}

#[test]
fn active_preference_failure_retains_actual_session_until_safe_shutdown() -> TestResult {
    let (_roots, locations, _id, mut worker) = library_worker()?;
    receive(&worker)?;
    worker.offer(Command::Presentation(Presentation::SmallCovers))?;
    receive(&worker)?;
    let navigation = locations.native.join("navigation.json");
    fs::write(&navigation, b"unconfirmed external preference bytes")?;
    worker.offer(Command::Presentation(Presentation::LargeCovers))?;
    assert_eq!(
        receive(&worker)?.phase,
        librairii_desktop::presentation::Phase::Recovery
    );
    assert!(matches!(
        NativeProfileStore::open_native(&locations.native),
        Err(librairii_storage::StorageError::ProfileInUse)
    ));
    assert!(worker.offer(Command::Request(Default::default())).is_err());
    assert_eq!(
        fs::read(&navigation)?,
        b"unconfirmed external preference bytes"
    );
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    assert!(store.load()?.ok_or("profile")?.profile().complete);
    assert_eq!(
        fs::read(&navigation)?,
        b"unconfirmed external preference bytes"
    );
    Ok(())
}

#[test]
fn manual_shelf_navigation_retains_sort_preference_across_history_and_reopen() -> TestResult {
    use librairii_core::{
        collection_codec::url::{Location, Scope},
        navigation::Navigation,
        query::QueryRequest,
    };
    let (roots, locations, id, mut worker) = library_worker()?;
    receive(&worker)?;
    worker.finish()?;
    let store = NativeProfileStore::open_native(&locations.native)?;
    let native = NativeLibrary::reopen(&store, &locations, &Default::default())?;
    let mut database = librairii_storage::database::Database::open_native(&native)?;
    let seeded = database.transaction(Default::default(), |tx| {
        for (id, title) in [(1, "Zebra"), (2, "Alpha")] {
            tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),?2,lower(?2))", (id, title))?;
            tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')", [id])?;
            tx.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(1,printf('10000000-0000-4000-8000-%012d',?1),?1-1,?2)", (id, title))?;
        }
        Ok(())
    })?.wait()?;
    assert!(matches!(
        seeded,
        librairii_storage::database::TransactionOutcome::Committed(())
    ));
    database.close()?;
    drop(database);
    native.finish().map_err(|e| e.to_string())?;
    let shelf = librairii_core::ShelfId::new(1)?;
    let request = QueryRequest {
        sort: "name_asc".into(),
        ..Default::default()
    };
    store.save_navigation(&Navigation::new(
        id,
        1,
        Location {
            scope: Scope::Shelf {
                id: shelf,
                manual_order: true,
            },
            request: request.clone(),
        },
    )?)?;
    drop(store);
    let mut worker = Worker::start_with(Environment {
        home: roots.profile().join("home").into_os_string(),
        config: roots.profile().join("config").into_os_string(),
        data: roots.library().join("data").into_os_string(),
    })?;
    let manual = receive(&worker)?;
    assert_eq!(
        manual
            .page
            .ok_or("manual page")?
            .stories
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        ["Zebra", "Alpha"]
    );
    assert_eq!(
        manual
            .navigation
            .ok_or("manual navigation")?
            .current()
            .location
            .request
            .sort,
        "name_asc"
    );
    worker.offer(Command::Navigate(Scope::Shelf {
        id: shelf,
        manual_order: false,
    }))?;
    receive(&worker)?;
    worker.offer(Command::Request(request))?;
    assert_eq!(
        receive(&worker)?
            .page
            .ok_or("sorted page")?
            .stories
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        ["Alpha", "Zebra"]
    );
    worker.offer(Command::Back)?;
    receive(&worker)?;
    worker.offer(Command::Back)?;
    let restored = receive(&worker)?;
    assert_eq!(
        restored.page.ok_or("restored page")?.stories[0].title,
        "Zebra"
    );
    assert_eq!(
        restored
            .navigation
            .ok_or("restored sort")?
            .current()
            .location
            .request
            .sort,
        "name_asc"
    );
    worker.finish()?;
    Ok(())
}
