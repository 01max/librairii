//! Production view callbacks backed by an explicitly owned synthetic library.
use super::{Result, check, view};
use librairii_app::localization::Language;
use librairii_core::navigation::Navigation;
use librairii_desktop::{
    presentation::{Command, Phase, Snapshot, Worker},
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
                tx.execute_batch("INSERT INTO tag_definitions(id,key,normalized_key,label,color,kind,source,presentation,position,is_protected) VALUES(101,'bedtime','bedtime','Bedtime','#334455','boolean','user','default',0,0),(102,'theme','theme','Theme','#334455','choice','user','default',1,0); INSERT INTO tag_values(id,definition_id,key,normalized_key,label,position) VALUES(201,102,'forest','forest','Forest',0),(202,102,'ocean','ocean','Ocean',1); INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) SELECT id,101,NULL,'manual' FROM stories WHERE id<=30; INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) SELECT id,102,CASE WHEN id<=30 THEN 201 ELSE 202 END,'manual' FROM stories;")?;
                tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(1,'Reverse','reverse',0); INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) SELECT 1,uuid,60-id,embedded_title FROM stories;")?;
                Ok(())
            })?
            .wait()?;
        check(
            matches!(
                seeded,
                librairii_storage::database::TransactionOutcome::Committed(())
            ),
            &format!("native callback fixture seeding did not commit: {seeded:?}"),
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
    fn select(&self, ui: &DesktopWindow, index: i32) -> Result {
        ui.set_selected_index(index);
        ui.invoke_select_story(index);
        self.settle(ui)?;
        Ok(())
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
            ui.on_filter_search(|_| {});
            ui.on_filter_collapse(|_| {});
            ui.on_filter_edit(|_, _| {});
            ui.on_filter_remove(|_| {});
            ui.on_filter_clear(|| {});
            ui.on_filter_clear_tags(|| {});
            ui.on_sort_selected(|_| {});
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

/// Actual production filter callbacks, SQL results and durable navigation.
pub fn filters(ui: &DesktopWindow, output: Option<&std::path::Path>) -> Result {
    use slint::Model;
    let fixture = Fixture::open(ui)?;
    check(
        ui.get_sort_index() == 1,
        "fresh default sort does not match name ordering",
    )?;
    fixture.search(ui, "Alpha ")?;
    ui.invoke_open_dialog(8);
    ui.invoke_sort_selected(1);
    fixture.settle(ui)?;
    let option = |label: &str| -> Result<i32> {
        let rows = ui.get_filter_rows();
        (0..rows.row_count())
            .filter_map(|i| rows.row_data(i))
            .find(|r| !r.heading && r.label.contains(label))
            .map(|r| r.index)
            .ok_or_else(|| format!("missing native filter {label}").into())
    };
    ui.invoke_filter_edit(option("Bedtime")?, 1);
    let tagged = fixture.settle(ui)?;
    let group = ui.get_filter_rows();
    let heading = (0..group.row_count())
        .filter_map(|i| group.row_data(i))
        .find(|r| r.heading && r.label == "Bedtime")
        .ok_or("tag heading missing")?;
    let saved = fixture.saved_navigation()?;
    ui.invoke_filter_collapse(heading.index);
    check(
        fixture.saved_navigation()?.current() == saved.current(),
        "collapsing a group changed the query",
    )?;
    check(
        !(0..ui.get_filter_rows().row_count())
            .filter_map(|i| ui.get_filter_rows().row_data(i))
            .any(|r| !r.heading && r.label == "Bedtime"),
        "collapsed tag remains exposed",
    )?;
    ui.invoke_filter_collapse(heading.index);
    check(
        (0..ui.get_filter_rows().row_count())
            .filter_map(|i| ui.get_filter_rows().row_data(i))
            .any(|r| !r.heading && r.label == "Bedtime" && r.state == 1),
        "expanded tag lost its selection",
    )?;
    check(
        tagged.page.as_ref().is_some_and(|p| p.total == 30),
        "boolean filter lost its full SQL scope",
    )?;
    ui.invoke_filter_edit(option("Forest")?, 1);
    fixture.settle(ui)?;
    ui.invoke_filter_edit(option("Ocean")?, 1);
    let combined = fixture.settle(ui)?;
    check(
        combined.page.as_ref().is_some_and(|p| p.total == 30),
        "choice values did not retain OR within AND groups",
    )?;
    check(
        ui.get_query() == "Alpha ",
        "filter edit trimmed the literal search editor",
    )?;
    if let Some(output) = output {
        super::capture(ui, &output.join("filters-compact-en.png"))?;
    }
    ui.invoke_close_dialog();
    let before = fixture.saved_navigation()?;
    check(
        before.current().location.request.sort == "name_asc",
        "filter editing changed explicit sort",
    )?;
    ui.invoke_open_dialog(8);
    check(
        fixture.saved_navigation()?.current() == before.current(),
        "collapsing filters changed criteria",
    )?;
    ui.invoke_filter_clear_tags();
    let cleared = fixture.settle(ui)?;
    check(
        cleared.navigation.as_ref().is_some_and(|n| {
            n.current()
                .location
                .request
                .criteria
                .boolean_filters
                .is_empty()
                && n.current()
                    .location
                    .request
                    .criteria
                    .choice_filters
                    .is_empty()
        }),
        "clear tag filters retained a predicate",
    )?;
    ui.invoke_filter_clear();
    let all = fixture.settle(ui)?;
    check(
        !ui.get_modal() && ui.get_query().is_empty() && ui.get_query_focused(),
        "clear all did not restore search focus",
    )?;
    check(
        all.page.as_ref().is_some_and(|p| p.total == 60)
            && all
                .navigation
                .as_ref()
                .is_some_and(|n| n.current().location.request.sort == "name_asc"),
        "clear all lost sort or broadened a partial result",
    )?;
    ui.invoke_change_language("fr".into());
    fixture.settle(ui)?;
    ui.invoke_open_dialog(8);
    check(
        ui.get_filter_label().starts_with("Filtres") && option("Bedtime").is_ok(),
        "French changed an authored label or left filters untranslated",
    )?;
    if let Some(output) = output {
        super::capture(ui, &output.join("filters-compact-fr.png"))?;
    }
    ui.invoke_close_dialog();
    let scope = fixture
        .view
        .borrow()
        .snapshot
        .groups
        .iter()
        .find(|g| g.name == "Reverse")
        .ok_or("manual shelf missing")?
        .scope
        .clone();
    fixture.worker.borrow().offer(Command::Navigate(scope))?;
    let manual = fixture.settle(ui)?;
    check(
        ui.get_sort_index() == 2
            && manual
                .page
                .as_ref()
                .is_some_and(|p| p.stories[0].title == "Beta"),
        "manual Shelf sort is misrepresented",
    )?;
    ui.invoke_sort_selected(1);
    let sorted = fixture.settle(ui)?;
    check(
        sorted
            .page
            .as_ref()
            .is_some_and(|p| p.stories[0].title == "Alpha")
            && ui.get_sort_index() == 1,
        "Shelf sort did not enter view order",
    )?;
    fixture.worker.borrow().offer(Command::Back)?;
    fixture.settle(ui)?;
    check(ui.get_sort_index() == 2, "Back lost Shelf manual order")?;
    fixture.worker.borrow().offer(Command::Forward)?;
    fixture.settle(ui)?;
    let restored = fixture.saved_navigation()?;
    check(
        matches!(
            restored.current().location.scope,
            librairii_core::collection_codec::url::Scope::Shelf {
                manual_order: false,
                ..
            }
        ) && ui.get_sort_index() == 1,
        "Forward/reopen lost Shelf view order",
    )?;
    fixture.finish()
}

/// Keep production bindings and the controller alive across rendered frames.
pub struct FilterSurface(Fixture);
impl std::fmt::Debug for FilterSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FilterSurface").finish_non_exhaustive()
    }
}
impl FilterSurface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        use slint::Model;
        let fixture = Fixture::open(ui)?;
        fixture.search(ui, "Alpha")?;
        ui.invoke_sort_selected(1);
        fixture.settle(ui)?;
        let rows = ui.get_filter_rows();
        let index = (0..rows.row_count())
            .filter_map(|i| rows.row_data(i))
            .find(|r| r.label == "Bedtime")
            .ok_or("filter surface tag missing")?
            .index;
        ui.invoke_filter_edit(index, 1);
        fixture.settle(ui)?;
        ui.invoke_open_dialog(8);
        Ok(Self(fixture))
    }
    pub fn poll(&self, ui: &DesktopWindow) -> Result {
        if let Some(snapshot) = self.0.worker.borrow().take_snapshot() {
            self.0
                .view
                .borrow_mut()
                .accept(ui, snapshot, self.0.worker.borrow().revision());
        }
        self.0.view.borrow_mut().resize(ui);
        check(
            ui.get_displayed_sort_index() == 1,
            "displayed native sort lost its selected value",
        )?;
        Ok(())
    }
    pub fn bottom(&self, ui: &DesktopWindow) -> Result {
        for _ in 0..40 {
            if ui.get_filter_clear_focused() {
                break;
            }
            super::key(ui, slint::platform::Key::Tab.into())?;
        }
        check(
            ui.get_filter_clear_focused(),
            "filter clear action is not keyboard reachable",
        )?;
        let y = ui.get_filter_clear_window_y();
        let height = ui.get_filter_clear_height();
        let top = ui.get_dialog_viewport_y();
        let viewport = ui.get_dialog_viewport_height();
        check(
            y + 0.5 >= top && y + height <= top + viewport + 0.5,
            "focused filter action remains clipped",
        )
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        ui.invoke_focus_filters();
        Ok(())
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

pub fn history(ui: &DesktopWindow) -> Result {
    let fixture = Fixture::open(ui)?;
    fixture.search(ui, "Alpha")?;
    fixture.select(ui, 2)?;
    fixture.search(ui, "Al")?;
    fixture.select(ui, 4)?;
    // Both frames contain these stories, so stale local selection cannot hide
    // behind the fallback for a story absent from the restored page.
    for (step, mode, language, query, selection) in [
        (-1, Some(1), None, "Alpha", 2),
        (1, None, Some("fr"), "Al", 4),
        (-1, None, Some("en"), "Alpha", 2),
        (1, Some(0), None, "Al", 4),
    ] {
        ui.invoke_history_step(step);
        if let Some(mode) = mode {
            ui.set_mode(mode);
            ui.invoke_mode_selected(mode);
        }
        if let Some(language) = language {
            ui.invoke_change_language(language.into());
        }
        fixture.settle(ui)?;
        check(
            ui.get_query() == query && ui.get_selected_index() == selection,
            "superseding snapshot lost history query or selection restoration",
        )?;
    }
    ui.set_query("unsent editor text".into());
    ui.invoke_change_language("fr".into());
    fixture.settle(ui)?;
    check(
        ui.get_query() == "unsent editor text" && ui.get_selected_index() == 4,
        "an already observed history transition reset unsaved input or selection",
    )?;
    ui.invoke_history_step(-1);
    ui.set_query("Alp".into());
    ui.invoke_search("Alp".into());
    ui.set_mode(1);
    ui.invoke_mode_selected(1);
    let latest = fixture.settle(ui)?;
    check(
        ui.get_query() == "Alp"
            && latest.navigation.as_ref().is_some_and(|navigation| {
                navigation.current().location.request.criteria.name == "Alp"
            }),
        "history restoration overwrote a newer search",
    )?;
    fixture.finish()
}

pub fn failed_query(ui: &DesktopWindow) -> Result {
    let fixture = Fixture::open(ui)?;
    let alpha = fixture.search(ui, "Alpha")?;
    fixture.select(ui, 2)?;
    let mut request = alpha
        .navigation
        .ok_or("query fixture navigation missing")?
        .current()
        .location
        .request
        .clone();
    // A restored/handover filter can reference a tag removed from the library.
    request
        .criteria
        .boolean_filters
        .push(librairii_core::collection_codec::BooleanFilter {
            definition_id: 999,
            state: "true".into(),
        });
    fixture.worker.borrow().offer(Command::Request(request))?;
    let failed = fixture.settle(ui)?;
    check(
        failed.page.is_none() && ui.get_back_enabled() && !ui.get_forward_enabled(),
        "query failure hid valid Back history",
    )?;
    ui.invoke_history_step(-1);
    let restored = fixture.settle(ui)?;
    check(
        restored.page.is_some() && ui.get_query() == "Alpha" && ui.get_selected_index() == 2,
        "Back did not recover the previous query and selection",
    )?;
    ui.invoke_history_step(1);
    check(
        fixture.settle(ui)?.page.is_none() && ui.get_back_enabled(),
        "revisiting a failed query disabled Back",
    )?;
    ui.invoke_request_panel(0);
    check(
        fixture.settle(ui)?.page.is_some(),
        "Library did not recover from the failed query",
    )?;
    ui.invoke_history_step(-1);
    check(
        fixture.settle(ui)?.page.is_none() && ui.get_back_enabled() && ui.get_forward_enabled(),
        "failed middle frame hid Back or Forward history",
    )?;
    ui.invoke_history_step(1);
    check(
        fixture.settle(ui)?.page.is_some(),
        "Forward did not recover the next valid page",
    )?;
    std::fs::write(
        fixture.locations.native.join("navigation.json"),
        b"unconfirmed external preferences",
    )?;
    ui.invoke_mode_selected(1);
    let recovery = fixture.settle(ui)?;
    check(
        recovery.phase == Phase::Recovery && !ui.get_back_enabled() && !ui.get_forward_enabled(),
        "recovery without navigation retained stale history controls",
    )?;
    fixture.finish()
}
