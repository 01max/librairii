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
        if let Some(snapshot) = worker.take_snapshot() {
            return Ok(snapshot);
        }
        if worker.finished() || Instant::now() > deadline {
            return Err("native worker did not publish a snapshot".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
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
                criteria: Default::default(),
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
        assert_eq!(
            receive(&worker)?
                .navigation
                .ok_or("scope")?
                .current()
                .location
                .scope,
            scope
        );
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
