//! Real setup, move, recovery and diagnostic callbacks on owned temporary roots.
use super::{Environment, Fixture, Result, check, receive, view};
use librairii_app::localization::Language;
use librairii_desktop::{
    maintenance::{Action, Mode, Notice},
    presentation::{Command, Phase, Snapshot, Worker},
    startup::StartupEnvironment,
};
use librairii_storage::profile::ProfileLocations;
use librairii_testkit::TestRoots;
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
use std::{cell::RefCell, ffi::OsString, path::PathBuf, rc::Rc};
use view::maintenance::Picker;

fn close(ui: &DesktopWindow) {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
}
fn setup(ui: &DesktopWindow) -> Result<Fixture> {
    close(ui);
    let roots = TestRoots::new()?;
    let home = roots.profile().join("home");
    let config = roots.profile().join("config");
    let data = roots.library().join("data");
    let locations = ProfileLocations::for_host(&home, Some(&config), Some(&data))?;
    let worker = Rc::new(RefCell::new(Worker::start_with(Environment {
        catalog: None,
        home: home.into_os_string(),
        config: config.into_os_string(),
        data: data.into_os_string(),
    })?));
    let initial = receive(&worker.borrow())?;
    check(
        initial.phase == Phase::Setup,
        "fresh startup did not reach explicit setup",
    )?;
    let v = view::install(ui, worker.clone(), initial)?;
    Ok(Fixture {
        catalog: None,
        ui: ui.as_weak(),
        worker,
        view: v,
        locations,
        id: "e0000000-0000-4000-8000-000000000014".parse()?,
        _roots: roots,
    })
}
fn choose(f: &Fixture, ui: &DesktopWindow, kind: Picker, path: Option<PathBuf>) {
    let generation = f.view.borrow().snapshot.generation.clone();
    view::maintenance::selected(&f.view, &f.worker, ui, generation, kind, path);
}
fn language(f: &Fixture, ui: &DesktopWindow, code: &str) -> Result<Snapshot> {
    ui.invoke_change_language(code.into());
    f.settle(ui)
}
fn selected_root(f: &Fixture) -> Result<PathBuf> {
    f.view
        .borrow()
        .snapshot
        .workflows
        .settings
        .current
        .clone()
        .ok_or_else(|| "current native root missing".into())
}
fn profile(f: &Fixture) -> Result<Vec<u8>> {
    Ok(std::fs::read(f.locations.native.join("settings.json"))?)
}
fn initialize(f: &mut Fixture, ui: &DesktopWindow) -> Result<PathBuf> {
    let root = f._roots.library().join("Bibliothèque confirmée été");
    choose(f, ui, Picker::Folder, Some(root.clone()));
    let selected = f.settle(ui)?;
    check(
        selected.workflows.settings.can_confirm && !root.exists(),
        "folder preview initialized the library",
    )?;
    ui.invoke_settings_confirm();
    let opened = f.settle(ui)?;
    check(
        opened.phase == Phase::Library && ui.get_dialog() == 2 && ui.get_settings_data().can_export,
        &format!(
            "confirmed setup did not activate usable native settings: phase={:?} dialog={} data={:?} settings={:?}",
            opened.phase,
            ui.get_dialog(),
            ui.get_settings_data(),
            opened.workflows.settings
        ),
    )?;
    f.id = opened
        .navigation
        .as_ref()
        .ok_or("new navigation missing")?
        .library();
    Ok(root)
}
fn setup_checks(ui: &DesktopWindow) -> Result {
    let mut f = setup(ui)?;
    let suggested = f
        .view
        .borrow()
        .snapshot
        .workflows
        .settings
        .proposed
        .clone()
        .ok_or("suggestion missing")?;
    check(
        ui.get_dialog() == 4 && !suggested.exists() && !f.locations.native.exists(),
        "setup created an unconfirmed root/profile",
    )?;
    let before = f.worker.borrow().revision();
    choose(&f, ui, Picker::Folder, None);
    check(
        f.worker.borrow().revision() == before && !suggested.exists(),
        "cancelled folder picker changed setup",
    )?;
    language(&f, ui, "fr")?;
    check(
        ui.get_french() && !suggested.exists(),
        "independent language selection initialized a library",
    )?;
    let invalid = f._roots.library().join("Non vide");
    std::fs::create_dir(&invalid)?;
    std::fs::write(invalid.join("keep"), b"do not overwrite")?;
    choose(&f, ui, Picker::Folder, Some(invalid.clone()));
    let refused = f.settle(ui)?;
    check(
        !refused.workflows.settings.can_confirm
            && refused.workflows.settings.error.is_some()
            && std::fs::read(invalid.join("keep"))? == b"do not overwrite",
        "unsafe setup destination was accepted or changed",
    )?;
    let root = initialize(&mut f, ui)?;
    check(
        ui.get_french() && selected_root(&f)? == root && !suggested.exists(),
        "confirmed root or saved language changed",
    )?;
    close(ui);
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Library
            && ui.get_french()
            && f.view
                .borrow()
                .snapshot
                .navigation
                .as_ref()
                .is_some_and(|n| n.library() == f.id),
        "restart lost the confirmed UUID/root/language",
    )?;
    f.finish()
}
fn move_checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let mut f = Fixture::open_with(ui, true)?;
    ui.invoke_show_panel(5);
    let source = selected_root(&f)?;
    std::fs::write(source.join("extra-root-file"), b"complete root sentinel")?;
    let counts = |path: &std::path::Path| -> Result<(i64, i64, i64)> {
        let c = rusqlite::Connection::open(path.join("db/librairii.sqlite3"))?;
        Ok(c.query_row("SELECT (SELECT count(*) FROM stories),(SELECT count(*) FROM shelf_stories),(SELECT count(*) FROM story_tag_assignments)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)
    };
    let original_counts = counts(&source)?;
    let old = f.view.borrow().snapshot.clone();
    let id = f.id;
    choose(&f, ui, Picker::Folder, Some(source.clone()));
    let unchanged = f.settle(ui)?;
    check(
        unchanged.workflows.settings.notice == Some(Notice::Unchanged)
            && !unchanged.workflows.settings.can_confirm,
        "current-root alias was not a no-op",
    )?;
    let target = f._roots.library().join("Bibliothèque déplacée été");
    choose(&f, ui, Picker::Folder, Some(target.clone()));
    let preview = f.settle(ui)?;
    let summary = preview
        .workflows
        .settings
        .summary
        .ok_or("closed move summary missing")?;
    check(
        summary.files > 0
            && summary.bytes > 0
            && ui.get_settings_data().can_confirm
            && !target.exists(),
        "move preview omitted complete-root counts or created destination",
    )?;
    // Freeze the preview, then change destination ownership before confirmation.
    std::fs::create_dir(&target)?;
    std::fs::write(target.join("foreign"), b"preserve target")?;
    ui.invoke_settings_confirm();
    let rejected = f.settle(ui)?;
    check(
        rejected.phase == Phase::Library
            && selected_root(&f)? == source
            && std::fs::read(target.join("foreign"))? == b"preserve target",
        "changed destination redirected or overwrote the library",
    )?;
    let target = f._roots.library().join("Destination indépendante été");
    choose(&f, ui, Picker::Folder, Some(target.clone()));
    f.settle(ui)?;
    language(&f, ui, "fr")?;
    ui.invoke_settings_confirm();
    let moved = f.settle(ui)?;
    check(
        moved.phase == Phase::Library
            && selected_root(&f)? == target
            && moved.navigation.as_ref().is_some_and(|n| n.library() == id)
            && ui.get_french(),
        "move lost root identity or independent language",
    )?;
    let old_stamp = old.navigation.as_ref().ok_or("old navigation")?.stamp();
    check(
        moved.generation != old.generation
            && moved
                .navigation
                .as_ref()
                .is_some_and(|n| !n.accepts(&old_stamp)),
        "move failed to invalidate old generations/queries",
    )?;
    let mut stale = old;
    stale.revision = f.worker.borrow().revision();
    check(
        !f.view
            .borrow_mut()
            .accept(ui, stale, f.worker.borrow().revision()),
        "old-root response overwrote the new active root",
    )?;
    check(
        std::fs::read(target.join("extra-root-file"))? == b"complete root sentinel",
        "complete-root move omitted an extra root file",
    )?;
    let c = rusqlite::Connection::open(target.join("db/librairii.sqlite3"))?;
    let (stories,members,tags):(i64,i64,i64)=c.query_row("SELECT (SELECT count(*) FROM stories),(SELECT count(*) FROM shelf_stories),(SELECT count(*) FROM story_tag_assignments)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?;
    check(
        (stories, members, tags) == original_counts,
        "move changed stories, approved membership or tags",
    )?;
    drop(c);
    close(ui);
    f.restart(ui)?;
    check(
        selected_root(&f)? == target && ui.get_french(),
        "restart reverted moved root/language",
    )?;
    f.finish()
}
fn diagnostics_checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let f = Fixture::open_with(ui, true)?;
    ui.invoke_show_panel(5);
    let root = selected_root(&f)?;
    std::fs::write(
        root.join("private-sentinel"),
        b"PRIVATE_SENTINEL_SHOULD_NOT_BE_EXPORTED",
    )?;
    let revision = f.worker.borrow().revision();
    choose(&f, ui, Picker::Diagnostics, None);
    check(
        revision == f.worker.borrow().revision(),
        "cancelled diagnostic save issued work",
    )?;
    let zip = f._roots.profile().join("Diagnostic privé été.zip");
    choose(&f, ui, Picker::Diagnostics, Some(zip.clone()));
    let saved = f.settle(ui)?;
    let bytes = std::fs::read(&zip)?;
    let text = String::from_utf8_lossy(&bytes);
    check(
        saved.workflows.settings.notice == Some(Notice::DiagnosticsSaved)
            && bytes.starts_with(b"PK\x03\x04")
            && bytes.len() < 2 * 1024 * 1024
            && text.contains("diagnostics.json")
            && !text.contains("SQLite format 3")
            && !text.contains("PRIVATE_SENTINEL_SHOULD_NOT_BE_EXPORTED")
            && !text.contains(root.to_str().ok_or("UTF8 root")?),
        "native diagnostic ZIP omitted bounds or disclosed private content",
    )?;
    choose(&f, ui, Picker::Diagnostics, Some(zip.clone()));
    let refused = f.settle(ui)?;
    check(
        refused.workflows.settings.error.is_some() && std::fs::read(zip)? == bytes,
        "diagnostic Save overwrote an existing file",
    )?;
    f.finish()
}
fn recovery_checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let mut f = Fixture::open(ui)?;
    let source = selected_root(&f)?;
    f.worker.borrow_mut().finish()?;
    let relocated = f._roots.library().join("Retrouvée même identité été");
    std::fs::rename(&source, &relocated)?;
    let before = profile(&f)?;
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Recovery
            && ui.get_dialog() == 5
            && ui.get_settings_data().can_retry
            && ui.get_settings_data().can_locate
            && !source.exists(),
        "missing root was replaced or recovery controls unavailable",
    )?;
    language(&f, ui, "fr")?;
    let wrong = f._roots.library().join("Different UUID");
    let other = librairii_storage::IsolatedLibrary::initialize_from_override(
        Some(wrong.as_os_str()),
        "e0000000-0000-4000-8000-000000000099".parse()?,
    )?;
    other.finish().map_err(|e| e.to_string())?;
    let language_profile = profile(&f)?;
    choose(&f, ui, Picker::Locate, Some(wrong));
    let refused = f.settle(ui)?;
    check(
        refused.phase == Phase::Recovery
            && refused.workflows.settings.error.is_some()
            && profile(&f)? == language_profile
            && !source.exists(),
        "Locate accepted a different UUID or rewrote the expected root",
    )?;
    choose(&f, ui, Picker::Locate, Some(relocated.clone()));
    let restored = f.settle(ui)?;
    check(
        restored.phase == Phase::Library
            && selected_root(&f)? == relocated
            && restored
                .navigation
                .as_ref()
                .is_some_and(|n| n.library() == f.id)
            && ui.get_french()
            && before != profile(&f)?,
        "same-UUID Locate did not restore actual library",
    )?;
    f.finish()
}
struct OverrideEnvironment {
    root: OsString,
}
impl StartupEnvironment for OverrideEnvironment {
    fn variable(&self, name: &str) -> Option<OsString> {
        assert_eq!(
            name,
            librairii_storage::DATA_ROOT_OVERRIDE,
            "override read production environment"
        );
        Some(self.root.clone())
    }
}
fn override_checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let roots = TestRoots::new()?;
    let root = roots.library().join("Explicit isolated development");
    let worker = Rc::new(RefCell::new(Worker::start_with(OverrideEnvironment {
        root: root.clone().into_os_string(),
    })?));
    let initial = receive(&worker.borrow())?;
    check(
        initial.phase == Phase::Library
            && initial.language == Language::English
            && initial.workflows.settings.mode == Mode::Override,
        "override did not bypass onboarding with a usable English library",
    )?;
    let id = initial
        .navigation
        .as_ref()
        .ok_or("override navigation")?
        .library();
    let v = view::install(ui, worker.clone(), initial)?;
    let locations = ProfileLocations::for_host(&roots.profile().join("never-read"), None, None)?;
    let f = Fixture {
        catalog: None,
        ui: ui.as_weak(),
        worker,
        view: v,
        locations,
        id,
        _roots: roots,
    };
    ui.invoke_show_panel(5);
    check(
        !ui.get_settings_data().can_choose && ui.get_settings_data().can_export,
        "override relocation was enabled or diagnostics unavailable",
    )?;
    language(&f, ui, "fr")?;
    f.worker.borrow().offer(Command::Maintenance {
        generation: f.view.borrow().snapshot.generation.clone(),
        action: Action::Select(f._roots.library().join("forbidden-move")),
    })?;
    let refused = f.settle(ui)?;
    check(
        refused.workflows.settings.error.is_some()
            && selected_root(&f)? == root
            && ui.get_french()
            && !f.locations.native.exists(),
        "override touched a production profile or allowed relocation",
    )?;
    f.finish()
}
pub fn checks(ui: &DesktopWindow) -> Result {
    eprintln!("maintenance: setup");
    setup_checks(ui)?;
    eprintln!("maintenance: move");
    move_checks(ui)?;
    eprintln!("maintenance: diagnostics");
    diagnostics_checks(ui)?;
    eprintln!("maintenance: recovery");
    recovery_checks(ui)?;
    eprintln!("maintenance: override");
    override_checks(ui)
}

pub struct Surface {
    fixture: Fixture,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaintenanceSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        Ok(Self {
            fixture: setup(ui)?,
        })
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(s) = self.fixture.worker.borrow().take_snapshot() {
            let revision = self.fixture.worker.borrow().revision();
            self.fixture.view.borrow_mut().accept(ui, s, revision);
        }
        self.fixture.view.borrow_mut().resize(ui);
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        language(&self.fixture, ui, "fr")?;
        Ok(())
    }
    pub fn confirmed(&mut self, ui: &DesktopWindow) -> Result {
        let root = self
            .fixture
            ._roots
            .library()
            .join("Bibliothèque confirmée été");
        choose(&self.fixture, ui, Picker::Folder, Some(root));
        self.fixture.settle(ui)?;
        ui.invoke_focus_settings_confirm();
        check(
            ui.get_settings_confirm_focused(),
            "setup Confirm focus missing",
        )?;
        super::super::key(ui, " ".into())?;
        let opened = self.fixture.settle(ui)?;
        check(
            opened.phase == Phase::Library && ui.get_settings_first_focused(),
            "native setup completion lost visible focus",
        )?;
        self.fixture.id = opened
            .navigation
            .as_ref()
            .ok_or("confirmed navigation")?
            .library();
        super::super::key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_modal(),
            "immediate Escape after focused setup confirmation failed",
        )?;
        ui.invoke_show_panel(5);
        Ok(())
    }
    pub fn preview(&self, ui: &DesktopWindow) -> Result {
        choose(
            &self.fixture,
            ui,
            Picker::Folder,
            Some(
                self.fixture
                    ._roots
                    .library()
                    .join("Nouvel emplacement sélectionné été"),
            ),
        );
        self.fixture.settle(ui)?;
        Ok(())
    }
    pub fn recover(&mut self, ui: &DesktopWindow) -> Result {
        close(ui);
        let source = selected_root(&self.fixture)?;
        self.fixture.worker.borrow_mut().finish()?;
        std::fs::rename(
            source,
            self.fixture._roots.library().join("Volume déconnecté"),
        )?;
        self.fixture.restart(ui)
    }
}
