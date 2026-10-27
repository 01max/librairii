//! Explicit reviews through the actual worker, SQLite plans and native view.
use super::{Fixture, Result, check};
use librairii_core::{
    CollectionId, ShelfId,
    collection_codec::{Query, url::Scope},
};
use librairii_desktop::{
    presentation::{Command, Snapshot},
    workflows::{
        Targets, flags,
        shelf_reviews::{Feedback, Outcome, Preview},
    },
};
use librairii_ui::native::DesktopWindow;
use slint::Model;

fn actor(f: &Fixture) -> Result<rusqlite::Connection> {
    Ok(rusqlite::Connection::open(
        f._roots.library().join("native/db/librairii.sqlite3"),
    )?)
}
fn seed(f: &Fixture) -> Result {
    let mut c = actor(f)?;
    let tx = c.transaction()?;
    let mut criteria = Query::default();
    criteria.name = "Alpha".into();
    librairii_storage::collections::execute(
        &tx,
        librairii_storage::collections::Command::Create {
            name: "Alpha source".into(),
            criteria,
        },
    )?;
    tx.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(2,'Review','review',1); INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(2,1,'Alpha source'); INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title,protected) SELECT 2,uuid,id-31,embedded_title,0 FROM stories WHERE id>30; INSERT INTO shelf_story_origins(shelf_id,story_uuid,source_id) SELECT 2,story_uuid,(SELECT id FROM shelf_sources WHERE shelf_id=2) FROM shelf_stories WHERE shelf_id=2; UPDATE stories SET embedded_description='<p>Readable &amp; safe</p><script>hidden()</script>' WHERE id=1;")?;
    tx.commit()?;
    Ok(())
}
fn navigate(f: &Fixture, ui: &DesktopWindow) -> Result {
    f.worker.borrow().offer(Command::Refresh)?;
    f.settle(ui)?;
    f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
        id: ShelfId::new(2)?,
        manual_order: true,
    }))?;
    f.settle(ui)?;
    Ok(())
}
fn plan(s: &Snapshot) -> Result<&Preview> {
    s.workflows
        .shelf_reviews
        .preview
        .as_ref()
        .ok_or_else(|| "review missing".into())
}
fn review(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    ui.invoke_review_updates();
    f.settle(ui)
}
fn refresh(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    ui.invoke_review_refresh();
    f.settle(ui)
}
fn count(c: &rusqlite::Connection, sql: &str) -> Result<i64> {
    Ok(c.query_row(sql, [], |r| r.get(0))?)
}
fn close(ui: &DesktopWindow) {
    if ui.get_review_shown() {
        ui.invoke_close_native_review();
    }
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
}

pub fn checks(ui: &DesktopWindow) -> Result {
    close(ui);
    let mut f = Fixture::open(ui)?;
    seed(&f)?;
    let c = actor(&f)?;
    navigate(&f, ui)?;
    check(
        ui.get_review_shown() && ui.get_review_data().section == 1,
        "Shelf entry did not prompt for affected removals",
    )?;
    let first = review(&f, ui)?;
    let p = plan(&first)?;
    check(
        ui.get_review_shown()
            && !ui.get_modal()
            && p.additions == 30
            && p.removals == 30
            && p.rows.len() == 25
            && p.decisions == 0,
        "review did not separate approved membership and full bounded proposal totals",
    )?;
    let revision = count(&c, "SELECT revision FROM shelves WHERE id=2")?;
    ui.invoke_review_apply();
    let noop = f.settle(ui)?;
    check(
        matches!(
            noop.workflows.shelf_reviews.feedback,
            Some(Feedback::Committed(Outcome::ShelfSaved {
                changed: false,
                ..
            }))
        ) && count(&c, "SELECT revision FROM shelves WHERE id=2")? == revision,
        "deferred-only Apply changed Shelf state",
    )?;
    let opened = review(&f, ui)?;
    let first_uuid = plan(&opened)?.rows[0].uuid.clone();
    ui.invoke_review_choose(first_uuid.clone().into(), 1);
    f.settle(ui)?;
    ui.invoke_review_inspect(first_uuid.clone().into());
    f.settle(ui)?;
    check(
        ui.get_review_data().preview_description == "Readable & safe"
            && !ui.get_review_data().preview_description.contains("hidden"),
        "review preview did not use inert story details",
    )?;
    let generation = f
        .view
        .borrow()
        .snapshot
        .generation
        .clone()
        .ok_or("generation missing")?;
    f.worker.borrow().offer(Command::Flags {
        generation: generation.clone(),
        action: flags::Action::Set {
            targets: Targets {
                generation,
                stories: vec![librairii_core::browsing::StoryIdentity {
                    id: 1,
                    uuid: first_uuid.clone(),
                }],
            },
            broken: true,
        },
    })?;
    let flagged = f.settle(ui)?;
    check(
        !flagged.workflows.shelf_reviews.stale
            && plan(&flagged)?.decisions == 1
            && plan(&flagged)?.rows[0].broken
            && ui.get_review_data().preview_broken,
        "Broken-only edit invalidated decisions or left a stale warning",
    )?;
    ui.invoke_review_page(1);
    let second = f.settle(ui)?;
    check(
        plan(&second)?.rows.len() == 5 && plan(&second)?.decisions == 1,
        "review paging lost full-set decisions",
    )?;
    ui.invoke_review_choose(first_uuid.clone().into(), 2);
    let rejected = f.settle(ui)?;
    check(
        matches!(
            rejected.workflows.shelf_reviews.feedback,
            Some(Feedback::Error { .. })
        ) && plan(&rejected)?.decisions == 1,
        "old page callback retargeted another candidate",
    )?;
    check(
        plan(&refresh(&f, ui)?)?.decisions == 0,
        "refresh retained old approvals",
    )?;
    ui.invoke_review_choose(first_uuid.clone().into(), 1);
    f.settle(ui)?;
    c.execute(
        "UPDATE stories SET embedded_title='Alpha edited' WHERE id=1",
        [],
    )?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    check(
        ui.get_review_data().stale
            && !ui.get_review_data().can_apply
            && count(&c, "SELECT count(*) FROM shelf_stories WHERE shelf_id=2")? == 30,
        "stale reviewed metadata committed",
    )?;
    refresh(&f, ui)?;
    ui.invoke_review_bulk(1);
    let bulk = f.settle(ui)?;
    check(
        plan(&bulk)?.decisions == 30,
        "bulk additions covered only the visible page",
    )?;
    ui.invoke_review_section(1);
    f.settle(ui)?;
    ui.invoke_review_bulk(1);
    let keep = f.settle(ui)?;
    let removed = plan(&keep)?.rows[0].uuid.clone();
    ui.invoke_review_choose(removed.into(), 2);
    let mixed = f.settle(ui)?;
    check(
        plan(&mixed)?.approved_additions == 30
            && plan(&mixed)?.approved_keeps == 29
            && plan(&mixed)?.approved_removals == 1
            && ui.get_review_data().apply_summary.contains("29"),
        "Apply summary lost approved action totals",
    )?;
    ui.invoke_review_apply();
    let applied = f.settle(ui)?;
    check(
        matches!(
            applied.workflows.shelf_reviews.feedback,
            Some(Feedback::Committed(Outcome::ShelfSaved {
                changed: true,
                ..
            }))
        ) && count(&c, "SELECT count(*) FROM shelf_stories WHERE shelf_id=2")? == 59
            && count(
                &c,
                "SELECT count(*) FROM shelf_stories WHERE shelf_id=2 AND protected=1",
            )? == 29
            && count(&c, "SELECT count(*) FROM shelf_exclusions WHERE shelf_id=2")? == 1,
        "mixed review failed to atomically Add, Keep and Remove/exclude",
    )?;
    let current = review(&f, ui)?;
    check(
        plan(&current)?.up_to_date
            && plan(&current)?.additions == 0
            && plan(&current)?.removals == 0,
        "valid empty review was not Up to date",
    )?;
    close(ui);
    // A healthy source remains useful while another source needs repair.
    c.execute_batch("INSERT INTO collections(id,name,normalized_name,query_version,query_payload,validity_state,position) VALUES(2,'Invalid','invalid',5,'{\"name\":[]}','needs_attention',1); INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(2,2,'Invalid'); UPDATE shelf_stories SET protected=0 WHERE shelf_id=2; DELETE FROM shelf_stories WHERE shelf_id=2 AND story_uuid=(SELECT uuid FROM stories WHERE id=2); DELETE FROM stories WHERE id=60;")?;
    let incomplete = review(&f, ui)?;
    check(
        plan(&incomplete)?.incomplete
            && !plan(&incomplete)?.up_to_date
            && plan(&incomplete)?.additions == 1
            && plan(&incomplete)?.removals == 0
            && plan(&incomplete)?.warnings >= 2,
        "incomplete coverage inferred removals or hid healthy additions/unavailability",
    )?;
    ui.invoke_review_section(2);
    let warnings = f.settle(ui)?;
    check(
        plan(&warnings)?.rows.iter().any(|r| r.title == "Invalid")
            && plan(&warnings)?.rows.iter().any(|r| r.unavailable),
        "review warnings did not identify invalid sources and unavailable UUIDs",
    )?;
    close(ui);
    // Detach the healthy source while the remaining source is unknown.
    ui.invoke_show_panel(10);
    ui.invoke_select_shelf(ui.get_shelf_index());
    f.settle(ui)?;
    ui.set_shelf_source_index(0);
    ui.invoke_toggle_shelf_source();
    ui.invoke_preview_shelf_sources();
    let detach = f.settle(ui)?;
    check(
        plan(&detach)?.incomplete
            && plan(&detach)?.required > 25
            && !ui.get_review_data().can_apply
            && !ui.get_review_data().can_bulk_remove
            && plan(&detach)?.rows.iter().all(|r| !r.can_remove),
        "uncertain detachment enabled inferred removal or implicit approvals",
    )?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    check(
        count(
            &c,
            "SELECT count(*) FROM shelf_sources WHERE shelf_id=2 AND active=1",
        )? == 2,
        "deferred affected decisions detached a source",
    )?;
    refresh(&f, ui)?;
    ui.invoke_review_keep_all();
    let all = f.settle(ui)?;
    check(
        plan(&all)?.decisions == plan(&all)?.required && ui.get_review_data().can_apply,
        "Keep all did not cover all affected pages",
    )?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    check(
        count(
            &c,
            "SELECT count(*) FROM shelf_sources WHERE shelf_id=2 AND active=1",
        )? == 1
            && count(
                &c,
                "SELECT count(*) FROM shelf_stories WHERE shelf_id=2 AND protected=0",
            )? == 0,
        "uncertain Keep all lost conservative retention",
    )?;
    // Last-source detachment makes a manual Shelf without losing UUIDs/order.
    ui.invoke_show_panel(10);
    ui.invoke_select_shelf(ui.get_shelf_index());
    f.settle(ui)?;
    ui.set_shelf_source_index(1);
    ui.invoke_toggle_shelf_source();
    ui.invoke_preview_shelf_sources();
    f.settle(ui)?;
    ui.invoke_review_keep_all();
    f.settle(ui)?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    check(
        count(
            &c,
            "SELECT count(*) FROM shelf_sources WHERE shelf_id=2 AND active=1",
        )? == 0
            && count(
                &c,
                "SELECT count(*) FROM shelf_stories WHERE shelf_id=2 AND protected=0",
            )? == 0,
        "last source detachment did not retain manual membership",
    )?;
    // Reattach is explicit and adds no members. Mismatch dismissal survives restart.
    ui.invoke_show_panel(10);
    ui.invoke_select_shelf(ui.get_shelf_index());
    f.settle(ui)?;
    ui.set_shelf_source_index(0);
    ui.invoke_toggle_shelf_source();
    ui.invoke_preview_shelf_sources();
    f.settle(ui)?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    c.execute("UPDATE shelf_stories SET protected=0 WHERE shelf_id=2", [])?;
    let mismatch = review(&f, ui)?;
    check(
        plan(&mismatch)?.mismatch_prompt,
        "source mismatch had no on-demand warning",
    )?;
    ui.invoke_review_acknowledge();
    let acknowledged = f.settle(ui)?;
    check(
        !plan(&acknowledged)?.mismatch_prompt && plan(&acknowledged)?.decisions == 0,
        "mismatch dismissal approved decisions",
    )?;
    close(ui);
    drop(c);
    f.restart(ui)?;
    check(
        !ui.get_review_shown(),
        "identical entry mismatch reappeared after restart",
    )?;
    let same = review(&f, ui)?;
    check(
        !plan(&same)?.mismatch_prompt,
        "acknowledged equivalent mismatch reappeared after restart",
    )?;
    close(ui);
    let c = actor(&f)?;
    c.execute_batch("UPDATE stories SET embedded_title='Beta renamed' WHERE id=32; UPDATE shelves SET revision=revision+1 WHERE id=2;")?;
    check(
        !plan(&review(&f, ui)?)?.mismatch_prompt,
        "unrelated title/order changes repeated dismissed removals",
    )?;
    close(ui);
    c.execute("UPDATE stories SET embedded_title='Beta now unmatched',display_name_normalized='beta now unmatched' WHERE id=1", [])?;
    check(
        plan(&review(&f, ui)?)?.mismatch_prompt,
        "newly affected removal reused old acknowledgement",
    )?;
    close(ui);
    // Deletion applies every linked Shelf together; cancelled review keeps its invoker.
    c.execute_batch("INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(1,1,'Alpha source'); UPDATE shelf_stories SET protected=0 WHERE shelf_id IN(1,2);")?;
    f.worker.borrow().offer(Command::Refresh)?;
    f.settle(ui)?;
    ui.invoke_show_panel(9);
    ui.invoke_select_collection(1);
    f.settle(ui)?;
    ui.invoke_delete_collection();
    f.settle(ui)?;
    ui.invoke_review_delete_collection();
    let deletion = f.settle(ui)?;
    check(
        plan(&deletion)?.shelves == 2
            && plan(&deletion)?.required > 25
            && !ui.get_review_data().can_apply,
        "linked deletion lacked full affected-Shelf requirements",
    )?;
    ui.invoke_close_native_review();
    check(
        ui.get_dialog() == 14,
        "Not now did not return to Collection deletion confirmation",
    )?;
    ui.invoke_review_delete_collection();
    f.settle(ui)?;
    ui.invoke_review_shelf(1);
    let next = f.settle(ui)?;
    check(
        plan(&next)?.shelf == 1 && plan(&next)?.rows.len() <= 25,
        "linked deletion did not bound one affected Shelf at a time",
    )?;
    ui.invoke_review_keep_all();
    f.settle(ui)?;
    ui.invoke_review_apply();
    let deleted = f.settle(ui)?;
    check(
        matches!(deleted.workflows.shelf_reviews.feedback,Some(Feedback::Committed(Outcome::CollectionDeleted(id))) if id==CollectionId::new(1)?)
            && count(&c, "SELECT count(*) FROM collections WHERE id=1")? == 0
            && count(
                &c,
                "SELECT count(*) FROM shelf_sources WHERE collection_id=1 AND active=1",
            )? == 0
            && count(&c, "SELECT count(*) FROM stories")? == 59,
        "linked deletion did not commit atomically or removed Library stories",
    )?;
    ui.invoke_review_delete_shelf();
    f.settle(ui)?;
    ui.invoke_close_native_review();
    check(
        count(&c, "SELECT count(*) FROM shelves WHERE id=2")? == 1,
        "Not now deleted a Shelf",
    )?;
    ui.invoke_review_delete_shelf();
    f.settle(ui)?;
    ui.invoke_review_apply();
    f.settle(ui)?;
    check(
        count(&c, "SELECT count(*) FROM shelves WHERE id=2")? == 0
            && count(&c, "SELECT count(*) FROM shelves WHERE id=1")? == 1
            && count(&c, "SELECT count(*) FROM stories")? == 59
            && count(&c, "SELECT count(*) FROM story_archives")? == 59,
        "Shelf deletion affected unrelated definitions or archives",
    )?;
    drop(c);
    f.finish()?;
    dismissal(ui)
}

fn dismissal(ui: &DesktopWindow) -> Result {
    use std::{cell::Cell, os::unix::fs::PermissionsExt, rc::Rc};
    let f = Fixture::open(ui)?;
    seed(&f)?;
    navigate(&f, ui)?;
    let c = actor(&f)?;
    c.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(3,'Other review','other review',2); INSERT INTO shelf_sources(shelf_id,collection_id,collection_name) VALUES(3,1,'Alpha source'); INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title,protected) SELECT 3,uuid,id-31,embedded_title,0 FROM stories WHERE id>30;")?;
    f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
        id: ShelfId::new(3)?,
        manual_order: true,
    }))?;
    let other = f.settle(ui)?;
    check(
        ui.get_review_shown()
            && ui.get_review_data().summary.starts_with("Other review")
            && plan(&other)?.shelf_id == Some(ShelfId::new(3)?)
            && ui.get_review_data().section == 1,
        "open review A prevented adoption of Shelf B's new removal prompt",
    )?;
    ui.invoke_close_native_review();
    f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
        id: ShelfId::new(2)?,
        manual_order: true,
    }))?;
    f.settle(ui)?;
    let quits = Rc::new(Cell::new(0));
    let observed = quits.clone();
    ui.on_quit(move || observed.set(observed.get() + 1));
    let permissions = std::fs::metadata(&f.locations.native)?.permissions();
    std::fs::set_permissions(&f.locations.native, std::fs::Permissions::from_mode(0o500))?;
    ui.invoke_close_native_review();
    ui.invoke_request_quit();
    check(
        ui.get_review_data().busy && quits.get() == 0,
        "Close then Quit bypassed admitted warning persistence",
    )?;
    ui.invoke_review_updates();
    let failed = f.settle(ui);
    std::fs::set_permissions(&f.locations.native, permissions)?;
    let failed = failed?;
    check(
        failed.workflows.shelf_reviews.warning_preference_error
            && ui.get_review_data().notice.contains("could not be saved")
            && count(&c, "SELECT count(*) FROM shelf_stories WHERE shelf_id=2")? == 30,
        "immediate reopen hid warning dismissal failure or changed membership",
    )?;
    check(
        failed.phase == librairii_desktop::presentation::Phase::Recovery
            && ui.get_status().contains("warning could not be saved"),
        "fenced preference failure lost its visible Recovery explanation",
    )?;
    drop(c);
    ui.on_quit(|| {});
    f.finish()?;
    retry_dismissal(ui)
}
fn retry_dismissal(ui: &DesktopWindow) -> Result {
    let mut f = Fixture::open(ui)?;
    seed(&f)?;
    navigate(&f, ui)?;
    let path = f.locations.native.join("shelf-warnings.json");
    std::fs::write(
        &path,
        serde_json::to_vec(
            &serde_json::json!({"version":99,"library":f.id.to_string(),"entries":[]}),
        )?,
    )?;
    ui.invoke_close_native_review();
    ui.invoke_review_updates();
    let failed = f.settle(ui)?;
    check(
        failed.phase == librairii_desktop::presentation::Phase::Library
            && failed.workflows.shelf_reviews.warning_preference_error
            && ui.get_review_data().notice.contains("could not be saved"),
        "recoverable warning validation failure was hidden by immediate reopen",
    )?;
    std::fs::remove_file(path)?;
    refresh(&f, ui)?;
    ui.invoke_review_acknowledge();
    let saved = f.settle(ui)?;
    check(
        !saved.workflows.shelf_reviews.warning_preference_error,
        "successful dismissal retry retained old failure",
    )?;
    close(ui);
    f.restart(ui)?;
    check(
        !ui.get_review_shown(),
        "saved dismissal reappeared on restored Shelf startup",
    )?;
    f.finish()
}

pub struct Surface(Fixture);
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReviewSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        close(ui);
        let f = Fixture::open(ui)?;
        seed(&f)?;
        navigate(&f, ui)?;
        review(&f, ui)?;
        ui.invoke_activate_native_review();
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
    pub fn french_removals(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.0.settle(ui)?;
        ui.invoke_review_section(1);
        self.0.settle(ui)?;
        Ok(())
    }
    pub fn native_choice(&self, ui: &DesktopWindow) -> Result {
        for _ in 0..60 {
            if !ui.get_review_focused_uuid().is_empty() {
                break;
            }
            super::super::key(ui, slint::platform::Key::Tab.into())?;
        }
        check(
            !ui.get_review_focused_uuid().is_empty()
                && ui.get_review_focused_y() + 0.5 >= ui.get_review_viewport_y()
                && ui.get_review_focused_y() + ui.get_review_focused_height()
                    <= ui.get_review_viewport_y() + ui.get_review_viewport_height() + 0.5,
            "native review choice was unreachable or clipped",
        )?;
        super::super::key(ui, slint::platform::Key::DownArrow.into())?;
        super::super::key(ui, slint::platform::Key::Return.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_review_data().rows.iter().any(|r| r.choice == 1),
            "native ComboBox did not approve explicit Keep",
        )?;
        Ok(())
    }
    pub fn collection_deletion(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_show_panel(9);
        ui.invoke_select_collection(1);
        self.0.settle(ui)?;
        ui.invoke_delete_collection();
        self.0.settle(ui)?;
        check(
            ui.get_collection_cancel_focused(),
            "linked deletion lost initial Cancel focus",
        )?;
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        super::super::key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_collection_review_focused(),
            "native Tab skipped Review affected stories",
        )?;
        super::super::key(ui, slint::platform::Key::Space.into())?;
        self.0.settle(ui)?;
        check(
            ui.get_review_shown() && ui.get_review_data().kind == 2 && !ui.get_modal(),
            "native Space did not enter separate linked-deletion review",
        )?;
        super::super::key(ui, slint::platform::Key::Escape.into())?;
        check(
            ui.get_dialog() == 14 && ui.get_collection_review_focused(),
            "review cancellation lost exact linked-deletion invoker",
        )?;
        Ok(())
    }
}
