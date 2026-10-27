//! Owned archive selections exercise the exact production post-picker boundary.
use super::{Fixture, Result, check, view};
use librairii_core::{OperationKind, operations::is_terminal};
use librairii_desktop::presentation::Snapshot;
use librairii_ui::native::DesktopWindow;
use slint::Model;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn source(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../preservation/corpus/archives")
        .join(name)
}
fn select(f: &Fixture, ui: &DesktopWindow, paths: Option<Vec<PathBuf>>) {
    let generation = f.view.borrow().snapshot.generation.clone();
    if let Some(generation) = generation {
        view::operations::selected(&f.view, &f.worker, ui, generation, paths);
    }
}
fn complete(f: &Fixture, ui: &DesktopWindow) -> Result<Snapshot> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let snapshot = f.settle(ui)?;
        if snapshot.workflows.operations.preparing.is_none()
            && snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .is_some_and(|d| is_terminal(d.card.operation.state))
        {
            return Ok(snapshot);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "native import did not finish: {:?}",
                snapshot.workflows.operations
            )
            .into());
        }
    }
}
pub fn checks(ui: &DesktopWindow) -> Result {
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    let mut f = Fixture::open(ui)?;
    let before = f.view.borrow().snapshot.workflows.operations.cards.len();
    ui.invoke_show_panel(2);
    f.settle(ui)?;
    select(&f, ui, None);
    check(
        !ui.get_picker_open()
            && f.view.borrow().snapshot.workflows.operations.cards.len() == before,
        "picker cancellation created an operation",
    )?;
    let originals = [
        source("clockwork-forest.plain.pk"),
        source("00112233445546778899aabbccddeeff.v1.pk"),
    ];
    let bytes = originals
        .iter()
        .map(std::fs::read)
        .collect::<std::io::Result<Vec<_>>>()?;
    select(
        &f,
        ui,
        Some(vec![
            originals[0].clone(),
            originals[0].clone(),
            originals[1].clone(),
            source("unsupported.txt"),
        ]),
    );
    let snapshot = complete(&f, ui)?;
    let details = snapshot
        .workflows
        .operations
        .details
        .as_ref()
        .ok_or("import report missing")?;
    let id = details.card.operation.identity.id.clone();
    check(
        details.card.succeeded == 1
            && details.card.skipped == 1
            && details.card.conflicted == 1
            && details.card.failed == 1,
        &format!(
            "native import did not retain imported/duplicate/conflict/failure totals: {details:?}"
        ),
    )?;
    check(
        ui.get_operation_items().row_count() == 4
            && ui
                .get_operation_items()
                .iter()
                .any(|s| s.contains("duplicate_checksum"))
            && ui
                .get_operation_items()
                .iter()
                .any(|s| s.contains("uuid_conflict")),
        "native import discarded item reasons",
    )?;
    check(
        ui.get_operation_retryable() && !ui.get_operation_cancellable(),
        "terminal import has wrong retry/cancel controls",
    )?;
    for (path, expected) in originals.iter().zip(bytes) {
        check(
            std::fs::read(path)? == expected,
            "native import changed an original source",
        )?;
    }
    // A cancelled retry picker leaves its original durable report intact.
    select(&f, ui, None);
    check(
        f.view
            .borrow()
            .snapshot
            .workflows
            .operations
            .details
            .as_ref()
            .is_some_and(|d| d.card.operation.identity.id == id),
        "cancelled retry replaced the old report",
    )?;
    ui.invoke_close_dialog();
    f.search(ui, "Alpha")?;
    select(&f, ui, Some(vec![originals[0].clone()]));
    let retried = complete(&f, ui)?;
    check(
        retried
            .navigation
            .as_ref()
            .is_some_and(|n| n.current().location.request.criteria.name == "Alpha"),
        "import changed the browser query/history",
    )?;
    check(
        retried
            .workflows
            .operations
            .details
            .as_ref()
            .is_some_and(|d| d.card.operation.identity.id != id && d.card.skipped == 1),
        "retry did not create a separate idempotent operation",
    )?;
    // Cancel is admitted through the real scheduler while a full batch owns work.
    select(&f, ui, Some(vec![source("clockwork-forest.plain.pk"); 32]));
    f.settle(ui)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if ui.get_operation_cancellable() {
            break;
        }
        if f.view
            .borrow()
            .snapshot
            .workflows
            .operations
            .details
            .as_ref()
            .is_some_and(|d| is_terminal(d.card.operation.state))
        {
            return Err("cancellation fixture finished before showing Cancel".into());
        }
        f.settle(ui)?;
        if Instant::now() > deadline {
            return Err("import cancellation control unavailable".into());
        }
    }
    let first = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .details
        .as_ref()
        .map(|d| d.card.operation.identity.id.clone())
        .ok_or("first active import missing")?;
    select(&f, ui, Some(vec![source("clockwork-forest.plain.pk"); 32]));
    loop {
        let snapshot = f.settle(ui)?;
        if snapshot.workflows.operations.preparing.is_none()
            && snapshot
                .workflows
                .operations
                .cards
                .iter()
                .filter(|c| !is_terminal(c.operation.state))
                .count()
                == 2
        {
            check(
                snapshot
                    .workflows
                    .operations
                    .status
                    .is_some_and(|s| s.running <= 2)
                    && ui
                        .get_operation_runtime()
                        .starts_with("Active operations: 2"),
                "concurrent import overview miscounted operations or workers",
            )?;
            break;
        }
        if Instant::now() > deadline {
            return Err("two concurrent imports were not observed".into());
        }
    }
    check(
        ui.get_operation_items().row_count() == 25 && ui.get_operation_next(),
        "operation item page was not bounded to25",
    )?;
    ui.invoke_operation_page(1);
    f.settle(ui)?;
    check(
        ui.get_operation_items().row_count() == 7
            && ui.get_operation_previous()
            && !ui.get_operation_next(),
        "operation report paging did not reach all32 items",
    )?;
    ui.invoke_operation_page(-1);
    f.settle(ui)?;
    let revision = f.worker.borrow().revision();
    select(
        &f,
        ui,
        Some(vec![source("clockwork-forest.plain.pk"); 1001]),
    );
    check(
        f.worker.borrow().revision() == revision && ui.get_operation_notice().contains("1,000"),
        "invalid picker batch created work or lacked corrective feedback",
    )?;
    f.settle(ui)?;
    check(
        ui.get_operation_notice().contains("1,000"),
        "progress erased picker validation feedback",
    )?;
    let second = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .details
        .as_ref()
        .map(|d| d.card.operation.identity.id.clone())
        .ok_or("second import missing")?;
    let first_index = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .cards
        .iter()
        .position(|c| c.operation.identity.id == first)
        .ok_or("first active import missing")?;
    ui.invoke_select_operation(first_index as i32);
    f.settle(ui)?;
    if ui.get_operation_cancellable() {
        ui.invoke_cancel_operation();
        f.settle(ui)?;
    }
    let second_index = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .cards
        .iter()
        .position(|c| c.operation.identity.id == second)
        .ok_or("second active import missing")?;
    ui.invoke_select_operation(second_index as i32);
    f.settle(ui)?;
    ui.invoke_cancel_operation();
    let cancelled = complete(&f, ui)?;
    check(
        cancelled
            .workflows
            .operations
            .details
            .as_ref()
            .is_some_and(|d| d.card.cancelled > 0),
        "Cancel did not retain cancelled item reports",
    )?;
    let index = f
        .view
        .borrow()
        .snapshot
        .workflows
        .operations
        .cards
        .iter()
        .position(|c| c.operation.identity.id == first)
        .ok_or("first import report missing")?;
    ui.invoke_select_operation(index as i32);
    f.settle(ui)?;
    if ui.get_operation_cancellable() {
        ui.invoke_cancel_operation();
        complete(&f, ui)?;
    }
    ui.invoke_close_dialog();
    f.restart(ui)?;
    check(
        f.view.borrow().snapshot.generation.is_some(),
        &format!(
            "import restart did not activate: {:?}",
            f.view.borrow().snapshot
        ),
    )?;
    ui.invoke_show_panel(2);
    let restored = f.settle(ui)?;
    check(
        restored
            .workflows
            .operations
            .cards
            .iter()
            .any(|c| c.operation.kind == OperationKind::Import && is_terminal(c.operation.state)),
        "import report did not survive restart",
    )?;
    ui.invoke_close_dialog();
    f.finish()
}
pub struct Surface {
    fixture: Fixture,
}
impl std::fmt::Debug for Surface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OperationSurface").finish_non_exhaustive()
    }
}
impl Surface {
    pub fn open(ui: &DesktopWindow) -> Result<Self> {
        checks(ui)?;
        let fixture = Fixture::open(ui)?;
        ui.invoke_show_panel(2);
        fixture.settle(ui)?;
        select(
            &fixture,
            ui,
            Some(vec![
                source("clockwork-forest.plain.pk"),
                source("00112233445546778899aabbccddeeff.v1.pk"),
                source("unsupported.txt"),
            ]),
        );
        complete(&fixture, ui)?;
        Ok(Self { fixture })
    }
    pub fn french(&self, ui: &DesktopWindow) -> Result {
        ui.invoke_change_language("fr".into());
        self.fixture.settle(ui)?;
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
