//! Approved Shelf curation through the production native controller callbacks.
use super::{Fixture, Result, check};
use librairii_core::{
    ShelfId,
    collection_codec::{Query, url::Scope},
    query::QueryRequest,
};
use librairii_desktop::{
    presentation::{Command, Snapshot},
    workflows::shelves::Feedback,
};
use librairii_ui::native::DesktopWindow;
use slint::Model;
fn actor(f: &Fixture) -> Result<rusqlite::Connection> {
    Ok(rusqlite::Connection::open(
        f._roots.library().join("native/db/librairii.sqlite3"),
    )?)
}
fn saved(s: &Snapshot) -> Result<ShelfId> {
    match s.workflows.shelves.feedback {
        Some(Feedback::Saved(id)) => Ok(id),
        _ => Err(format!("Shelf save failed: {:?}", s.workflows.shelves.feedback).into()),
    }
}
fn choose(f: &Fixture, ui: &DesktopWindow, id: ShelfId) -> Result {
    let index = f
        .view
        .borrow()
        .snapshot
        .groups
        .iter()
        .filter(|g| matches!(g.scope, Scope::Shelf { .. }))
        .position(|g| matches!(g.scope,Scope::Shelf{id:i,..} if i==id))
        .ok_or("Shelf missing")?;
    ui.invoke_select_shelf(index as i32 + 1);
    f.settle(ui)?;
    Ok(())
}
fn preview(f: &Fixture, ui: &DesktopWindow, name: &str) -> Result {
    ui.invoke_show_panel(10);
    ui.invoke_select_shelf(0);
    ui.set_shelf_name(name.into());
    ui.invoke_preview_shelf_creation();
    f.settle(ui)?;
    Ok(())
}
fn order(c: &rusqlite::Connection, id: ShelfId) -> Result<Vec<String>> {
    Ok(c.prepare(
        "SELECT story_uuid FROM shelf_stories WHERE shelf_id=? ORDER BY position,story_uuid",
    )?
    .query_map([id.get()], |r| r.get(0))?
    .collect::<rusqlite::Result<_>>()?)
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let mut f = Fixture::open(ui)?;
    let mut c = actor(&f)?;
    // Definition namespaces are independent; no selection is silently approved.
    preview(&f, ui, "Reverse")?;
    check(
        ui.get_dialog() == 16
            && ui.get_shelf_cancel_focused()
            && ui.get_shelf_confirm_label() == "Create 0",
        "creation did not start with explicit zero choices and Cancel focus",
    )?;
    ui.invoke_confirm_shelf_preview();
    f.settle(ui)?;
    check(
        ui.get_shelf_notice().contains("already exists") && !ui.get_shelf_confirm_enabled(),
        "duplicate-name create reused a consumed plan",
    )?;
    ui.invoke_close_dialog();
    ui.set_shelf_name("Empty".into());
    ui.invoke_preview_shelf_creation();
    f.settle(ui)?;
    ui.invoke_confirm_shelf_preview();
    let empty = saved(&f.settle(ui)?)?;
    check(
        ui.get_workspace() && ui.get_story_count() == 0,
        "empty Shelf did not open approved workspace",
    )?;
    // Select a manual target and a live source which also contains it.
    f.worker.borrow().offer(Command::Navigate(Scope::Library))?;
    f.settle(ui)?;
    f.select(ui, 0)?;
    ui.invoke_toggle_bulk(0);
    let mut criteria = Query::default();
    criteria.name = "Alpha".into();
    let tx = c.transaction()?;
    librairii_storage::collections::execute(
        &tx,
        librairii_storage::collections::Command::Create {
            name: "Empty".into(),
            criteria,
        },
    )?;
    tx.commit()?;
    f.worker.borrow().offer(Command::Refresh)?;
    f.settle(ui)?;
    ui.invoke_show_panel(10);
    ui.invoke_select_shelf(0);
    ui.set_shelf_name("Approved".into());
    ui.invoke_toggle_shelf_source();
    ui.invoke_preview_shelf_creation();
    let initial = f.settle(ui)?;
    let p = initial
        .workflows
        .shelves
        .preview
        .as_ref()
        .ok_or("initial preview missing")?;
    check(
        p.total == 30
            && p.selected == 0
            && p.rows.len() == 25
            && p.rows[0].manual
            && p.rows[0].sources.len() == 1,
        "manual/source candidates were not deduplicated or started approved",
    )?;
    ui.invoke_shelf_preview_toggle(0);
    f.settle(ui)?;
    ui.invoke_shelf_preview_page(1);
    let p = f.settle(ui)?;
    check(
        p.workflows
            .shelves
            .preview
            .as_ref()
            .is_some_and(|p| p.rows.len() == 5 && p.selected == 1),
        "creation paging lost explicit choices",
    )?;
    // A stale source invalidates the whole plan and requires fresh approvals.
    c.execute(
        "UPDATE collections SET name='Alpha changed' WHERE name='Empty'",
        [],
    )?;
    ui.invoke_confirm_shelf_preview();
    f.settle(ui)?;
    check(
        ui.get_shelf_notice().contains("preview changed") && !ui.get_shelf_confirm_enabled(),
        "stale creation was accepted",
    )?;
    ui.invoke_refresh_shelf_preview();
    let renewed = f.settle(ui)?;
    check(
        renewed
            .workflows
            .shelves
            .preview
            .as_ref()
            .is_some_and(|p| p.selected == 0)
            && ui.get_shelf_name() == "Approved",
        "preview refresh retained approvals or lost draft",
    )?;
    ui.invoke_shelf_preview_all(true);
    f.settle(ui)?;
    ui.invoke_confirm_shelf_preview();
    let approved = saved(&f.settle(ui)?)?;
    check(
        ui.get_workspace() && ui.get_story_count() == 24 && ui.get_shelf_active(),
        "created Shelf did not open bounded approved workspace",
    )?;
    let original = order(&c, approved)?;
    // Temporary view changes never rewrite stored order. Moves use the complete
    // membership, including gaps and members across a visible page boundary.
    c.execute(
        "UPDATE shelf_stories SET position=position+1000 WHERE shelf_id=?",
        [approved.get()],
    )?;
    c.execute(
        "UPDATE shelf_stories SET position=(position-1000)*7 WHERE shelf_id=?",
        [approved.get()],
    )?;
    f.worker.borrow().offer(Command::Request(QueryRequest {
        page: 2,
        page_size: 24,
        ..Default::default()
    }))?;
    f.settle(ui)?;
    f.select(ui, 0)?;
    let moved = f
        .view
        .borrow()
        .selected
        .clone()
        .ok_or("member selection missing")?;
    check(
        ui.get_shelf_member_up() && ui.get_shelf_member_down(),
        "gapped ordinal boundaries disabled a legal move",
    )?;
    ui.invoke_shelf_move_member(-1);
    f.settle(ui)?;
    let ordered = order(&c, approved)?;
    check(
        ordered[23] == moved
            && ui.get_selected_story().uuid == moved
            && ui.get_selected_index() == 23,
        "full-order move lost its UUID across the page boundary",
    )?;
    f.search(ui, "Alpha")?;
    let before = order(&c, approved)?;
    ui.invoke_shelf_move_member(1);
    check(
        !ui.get_shelf_member_up() && order(&c, approved)? == before,
        "refinement enabled stored-order edits",
    )?;
    ui.invoke_clear_shelf_refinement();
    f.settle(ui)?;
    check(
        ui.get_query().is_empty() && !ui.get_shelf_refined(),
        "reset did not restore unfiltered manual order",
    )?;
    // Ordinary removal differs from explicit remove-and-exclude.
    f.select(ui, 0)?;
    let removed = f
        .view
        .borrow()
        .selected
        .clone()
        .ok_or("selection missing")?;
    ui.invoke_shelf_remove_members(false);
    f.settle(ui)?;
    check(
        ui.get_shelf_background_blocked(),
        "toolbar removal left background controls enabled",
    )?;
    ui.invoke_close_dialog();
    check(
        order(&c, approved)? == before,
        "cancelled removal changed membership",
    )?;
    ui.invoke_shelf_remove_members(false);
    f.settle(ui)?;
    ui.invoke_confirm_shelf_preview();
    f.settle(ui)?;
    let exclusions: i64 = c.query_row(
        "SELECT count(*) FROM shelf_exclusions WHERE shelf_id=?",
        [approved.get()],
        |r| r.get(0),
    )?;
    check(
        exclusions == 0 && !order(&c, approved)?.contains(&removed),
        "ordinary removal silently excluded a UUID",
    )?;
    f.select(ui, 0)?;
    let excluded = f
        .view
        .borrow()
        .selected
        .clone()
        .ok_or("selection missing")?;
    ui.invoke_shelf_remove_members(true);
    f.settle(ui)?;
    ui.invoke_confirm_shelf_preview();
    f.settle(ui)?;
    ui.invoke_show_shelf_exclusions();
    f.settle(ui)?;
    check(
        ui.get_shelf_background_blocked(),
        "toolbar exclusions left background controls enabled",
    )?;
    check(
        ui.get_shelf_exclusion_rows().row_count() == 1,
        "exclusion browser omitted decision",
    )?;
    ui.invoke_allow_shelf_suggestions(0);
    f.settle(ui)?;
    check(
        ui.get_shelf_exclusion_rows().row_count() == 0 && !order(&c, approved)?.contains(&excluded),
        "clear exclusion silently added membership",
    )?;
    ui.invoke_close_dialog();
    // Manual add is idempotent, clears exclusion, and is distinct from new matches.
    f.worker.borrow().offer(Command::Navigate(Scope::Library))?;
    f.settle(ui)?;
    f.select(ui, 0)?;
    ui.invoke_clear_bulk();
    ui.invoke_show_panel(10);
    choose(&f, ui, approved)?;
    ui.invoke_add_manual_shelf();
    saved(&f.settle(ui)?)?;
    let count = order(&c, approved)?.len();
    ui.invoke_add_manual_shelf();
    f.settle(ui)?;
    check(
        order(&c, approved)?.len() == count,
        "manual add duplicated UUID",
    )?;
    ui.invoke_close_dialog();
    // Unavailable UUIDs retain cached titles, origins and order, and can be removed.
    c.execute("DELETE FROM stories WHERE uuid=?", [&original[2]])?;
    f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
        id: approved,
        manual_order: true,
    }))?;
    let snap = f.settle(ui)?;
    let missing = snap
        .page
        .as_ref()
        .ok_or("Shelf page missing")?
        .stories
        .iter()
        .position(|s| s.uuid == original[2])
        .ok_or("unavailable member missing")?;
    f.select(ui, missing as i32)?;
    check(
        ui.get_selected_story()
            .member_status
            .contains("Unavailable")
            && ui.get_selected_story().matches.contains("unknown")
            && !ui.get_panel_flag_enabled()
            && ui.get_shelf_member_removable(),
        "unavailable member lost identity/context or enabled mutating archive controls",
    )?;
    ui.invoke_show_panel(10);
    choose(&f, ui, approved)?;
    ui.set_shelf_name("Copy".into());
    ui.invoke_duplicate_shelf();
    f.settle(ui)?;
    check(
        ui.get_shelf_preview_summary().contains("unavailable UUIDs"),
        "duplicate confirmation omitted unavailable state",
    )?;
    ui.invoke_confirm_shelf_preview();
    let copy = saved(&f.settle(ui)?)?;
    check(
        order(&c, copy)? == order(&c, approved)?,
        "duplicate evaluated sources instead of copying approved order",
    )?;
    ui.invoke_show_panel(10);
    choose(&f, ui, copy)?;
    ui.set_shelf_name("Copie indépendante".into());
    ui.invoke_rename_shelf();
    f.settle(ui)?;
    ui.invoke_move_shelf(-1);
    f.settle(ui)?;
    // Same UUID reconnects independently of its old numeric row ID.
    c.execute("INSERT INTO stories(id,uuid,embedded_title,display_name_normalized) VALUES(400,?,'Returned Alpha','returned alpha')",[&original[2]])?;
    c.execute("INSERT INTO story_archives(story_id,original_filename,detected_format,sha256,byte_size,managed_path,validation_state) VALUES(400,'returned.zip','zip',printf('%064d',400),1,'archives/400','valid')",[])?;
    f.worker.borrow().offer(Command::Refresh)?;
    let reconnected = f.settle(ui)?;
    check(
        reconnected.page.as_ref().is_some_and(|p| {
            p.stories
                .iter()
                .any(|s| s.uuid == original[2] && s.summary.as_ref().is_some_and(|s| s.id == 400))
        }),
        "same UUID reimport did not reconnect approved member",
    )?;
    // Exclusions have independent 25-row paging and clamp after clearing the last row.
    for uuid in original.iter().take(26) {
        c.execute(
            "INSERT INTO shelf_exclusions(shelf_id,story_uuid) VALUES(?,?)",
            (empty.get(), uuid),
        )?;
    }
    ui.invoke_show_panel(10);
    choose(&f, ui, empty)?;
    ui.invoke_show_shelf_exclusions();
    f.settle(ui)?;
    check(
        ui.get_shelf_exclusion_rows().row_count() == 25 && ui.get_shelf_exclusion_next(),
        "exclusions were unbounded",
    )?;
    ui.invoke_shelf_exclusion_page(1);
    f.settle(ui)?;
    check(
        ui.get_shelf_exclusion_rows().row_count() == 1,
        "last exclusion page missing",
    )?;
    ui.invoke_allow_shelf_suggestions(0);
    let clamped = f.settle(ui)?;
    check(
        clamped
            .workflows
            .shelves
            .exclusions
            .as_ref()
            .is_some_and(|e| e.page == 0 && e.rows.len() == 25 && e.total == 25)
            && order(&c, empty)?.is_empty(),
        "clearing last exclusion page lost remaining rows or approved a story",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_close_dialog();
    drop(c);
    f.restart(ui)?;
    check(
        f.view
            .borrow()
            .snapshot
            .groups
            .iter()
            .any(|g| g.name == "Copie indépendante")
            && f.view
                .borrow()
                .snapshot
                .groups
                .iter()
                .any(|g| matches!(g.scope,Scope::Shelf{id,..} if id==empty)),
        "Shelf definitions did not survive restart",
    )?;
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    f.finish()?;
    recovery(ui)
}
fn recovery(ui: &DesktopWindow) -> Result {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::open_with(ui, true)?;
    f.search(ui, "Clockwork")?;
    f.select(ui, 0)?;
    ui.invoke_open_dialog(3);
    f.settle(ui)?;
    ui.invoke_begin_removal();
    ui.invoke_remove_story();
    f.settle(ui)?;
    preview(&f, ui, "Saved before recovery")?;
    let permissions = std::fs::metadata(&f.locations.native)?.permissions();
    std::fs::set_permissions(&f.locations.native, std::fs::Permissions::from_mode(0o500))?;
    ui.invoke_confirm_shelf_preview();
    let result = f.settle(ui);
    std::fs::set_permissions(&f.locations.native, permissions)?;
    let result = result?;
    check(
        result.phase == librairii_desktop::presentation::Phase::Recovery
            && matches!(result.workflows.shelves.feedback, Some(Feedback::Saved(_)))
            && ui.get_status().contains("Shelf was saved")
            && ui.get_status().contains("story was removed")
            && ui.get_dialog() == 0,
        "postcommit navigation failure hid confirmed Shelf save",
    )?;
    f.finish()
}
pub struct Surface(Fixture);
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShelfSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let f = Fixture::open(ui)?;
        f.worker.borrow().offer(Command::Presentation(
            librairii_core::navigation::Presentation::LargeCovers,
        ))?;
        f.settle(ui)?;
        f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
            id: ShelfId::new(1)?,
            manual_order: true,
        }))?;
        f.settle(ui)?;
        check(
            f.view.borrow().columns == 1
                && ui
                    .get_story_groups()
                    .iter()
                    .all(|g| g.cards.row_count() == 1),
            "Shelf accept retained cover-grid groups",
        )?;
        ui.invoke_change_language("en".into());
        f.settle(ui)?;
        ui.invoke_close_sidebar();
        ui.invoke_focus_browser();
        Ok(Self(f))
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(s) = self.0.worker.borrow().take_snapshot() {
            self.0
                .view
                .borrow_mut()
                .accept(ui, s, self.0.worker.borrow().revision());
        }
        self.0.view.borrow_mut().resize(ui);
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        check(
            ui.get_selected_story().member_status.contains("conservée") && ui.get_shelf_active(),
            "French membership context missing",
        )
    }
    pub fn bulk(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_focus_browser();
        super::super::key(ui, slint::platform::Key::Space.into())?;
        check(
            ui.get_observed_bulk_checked() == ui.get_bulk_flag_checked(),
            "native Broken checkbox disagrees with owned bulk state",
        )?;
        check(
            ui.get_observed_approved_height() >= 86.,
            &format!(
                "compact approved bulk viewport too small: {}",
                ui.get_observed_approved_height()
            ),
        )?;
        for _ in 0..150 {
            if ui.get_shelf_action_focused() {
                break;
            }
            super::super::key(ui, slint::platform::Key::Tab.into())?;
        }
        check(
            ui.get_shelf_action_focused()
                && ui.get_shelf_action_y() + 0.5 >= ui.get_controls_viewport_y()
                && ui.get_shelf_action_y() + ui.get_shelf_action_height()
                    <= ui.get_controls_viewport_y() + ui.get_controls_viewport_height() + 0.5,
            "native Shelf action was unreachable or outside bounded controls",
        )?;
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_shelf_run_focused(),
            "native Tab did not reach Shelf Run",
        )?;
        super::super::key(ui, slint::platform::Key::Space.into())?;
        check(
            ui.get_dialog() == 15,
            "native Shelf action did not open manager",
        )?;
        ui.invoke_close_dialog();
        check(
            ui.get_shelf_run_focused(),
            "manager Close lost exact toolbar invoker",
        )
    }
    pub fn clear_bulk(&self, ui: &DesktopWindow) {
        ui.invoke_clear_bulk();
    }
    pub fn creation(&self, ui: &DesktopWindow) -> Result {
        self.0.select(ui, 0)?;
        ui.invoke_show_panel(10);
        ui.invoke_select_shelf(0);
        ui.set_shelf_name("Étagère à garder · draft".into());
        ui.set_shelf_include_manual(true);
        ui.invoke_preview_shelf_creation();
        self.0.settle(ui)?;
        check(
            ui.get_shelf_cancel_focused() && ui.get_shelf_return_dialog() == 15,
            &format!(
                "shown creation did not focus Cancel/return manager: dialog={} return={}",
                ui.get_dialog(),
                ui.get_shelf_return_dialog()
            ),
        )
    }
    pub fn name(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        check(
            ui.get_dialog() == 15,
            &format!(
                "cancel did not return manager: dialog={} return={}",
                ui.get_dialog(),
                ui.get_shelf_return_dialog()
            ),
        )?;
        ui.invoke_focus_shelf_name();
        Ok(())
    }
    pub fn exclusions(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        self.0.select(ui, 0)?;
        ui.invoke_shelf_remove_members(true);
        self.0.settle(ui)?;
        ui.invoke_confirm_shelf_preview();
        self.0.settle(ui)?;
        ui.invoke_show_shelf_exclusions();
        self.0.settle(ui)?;
        Ok(())
    }
}
