//! Main-thread picker ownership and native projection. All archive/database work
//! is performed after selection on the existing controller/job workers.
use super::View;
use librairii_app::{
    lifecycle::LibraryGeneration,
    localization::{Catalogs, Language, OutcomeStatus},
};
use librairii_core::{
    OperationKind, OperationState,
    operations::{ItemState, is_terminal},
};
use librairii_desktop::{
    presentation::{Command, Snapshot, Worker},
    workflows::operations::Action,
};
use librairii_ui::native::DesktopWindow;
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
#[derive(Debug, Default)]
pub struct State {
    pending: Option<u64>,
    keys: Vec<String>,
    local_error: Option<LocalError>,
}
#[derive(Debug)]
enum LocalError {
    Selection,
    Busy(String),
    Picker(String),
}
impl LocalError {
    fn message(&self, language: Language) -> String {
        match self {
            Self::Selection => words(
                language,
                "Choose 1 to 1,000 files; use a smaller selection if their paths are very long.",
                "Choisissez de 1 à 1 000 fichiers, dans une sélection plus petite si leurs chemins sont très longs.",
            ),
            Self::Busy(code) => format!(
                "{} [{code}]",
                words(
                    language,
                    "Librairii is handling another request. Try the selection again in a moment.",
                    "Librairii traite une autre demande. Réessayez la sélection dans un instant."
                )
            ),
            Self::Picker(code) => format!(
                "{} [{code}]",
                words(
                    language,
                    "The file picker is unavailable. Try choosing files again.",
                    "Le sélecteur de fichiers est indisponible. Réessayez de choisir les fichiers."
                )
            ),
        }
    }
}
fn local_error(state: &Rc<RefCell<View>>, ui: &DesktopWindow, error: LocalError) {
    let message = error.message(if ui.get_french() {
        Language::French
    } else {
        Language::English
    });
    state.borrow_mut().operations.local_error = Some(error);
    ui.set_operation_notice(message.into());
}
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French {
        fr.into()
    } else {
        en.into()
    }
}
fn kind(kind: OperationKind, language: Language) -> String {
    match kind {
        OperationKind::Import => words(language, "Import", "Import"),
        OperationKind::Export => words(language, "Export", "Export"),
        OperationKind::MetadataSync => {
            words(language, "Official metadata", "Métadonnées officielles")
        }
    }
}
fn status(state: OperationState, language: Language) -> String {
    words(
        language,
        match state {
            OperationState::Queued => "Queued",
            OperationState::Running => "In progress",
            OperationState::Succeeded => "Completed",
            OperationState::PartiallySucceeded => "Completed with exceptions",
            OperationState::Failed => "Failed",
            OperationState::Cancelled => "Cancelled",
            OperationState::Interrupted => "Interrupted",
        },
        match state {
            OperationState::Queued => "En attente",
            OperationState::Running => "En cours",
            OperationState::Succeeded => "Terminée",
            OperationState::PartiallySucceeded => "Terminée avec des exceptions",
            OperationState::Failed => "Échec",
            OperationState::Cancelled => "Annulée",
            OperationState::Interrupted => "Interrompue",
        },
    )
}
fn reason(
    catalogs: &Catalogs,
    language: Language,
    outcome: &librairii_core::operations::RawOutcome,
    state: OutcomeStatus,
) -> String {
    let mut text = String::new();
    if let Some(message) = &outcome.message
        && let Ok(localized) = catalogs.historical(language, message, state)
    {
        text = localized.summary;
        if let Some(detail) = localized.technical_detail {
            text.push_str(&format!(
                "\n{}: {detail}",
                words(language, "Technical detail", "Détail technique")
            ));
        }
    }
    if let Some(code) = &outcome.code {
        text.push_str(&format!(" [{code}]"));
    }
    text
}
impl State {
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot, catalogs: &Catalogs) {
        let data = &snapshot.workflows.operations;
        let language = snapshot.language;
        if self.pending.is_some_and(|r| data.revision >= r) {
            self.pending = None;
        }
        ui.set_operation_busy(self.pending.is_some());
        let selected = data
            .details
            .as_ref()
            .map(|d| d.card.operation.identity.id.as_str());
        self.keys = data
            .cards
            .iter()
            .map(|c| c.operation.identity.id.clone())
            .collect();
        let choices = data
            .cards
            .iter()
            .map(|c| {
                format!(
                    "{} · {} · {} / {} · {}",
                    kind(c.operation.kind, language),
                    status(c.operation.state, language),
                    language.number(c.operation.counts.completed),
                    language.number(c.operation.counts.total),
                    c.operation.identity.id
                )
                .into()
            })
            .collect::<Vec<slint::SharedString>>();
        let current = ui.get_operation_choices();
        if let Some(model) = current
            .as_any()
            .downcast_ref::<VecModel<slint::SharedString>>()
            .filter(|m| m.row_count() == choices.len())
        {
            for (i, row) in choices.into_iter().enumerate() {
                if model.row_data(i).as_ref() != Some(&row) {
                    model.set_row_data(i, row);
                }
            }
        } else {
            ui.set_operation_choices(ModelRc::new(VecModel::from(choices)));
        }
        ui.set_operation_index(
            selected
                .and_then(|id| self.keys.iter().position(|key| key == id))
                .map_or(-1, |i| i as i32),
        );
        let mut notice=data.error.as_ref().map(|error|format!("{}\n{error}",words(language,"The operation could not be started or updated. Refresh and try again.","L’opération n’a pas pu être démarrée ou actualisée. Actualisez puis réessayez."))).unwrap_or_default();
        if data.preparing.is_some() {
            notice = words(
                language,
                "Preparing the selected files…",
                "Préparation des fichiers sélectionnés…",
            );
        }
        ui.set_operation_notice(
            self.local_error
                .as_ref()
                .map(|error| error.message(language))
                .unwrap_or(notice)
                .into(),
        );
        let active_cards = data
            .cards
            .iter()
            .filter(|c| !is_terminal(c.operation.state))
            .collect::<Vec<_>>();
        let active =
            active_cards.len()
                + usize::from(data.preparing.as_ref().is_some_and(|id| {
                    !active_cards.iter().any(|c| c.operation.identity.id == *id)
                }));
        ui.set_pending(active > 0 || data.status.is_some_and(|s| s.queued + s.running > 0));
        let completed = active_cards
            .iter()
            .map(|c| c.operation.counts.completed)
            .sum::<i64>();
        let total = active_cards
            .iter()
            .map(|c| c.operation.counts.total)
            .sum::<i64>();
        ui.set_indeterminate(total == 0);
        ui.set_progress(if total > 0 {
            completed as f32 / total as f32
        } else {
            0.
        });
        ui.set_operation_status(if active > 0 {
            format!(
                "{}: {} · {} / {} {}",
                words(language, "Imports and operations", "Imports et opérations"),
                language.number(active as i64),
                language.number(completed),
                language.number(total),
                words(language, "files", "fichiers")
            )
            .into()
        } else {
            "".into()
        });
        ui.set_operation_runtime(
            format!(
                "{}: {} · {}",
                words(language, "Active operations", "Opérations actives"),
                language.number(active as i64),
                words(
                    language,
                    "Imports continue in the background · cancellation preserves completed results",
                    "Les imports continuent en arrière-plan · l’annulation conserve les résultats enregistrés"
                )
            )
            .into(),
        );
        ui.set_operation_cancellable(false);
        ui.set_operation_retryable(false);
        ui.set_operation_previous(false);
        ui.set_operation_next(false);
        ui.set_operation_items(ModelRc::default());
        ui.set_operation_summary("".into());
        if let Some(details) = &data.details {
            let c = &details.card;
            let h = &c.operation;
            if h.cancel_requested && !is_terminal(h.state) && self.local_error.is_none() {
                ui.set_operation_notice(words(language,"Cancellation requested. Waiting for owned work to settle; committed results are preserved.","Annulation demandée. En attente de la fin des traitements détenus ; les résultats enregistrés sont conservés.").into());
            }
            ui.set_operation_cancellable(!is_terminal(h.state) && !h.cancel_requested);
            ui.set_operation_retryable(
                h.kind == OperationKind::Import
                    && is_terminal(h.state)
                    && h.state != OperationState::Succeeded,
            );
            ui.set_operation_previous(details.page > 0);
            ui.set_operation_next(details.page + 1 < details.pages);
            let mut bytes = c.completed_bytes;
            for progress in &data.progress {
                if let Some((_, saved)) = c.running.iter().find(|(id, _)| *id == progress.item) {
                    bytes = bytes.saturating_add(progress.completed.saturating_sub(*saved).max(0));
                }
            }
            let summary = format!(
                "{} · {}\n{} / {} · {} / {} {}\n{}: {} · {}: {} · {}: {} · {}: {} · {}: {}\n{}: {} / {}\n{}",
                kind(h.kind, language),
                status(h.state, language),
                language.number(h.counts.completed),
                language.number(h.counts.total),
                language.number(bytes.min(c.total_bytes).max(0)),
                language.number(c.total_bytes),
                words(language, "bytes", "octets"),
                words(language, "Imported", "Importées"),
                language.number(c.succeeded),
                words(language, "Duplicates skipped", "Doublons ignorés"),
                language.number(c.skipped),
                words(language, "Conflicts", "Conflits"),
                language.number(c.conflicted),
                words(language, "Failed", "Échecs"),
                language.number(c.failed),
                words(language, "Cancelled", "Annulées"),
                language.number(c.cancelled),
                words(language, "Results page", "Page des résultats"),
                language.number(details.page as i64 + 1),
                language.number(details.pages as i64),
                reason(catalogs, language, &h.outcome, h.state.into())
            );
            ui.set_operation_summary(summary.into());
            let items = details
                .items
                .iter()
                .map(|item| {
                    let label = words(
                        language,
                        match item.state {
                            ItemState::Pending => "Queued",
                            ItemState::Running => "In progress",
                            ItemState::Succeeded => "Imported",
                            ItemState::Skipped => "Skipped",
                            ItemState::Conflicted => "UUID conflict",
                            ItemState::Failed => "Failed",
                            ItemState::Cancelled => "Cancelled",
                        },
                        match item.state {
                            ItemState::Pending => "En attente",
                            ItemState::Running => "En cours",
                            ItemState::Succeeded => "Importée",
                            ItemState::Skipped => "Ignorée",
                            ItemState::Conflicted => "Conflit d’UUID",
                            ItemState::Failed => "Échec",
                            ItemState::Cancelled => "Annulée",
                        },
                    );
                    format!(
                        "{} · {}\n{}",
                        item.story_title
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&item.source_name),
                        label,
                        reason(catalogs, language, &item.outcome, item.state.into())
                    )
                    .into()
                })
                .collect::<Vec<slint::SharedString>>();
            ui.set_operation_items(ModelRc::new(VecModel::from(items)));
        }
    }
}
fn send(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
) {
    state.borrow_mut().operations.local_error = None;
    if !action.bounded() {
        local_error(state, ui, LocalError::Selection);
        return;
    }
    let generation = state.borrow().snapshot.generation.clone();
    if let Some(generation) = generation {
        match worker
            .borrow()
            .offer(Command::Operations { generation, action })
        {
            Ok(revision) => {
                state.borrow_mut().operations.pending = Some(revision);
                ui.set_operation_busy(true);
            }
            Err(error) => local_error(state, ui, LocalError::Busy(error.to_string())),
        }
    }
}
/// The exact post-picker seam used by the real native dialog and owned fixtures.
pub(crate) fn selected(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    generation: LibraryGeneration,
    paths: Option<Vec<PathBuf>>,
) {
    ui.set_picker_open(false);
    if state.borrow().snapshot.generation.as_ref() != Some(&generation) {
        return;
    }
    if let Some(paths) = paths {
        send(state, worker, ui, Action::Import(paths));
    }
    ui.invoke_focus_operations();
}
fn pick(state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>, ui: &DesktopWindow) {
    if ui.get_picker_open() || ui.get_operation_busy() {
        return;
    }
    let Some(generation) = state.borrow().snapshot.generation.clone() else {
        return;
    };
    let french = ui.get_french();
    state.borrow_mut().operations.local_error = None;
    let dialog = rfd::AsyncFileDialog::new()
        .set_title(if french {
            "Importer des archives"
        } else {
            "Import archives"
        })
        .add_filter(
            if french {
                "Archives d’histoires"
            } else {
                "Story archives"
            },
            &["pk", "zip", "7z", "PK", "ZIP", "7Z"],
        )
        .add_filter(
            if french {
                "Tous les fichiers"
            } else {
                "All files"
            },
            &["*"],
        );
    let dialog = ui
        .window()
        .with_winit_window(|window| dialog.set_parent(window));
    let Some(dialog) = dialog else {
        local_error(&state, ui, LocalError::Picker("window_unavailable".into()));
        return;
    };
    ui.set_picker_open(true);
    let weak = ui.as_weak();
    let failure_state = state.clone();
    let result = slint::spawn_local(async move {
        let paths = dialog.pick_files().await.map(|files| {
            files
                .into_iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        if let Some(ui) = weak.upgrade() {
            selected(&state, &worker, &ui, generation, paths);
        }
    });
    if let Err(error) = result {
        ui.set_picker_open(false);
        local_error(&failure_state, ui, LocalError::Picker(error.to_string()));
    }
}
pub fn install(ui: &DesktopWindow, state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_pick_import(move || {
        if let Some(ui) = weak.upgrade() {
            pick(s.clone(), w.clone(), &ui);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_retry_import(move || {
        if let Some(ui) = weak.upgrade()
            && ui.get_operation_retryable()
        {
            pick(s.clone(), w.clone(), &ui);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_select_operation(move |index| {
        if let Some(ui) = weak.upgrade() {
            let id = usize::try_from(index)
                .ok()
                .and_then(|i| s.borrow().operations.keys.get(i).cloned());
            if let Some(id) = id {
                send(&s, &w, &ui, Action::Inspect { id, page: 0 });
            }
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_cancel_operation(move || {
        if let Some(ui) = weak.upgrade() {
            let id = s
                .borrow()
                .snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .map(|d| d.card.operation.identity.id.clone());
            if let Some(id) = id {
                send(&s, &w, &ui, Action::Cancel(id));
            }
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_operation_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let target = s
                .borrow()
                .snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .map(|d| {
                    (
                        d.card.operation.identity.id.clone(),
                        d.page.saturating_add_signed(delta as isize),
                    )
                });
            if let Some((id, page)) = target {
                send(&s, &w, &ui, Action::Inspect { id, page });
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_refresh_operations(move || {
        if let Some(ui) = weak.upgrade() {
            send(&state, &worker, &ui, Action::Refresh);
        }
    });
}
