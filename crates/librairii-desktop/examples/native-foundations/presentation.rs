//! Production view callbacks backed by an explicitly owned synthetic library.
#[path = "flags.rs"]
mod flag_checks;
use super::{Result, check, view};
pub use flag_checks::{Surface as FlagSurface, checks as flags};
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
        Self::open_with(ui, false)
    }
    fn open_with(ui: &DesktopWindow, real: bool) -> Result<Self> {
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
        if real {
            let source = roots.profile().join("Bibliothèque originale.plain.pk");
            std::fs::copy(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../preservation/corpus/archives/clockwork-forest.plain.pk"),
                &source,
            )?;
            let executor = librairii_desktop::imports::ImportExecutor::new(&database);
            let repository =
                librairii_storage::operations_repository::TerminalRepository::new(&database, 1);
            librairii_app::native_jobs::with_runtime(
                &repository,
                &executor,
                Default::default(),
                |handle| -> Result {
                    let deadline = Instant::now() + Duration::from_secs(15);
                    let mut prepared = librairii_desktop::imports::PreparedImport::new(
                        "e0000000-0000-4000-8000-000000000015".parse()?,
                        vec![source],
                    )?;
                    loop {
                        match executor.offer(&handle, prepared) {
                            Ok(()) => break,
                            Err((librairii_desktop::imports::PrepareError::Busy, original))
                                if Instant::now() < deadline =>
                            {
                                prepared = original;
                                std::thread::sleep(Duration::from_millis(2));
                            }
                            Err((error, _)) => return Err(error.into()),
                        }
                    }
                    loop {
                        let operation = database
                            .operation_snapshot("e0000000-0000-4000-8000-000000000015".into())?
                            .wait()?;
                        if operation.is_some_and(|s| {
                            !matches!(
                                s.operation.state,
                                librairii_app::importing::OperationState::Running
                                    | librairii_app::importing::OperationState::Queued
                            )
                        }) {
                            break;
                        }
                        if Instant::now() >= deadline {
                            return Err("native workflow seed did not finish".into());
                        }
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Ok(())
                },
            )??;
            let seeded = database.transaction(Default::default(), |tx| {
                let description = format!("<p>A <b>clockwork</b> forest &amp; friends.</p><script>hidden()</script><ul><li>Listen</li><li>Imagine</li></ul><p>&lt;b&gt;literal&lt;/b&gt; &amp;lt;</p>{}", "<p>A long readable paragraph for native scrolling.</p>".repeat(20));
                tx.execute("UPDATE stories SET embedded_description=? WHERE embedded_title='The Clockwork Forest'",[description])?;
                tx.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) SELECT id,101,NULL,'manual' FROM stories WHERE embedded_title='The Clockwork Forest'",[])?;
                Ok(())
            })?.wait()?;
            check(
                matches!(
                    seeded,
                    librairii_storage::database::TransactionOutcome::Committed(())
                ),
                "native workflow metadata seed failed",
            )?;
        }
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
    fn restart(&mut self, ui: &DesktopWindow) -> Result {
        self.worker.borrow_mut().finish()?;
        *self.worker.borrow_mut() = Worker::start_with(Environment {
            home: self._roots.profile().join("home").into_os_string(),
            config: self._roots.profile().join("config").into_os_string(),
            data: self._roots.library().join("data").into_os_string(),
        })?;
        let initial = receive(&self.worker.borrow())?;
        self.view = view::install(ui, self.worker.clone(), initial)?;
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
            ui.on_detail_flag_change(|_| {});
            ui.on_panel_flag_change(|_| {});
            ui.on_bulk_flag_change(|_| {});
            ui.on_undo_flags(|| {});
            ui.on_dismiss_flag_feedback(|| {});
            ui.on_toggle_bulk(|_| {});
            ui.on_clear_bulk(|| {});
            ui.on_select_tag_definition(|_| {});
            ui.on_select_tag_value(|_| {});
            ui.on_save_tag_definition(|| {});
            ui.on_save_tag_value(|| {});
            ui.on_move_tag_definition(|_| {});
            ui.on_move_tag_value(|_| {});
            ui.on_delete_tag_definition(|| {});
            ui.on_delete_tag_value(|| {});
            ui.on_confirm_tag_delete(|| {});
            ui.on_cancel_tag_delete(|| {});
            ui.on_edit_tags(|| {});
            ui.on_refresh_assignments(|| {});
            ui.on_assignment_edit(|_, _| {});
            ui.on_request_details(|| {});
            ui.on_begin_removal(|| {});
            ui.on_remove_story(|| {});
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

/// Frozen confirmation uses real immutable managed bytes and production callbacks.
pub fn details(ui: &DesktopWindow) -> Result {
    let fixture = Fixture::open_with(ui, true)?;
    ui.invoke_request_details();
    fixture.settle(ui)?;
    check(
        ui.get_selected_story().technical.contains("story.zip"),
        "automatic selection detail callback did not resolve archive facts",
    )?;
    let page = fixture.search(ui, "Clockwork")?;
    check(
        page.page.as_ref().is_some_and(|p| {
            p.stories[0]
                .summary
                .as_ref()
                .is_some_and(|s| s.description.contains("<b>literal</b> &lt;"))
        }),
        "page decoded entity-escaped markup more than once",
    )?;
    fixture.select(ui, 0)?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    let row = ui.get_selected_story();
    check(
        row.description
            .starts_with("A clockwork forest & friends.\n\n• Listen")
            && !row.description.contains("hidden"),
        "details exposed markup or lost readable structure",
    )?;
    check(
        row.technical.contains("Bibliothèque originale.plain.pk")
            && row.technical.contains("SHA-256:")
            && row.display_sources.contains("Archive metadata"),
        "technical facts or independent provenance missing",
    )?;
    check(
        row.description.contains("<b>literal</b> &lt;"),
        "detail and page disagree on escaped literal text",
    )?;
    let source = fixture
        ._roots
        .profile()
        .join("Bibliothèque originale.plain.pk");
    let original = std::fs::read(&source)?;
    let path = fixture._roots.library().join("native/db/librairii.sqlite3");
    let connection = rusqlite::Connection::open(&path)?;
    let checksum:String=connection.query_row("SELECT sha256 FROM story_archives WHERE story_id=(SELECT id FROM stories WHERE embedded_title='The Clockwork Forest')",[],|r|r.get(0))?;
    connection.execute("UPDATE story_archives SET original_filename='Refreshed archive.plain.pk',sha256=? WHERE story_id=(SELECT id FROM stories WHERE embedded_title='The Clockwork Forest')",["a".repeat(64)])?;
    connection.execute("UPDATE stories SET embedded_description='<p>Fresh &amp; readable.</p>' WHERE embedded_title='The Clockwork Forest'",[])?;
    fixture.worker.borrow().offer(Command::Refresh)?;
    fixture.settle(ui)?;
    check(
        ui.get_selected_story()
            .technical
            .contains("Refreshed archive.plain.pk")
            && ui.get_selected_story().technical.contains(&"a".repeat(64))
            && ui.get_selected_story().description == "Fresh & readable.",
        "Refresh retained cached detail/archive facts",
    )?;
    check(
        !ui.get_selected_story().facts.contains("2026-")
            && ui.get_selected_story().facts.contains("Imported:"),
        "detail date remains an unformatted storage timestamp",
    )?;
    connection.execute("UPDATE story_archives SET sha256=? WHERE story_id=(SELECT id FROM stories WHERE embedded_title='The Clockwork Forest')",[checksum])?;
    drop(connection);
    ui.invoke_begin_removal();
    check(
        ui.get_dialog() == 9 && ui.get_removal_keep_focused(),
        "removal did not default to Keep",
    )?;
    ui.invoke_close_dialog();
    check(
        ui.get_dialog() == 3,
        "Keep closed details instead of returning to removal invoker",
    )?;
    ui.invoke_begin_removal();
    let mut stale = librairii_core::query::QueryRequest::default();
    stale.criteria.name = "Beta".into();
    fixture.worker.borrow().offer(Command::Request(stale))?;
    fixture.settle(ui)?;
    ui.invoke_remove_story();
    fixture.settle(ui)?;
    check(
        ui.get_dialog() == 9 && !ui.get_removal_error().is_empty() && !ui.get_detail_pending(),
        "stale removal silently retargeted or lacked failure feedback",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_close_dialog();
    fixture.worker.borrow().offer(Command::Back)?;
    fixture.settle(ui)?;
    fixture.select(ui, 0)?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    ui.invoke_begin_removal();
    ui.invoke_remove_story();
    check(
        ui.get_detail_pending(),
        "removal did not fence dismissal before worker response",
    )?;
    ui.invoke_close_dialog();
    check(ui.get_dialog() == 9, "pending removal allowed dismissal")?;
    let removed = fixture.settle(ui)?;
    check(
        ui.get_dialog() == 0 && removed.page.as_ref().is_some_and(|p| p.total == 0),
        "successful removal did not close and refresh details",
    )?;
    check(
        std::fs::read(&source)? == original,
        "removal changed the original source",
    )?;
    let mut retained = false;
    for entry in std::fs::read_dir(fixture._roots.library().join("native/trash/removals"))? {
        for file in std::fs::read_dir(entry?.path())? {
            if std::fs::read(file?.path())? == original {
                retained = true;
            }
        }
    }
    check(
        retained,
        "removal lost the managed archive instead of moving it to application trash",
    )?;
    fixture.finish()?;
    details_recovery(ui)
}
fn details_recovery(ui: &DesktopWindow) -> Result {
    let fixture = Fixture::open_with(ui, true)?;
    fixture.search(ui, "Clockwork")?;
    fixture.select(ui, 0)?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    ui.invoke_begin_removal();
    use std::os::unix::fs::PermissionsExt;
    let permissions = std::fs::metadata(&fixture.locations.native)?.permissions();
    std::fs::set_permissions(
        &fixture.locations.native,
        std::fs::Permissions::from_mode(0o500),
    )?;
    ui.invoke_remove_story();
    fixture.worker.borrow().offer(Command::Presentation(
        librairii_core::navigation::Presentation::SmallCovers,
    ))?;
    let recovered = fixture.settle(ui);
    std::fs::set_permissions(&fixture.locations.native, permissions)?;
    let recovered = recovered?;
    check(
        recovered.phase == Phase::Recovery
            && recovered
                .workflows
                .removal
                .as_ref()
                .is_some_and(|r| r.is_ok()),
        &format!(
            "postcommit preference failure lost confirmed removal outcome: phase={:?}, removal={:?}, status={:?}",
            recovered.phase, recovered.workflows.removal, recovered.status
        ),
    )?;
    check(
        !ui.get_detail_pending()
            && ui.get_dialog() == 0
            && ui.get_status().contains("removed")
            && ui.get_removal_error().is_empty(),
        "confirmed removal was falsely reported as failure during recovery",
    )?;
    fixture.finish()
}

pub struct DetailSurface(Fixture);
impl std::fmt::Debug for DetailSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetailSurface").finish_non_exhaustive()
    }
}
impl DetailSurface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        Ok(Self(Fixture::open_with(ui, true)?))
    }
    pub fn first_disclosure(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_focus_inspector_technical();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: " ".into() });
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: " ".into() });
        self.0.settle(ui)?;
        check(
            ui.get_selected_story().technical.contains("story.zip"),
            "shown wide disclosure never loaded automatic selection",
        )?;
        ui.invoke_show_panel(5);
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Escape.into(),
            });
        check(
            ui.get_inspector_technical_focused(),
            "Settings did not return focus to the exact inspector invoker",
        )?;
        ui.invoke_open_dialog(1);
        ui.invoke_close_dialog();
        check(
            ui.get_inspector_technical_focused(),
            "About did not return focus to the inspector invoker",
        )?;
        Ok(())
    }
    pub fn begin_details(&self, ui: &DesktopWindow) -> Result {
        self.0.search(ui, "Clockwork")?;
        self.0.select(ui, 0)?;
        ui.invoke_open_dialog(3);
        self.0.settle(ui)?;
        Ok(())
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(snapshot) = self.0.worker.borrow().take_snapshot() {
            self.0
                .view
                .borrow_mut()
                .accept(ui, snapshot, self.0.worker.borrow().revision());
        }
        self.0.view.borrow_mut().resize(ui);
    }
    pub fn technical(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_focus_details_technical();
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: " ".into() });
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: " ".into() });
        ui.invoke_scroll_details(slint::platform::Key::End.into());
        Ok(())
    }
    pub fn french_reopen(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        check(
            ui.get_focused_key() == 31,
            "hidden wide inspector did not restore focus to the browser",
        )?;
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        ui.invoke_open_dialog(3);
        self.0.settle(ui)?;
        check(
            ui.get_details_scroll_position() == 0.0,
            "reopening details retained an old scroll offset",
        )?;
        Ok(())
    }
}

fn choose_tag(fixture: &Fixture, ui: &DesktopWindow, id: i64) -> Result {
    let position = fixture
        .view
        .borrow()
        .snapshot
        .facets
        .tags
        .iter()
        .filter(|e| e.definition.source != "builtin" || e.definition.key != "broken")
        .position(|e| e.definition.id == id)
        .ok_or("tag definition missing")?;
    ui.invoke_select_tag_definition((position + 1) as i32);
    Ok(())
}
fn assignment_index(ui: &DesktopWindow, text: &str) -> Result<i32> {
    use slint::Model;
    let rows = ui.get_assignment_rows();
    (0..rows.row_count())
        .filter_map(|i| rows.row_data(i))
        .find(|r| r.label.contains(text))
        .map(|r| r.index)
        .ok_or_else(|| format!("assignment row missing: {text}").into())
}
pub fn tags(ui: &DesktopWindow) -> Result {
    let mut fixture = Fixture::open(ui)?;
    ui.invoke_show_panel(1);
    ui.set_tag_key("listen-later".into());
    ui.set_tag_label("Listen later".into());
    ui.set_tag_color("#224466".into());
    ui.set_tag_kind(1);
    ui.invoke_save_tag_definition();
    check(ui.get_tag_busy(), "tag mutation did not fence dismissal")?;
    fixture.settle(ui)?;
    let entry = fixture
        .view
        .borrow()
        .snapshot
        .facets
        .tags
        .iter()
        .find(|e| e.definition.key == "listen-later")
        .cloned()
        .ok_or("new choice tag missing")?;
    choose_tag(&fixture, ui, entry.definition.id)?;
    for (key, label) in [("first", "First value"), ("second", "Second value")] {
        ui.invoke_select_tag_value(0);
        ui.set_tag_value_key(key.into());
        ui.set_tag_value_label(label.into());
        ui.invoke_save_tag_value();
        fixture.settle(ui)?;
    }
    ui.invoke_select_tag_value(1);
    ui.set_tag_value_label("Renamed value".into());
    ui.invoke_save_tag_value();
    fixture.settle(ui)?;
    ui.invoke_move_tag_value(1);
    fixture.settle(ui)?;
    let entry = fixture
        .view
        .borrow()
        .snapshot
        .facets
        .tags
        .iter()
        .find(|e| e.definition.id == entry.definition.id)
        .cloned()
        .ok_or("choice tag disappeared")?;
    check(
        entry.values[1].key == "first" && entry.values[1].label == "Renamed value",
        "value rename/order changed an immutable key",
    )?;
    ui.invoke_move_tag_definition(-1);
    fixture.settle(ui)?;
    let path = fixture._roots.library().join("native/db/librairii.sqlite3");
    let mut connection = rusqlite::Connection::open(&path)?;
    ui.invoke_delete_tag_value();
    fixture.settle(ui)?;
    check(
        ui.get_dialog() == 12 && ui.get_tag_delete_cancel_focused(),
        "tag deletion did not initially focus Cancel",
    )?;
    ui.invoke_close_dialog();
    check(
        ui.get_dialog() == 10 && ui.get_tag_delete_invoker_focused(),
        "tag cancellation lost its exact delete invoker",
    )?;
    ui.invoke_delete_tag_value();
    fixture.settle(ui)?;
    let value_id = entry.values[1].id;
    connection.execute("INSERT INTO story_tag_assignments(story_id,definition_id,value_id,source) VALUES(1,?1,?2,'manual')",rusqlite::params![entry.definition.id,value_id])?;
    let transaction = connection.transaction()?;
    let mut query = librairii_core::collection_codec::Query::default();
    query
        .choice_filters
        .push(librairii_core::collection_codec::ChoiceFilter {
            definition_id: entry.definition.id,
            value_ids: vec![value_id],
        });
    librairii_storage::collections::execute(
        &transaction,
        librairii_storage::collections::Command::Create {
            name: "Tag-dependent".into(),
            criteria: query,
        },
    )?;
    transaction.commit()?;
    ui.invoke_confirm_tag_delete();
    fixture.settle(ui)?;
    check(
        ui.get_dialog() == 12 && ui.get_tag_notice().contains("impact changed"),
        "stale destructive impact was silently applied",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_delete_tag_value();
    let preview = fixture.settle(ui)?;
    check(
        preview
            .workflows
            .tags
            .preview
            .as_ref()
            .is_some_and(|p| p.assignments == 1 && p.collections == 1),
        "destructive impact counts are incomplete",
    )?;
    let ticket = preview
        .workflows
        .tags
        .preview
        .as_ref()
        .ok_or("preview missing")?
        .ticket
        .clone();
    check(
        ui.get_tag_impact().contains("1 assignment")
            && ui.get_tag_impact().contains("1 affected Collection"),
        "singular impact was not localized",
    )?;
    ui.invoke_change_language("fr".into());
    let french = fixture.settle(ui)?;
    check(
        ui.get_dialog() == 12
            && ui.get_tag_impact().contains("1 attribution")
            && ui.get_tag_impact().contains("1 Collection concernée")
            && french
                .workflows
                .tags
                .preview
                .as_ref()
                .is_some_and(|p| p.ticket == ticket),
        "language change replaced confirmation authority or retained English impact",
    )?;
    ui.invoke_change_language("en".into());
    fixture.settle(ui)?;
    ui.invoke_confirm_tag_delete();
    let deleted = fixture.settle(ui)?;
    check(
        ui.get_dialog() == 10
            && deleted
                .groups
                .iter()
                .any(|g| g.name == "Tag-dependent" && !g.available),
        "tag deletion silently dropped unavailable Collection criteria",
    )?;
    ui.invoke_delete_tag_definition();
    fixture.settle(ui)?;
    ui.invoke_confirm_tag_delete();
    fixture.settle(ui)?;
    let protected = fixture
        .view
        .borrow()
        .snapshot
        .facets
        .tags
        .iter()
        .find(|e| e.definition.source == "derived")
        .cloned()
        .ok_or("protected age facet was not seeded")?;
    choose_tag(&fixture, ui, protected.definition.id)?;
    check(
        !ui.get_tag_editable(),
        "protected identity exposed user editing",
    )?;
    ui.set_tag_label("Should remain protected".into());
    ui.set_tag_color("#1B5E20".into());
    ui.invoke_save_tag_definition();
    let colored = fixture.settle(ui)?;
    check(
        colored.facets.tags.iter().any(|e| {
            e.definition.id == protected.definition.id
                && e.definition.label == protected.definition.label
                && e.definition.color == "#1B5E20"
        }),
        "protected color editing changed identity or was refused",
    )?;
    choose_tag(&fixture, ui, 102)?;
    ui.set_tag_label("Would roll back".into());
    ui.set_tag_color("#FFFFFF".into());
    ui.invoke_save_tag_definition();
    let failed = fixture.settle(ui)?;
    check(
        failed
            .facets
            .tags
            .iter()
            .any(|e| e.definition.id == 102 && e.definition.label == "Theme")
            && !ui.get_tag_notice().is_empty(),
        "invalid color partially committed a rename",
    )?;
    ui.invoke_change_language("fr".into());
    fixture.settle(ui)?;
    check(
        ui.get_tag_label() == "Would roll back" && ui.get_tag_color() == "#FFFFFF",
        "language switch overwrote unsaved tag input",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_change_language("en".into());
    fixture.settle(ui)?;
    fixture
        .worker
        .borrow()
        .offer(Command::Request(librairii_core::query::QueryRequest {
            page_size: 100,
            ..Default::default()
        }))?;
    fixture.settle(ui)?;
    let inspected = ui.get_selected_story().uuid;
    ui.invoke_toggle_bulk(0);
    ui.invoke_toggle_bulk(30);
    check(
        ui.get_selected_story().uuid == inspected && ui.get_bulk_summary().starts_with("2"),
        "bulk selection changed single inspection",
    )?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    ui.invoke_edit_tags();
    let mixed = fixture.settle(ui)?;
    check(
        mixed
            .workflows
            .tags
            .workspace
            .as_ref()
            .is_some_and(|w| w.requested_stories == 2),
        "bulk editor did not freeze both original targets",
    )?;
    let bedtime = assignment_index(ui, "Bedtime")?;
    let forest = assignment_index(ui, "Forest")?;
    use slint::Model;
    check(
        (0..ui.get_assignment_rows().row_count())
            .filter_map(|i| ui.get_assignment_rows().row_data(i))
            .any(|r| r.index == bedtime && r.state == 2),
        "mixed assignment was announced as unchecked",
    )?;
    ui.invoke_assignment_edit(forest, 1);
    let assigned = fixture.settle(ui)?;
    let workspace = assigned
        .workflows
        .tags
        .workspace
        .as_ref()
        .ok_or("assigned workspace missing")?;
    let theme = workspace
        .states
        .iter()
        .find(|s| s.definition_id == 102)
        .ok_or("choice states missing")?;
    check(
        theme
            .values
            .iter()
            .any(|v| v.value_id == 201 && v.assigned_stories == 2)
            && theme
                .values
                .iter()
                .any(|v| v.value_id == 202 && v.assigned_stories == 1),
        "additive choice toggle replaced unrelated values",
    )?;
    ui.invoke_assignment_edit(assignment_index(ui, "Forest")?, 1);
    fixture.settle(ui)?;
    check(
        ui.get_tag_notice().contains("No stories changed"),
        "idempotent assignment omitted no-op feedback",
    )?;
    fixture.search(ui, "Beta")?;
    ui.invoke_assignment_edit(assignment_index(ui, "Bedtime")?, 1);
    let frozen = fixture.settle(ui)?;
    check(
        frozen
            .workflows
            .tags
            .targets
            .as_ref()
            .is_some_and(|t| t.stories.iter().map(|s| s.id).collect::<Vec<_>>() == vec![1, 31])
            && frozen.workflows.tags.workspace.as_ref().is_some_and(|w| {
                w.states
                    .iter()
                    .any(|s| s.definition_id == 101 && s.assigned_stories == 2)
            }),
        "filter change retargeted the assignment batch",
    )?;
    ui.invoke_close_dialog();
    check(
        ui.get_dialog() == 0,
        "tag editor returned to details for a story no longer in the result",
    )?;
    drop(connection);
    fixture.restart(ui)?;
    check(
        fixture
            .view
            .borrow()
            .snapshot
            .facets
            .tags
            .iter()
            .any(|e| e.definition.id == protected.definition.id && e.definition.color == "#1B5E20"),
        "startup reseeding erased a protected custom color",
    )?;
    fixture.finish()
}

pub struct TagSurface(Fixture);
impl std::fmt::Debug for TagSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TagSurface").finish_non_exhaustive()
    }
}
impl TagSurface {
    pub fn open(ui: &DesktopWindow, _menus: &librairii_desktop::menus::Menus) -> Result<Self> {
        let fixture = Fixture::open(ui)?;
        let inspected = fixture.view.borrow().selected.clone();
        ui.invoke_focus_bulk_story(0);
        super::key(ui, slint::platform::Key::Space.into())?;
        check(
            fixture.view.borrow().tags.selection.len() == 1 && !ui.get_bulk_summary().is_empty(),
            "actual story Space did not create a visible bulk selection",
        )?;
        check(
            fixture.view.borrow().selected == inspected,
            "bulk Space changed inspection",
        )?;
        #[cfg(target_os = "macos")]
        super::native_menu::bulk_focus(ui, _menus)?;
        ui.invoke_focus_bulk_story(0);
        super::key(ui, slint::platform::Key::Space.into())?;
        check(
            fixture.view.borrow().tags.selection.is_empty() && ui.get_bulk_summary().is_empty(),
            "actual story Space did not clear the bulk selection",
        )?;
        let scope = fixture
            .view
            .borrow()
            .snapshot
            .groups
            .iter()
            .find(|g| g.name == "Reverse")
            .ok_or("Shelf missing")?
            .scope
            .clone();
        fixture.worker.borrow().offer(Command::Navigate(scope))?;
        fixture.settle(ui)?;
        // Section15.6 composes workspace navigation; exercise the shared
        // approved-row input path here with the real Shelf page already loaded.
        ui.set_workspace(true);
        ui.invoke_focus_bulk_story(0);
        super::key(ui, slint::platform::Key::Space.into())?;
        check(
            fixture.view.borrow().tags.selection.len() == 1 && !ui.get_bulk_summary().is_empty(),
            "approved-member Space did not create a visible selection",
        )?;
        super::key(ui, slint::platform::Key::Space.into())?;
        check(
            fixture.view.borrow().tags.selection.is_empty(),
            "approved-member Space lost focus or the toggle binding",
        )?;
        ui.set_workspace(false);
        ui.invoke_show_panel(0);
        fixture.settle(ui)?;
        ui.invoke_show_panel(1);
        choose_tag(&fixture, ui, 102)?;
        ui.invoke_select_tag_value(1);
        Ok(Self(fixture))
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(snapshot) = self.0.worker.borrow().take_snapshot() {
            self.0
                .view
                .borrow_mut()
                .accept(ui, snapshot, self.0.worker.borrow().revision());
        }
        self.0.view.borrow_mut().resize(ui);
    }
    pub fn bottom(&self, ui: &DesktopWindow) -> Result {
        for _ in 0..50 {
            if ui.get_close_focused() {
                break;
            }
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::Tab.into(),
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::Tab.into(),
                });
        }
        check(
            ui.get_close_focused(),
            "tag manager Close is not keyboard reachable",
        )?;
        Ok(())
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        ui.invoke_close_dialog();
        ui.invoke_show_panel(1);
        choose_tag(&self.0, ui, 102)?;
        ui.invoke_select_tag_value(1);
        Ok(())
    }
    pub fn assignment(&self, ui: &DesktopWindow, language: &str) -> Result {
        ui.invoke_close_dialog();
        if ui.get_dialog() == 3 {
            ui.invoke_close_dialog();
        }
        ui.invoke_change_language(language.into());
        self.0.settle(ui)?;
        self.0
            .worker
            .borrow()
            .offer(Command::Request(librairii_core::query::QueryRequest {
                page_size: 100,
                ..Default::default()
            }))?;
        self.0.settle(ui)?;
        ui.invoke_clear_bulk();
        ui.invoke_toggle_bulk(0);
        ui.invoke_toggle_bulk(30);
        ui.invoke_open_dialog(3);
        self.0.settle(ui)?;
        ui.invoke_edit_tags();
        self.0.settle(ui)?;
        Ok(())
    }
    pub fn confirm(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        if ui.get_dialog() == 3 {
            self.0.settle(ui)?;
            ui.invoke_close_dialog();
        }
        ui.invoke_show_panel(1);
        choose_tag(&self.0, ui, 102)?;
        ui.invoke_delete_tag_definition();
        self.0.settle(ui)?;
        check(
            ui.get_tag_delete_cancel_focused(),
            "impact confirmation did not focus Cancel",
        )?;
        Ok(())
    }
}
