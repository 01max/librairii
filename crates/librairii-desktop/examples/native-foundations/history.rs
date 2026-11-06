//! Real bounded History reads and durable/session dismissal on owned libraries.
use super::{Fixture, Result, check};
use librairii_core::OperationState;
use librairii_desktop::presentation::Snapshot;
use librairii_ui::native::DesktopWindow;
use slint::Model;
fn id(n: usize) -> String {
    format!("f0000000-0000-4000-8000-{n:012}")
}
fn connection(f: &Fixture) -> Result<rusqlite::Connection> {
    Ok(rusqlite::Connection::open(
        f._roots.library().join("native/db/librairii.sqlite3"),
    )?)
}
fn insert(c: &rusqlite::Connection, f: &Fixture, n: usize) -> Result {
    let kind = ["import", "metadata_sync", "export"][n % 3];
    let state = if n.is_multiple_of(4) {
        "failed"
    } else {
        "succeeded"
    };
    let destination = f._roots.profile().join("Historical export");
    if !destination.exists() {
        std::fs::create_dir(&destination)?;
    }
    c.execute("INSERT INTO file_operations(id,kind,status,completed_items,total_items,created_at,finished_at,error_code,error_message,export_source_type,export_source_shelf_ids,export_source_shelf_names,export_destination) VALUES(?1,?2,?3,1,1,'2026-10-27 09:00:00','2026-10-27 09:01:00','future.code','Opaque historical detail',?4,?5,?6,?7)",rusqlite::params![id(n),kind,state,if kind=="export"{Some("curated_shelf")}else{None},if kind=="export"{Some("[10]")}else{None},if kind=="export"{Some("[\"Approved stories\"]")}else{None},if kind=="export"{destination.to_str()}else{None}])?;
    c.execute("INSERT INTO file_operation_items(operation_id,source_name,status,outcome_code,outcome_message,completed_bytes,total_bytes,story_id,resolved_story_id,story_uuid,story_title,output_name,archive_relative_path,archive_sha256) VALUES(?1,'Original été.zip',?2,'future.code','Opaque historical detail',?3,10,1,1,?4,'Historical story','Original été.zip','stories/historical.zip',?5)",rusqlite::params![id(n),state,if state=="succeeded"{10}else{0},id(n),"0".repeat(64)])?;
    Ok(())
}
fn seed(f: &Fixture, ui: &DesktopWindow) -> Result {
    let mut c = connection(f)?;
    // This is fixed historical input, prepared before the real History reads.
    // Publish it together instead of flushing ninety separate fixture writes.
    let tx = c.transaction()?;
    for n in 1..=45 {
        insert(&tx, f, n)?;
    }
    tx.commit()?;
    drop(c);
    ui.invoke_refresh_operations();
    f.settle(ui)?;
    Ok(())
}
fn ids(s: &Snapshot) -> Vec<String> {
    s.workflows
        .operations
        .history
        .cards
        .iter()
        .map(|c| c.operation.identity.id.clone())
        .collect()
}
fn dismiss_all(f: &Fixture, ui: &DesktopWindow) -> Result {
    while ui.get_notice_count() > 0 {
        ui.invoke_dismiss_notice();
        f.settle(ui)?;
    }
    Ok(())
}
fn oversize(f: &Fixture, value: bool) -> Result {
    connection(f)?.execute(
        "UPDATE file_operations SET error_message=?1 WHERE id=?2",
        rusqlite::params![
            if value {
                "x".repeat(16 * 1024 * 1024 + 1)
            } else {
                "Opaque historical detail".into()
            },
            id(19)
        ],
    )?;
    Ok(())
}
/// Leave an actual failed historical report intent before the next workflow.
pub(super) fn leave_failed_report(f: &Fixture, ui: &DesktopWindow, id: &str) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    ui.invoke_show_panel(4);
    f.settle(ui)?;
    let index = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .history
        .cards
        .iter()
        .position(|c| c.operation.identity.id == id)
        .ok_or("prior export History row")?;
    ui.invoke_inspect_history(index as i32);
    f.settle(ui)?;
    ui.invoke_close_dialog();
    let c = connection(f)?;
    let(item,message):(i64,Option<String>)=c.query_row("SELECT id,outcome_message FROM file_operation_items WHERE operation_id=? ORDER BY id LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?)))?;
    c.execute(
        "UPDATE file_operation_items SET outcome_message=?1 WHERE id=?2",
        rusqlite::params!["x".repeat(16 * 1024 * 1024 + 1), item],
    )?;
    drop(c);
    ui.invoke_inspect_history(index as i32);
    f.settle(ui)?;
    check(
        ui.get_operation_loading(),
        "prior History read did not remain explicitly failed",
    )?;
    connection(f)?.execute(
        "UPDATE file_operation_items SET outcome_message=?1 WHERE id=?2",
        rusqlite::params![message, item],
    )?;
    ui.invoke_close_dialog();
    ui.invoke_close_dialog();
    Ok(())
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let mut f = Fixture::open(ui)?;
    seed(&f, ui)?;
    check(
        ui.get_notice_count() == 2,
        "latest terminal notices missing",
    )?;
    dismiss_all(&f, ui)?;
    let saved = std::fs::read(f.locations.native.join("interface.json"))?;
    ui.invoke_show_panel(4);
    let first = f.settle(ui)?;
    check(
        ids(&first) == (26..=45).rev().map(id).collect::<Vec<_>>()
            && ui.get_history_entries().row_count() == 20
            && ui.get_history_older(),
        "native History first page changed raw cursor/count",
    )?;
    check(
        ui.get_notice_count() == 0,
        "opening History resurrected dismissed notices",
    )?;
    // A newer insertion cannot change the next raw-ID page.
    insert(&connection(&f)?, &f, 48)?;
    ui.invoke_history_load_older();
    let second = f.settle(ui)?;
    check(
        ids(&second) == (6..=25).rev().map(id).collect::<Vec<_>>(),
        "native History repeated or omitted older records",
    )?;
    ui.invoke_inspect_history(0);
    check(
        ui.get_operation_loading(),
        "History expansion exposed a prior report before its receipt",
    )?;
    let report = f.settle(ui)?;
    check(
        !ui.get_operation_loading()
            && report
                .workflows
                .operations
                .details
                .as_ref()
                .is_some_and(|d| d.card.operation.identity.id == id(25))
            && ui
                .get_operation_items()
                .iter()
                .any(|t| t.contains("future.code") && t.contains("Opaque historical detail")),
        "History report lost its requested identity or opaque outcome",
    )?;
    ui.invoke_close_dialog();
    check(
        ui.get_dialog() == 21,
        "History report did not return to its page",
    )?;
    // A failed fresh read of the same ID cannot certify its old detail rows.
    connection(&f)?.execute(
        "UPDATE file_operation_items SET outcome_message=?1 WHERE operation_id=?2",
        rusqlite::params!["x".repeat(16 * 1024 * 1024 + 1), id(25)],
    )?;
    ui.invoke_inspect_history(0);
    let refused = f.settle(ui)?;
    check(
        ui.get_operation_loading()
            && refused.workflows.operations.details.is_none()
            && ui.get_operation_items().row_count() == 0
            && !ui.get_operation_cancellable()
            && !ui.get_operation_retryable()
            && !ui.get_operation_revealable(),
        "same-ID failed report read restored earlier rows/actions",
    )?;
    connection(&f)?.execute("UPDATE file_operation_items SET outcome_message='Opaque historical detail' WHERE operation_id=?1",[id(25)])?;
    ui.invoke_refresh_operations();
    f.settle(ui)?;
    check(
        !ui.get_operation_loading(),
        "repaired same-ID report remained unavailable",
    )?;
    ui.invoke_close_dialog();
    ui.invoke_history_load_older();
    let third = f.settle(ui)?;
    check(
        ids(&third) == (1..=5).rev().map(id).collect::<Vec<_>>() && !ui.get_history_older(),
        "native History final page failed",
    )?;
    ui.invoke_history_refresh();
    f.settle(ui)?;
    oversize(&f, true)?;
    ui.invoke_history_load_older();
    f.settle(ui)?;
    check(
        !ui.get_history_error().is_empty() && ui.get_history_entries().row_count() == 20,
        "History read refusal lost its previous bounded page/error",
    )?;
    oversize(&f, false)?;
    ui.invoke_history_retry();
    let retried = f.settle(ui)?;
    check(
        ui.get_history_error().is_empty()
            && ids(&retried) == (7..=26).rev().map(id).collect::<Vec<_>>(),
        "History Retry did not repeat the exact failed cursor",
    )?;
    ui.invoke_close_dialog();
    f.restart(ui)?;
    check(
        std::fs::read(f.locations.native.join("interface.json"))? == saved
            && f.view
                .borrow()
                .snapshot
                .workflows
                .operations
                .dismissed_operations
                .contains(&id(44)),
        "notice dismissal changed or was lost on restart",
    )?;
    // The newly created exact ID remains visible despite old dismissals.
    check(
        ui.get_notice_count() == 1,
        "new operation inherited another ID’s dismissal",
    )?;
    dismiss_all(&f, ui)?;
    ui.invoke_change_language("fr".into());
    f.settle(ui)?;
    ui.invoke_show_panel(4);
    f.settle(ui)?;
    check(
        ui.get_history_entries()
            .iter()
            .any(|s| s.contains("Terminée") || s.contains("Échec"))
            && ui.get_notice_count() == 0,
        "French History restored notices or lost localized saved states",
    )?;
    ui.invoke_close_dialog();
    // Reobserve an existing displayed active identity, including when the feed
    // no longer contains this older metadata operation after it settles.
    let c = connection(&f)?;
    c.execute("INSERT INTO file_operations(id,kind,status,total_items,created_at) VALUES(?1,'metadata_sync','running',1,'2026-10-27 10:00:00')",[id(47)])?;
    drop(c);
    ui.invoke_show_panel(4);
    f.settle(ui)?;
    let before = ids(&f.view.borrow().snapshot);
    connection(&f)?.execute("UPDATE file_operations SET status='failed',completed_items=1,finished_at='2026-10-27 10:01:00' WHERE id=?1",[id(47)])?;
    ui.invoke_refresh_operations();
    let terminal = f.settle(ui)?;
    check(
        ids(&terminal) == before
            && terminal.workflows.operations.history.cards[0]
                .operation
                .state
                == OperationState::Failed,
        "History did not reconcile live completion without changing its cursor/order",
    )?;
    ui.invoke_close_dialog();
    f.worker.borrow_mut().finish()?;
    let future = b"{\"format\":\"librairii.native-interface\",\"version\":999}";
    std::fs::write(f.locations.native.join("interface.json"), future)?;
    f.restart(ui)?;
    check(
        ui.get_notice_count() > 0,
        "future preference fallback hid unknown notices",
    )?;
    dismiss_all(&f, ui)?;
    ui.invoke_show_panel(4);
    f.settle(ui)?;
    ui.invoke_history_refresh();
    f.settle(ui)?;
    check(
        ui.get_notice_count() == 0
            && !ui.get_terminal_notice_error().is_empty()
            && std::fs::read(f.locations.native.join("interface.json"))? == future,
        "unavailable preferences refused session dismissal or rewrote future bytes",
    )?;
    ui.invoke_close_dialog();
    f.finish()
}
pub struct Surface {
    pub(super) fixture: Fixture,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistorySurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        let fixture = Fixture::open(ui)?;
        seed(&fixture, ui)?;
        ui.invoke_show_panel(4);
        fixture.settle(ui)?;
        Ok(Self { fixture })
    }
    pub fn report(&self, ui: &DesktopWindow) -> Result {
        let i = self
            .fixture
            .view
            .borrow()
            .snapshot
            .workflows
            .operations
            .history
            .cards
            .iter()
            .position(|c| c.operation.identity.id == id(44))
            .ok_or("historical export")?;
        ui.invoke_inspect_history(i as i32);
        self.fixture.settle(ui)?;
        Ok(())
    }
    pub fn french_error(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_close_dialog();
        check(
            ui.get_dialog() == 21,
            "History error preparation did not return to History",
        )?;
        ui.invoke_change_language("fr".into());
        self.fixture.settle(ui)?;
        check(
            ui.get_dialog() == 21,
            "language changed the open History surface",
        )?;
        oversize(&self.fixture, true)?;
        ui.invoke_history_load_older();
        self.fixture.settle(ui)?;
        check(
            ui.get_dialog() == 21 && !ui.get_history_error().is_empty(),
            "failed older History request changed the open dialog or lost its error",
        )?;
        oversize(&self.fixture, false)?;
        ui.invoke_focus_history_retry();
        check(
            ui.get_history_retry_focused(),
            "History Retry did not take native focus",
        )?;
        Ok(())
    }
    pub fn another_error(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_show_panel(4);
        self.fixture.settle(ui)?;
        self.report(ui)?;
        self.french_error(ui)
    }
    pub fn settled(&self, ui: &DesktopWindow) -> Result<Snapshot> {
        self.fixture.settle(ui)
    }
    pub fn poll(&self, ui: &DesktopWindow) {
        if let Some(s) = self.fixture.worker.borrow().take_snapshot() {
            self.fixture
                .view
                .borrow_mut()
                .accept(ui, s, self.fixture.worker.borrow().revision());
        }
        self.fixture.view.borrow_mut().resize(ui);
    }
}
