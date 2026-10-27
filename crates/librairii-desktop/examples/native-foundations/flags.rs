//! Real controller/SQLite flag checks and shown controls, using owned fixtures.
use super::{Fixture, Result, check};
use librairii_desktop::presentation::{Command, Snapshot};
use librairii_ui::native::DesktopWindow;

fn states(fixture: &Fixture) -> Result<Vec<bool>> {
    let connection =
        rusqlite::Connection::open(fixture._roots.library().join("native/db/librairii.sqlite3"))?;
    Ok(connection
        .prepare("SELECT is_broken FROM stories ORDER BY id")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}
fn stable(
    fixture: &Fixture,
    before: &Snapshot,
    after: &Snapshot,
    selection: &[String],
    inspected: &Option<String>,
) -> Result {
    let old = before.page.as_ref().ok_or("page missing")?;
    let new = after.page.as_ref().ok_or("page missing")?;
    check(
        old.total == new.total
            && old.page == new.page
            && old.pages == new.pages
            && old
                .stories
                .iter()
                .map(|s| &s.uuid)
                .eq(new.stories.iter().map(|s| &s.uuid))
            && before.navigation == after.navigation,
        "flagging changed result membership, ordering, paging or navigation",
    )?;
    let view = fixture.view.borrow();
    check(
        view.tags.selection == selection && &view.selected == inspected,
        "flagging changed selection or inspection",
    )
}
pub fn checks(ui: &DesktopWindow) -> Result {
    for _ in 0..3 {
        if ui.get_dialog() != 0 {
            ui.invoke_close_dialog();
        }
    }
    check(ui.get_dialog() == 0, "prior native modal did not settle")?;
    let fixture = Fixture::open(ui)?;
    check(
        ui.get_flag_notice().is_empty() && !ui.get_flag_undo_enabled(),
        "flag feedback appeared before a toggle",
    )?;
    let before = fixture.view.borrow().snapshot.clone();
    let inspected = fixture.view.borrow().selected.clone();
    ui.invoke_detail_flag_change(true);
    check(
        ui.get_flag_pending() && !ui.get_flag_undo_enabled(),
        "flag admission did not publish pending",
    )?;
    let first = fixture.settle(ui)?;
    stable(&fixture, &before, &first, &[], &inspected)?;
    check(
        first
            .workflows
            .flags
            .result
            .as_ref()
            .is_some_and(|r| r.as_ref().is_ok_and(|r| r.changed_stories == 1))
            && ui.get_flag_undo_enabled()
            && ui.get_detail_flag_checked(),
        "single flag did not offer committed Undo",
    )?;
    ui.invoke_detail_flag_change(true);
    fixture.settle(ui)?;
    check(
        ui.get_flag_notice().contains("No Broken") && !ui.get_flag_undo_enabled(),
        "no-op offered a false Undo",
    )?;
    for index in 0..3 {
        ui.invoke_toggle_bulk(index);
    }
    let selection = fixture.view.borrow().tags.selection.clone();
    let before = fixture.view.borrow().snapshot.clone();
    ui.invoke_bulk_flag_change(true);
    let bulk = fixture.settle(ui)?;
    stable(&fixture, &before, &bulk, &selection, &inspected)?;
    check(
        bulk.workflows.flags.result.as_ref().is_some_and(|r| {
            r.as_ref()
                .is_ok_and(|r| r.requested_stories == 3 && r.changed_stories == 2)
        }) && states(&fixture)?[..3] == [true, true, true],
        "mixed bulk did not retain changed-only Undo",
    )?;
    ui.invoke_undo_flags();
    let restored = fixture.settle(ui)?;
    stable(&fixture, &before, &restored, &selection, &inspected)?;
    check(
        states(&fixture)?[..3] == [true, false, false]
            && !ui.get_flag_undo_enabled()
            && ui.get_flag_notice().contains("restored"),
        "Undo changed an originally flagged story or offered redo",
    )?;
    ui.invoke_bulk_flag_change(true);
    fixture.settle(ui)?;
    let connection =
        rusqlite::Connection::open(fixture._roots.library().join("native/db/librairii.sqlite3"))?;
    connection.execute("UPDATE stories SET is_broken=0 WHERE id=3", [])?;
    ui.invoke_undo_flags();
    let stale = fixture.settle(ui)?;
    stable(&fixture, &before, &stale, &selection, &inspected)?;
    check(
        states(&fixture)?[..3] == [true, true, false]
            && !ui.get_flag_undo_enabled()
            && ui.get_flag_notice().contains("Nothing was restored"),
        "stale Undo partially restored a batch",
    )?;
    connection.execute_batch("CREATE TRIGGER fail_native_flag BEFORE UPDATE OF is_broken ON stories WHEN NEW.id=2 BEGIN SELECT RAISE(ABORT,'owned native flag failure'); END;")?;
    ui.invoke_bulk_flag_change(false);
    fixture.settle(ui)?;
    check(
        states(&fixture)?[..3] == [true, true, false]
            && !ui.get_flag_undo_enabled()
            && ui.get_flag_notice().contains("could not"),
        "failed write changed flags or offered Undo",
    )?;
    connection.execute_batch("DROP TRIGGER fail_native_flag;")?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    ui.invoke_detail_flag_change(false);
    ui.invoke_close_dialog();
    fixture.settle(ui)?;
    check(
        ui.get_flag_notice().is_empty() && !ui.get_flag_undo_enabled() && !states(&fixture)?[0],
        "closing details retained/resurrected pending flag feedback",
    )?;
    fixture.worker.borrow().offer(Command::Refresh)?;
    fixture.settle(ui)?;
    check(
        ui.get_flag_notice().is_empty(),
        "refresh resurrected dismissed flag feedback",
    )?;
    ui.invoke_open_dialog(3);
    fixture.settle(ui)?;
    ui.invoke_detail_flag_change(true);
    fixture.settle(ui)?;
    ui.invoke_close_dialog();
    // No intervening response/project may consume dismissal of the OLD action.
    ui.invoke_panel_flag_change(false);
    fixture.settle(ui)?;
    check(
        !states(&fixture)?[0] && ui.get_flag_undo_enabled() && !ui.get_flag_notice().is_empty(),
        "closing details suppressed the next panel action's committed feedback",
    )?;
    ui.invoke_dismiss_flag_feedback();
    let mut targets = super::view::flags::targets(&fixture.view.borrow(), false)
        .ok_or("single target missing")?;
    targets.stories.push(targets.stories[0].clone());
    fixture.worker.borrow().offer(Command::Flags {
        generation: targets.generation.clone(),
        action: librairii_desktop::workflows::flags::Action::Set {
            targets,
            broken: true,
        },
    })?;
    let duplicate = fixture.settle(ui)?;
    let ticket = duplicate
        .workflows
        .flags
        .undo
        .clone()
        .ok_or("deduplicated flag Undo missing")?;
    fixture.worker.borrow().offer(Command::Flags {
        generation: duplicate.generation.ok_or("generation missing")?,
        action: librairii_desktop::workflows::flags::Action::Undo(ticket),
    })?;
    fixture.settle(ui)?;
    check(
        !states(&fixture)?[0],
        "duplicate frozen identities made committed Undo unusable",
    )?;
    // A missing approved UUID is retained, counted and not offered mutation.
    connection.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(1,'99999999-0000-4000-8000-000000000099',60,'Unavailable member')",[])?;
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
    fixture
        .worker
        .borrow()
        .offer(Command::Request(librairii_core::query::QueryRequest {
            page: 3,
            page_size: 100,
            ..Default::default()
        }))?;
    let shelf = fixture.settle(ui)?;
    let index = shelf
        .page
        .as_ref()
        .ok_or("Shelf page missing")?
        .stories
        .iter()
        .position(|s| s.summary.is_none())
        .ok_or("unavailable member missing")?;
    ui.invoke_clear_bulk();
    ui.invoke_toggle_bulk(index as i32);
    check(
        !ui.get_bulk_flag_enabled() && ui.get_bulk_flag_summary().contains("Unavailable"),
        "unavailable bulk target offered flagging",
    )?;
    fixture.finish()
}

pub struct Surface(Fixture);
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlagSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let fixture = Fixture::open(ui)?;
        ui.invoke_open_dialog(3);
        fixture.settle(ui)?;
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
    pub fn toggle(&self, ui: &DesktopWindow) -> Result {
        check(ui.get_close_focused(), "details did not begin on Close")?;
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_detail_flag_focused(),
            "detail flag did not receive keyboard focus",
        )?;
        super::super::key(ui, slint::platform::Key::Space.into())?;
        check(
            ui.get_flag_pending(),
            "native detail Space did not reach flag controller",
        )?;
        self.0.settle(ui)?;
        check(
            ui.get_detail_flag_checked()
                && ui.get_flag_undo_enabled()
                && ui.get_detail_flag_focused(),
            "flag completion changed native focus or lost Undo",
        )?;
        Ok(())
    }
    pub fn french_bulk(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        for index in 0..3 {
            ui.invoke_toggle_bulk(index);
        }
        ui.invoke_focus_bulk_flag();
        super::super::key(ui, slint::platform::Key::Space.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_bulk_flag_checked() && ui.get_flag_undo_enabled(),
            "French bulk control did not reach controller",
        )?;
        Ok(())
    }
    pub fn panel(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_clear_bulk();
        ui.invoke_focus_panel_flag();
        super::super::key(ui, slint::platform::Key::Space.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_panel_flag_focused() && !ui.get_panel_flag_checked(),
            "panel Space lost focus or targeted the wrong story",
        )?;
        Ok(())
    }
    pub fn focus_undo(&self, ui: &DesktopWindow) -> Result {
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_bulk_flag_undo_focused(),
            "bulk Undo was not reachable by Tab",
        )
    }
    pub fn undo(&self, ui: &DesktopWindow) -> Result {
        super::super::key(ui, slint::platform::Key::Space.into())?;
        self.0.settle(ui)?;
        check(
            states(&self.0)?[..3] == [true, false, false] && !ui.get_flag_undo_enabled(),
            "native Undo did not restore only changed targets",
        )
    }
}
