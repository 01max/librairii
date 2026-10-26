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
#[test]
fn real_native_library_restores_and_persists_each_mode_before_joined_shutdown() -> TestResult {
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
    database.close()?;
    drop(database);
    native.finish().map_err(|e| e.to_string())?;
    drop(store);
    let mut worker = Worker::start_with(Environment {
        home: home.into_os_string(),
        config: config.into_os_string(),
        data: data.into_os_string(),
    })?;
    let initial = receive(&worker)?;
    assert_eq!(initial.language, Language::French);
    assert_eq!(initial.page.ok_or("page")?.total, 0);
    for mode in [
        Presentation::SmallCovers,
        Presentation::LargeCovers,
        Presentation::List,
    ] {
        worker.commands.try_send(Command::Presentation(mode))?;
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
