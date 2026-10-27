//! Production view callbacks backed by an explicitly owned synthetic library.
use super::{Result, check, view};
use librairii_app::localization::Language;
use librairii_core::navigation::Navigation;
use librairii_desktop::{
    presentation::{Phase, Snapshot, Worker},
    startup::StartupEnvironment,
};
use librairii_storage::{
    native::NativeLibrary,
    profile::{NativeProfileStore, ProfileLocations},
    settings::TargetPreview,
};
use librairii_testkit::TestRoots;
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    ffi::OsString,
    rc::Rc,
    time::{Duration, Instant},
};

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

struct Fixture {
    ui: slint::Weak<DesktopWindow>,
    worker: Rc<RefCell<Worker>>,
    view: Rc<RefCell<view::View>>,
    locations: ProfileLocations,
    id: librairii_core::LibraryId,
    _roots: TestRoots,
}
impl Fixture {
    fn open(ui: &DesktopWindow) -> Result<Self> {
        let roots = TestRoots::new()?;
        let home = roots.profile().join("home");
        let config = roots.profile().join("config");
        let data = roots.library().join("data");
        let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
        std::fs::create_dir_all(locations.native.parent().ok_or("profile parent missing")?)?;
        std::fs::create_dir_all(&locations.data)?;
        let store = NativeProfileStore::create_fresh(&locations.native)?;
        let native = NativeLibrary::initialize_confirmed(
            TargetPreview::inspect(&roots.library().join("native"), &[], &Default::default())?,
            &store,
            &locations,
            "e0000000-0000-4000-8000-000000000014".parse()?,
            Language::English,
            &Default::default(),
        )
        .map_err(|error| error.to_string())?;
        let id = native.identity().typed();
        let mut database = librairii_storage::database::Database::open_native(&native)?;
        let seeded = database
            .transaction(Default::default(), |tx| {
                for id in 1..=60 {
                    let title = if id <= 30 { "Alpha" } else { "Beta" };
                    tx.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(?1,printf('10000000-0000-4000-8000-%012d',?1),?2,lower(?2))", (id, title))?;
                    tx.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(?1,'story.zip','zip',printf('%064d',?1),1,'archives/'||?1,'valid')", [id])?;
                }
                Ok(())
            })?
            .wait()?;
        check(
            matches!(
                seeded,
                librairii_storage::database::TransactionOutcome::Committed(())
            ),
            "native callback fixture seeding did not commit",
        )?;
        database.close()?;
        drop(database);
        native.finish().map_err(|error| error.to_string())?;
        drop(store);
        let worker = Rc::new(RefCell::new(Worker::start_with(Environment {
            home: home.into_os_string(),
            config: config.into_os_string(),
            data: data.into_os_string(),
        })?));
        let initial = receive(&worker.borrow())?;
        let view = view::install(ui, worker.clone(), initial)?;
        Ok(Self {
            ui: ui.as_weak(),
            worker,
            view,
            locations,
            id,
            _roots: roots,
        })
    }
    fn settle(&self, ui: &DesktopWindow) -> Result<Snapshot> {
        let snapshot = receive(&self.worker.borrow())?;
        check(
            self.view
                .borrow_mut()
                .accept(ui, snapshot.clone(), self.worker.borrow().revision()),
            "current native callback response was rejected",
        )?;
        Ok(snapshot)
    }
    fn search(&self, ui: &DesktopWindow, text: &str) -> Result<Snapshot> {
        ui.set_query(text.into());
        ui.invoke_search(text.into());
        self.settle(ui)
    }
    fn saved_navigation(&self) -> Result<Navigation> {
        Ok(Navigation::decode(
            &std::fs::read(self.locations.native.join("navigation.json"))?,
            self.id,
            1,
        )?)
    }
    fn finish(self) -> Result {
        let closed = self.worker.borrow_mut().finish();
        closed.map_err(Into::into)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(ui) = self.ui.upgrade() {
            ui.on_mode_selected(|_| {});
            ui.on_select_story(|_| {});
            ui.on_change_page(|_| {});
            ui.on_navigate_group(|_| {});
            ui.on_history_step(|_| {});
            ui.on_request_panel(|_| {});
            ui.on_change_language(|_| {});
            ui.on_search(|_| {});
            ui.set_mode(0);
            ui.set_query("".into());
            ui.set_back_enabled(false);
            ui.set_forward_enabled(false);
        }
        let _closed = self.worker.borrow_mut().finish();
    }
}
fn receive(worker: &Worker) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(snapshot) = worker.take_snapshot()
            && snapshot.phase != Phase::Opening
        {
            return Ok(snapshot);
        }
        if worker.finished() || Instant::now() >= deadline {
            return Err("native callback fixture did not receive a response".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub fn paging(ui: &DesktopWindow) -> Result {
    let fixture = Fixture::open(ui)?;
    fixture.search(ui, "Alpha")?;
    check(ui.get_next_enabled(), "paging fixture needs a second page")?;
    ui.set_query("Beta".into());
    ui.invoke_search("Beta".into());
    // Deliberately keep the displayed Alpha snapshot until both callbacks run.
    ui.invoke_change_page(1);
    let loaded = fixture.settle(ui)?;
    let navigation = loaded.navigation.ok_or("paging navigation missing")?;
    check(
        ui.get_query() == "Beta"
            && navigation.current().location.request.criteria.name == "Beta"
            && loaded.page.as_ref().is_some_and(|page| {
                page.page == 2 && page.stories.iter().all(|story| story.title == "Beta")
            }),
        "paging restored a superseded search",
    )?;
    let saved = fixture.saved_navigation()?;
    check(
        saved.current().location.request.criteria.name == "Beta"
            && saved.current().location.request.page == 2,
        "paging persisted superseded criteria",
    )?;
    ui.invoke_change_page(-1);
    ui.invoke_change_page(1);
    let repeated = fixture.settle(ui)?;
    check(
        repeated.page.as_ref().is_some_and(|page| page.page == 2),
        "queued page changes lost their order",
    )?;
    fixture.finish()
}
