//! Actual native callbacks and durable shared-runtime exports on owned data.
use super::{Fixture, Result, check, view};
use librairii_core::{
    OperationKind,
    collection_codec::{self, Query, url::Scope},
    operations::{ExportSourceType, is_terminal},
};
use librairii_desktop::presentation::{Command, Snapshot};
use librairii_ui::native::DesktopWindow;
use slint::Model;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
const REAL: &str = "00112233-4455-4677-8899-aabbccddeeff";
const MISSING: &str = "20000000-0000-4000-8000-000000000010";
fn destination(f: &Fixture, name: &str) -> Result<PathBuf> {
    let p = f._roots.profile().join(name);
    std::fs::create_dir(&p)?;
    Ok(p)
}
fn choose(f: &Fixture, ui: &DesktopWindow, path: Option<PathBuf>) {
    let generation = f.view.borrow().snapshot.generation.clone();
    if let Some(g) = generation {
        view::exports::selected(&f.view, &f.worker, ui, g, path);
    }
}
fn page(f: &Fixture) -> Result<librairii_desktop::workflows::exports::Page> {
    Ok(f.view
        .borrow()
        .snapshot
        .workflows
        .exports
        .preview
        .clone()
        .ok_or("native export preview missing")?)
}
fn complete(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let s = settled(f, ui)?;
        if s.workflows.operations.preparing.is_none()
            && !s.workflows.operations.critical
            && s.workflows.operations.details.as_ref().is_some_and(|d| {
                d.card.operation.kind == OperationKind::Export
                    && is_terminal(d.card.operation.state)
            })
        {
            return Ok(s);
        }
        if Instant::now() > deadline {
            return Err("native export did not finish".into());
        }
    }
}
fn close(f: &Fixture, ui: &DesktopWindow) -> Result {
    let exporting = ui.get_dialog() == 20;
    ui.invoke_close_dialog();
    if exporting {
        settled(f, ui)?;
    }
    Ok(())
}
fn open(f: &Fixture, ui: &DesktopWindow) -> Result {
    ui.invoke_show_panel(3);
    settled(f, ui)?;
    Ok(())
}
fn seed(f: &Fixture, ui: &DesktopWindow) -> Result {
    let connection =
        rusqlite::Connection::open(f._roots.library().join("native/db/librairii.sqlite3"))?;
    let mut query = Query::default();
    query.name = "Clockwork".into();
    let q = collection_codec::encode(query)?;
    for (id, name, payload) in [
        (10, "Export first", q.as_str()),
        (11, "Export overlap", q.as_str()),
        (12, "Export repair", "{\"unknownCriterion\":true}"),
    ] {
        connection.execute("INSERT INTO collections(id,name,normalized_name,position,query_version,query_payload,validity_state) VALUES(?1,?2,lower(?2),?1,5,?3,?4)",rusqlite::params![id,name,payload,if id==12{"needs_attention"}else{"valid"}])?;
    }
    connection.execute_batch("INSERT INTO shelves(id,name,normalized_name,position) VALUES(10,'Export approved','export approved',10),(11,'Export overlap','export overlap',11);UPDATE stories SET is_broken=1 WHERE id=1;")?;
    for shelf in [10, 11] {
        for (position, uuid, title) in [
            (0, REAL, "Clockwork"),
            (1, MISSING, "Unavailable approved"),
            (2, "10000000-0000-4000-8000-000000000001", "Broken approved"),
        ] {
            connection.execute("INSERT INTO shelf_stories(shelf_id,story_uuid,position,cached_title) VALUES(?1,?2,?3,?4)",rusqlite::params![shelf,uuid,position,title])?;
        }
    }
    drop(connection);
    f.worker.borrow().offer(Command::Refresh)?;
    settled(f, ui)?;
    Ok(())
}
fn toggle(f: &Fixture, ui: &DesktopWindow, label: &str) -> Result {
    let i = ui
        .get_export_groups()
        .iter()
        .position(|g| g.label.starts_with(label))
        .ok_or("export group missing")?;
    ui.invoke_export_toggle(i as i32);
    settled(f, ui)?;
    Ok(())
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let mut f = Fixture::open_with(ui, true)?;
    seed(&f, ui)?;
    let first = destination(&f, "Export destination été")?;
    open(&f, ui)?;
    choose(&f, ui, None);
    check(
        !ui.get_picker_open() && f.view.borrow().snapshot.workflows.exports.preview.is_none(),
        "cancelled export picker changed state",
    )?;
    choose(&f, ui, Some(first.clone()));
    settled(&f, ui)?;
    let p = page(&f)?;
    check(
        p.intended == 61
            && p.ready == 1
            && p.pages == 3
            && p.items.len() == 25
            && ui.get_export_data().can_start,
        "full-result export truncated pages or hid unavailable/broken members",
    )?;
    ui.invoke_export_page(1);
    settled(&f, ui)?;
    check(
        page(&f)?.page == 1 && page(&f)?.items.len() == 25,
        "export preview paging failed",
    )?;
    // A later request does not replace the frozen preparation's member set.
    f.search(ui, "Clockwork")?;
    ui.invoke_start_export();
    let result = complete(&f, ui)?;
    let d = result
        .workflows
        .operations
        .details
        .as_ref()
        .ok_or("export report missing")?;
    let original = std::fs::read(f._roots.profile().join("Bibliothèque originale.plain.pk"))?;
    let output = first.join("Bibliothèque originale.plain.pk");
    check(
        d.card.operation.counts.total == 61
            && d.card.succeeded == 1
            && d.card.skipped == 60
            && ui.get_operation_groups().row_count() > 0
            && ui.get_operation_revealable(),
        "native export did not retain grouped full-scope results",
    )?;
    check(
        std::fs::read(&output)? == original && std::fs::read_dir(&first)?.count() == 1,
        "export bytes changed or left unrelated output",
    )?;
    check(
        ui.get_query() == "Clockwork"
            && result
                .navigation
                .as_ref()
                .is_some_and(|n| n.current().location.request.criteria.name == "Clockwork"),
        "export changed browsing history/query",
    )?;

    let moved = first.with_extension("disconnected");
    std::fs::rename(&first, &moved)?;
    ui.invoke_reveal_export();
    settled(&f, ui)?;
    check(
        !ui.get_operation_reveal_notice().is_empty(),
        "missing reveal destination did not report its validation failure",
    )?;
    std::fs::rename(&moved, &first)?;
    super::history_checks::leave_failed_report(&f, ui, &d.card.operation.identity.id)?;
    close(&f, ui)?;
    f.select(ui, 0)?;
    open(&f, ui)?;
    ui.invoke_export_scope(1);
    settled(&f, ui)?;
    choose(&f, ui, Some(first.clone()));
    settled(&f, ui)?;
    check(
        page(&f)?.ready == 0
            && !ui.get_export_data().can_start
            && ui.get_export_items().iter().any(|s| s.contains("filename")),
        "existing destination did not disable zero-ready export with a conflict reason",
    )?;
    let second = destination(&f, "Reselected output")?;
    choose(&f, ui, Some(second.clone()));
    settled(&f, ui)?;
    check(
        page(&f)?.ready == 1,
        "destination reselection reused stale conflicts",
    )?;
    // A real flag write after preflight must refuse Start without creating work.
    ui.invoke_panel_flag_change(true);
    settled(&f, ui)?;
    ui.invoke_start_export();
    let stale = settled(&f, ui)?;
    check(
        stale.workflows.exports.error.as_deref() == Some("export.prepared_stale")
            && !ui.get_export_data().can_start
            && std::fs::read_dir(&second)?.next().is_none(),
        "flag change bypassed stale preview protection",
    )?;
    ui.invoke_refresh_export();
    settled(&f, ui)?;
    check(
        page(&f)?.ready == 0
            && page(&f)?.items.iter().all(|i| {
                matches!(
                    i.disposition,
                    librairii_core::export::Disposition::Skipped(
                        librairii_core::export::Issue::StoryMarkedBroken
                    )
                )
            }),
        "fresh preview failed to explain universal Broken exclusion",
    )?;
    ui.invoke_undo_flags();
    settled(&f, ui)?;
    ui.invoke_refresh_export();
    settled(&f, ui)?;
    check(
        page(&f)?.ready == 1,
        "flag Undo did not enable a fresh export preview",
    )?;
    close(&f, ui)?;
    open(&f, ui)?;
    ui.invoke_export_scope(2);
    settled(&f, ui)?;
    toggle(&f, ui, "Export first")?;
    toggle(&f, ui, "Export overlap")?;
    choose(&f, ui, Some(second.clone()));
    settled(&f, ui)?;
    check(
        page(&f)?.intended == 1 && page(&f)?.overlap == 1,
        "Collection export union was not reevaluated/deduplicated",
    )?;
    toggle(&f, ui, "Export repair")?;
    check(
        f.view.borrow().snapshot.workflows.exports.error.is_some()
            && !ui.get_export_data().can_start,
        "invalid Collection export broadened or started",
    )?;
    close(&f, ui)?;
    f.worker.borrow().offer(Command::Navigate(Scope::Shelf {
        id: librairii_core::ShelfId::new(10)?,
        manual_order: true,
    }))?;
    settled(&f, ui)?;
    // Shelf entry may show a source review; the explicit export modal remains separate.
    ui.invoke_close_native_review();
    f.search(ui, "Clockwork")?;
    open(&f, ui)?;
    choose(&f, ui, Some(second.clone()));
    settled(&f, ui)?;
    check(
        page(&f)?.intended == 1 && page(&f)?.ready == 1,
        "current Shelf result ignored temporary refinement",
    )?;
    ui.invoke_export_scope(3);
    settled(&f, ui)?;
    check(
        page(&f)?.intended == 3 && page(&f)?.ready == 1,
        "whole-Shelf export adopted refinement or pending membership",
    )?;
    toggle(&f, ui, "Export overlap")?;
    check(
        page(&f)?.intended == 3 && page(&f)?.overlap == 3,
        "Shelf union lost approved UUID deduplication",
    )?;
    ui.invoke_start_export();
    let shelf = complete(&f, ui)?;
    let d = shelf
        .workflows
        .operations
        .details
        .as_ref()
        .ok_or("Shelf export report")?;
    check(
        d.card.operation.export_scope.source_type()? == Some(ExportSourceType::CuratedShelves)
            && d.card.operation.export_scope.group_ids.as_deref() == Some("[10,11]")
            && d.card.succeeded == 1
            && d.card.skipped == 2,
        "Shelf report lost distinct source identity or partial results",
    )?;
    let saved_latest = d.card.operation.identity.id.clone();
    let connection =
        rusqlite::Connection::open(f._roots.library().join("native/db/librairii.sqlite3"))?;
    let missing:i64=connection.query_row("SELECT count(*) FROM file_operation_items WHERE operation_id=? AND story_uuid=? AND status='skipped' AND outcome_code='story_unavailable'",rusqlite::params![d.card.operation.identity.id,MISSING],|r|r.get(0))?;
    drop(connection);
    check(
        missing == 1,
        "unavailable approved UUID was not retained in the durable report",
    )?;
    check(
        std::fs::read(second.join("Bibliothèque originale.plain.pk"))? == original
            && std::fs::read(&output)? == original,
        "partial export overwrote existing or changed managed bytes",
    )?;
    ui.invoke_change_language("fr".into());
    settled(&f, ui)?;
    check(
        ui.get_operation_summary().contains("Étagères approuvées")
            && ui
                .get_operation_groups()
                .iter()
                .any(|g| g.heading.contains("Exportées")),
        "export source/grouped outcomes did not localize",
    )?;
    close(&f, ui)?;
    f.restart(ui)?;
    ui.invoke_show_panel(2);
    settled(&f, ui)?;
    check(
        f.view
            .borrow()
            .snapshot
            .workflows
            .operations
            .cards
            .iter()
            .any(|c| c.operation.identity.id == saved_latest),
        "latest or recent exports were lost after restart",
    )?;
    close(&f, ui)?;
    f.finish()
}

fn settled(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let s = f.settle(ui)?;
        if !s.workflows.exports.preparing {
            return Ok(s);
        }
        if Instant::now() > deadline {
            return Err("native export preparation did not finish".into());
        }
    }
}

pub struct Surface {
    fixture: Fixture,
    destination: PathBuf,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let fixture = Fixture::open_with(ui, true)?;
        seed(&fixture, ui)?;
        let destination = destination(&fixture, "Native export frames")?;
        open(&fixture, ui)?;
        choose(&fixture, ui, Some(destination.clone()));
        settled(&fixture, ui)?;
        Ok(Self {
            fixture,
            destination,
        })
    }
    pub fn finish_export(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_start_export();
        complete(&self.fixture, ui)?;
        Ok(())
    }
    pub fn french_zero(&self, ui: &DesktopWindow) -> Result {
        close(&self.fixture, ui)?;
        ui.invoke_change_language("fr".into());
        settled(&self.fixture, ui)?;
        self.fixture.search(ui, "Clockwork")?;
        open(&self.fixture, ui)?;
        choose(&self.fixture, ui, Some(self.destination.clone()));
        settled(&self.fixture, ui)?;
        check(
            !ui.get_export_data().can_start && page(&self.fixture)?.ready == 0,
            "compact French zero-ready export enabled Start",
        )?;
        Ok(())
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(snapshot) = self.fixture.worker.borrow().take_snapshot() {
            self.fixture.view.borrow_mut().accept(
                ui,
                snapshot,
                self.fixture.worker.borrow().revision(),
            );
        }
        self.fixture.view.borrow_mut().resize(ui);
    }
}
