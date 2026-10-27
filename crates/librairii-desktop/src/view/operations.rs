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
    workflows::operations::{Action, HistoryAction},
};
use librairii_ui::native::{DesktopWindow, ReportGroup};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
#[derive(Debug, Default)]
pub struct State {
    pending: Option<u64>,
    keys: Vec<String>,
    local_error: Option<LocalError>,
    history_keys: Vec<String>,
    notice_ids: Vec<String>,
    notice_index: usize,
    requested_report: Option<(String, Option<u64>)>,
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
fn replace_strings(
    current: ModelRc<slint::SharedString>,
    rows: Vec<slint::SharedString>,
) -> ModelRc<slint::SharedString> {
    if let Some(model) = current
        .as_any()
        .downcast_ref::<VecModel<slint::SharedString>>()
        .filter(|m| m.row_count() == rows.len())
    {
        for (i, row) in rows.into_iter().enumerate() {
            if model.row_data(i).as_ref() != Some(&row) {
                model.set_row_data(i, row);
            }
        }
        current
    } else {
        ModelRc::new(VecModel::from(rows))
    }
}
impl State {
    pub fn follow_started_export(&mut self) {
        self.requested_report = None;
    }
    fn project_activity(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let data = &snapshot.workflows.operations;
        let l = snapshot.language;
        let history = &data.history;
        // Hand native focus to a surviving control before Retry becomes hidden.
        // A deferred Slint `changed error` alone leaves a gap where Escape has
        // no visible focused ancestor after the successful worker response.
        if history.error.is_none()
            && !ui.get_history_error().is_empty()
            && ui.get_history_retry_focused()
        {
            ui.invoke_focus_history_refresh();
        }
        self.history_keys = history
            .cards
            .iter()
            .map(|c| c.operation.identity.id.clone())
            .collect();
        let entries = history
            .cards
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let h = &c.operation;
                let date = history
                    .dates
                    .get(i)
                    .copied()
                    .flatten()
                    .map(|at| l.date(at))
                    .unwrap_or_else(|| format!("{} UTC", h.identity.created_at));
                format!(
                    "{} · {} · {date}\n{} / {} · {}",
                    kind(h.kind, l),
                    status(h.state, l),
                    l.number(h.counts.completed),
                    l.number(h.counts.total),
                    h.identity.id
                )
                .into()
            })
            .collect();
        ui.set_history_entries(replace_strings(ui.get_history_entries(), entries));
        ui.set_history_summary(if history.cards.is_empty(){words(l,"No activity yet. Import stories or request a sync to get started.","Aucune activité pour le moment. Importez des histoires ou lancez une synchronisation.")}else{format!("{} {} · {} {}",words(l,"History page","Page de l’historique"),l.number(history.page as i64+1),l.number(history.cards.len() as i64),words(l,"operations","opérations"))}.into());
        ui.set_history_older(history.next_before.is_some() && history.error.is_none());
        ui.set_history_error(history.error.as_ref().map(|e|format!("{} [{e}]",words(l,"History could not be loaded. Try again or refresh; the previous page is retained.","Impossible de charger l’historique. Réessayez ou actualisez ; la page précédente est conservée."))).unwrap_or_default().into());
        let mut notices = data
            .cards
            .iter()
            .filter(|c| {
                is_terminal(c.operation.state)
                    && !data.dismissed_operations.contains(&c.operation.identity.id)
            })
            .collect::<Vec<_>>();
        notices.sort_by(|a, b| {
            b.operation
                .identity
                .created_at
                .cmp(&a.operation.identity.created_at)
                .then_with(|| b.operation.identity.id.cmp(&a.operation.identity.id))
        });
        self.notice_ids = notices
            .iter()
            .map(|c| c.operation.identity.id.clone())
            .collect();
        self.notice_index = self.notice_index.min(notices.len().saturating_sub(1));
        ui.set_notice_index(self.notice_index as i32);
        ui.set_notice_count(notices.len() as i32);
        ui.set_terminal_notice_error(data.notice_error.as_ref().map(|e|format!("{} [{e}]",if e.contains("resource_limit"){words(l,"The dismissal limit is reached. Earlier dismissals and operation records are retained.","La limite des notifications fermées est atteinte. Les choix précédents et les opérations sont conservés.")}else{words(l,"Notice dismissed for this session. Preferences could not be saved; the prior file is preserved.","Notification fermée pour cette session. Les préférences n’ont pas pu être enregistrées ; le fichier précédent est conservé.")})).unwrap_or_default().into());
        ui.set_terminal_notice(
            notices
                .get(self.notice_index)
                .map(|c| {
                    format!(
                        "{} · {} · {} / {} · {} / {}",
                        kind(c.operation.kind, l),
                        status(c.operation.state, l),
                        l.number(c.operation.counts.completed),
                        l.number(c.operation.counts.total),
                        l.number(self.notice_index as i64 + 1),
                        l.number(notices.len() as i64)
                    )
                })
                .unwrap_or_default()
                .into(),
        );
    }
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot, catalogs: &Catalogs) {
        let data = &snapshot.workflows.operations;
        let language = snapshot.language;
        if self.pending.is_some_and(|r| data.revision >= r) {
            self.pending = None;
        }
        ui.set_operation_busy(self.pending.is_some());
        if self
            .requested_report
            .as_ref()
            .is_some_and(|(id, revision)| {
                revision.is_some_and(|r| data.revision >= r)
                    && data
                        .details
                        .as_ref()
                        .is_some_and(|d| d.card.operation.identity.id == *id)
            })
        {
            self.requested_report = None;
        }
        let loading = self.pending.is_some() || self.requested_report.is_some();
        ui.set_operation_loading(loading);
        self.project_activity(ui, snapshot);
        ui.set_commit_pending(data.critical || snapshot.workflows.exports.recovery_required);
        let selected = data
            .details
            .as_ref()
            .map(|d| d.card.operation.identity.id.as_str());
        let mut cards = data.cards.iter().collect::<Vec<_>>();
        if let Some(details) = &data.details
            && !cards
                .iter()
                .any(|c| c.operation.identity.id == details.card.operation.identity.id)
        {
            cards.push(&details.card);
        }
        self.keys = cards
            .iter()
            .map(|c| c.operation.identity.id.clone())
            .collect();
        let choices = cards
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
        ui.set_operation_choices(replace_strings(ui.get_operation_choices(), choices));
        ui.set_operation_index(
            selected
                .and_then(|id| self.keys.iter().position(|key| key == id))
                .map_or(-1, |i| i as i32),
        );
        ui.set_operation_choice(
            usize::try_from(ui.get_operation_index())
                .ok()
                .and_then(|index| ui.get_operation_choices().row_data(index))
                .unwrap_or_default(),
        );
        ui.invoke_sync_operation_choice();
        let mut notice=data.error.as_ref().map(|error|format!("{}\n{error}",words(language,"The operation could not be started or updated. Refresh and try again.","L’opération n’a pas pu être démarrée ou actualisée. Actualisez puis réessayez."))).unwrap_or_default();
        if data.preparing.is_some() {
            notice = words(
                language,
                "Preparing the operation…",
                "Préparation de l’opération…",
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
        ui.set_pending(
            active > 0
                || data.status.is_some_and(|s| s.queued + s.running > 0)
                || snapshot.workflows.exports.recovery_required,
        );
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
                words(language, "Active operations", "Opérations actives"),
                language.number(active as i64),
                language.number(completed),
                language.number(total),
                words(language, "items", "éléments")
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
                    "Operations continue in the background · cancellation preserves completed results",
                    "Les opérations continuent en arrière-plan · l’annulation conserve les résultats enregistrés"
                )
            )
            .into(),
        );
        ui.set_operation_revealable(false);
        ui.set_operation_cancellable(false);
        ui.set_operation_retryable(false);
        ui.set_operation_retry_label(
            words(
                language,
                "Retry · select files again…",
                "Réessayer · sélectionner les fichiers…",
            )
            .into(),
        );
        ui.set_operation_previous(false);
        ui.set_operation_next(false);
        ui.set_operation_groups(ModelRc::default());
        ui.set_operation_items(ModelRc::default());
        ui.set_operation_summary("".into());
        if loading {
            ui.set_operation_summary(
                words(
                    language,
                    if self.pending.is_some() {
                        "Loading the requested operation…"
                    } else {
                        "The requested report is unavailable. Return to History and refresh."
                    },
                    if self.pending.is_some() {
                        "Chargement de l’opération demandée…"
                    } else {
                        "Le rapport demandé est indisponible. Revenez à l’historique et actualisez."
                    },
                )
                .into(),
            );
            return;
        }
        if let Some(details) = &data.details {
            let c = &details.card;
            let h = &c.operation;
            if h.cancel_requested && !is_terminal(h.state) && self.local_error.is_none() {
                ui.set_operation_notice(words(language,"Cancellation requested. Waiting for owned work to settle; committed results are preserved.","Annulation demandée. En attente de la fin des traitements détenus ; les résultats enregistrés sont conservés.").into());
            }
            ui.set_operation_revealable(
                h.kind == OperationKind::Export && is_terminal(h.state) && h.destination.is_some(),
            );
            ui.set_operation_cancellable(!is_terminal(h.state) && !h.cancel_requested);
            ui.set_operation_retryable(
                is_terminal(h.state) && h.state != OperationState::Succeeded,
            );
            ui.set_operation_retry_label(
                match h.kind {
                    OperationKind::Import => words(
                        language,
                        "Retry · select files again…",
                        "Réessayer · sélectionner les fichiers…",
                    ),
                    OperationKind::MetadataSync => words(
                        language,
                        "Retry official metadata sync",
                        "Réessayer la synchronisation officielle",
                    ),
                    OperationKind::Export => words(
                        language,
                        "Prepare a new export · choose scope and destination…",
                        "Préparer un nouvel export · choisir la portée et la destination…",
                    ),
                }
                .into(),
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
                match h.kind {
                    OperationKind::Import => words(language, "Imported", "Importées"),
                    OperationKind::Export => words(language, "Exported", "Exportées"),
                    OperationKind::MetadataSync => words(language, "Refreshed", "Actualisées"),
                },
                language.number(c.succeeded),
                if h.kind == OperationKind::Import {
                    words(language, "Duplicates skipped", "Doublons ignorés")
                } else {
                    words(language, "Skipped", "Ignorées")
                },
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
            let source = h
                .export_scope
                .source_type()
                .ok()
                .flatten()
                .map(|source_type| {
                    use librairii_core::operations::ExportSourceType as T;
                    let name = match source_type {
                        T::Selection => {
                            words(language, "Selected stories", "Histoires sélectionnées")
                        }
                        T::CurrentQuery => words(language, "Current results", "Résultats actuels"),
                        T::Shelf | T::Shelves | T::Collection | T::Collections => {
                            words(language, "Collections", "Collections")
                        }
                        T::CuratedShelf | T::CuratedShelves => {
                            words(language, "Approved Shelves", "Étagères approuvées")
                        }
                    };
                    let names = h
                        .export_scope
                        .group_names
                        .as_deref()
                        .map(|raw| {
                            serde_json::from_str::<Vec<String>>(raw)
                                .map(|names| names.join(" · "))
                                .unwrap_or_else(|_| raw.into())
                        })
                        .unwrap_or_default();
                    format!("{}: {name} {names}\n", words(language, "Source", "Source"))
                })
                .unwrap_or_default();
            ui.set_operation_summary(format!("{source}{summary}").into());
            let items = details
                .items
                .iter()
                .map(|item| {
                    let label = words(
                        language,
                        match item.state {
                            ItemState::Pending => "Queued",
                            ItemState::Running => "In progress",
                            ItemState::Succeeded => match h.kind {
                                OperationKind::Import => "Imported",
                                OperationKind::Export => "Exported",
                                OperationKind::MetadataSync => "Refreshed",
                            },
                            ItemState::Skipped => "Skipped",
                            ItemState::Conflicted => {
                                if h.kind == OperationKind::Export {
                                    "Filename conflict"
                                } else {
                                    "UUID conflict"
                                }
                            }
                            ItemState::Failed => "Failed",
                            ItemState::Cancelled => "Cancelled",
                        },
                        match item.state {
                            ItemState::Pending => "En attente",
                            ItemState::Running => "En cours",
                            ItemState::Succeeded => match h.kind {
                                OperationKind::Import => "Importée",
                                OperationKind::Export => "Exportée",
                                OperationKind::MetadataSync => "Actualisée",
                            },
                            ItemState::Skipped => "Ignorée",
                            ItemState::Conflicted => {
                                if h.kind == OperationKind::Export {
                                    "Conflit de nom de fichier"
                                } else {
                                    "Conflit d’UUID"
                                }
                            }
                            ItemState::Failed => "Échec",
                            ItemState::Cancelled => "Annulée",
                        },
                    );
                    let identity = if h.kind == OperationKind::Export {
                        let mut completed = item.progress.completed;
                        if item.state == ItemState::Running
                            && c.running.iter().any(|(id, _)| *id == item.identity.id)
                            && let Some(progress) =
                                data.progress.iter().find(|p| p.item == item.identity.id)
                        {
                            completed = completed.max(progress.completed).min(item.progress.total);
                        }
                        format!(
                            "\n{} · {}\n{} / {} {}",
                            item.identity.story_uuid.as_deref().unwrap_or(""),
                            item.output_name
                                .as_ref()
                                .map(|p| p.display_name())
                                .unwrap_or_default(),
                            language.number(completed.max(0)),
                            language.number(item.progress.total),
                            words(language, "bytes", "octets")
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        "{} · {}{}\n{}",
                        item.story_title
                            .as_deref()
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&item.source_name),
                        label,
                        identity,
                        reason(catalogs, language, &item.outcome, item.state.into())
                    )
                    .into()
                })
                .collect::<Vec<slint::SharedString>>();
            if h.kind == OperationKind::Export {
                let categories = [
                    (ItemState::Succeeded, "Exported", "Exportées", c.succeeded),
                    (ItemState::Skipped, "Skipped", "Ignorées", c.skipped),
                    (
                        ItemState::Conflicted,
                        "Conflicted",
                        "En conflit",
                        c.conflicted,
                    ),
                    (ItemState::Failed, "Failed", "Échecs", c.failed),
                    (ItemState::Cancelled, "Cancelled", "Annulées", c.cancelled),
                    (
                        ItemState::Running,
                        "In progress",
                        "En cours",
                        c.running.len() as i64,
                    ),
                    (
                        ItemState::Pending,
                        "Queued",
                        "En attente",
                        h.counts.total - h.counts.completed - c.running.len() as i64,
                    ),
                ];
                let groups = categories
                    .into_iter()
                    .filter_map(|(state, en, fr, count)| {
                        let rows = details
                            .items
                            .iter()
                            .zip(&items)
                            .filter(|(i, _)| i.state == state)
                            .map(|(_, text)| text.clone())
                            .collect::<Vec<_>>();
                        if rows.is_empty() {
                            return None;
                        }
                        Some(ReportGroup {
                            heading: format!(
                                "{} · {} {}",
                                words(language, en, fr),
                                language.number(count),
                                words(language, "total", "au total")
                            )
                            .into(),
                            entries: ModelRc::new(VecModel::from(rows)),
                        })
                    })
                    .collect::<Vec<_>>();
                ui.set_operation_groups(ModelRc::new(VecModel::from(groups)));
            }
            ui.set_operation_items(ModelRc::new(VecModel::from(items)));
        }
    }
}
pub(super) fn send(
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
    let inspecting = if let Action::Inspect { id, .. } = &action {
        Some(id.clone())
    } else {
        None
    };
    if let Some(id) = &inspecting {
        state.borrow_mut().operations.requested_report = Some((id.clone(), None));
    } else if matches!(&action, Action::Import(_) | Action::Sync) {
        state.borrow_mut().operations.requested_report = None;
    }
    if inspecting.is_some() {
        ui.set_operation_loading(true);
        ui.set_operation_summary(
            words(
                if ui.get_french() {
                    Language::French
                } else {
                    Language::English
                },
                "Loading the requested operation…",
                "Chargement de l’opération demandée…",
            )
            .into(),
        );
        ui.set_operation_items(ModelRc::default());
        ui.set_operation_groups(ModelRc::default());
    }
    let generation = state.borrow().snapshot.generation.clone();
    if let Some(generation) = generation {
        match worker
            .borrow()
            .offer(Command::Operations { generation, action })
        {
            Ok(revision) => {
                state.borrow_mut().operations.pending = Some(revision);
                if let Some(id) = inspecting {
                    state.borrow_mut().operations.requested_report = Some((id, Some(revision)));
                }
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
            && !ui.get_operation_loading()
        {
            let kind = s
                .borrow()
                .snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .map(|d| d.card.operation.kind);
            match kind {
                Some(OperationKind::Import) => pick(s.clone(), w.clone(), &ui),
                Some(OperationKind::MetadataSync) => send(&s, &w, &ui, Action::Sync),
                Some(OperationKind::Export) => ui.invoke_prepare_new_export(),
                None => {}
            }
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
        if let Some(ui) = weak.upgrade()
            && !ui.get_operation_loading()
        {
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
        if let Some(ui) = weak.upgrade()
            && !ui.get_operation_loading()
        {
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
    for (action, install) in [
        (HistoryAction::Open, 0),
        (HistoryAction::Refresh, 1),
        (HistoryAction::Older, 2),
        (HistoryAction::Retry, 3),
    ] {
        let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                send(&s, &w, &ui, Action::History(action));
            }
        };
        match install {
            0 => ui.on_open_history(callback),
            1 => ui.on_history_refresh(callback),
            2 => ui.on_history_load_older(callback),
            _ => ui.on_history_retry(callback),
        }
    }
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_inspect_history(move |i| {
        let id = usize::try_from(i)
            .ok()
            .and_then(|i| s.borrow().operations.history_keys.get(i).cloned());
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            ui.invoke_show_history_report(i);
            send(&s, &w, &ui, Action::Inspect { id, page: 0 });
        }
    });
    let (s, weak) = (state.clone(), ui.as_weak());
    ui.on_notice_step(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let mut v = s.borrow_mut();
            v.operations.notice_index = v
                .operations
                .notice_index
                .saturating_add_signed(delta as isize);
            let snapshot = v.snapshot.clone();
            v.operations.project_activity(&ui, &snapshot);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_show_notice(move || {
        let id = {
            let v = s.borrow();
            v.operations
                .notice_ids
                .get(v.operations.notice_index)
                .cloned()
        };
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            send(&s, &w, &ui, Action::Inspect { id, page: 0 });
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_dismiss_notice(move || {
        let id = {
            let v = s.borrow();
            v.operations
                .notice_ids
                .get(v.operations.notice_index)
                .cloned()
        };
        if let (Some(id), Some(ui)) = (id, weak.upgrade()) {
            send(&s, &w, &ui, Action::Dismiss(id));
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_open_operations(move || {
        if let Some(ui) = weak.upgrade() {
            s.borrow_mut().operations.requested_report = None;
            send(&s, &w, &ui, Action::Refresh);
        }
    });
    let weak = ui.as_weak();
    ui.on_refresh_operations(move || {
        if let Some(ui) = weak.upgrade() {
            let target = if ui.get_dialog() == 19 {
                let v = state.borrow();
                v.operations
                    .requested_report
                    .as_ref()
                    .map(|(id, _)| (id.clone(), 0))
                    .or_else(|| {
                        v.snapshot
                            .workflows
                            .operations
                            .details
                            .as_ref()
                            .map(|d| (d.card.operation.identity.id.clone(), d.page))
                    })
            } else {
                None
            };
            let action = target.map_or(Action::Refresh, |(id, page)| Action::Inspect { id, page });
            send(&state, &worker, &ui, action);
        }
    });
}
