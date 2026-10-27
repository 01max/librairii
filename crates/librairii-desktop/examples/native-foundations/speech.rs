//! Shown production workflows on explicitly owned fixture roots. The external
//! reader drives real keyboard input; this module never fabricates AT-SPI or speech.
use super::*;
use std::path::Path;

enum Surface {
    Filters(FilterSurface),
    Details(DetailSurface),
    Tags(TagSurface),
    Flags(FlagSurface),
    Collections(CollectionSurface),
    Shelves(ShelfSurface),
    Review(ReviewSurface),
    Operations(OperationSurface),
    Sync(SyncSurface),
    Exports(ExportSurface),
    History(HistorySurface),
    Maintenance(MaintenanceSurface),
}
impl Surface {
    fn fixture(&self) -> &Fixture {
        match self {
            Self::Filters(s) => &s.0,
            Self::Details(s) => &s.0,
            Self::Tags(s) => &s.0,
            Self::Flags(s) => &s.0,
            Self::Collections(s) => &s.0,
            Self::Shelves(s) => &s.0,
            Self::Review(s) => &s.0,
            Self::Operations(s) => &s.fixture,
            Self::Sync(s) => &s.fixture,
            Self::Exports(s) => &s.fixture,
            Self::History(s) => &s.fixture,
            Self::Maintenance(s) => &s.fixture,
        }
    }
    fn open(
        ui: &DesktopWindow,
        menus: &librairii_desktop::menus::Menus,
        case: &str,
        code: &str,
    ) -> Result<Self> {
        let surface = match case {
            "browse" | "filters" => {
                let s = FilterSurface::open(ui)?;
                if case == "browse" {
                    ui.invoke_close_dialog();
                }
                Self::Filters(s)
            }
            "details" | "removal" => {
                let s = DetailSurface::open(ui)?;
                s.begin_details(ui)?;
                if case == "removal" {
                    ui.invoke_begin_removal();
                    check(
                        ui.get_dialog() == 9,
                        "spoken removal did not freeze its confirmation",
                    )?;
                }
                Self::Details(s)
            }
            "tag-manager" | "tag-assignment" | "tag-delete" => {
                let s = TagSurface::open(ui, menus)?;
                if case == "tag-assignment" {
                    s.assignment(ui, code)?;
                }
                if case == "tag-delete" {
                    s.confirm(ui)?;
                }
                Self::Tags(s)
            }
            "flag-bulk" => {
                let s = FlagSurface::open(ui)?;
                s.toggle(ui)?;
                ui.invoke_close_dialog();
                for index in 0..3 {
                    ui.invoke_toggle_bulk(index);
                }
                check(
                    ui.get_bulk_flag_mixed(),
                    "spoken batch fixture is not Mixed",
                )?;
                Self::Flags(s)
            }
            "collections" | "collection-union" | "collection-impact" => {
                let s = CollectionSurface::open(ui)?;
                if case != "collections" {
                    ui.invoke_select_collection(8);
                    s.0.settle(ui)?;
                    ui.invoke_toggle_selected_collection_union();
                    s.0.settle(ui)?;
                }
                if case == "collection-impact" {
                    s.impact(ui)?;
                }
                Self::Collections(s)
            }
            "shelf-members" | "shelf-create" | "shelf-exclusions" => {
                let s = ShelfSurface::open(ui)?;
                if case == "shelf-members" {
                    s.0.select(ui, 1)?;
                }
                if case == "shelf-create" {
                    s.creation(ui)?;
                }
                if case == "shelf-exclusions" {
                    s.exclusions(ui)?;
                }
                Self::Shelves(s)
            }
            "review-additions" | "review-removals" | "review-delete" => {
                let s = ReviewSurface::open(ui)?;
                if case == "review-removals" {
                    ui.invoke_review_section(1);
                    s.0.settle(ui)?;
                }
                if case == "review-delete" {
                    ui.invoke_show_panel(9);
                    ui.invoke_select_collection(1);
                    s.0.settle(ui)?;
                    ui.invoke_delete_collection();
                    s.0.settle(ui)?;
                    ui.invoke_review_delete_collection();
                    s.0.settle(ui)?;
                    check(
                        ui.get_review_shown() && !ui.get_modal(),
                        "spoken linked deletion review missing",
                    )?;
                }
                Self::Review(s)
            }
            "import" => Self::Operations(OperationSurface::open(ui)?),
            "sync" => {
                let s = SyncSurface::open(ui)?;
                s.success(ui)?;
                Self::Sync(s)
            }
            "export" | "export-report" => {
                let s = ExportSurface::open(ui)?;
                if case == "export-report" {
                    s.finish_export(ui)?;
                }
                Self::Exports(s)
            }
            "history" | "history-error" => {
                let s = HistorySurface::open(ui)?;
                if case == "history-error" {
                    s.report(ui)?;
                    s.french_error(ui)?;
                }
                Self::History(s)
            }
            "setup" | "settings" | "relocation" | "recovery" | "schema" => {
                Self::Maintenance(MaintenanceSurface::speech(ui, case, code)?)
            }
            _ => return Err(format!("unknown bounded native workflow case: {case}").into()),
        };
        ui.invoke_change_language(code.into());
        surface.fixture().settle(ui)?;
        // Give dismissible workflow dialogs one explicit, enabled invoker.
        // The external reader must prove Escape returns to that same control.
        if matches!(case, "filters" | "collection-union" | "settings") {
            let dialog = ui.get_dialog();
            ui.invoke_close_dialog();
            check(ui.get_dialog() == 0, "spoken dialog could not close")?;
            ui.invoke_focus_search();
            check(ui.get_focused_key() == 30, "spoken dialog invoker missing")?;
            ui.invoke_open_dialog(dialog);
        }
        Ok(surface)
    }
    fn poll(&self, ui: &DesktopWindow) -> Result {
        let f = self.fixture();
        if let Some(snapshot) = f.worker.borrow().take_snapshot() {
            let revision = f.worker.borrow().revision();
            f.view.borrow_mut().accept(ui, snapshot, revision);
        }
        f.view.borrow_mut().resize(ui);
        Ok(())
    }
}
fn receipt(ui: &DesktopWindow, surface: &Surface) -> serde_json::Value {
    let f = surface.fixture();
    let view = f.view.borrow();
    let workspace = view.snapshot.workflows.tags.workspace.as_ref();
    let assignment = match &view.snapshot.workflows.tags.feedback {
        Some(librairii_desktop::workflows::tags::Feedback::Assignment(result)) => Some(
            serde_json::json!({"requested":result.requested_stories,"changed":result.changed_stories,"added":result.assignments_added,"removed":result.assignments_removed}),
        ),
        _ => None,
    };
    let settings = &view.snapshot.workflows.settings;
    serde_json::json!({"revision":view.snapshot.revision, "offered_revision":f.worker.borrow().revision(), "phase":format!("{:?}",view.snapshot.phase),
        "dialog":ui.get_dialog(), "french":ui.get_french(), "query":ui.get_query().as_str(),
        "focused_key":ui.get_focused_key(),
        "selected_uuid":view.selected,
        "member_order":view.snapshot.page.as_ref().map(|p|p.stories.iter().map(|s|s.uuid.as_str()).collect::<Vec<_>>()),
        "assignment_requested":workspace.map(|w|w.requested_stories),
        "bedtime_assigned":workspace.and_then(|w|w.states.iter().find(|s|s.definition_id==101).map(|s|s.assigned_stories)),
        "assignment_result":assignment,
        "mixed":ui.get_bulk_flag_mixed(), "broken":ui.get_bulk_flag_checked(),
        "flag_pending":ui.get_flag_pending(), "undo":ui.get_flag_undo_enabled(),
        "flag_summary":ui.get_bulk_flag_summary().as_str(),
        "technical":ui.get_selected_story().technical.as_str(),"technical_open":ui.get_details_technical_open(),
        "history_error":ui.get_history_error().as_str(),
        "export_start":ui.get_export_data().can_start,
        "settings_busy":ui.get_settings_data().busy,
        "settings_notice":ui.get_settings_data().notice.as_str(),
        "settings_current":settings.current,"settings_proposed":settings.proposed,
        "settings_activation":settings.activation,"settings_result":settings.notice.map(|n|format!("{n:?}")),"settings_error":settings.error,
        "schema":ui.get_settings_data().can_preserve,
            "review_shown":ui.get_review_shown(),
        "page_total":view.snapshot.page.as_ref().map(|p|p.total)})
}
pub fn run(
    ui: &DesktopWindow,
    menus: &librairii_desktop::menus::Menus,
    output: &Path,
    case: &str,
    code: &str,
) -> Result {
    Language::explicit(code)?;
    let surface = Rc::new(Surface::open(ui, menus, case, code)?);
    let weak = ui.as_weak();
    ui.on_quit(move || {
        if let Some(ui) = weak.upgrade() {
            let _hidden = ui.hide();
        }
        let _quit = slint::quit_event_loop();
    });
    let error = Rc::new(RefCell::new(None));
    let errors = error.clone();
    let weak = ui.as_weak();
    let state = surface.clone();
    let destination = output.to_owned();
    let mut previous = None;
    let started = Instant::now();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(50),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let outcome = (|| -> Result {
                state.poll(&ui)?;
                let value = receipt(&ui, &state);
                if previous.as_ref() != Some(&value) {
                    std::fs::write(
                        destination.join("state.tmp"),
                        serde_json::to_vec_pretty(&value)?,
                    )?;
                    std::fs::rename(
                        destination.join("state.tmp"),
                        destination.join("state.json"),
                    )?;
                    previous = Some(value);
                }
                let stop = destination.join("finish");
                if stop.exists() {
                    check(
                        std::fs::read(stop)? == b"workflow-finish-v1\n",
                        "invalid owned fixture finish request",
                    )?;
                    ui.invoke_quit();
                }
                check(
                    started.elapsed() < Duration::from_secs(180),
                    "native spoken case exceeded its deadline",
                )
            })();
            if let Err(e) = outcome {
                *errors.borrow_mut() = Some(e.to_string());
                ui.invoke_quit();
            }
        },
    );
    ui.run()?;
    timer.stop();
    drop(timer);
    surface.fixture().worker.borrow_mut().finish()?;
    if let Some(error) = error.borrow_mut().take() {
        return Err(error.into());
    }
    std::fs::write(
        output.join("closed.json"),
        b"{\"actual_worker_joined\":true}\n",
    )?;
    Ok(())
}
