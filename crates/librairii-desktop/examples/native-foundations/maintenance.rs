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
fn adoption_language_checks(ui: &DesktopWindow) -> Result {
    let mut f = setup(ui)?;
    language(&f, ui, "en")?;
    f.worker.borrow_mut().finish()?;
    let source = f._roots.library().join("Legacy French library");
    std::fs::create_dir(&source)?;
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/libraries/schema018");
    let manifest: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../preservation/corpus/profiles/library-manifest.json"
    ))?;
    let mut original = Vec::new();
    for entry in manifest.as_array().ok_or("legacy manifest")? {
        let name = entry["path"].as_str().ok_or("legacy member")?;
        if entry["directory"].as_bool() == Some(true) {
            std::fs::create_dir(source.join(name))?;
        } else {
            let bytes = std::fs::read(corpus.join(name))?;
            std::fs::write(source.join(name), &bytes)?;
            original.push((name.to_owned(), bytes));
        }
    }
    let legacy = f
        .locations
        .legacy
        .clone()
        .ok_or("legacy profile namespace")?;
    std::fs::create_dir_all(&legacy)?;
    let source_profile = serde_json::to_vec(&librairii_storage::profile::Profile {
        version: 1,
        language: "fr".into(),
        root: source.to_str().ok_or("legacy root")?.into(),
        library_id: "a0000000-0000-4000-8000-000000000001".into(),
        complete: true,
        recovering: false,
    })?;
    std::fs::write(legacy.join("settings.json"), &source_profile)?;
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Adoption
            && f.view.borrow().snapshot.language == Language::English
            && !ui.get_french(),
        "independent native preference lost to legacy adoption language",
    )?;
    let destination = f._roots.library().join("Independent adopted library");
    choose(&f, ui, Picker::Folder, Some(destination));
    f.settle(ui)?;
    ui.invoke_settings_confirm();
    let opened = f.settle(ui)?;
    check(
        opened.phase == Phase::Library && opened.language == Language::English && !ui.get_french(),
        "bound native library ignored its actual saved language",
    )?;
    f.id = opened
        .navigation
        .as_ref()
        .ok_or("adopted navigation")?
        .library();
    for (name, bytes) in original {
        check(
            std::fs::read(source.join(name))? == bytes,
            "native adoption changed legacy source bytes",
        )?;
    }
    check(
        std::fs::read(legacy.join("settings.json"))? == source_profile,
        "native adoption rewrote the legacy profile",
    )?;
    close(ui);
    f.restart(ui)?;
    check(
        !ui.get_french(),
        "native adoption restart lost the independent choice",
    )?;
    f.finish()
}

fn archive_payloads(
    root: &std::path::Path,
) -> Result<std::collections::BTreeMap<PathBuf, Vec<u8>>> {
    let mut pending = vec![root.to_owned()];
    let mut payloads = std::collections::BTreeMap::new();
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            } else {
                payloads.insert(
                    entry.path().strip_prefix(root)?.to_owned(),
                    std::fs::read(entry.path())?,
                );
            }
        }
    }
    Ok(payloads)
}
fn schema_checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let mut f = Fixture::open_with(ui, true)?;
    let root = selected_root(&f)?;
    f.worker.borrow_mut().finish()?;
    let original_archive = archive_payloads(&root.join("archives"))?;
    for (name, bytes) in [
        ("librairii.sqlite3", b"foreign native database".as_slice()),
        ("librairii.sqlite3-wal", b"foreign native WAL"),
        ("librairii.sqlite3-shm", b"foreign native SHM"),
    ] {
        std::fs::write(root.join("db").join(name), bytes)?;
    }
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Recovery
            && ui.get_settings_data().can_preserve
            && !ui.get_settings_data().busy,
        "foreign schema recovery is unreachable or mutation was silently admitted",
    )?;
    let revision = f.worker.borrow().revision();
    ui.invoke_show_schema_confirmation();
    check(
        ui.get_dialog() == 22 && ui.get_schema_keep_focused(),
        "foreign schema confirmation did not focus Keep",
    )?;
    super::super::key(ui, slint::platform::Key::Escape.into())?;
    check(
        f.worker.borrow().revision() == revision
            && ui.get_dialog() == 5
            && std::fs::read(root.join("db/librairii.sqlite3"))? == b"foreign native database",
        "schema cancellation mutated or submitted recovery",
    )?;
    language(&f, ui, "fr")?;
    ui.invoke_show_schema_confirmation();
    check(
        ui.get_settings_data()
            .schema_explanation
            .contains("base vide")
            && ui.get_schema_keep_focused(),
        &format!(
            "French schema recovery lost its consequence or safe initial focus: dialog={} keep={} french={} explanation={}",
            ui.get_dialog(),
            ui.get_schema_keep_focused(),
            ui.get_french(),
            ui.get_settings_data().schema_explanation
        ),
    )?;
    super::super::key(ui, slint::platform::Key::Tab.into())?;
    super::super::key(ui, slint::platform::Key::Space.into())?;
    let opened = f.settle(ui)?;
    check(
        opened.phase == Phase::Library
            && opened.language == Language::French
            && opened.workflows.settings.notice == Some(Notice::SchemaRecovered)
            && opened.page.as_ref().is_some_and(|p| p.total == 0),
        &format!(
            "confirmed foreign schema did not create an explicitly empty database: phase={:?} language={:?} total={:?} settings={:?}",
            opened.phase,
            opened.language,
            opened.page.as_ref().map(|p| p.total),
            opened.workflows.settings
        ),
    )?;
    let path = opened
        .workflows
        .settings
        .retained
        .first()
        .ok_or("schema preserved path missing")?;
    for (name, bytes) in [
        ("librairii.sqlite3", b"foreign native database".as_slice()),
        ("librairii.sqlite3-wal", b"foreign native WAL"),
        ("librairii.sqlite3-shm", b"foreign native SHM"),
    ] {
        check(
            std::fs::read(path.join(name))? == bytes,
            "schema recovery did not preserve the original family",
        )?;
    }
    check(
        archive_payloads(&root.join("archives"))? == original_archive,
        "schema recovery changed managed archive bytes",
    )?;
    // A compatible replacement without its final receipt is still an explicit
    // recovery, including when the preceding native close ACK was durable.
    // The storage crash test separately kills the actual process before that ACK.
    let preserved = path.clone();
    close(ui);
    f.worker.borrow_mut().finish()?;
    std::fs::remove_file(preserved.join("replacement-created.json"))?;
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Recovery
            && ui.get_settings_data().can_preserve
            && ui
                .get_settings_data()
                .schema_explanation
                .contains("compatible"),
        "compatible unfinished replacement bypassed explicit startup recovery",
    )?;
    ui.invoke_show_schema_confirmation();
    super::super::key(ui, slint::platform::Key::Tab.into())?;
    super::super::key(ui, slint::platform::Key::Space.into())?;
    check(
        f.settle(ui)?.phase == Phase::Library
            && preserved.join("replacement-created.json").is_file(),
        "explicit compatible replacement did not complete its durable receipt",
    )?;
    close(ui);
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.phase == Phase::Library && ui.get_french(),
        "recovered database or native language did not survive restart",
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
    eprintln!("maintenance: adoption language");
    adoption_language_checks(ui)?;
    eprintln!("maintenance: foreign schema");
    schema_checks(ui)?;
    eprintln!("maintenance: override");
    override_checks(ui)
}

pub struct Surface {
    pub(super) fixture: Fixture,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaintenanceSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn speech(ui: &DesktopWindow, case: &str, code: &str) -> Result<Self> {
        let mut surface = Self::open(ui)?;
        language(&surface.fixture, ui, code)?;
        match case {
            "setup" => {
                let root = surface
                    .fixture
                    ._roots
                    .library()
                    .join("Explicit speech fixture library");
                choose(&surface.fixture, ui, Picker::Folder, Some(root));
                surface.fixture.settle(ui)?;
            }
            "settings" | "relocation" | "recovery" => {
                surface.confirmed(ui)?;
                if case == "relocation" {
                    surface.preview(ui)?;
                }
                if case == "recovery" {
                    surface.recover(ui)?;
                }
            }
            "schema" => {
                close(ui);
                surface.fixture.finish()?;
                let mut fixture = Fixture::open_with(ui, true)?;
                language(&fixture, ui, code)?;
                let root = selected_root(&fixture)?;
                fixture.worker.borrow_mut().finish()?;
                for name in [
                    "librairii.sqlite3",
                    "librairii.sqlite3-wal",
                    "librairii.sqlite3-shm",
                ] {
                    std::fs::write(root.join("db").join(name), name.as_bytes())?;
                }
                fixture.restart(ui)?;
                check(
                    ui.get_dialog() == 5 && ui.get_settings_data().can_preserve,
                    "spoken schema recovery missing",
                )?;
                ui.invoke_show_schema_confirmation();
                check(ui.get_dialog() == 22, "spoken schema confirmation missing")?;
                return Ok(Self { fixture });
            }
            _ => return Err("unknown maintenance speech case".into()),
        }
        Ok(surface)
    }
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
