//! Collection manager drafts, explicit repair/impact confirmation and union selection.
use super::View;
use librairii_app::localization::Language;
use librairii_core::{
    collection_codec::{Query, url::Scope},
    collections::Attention,
};
use librairii_desktop::{
    browsing::Group,
    presentation::{Command, Phase, Snapshot, Worker},
    workflows::collections::{Action, Feedback, Ticket},
};
use librairii_ui::native::{CollectionChoice, DesktopWindow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone, Copy, Debug)]
enum Pending {
    Inspect,
    Save,
    Plan,
    Confirm,
    Page,
    Union,
    Open,
    Edit,
}
#[derive(Debug, Default)]
pub struct State {
    selected: Option<i64>,
    union: Vec<i64>,
    pending: Option<(u64, Pending)>,
    feedback: Option<Feedback>,
    ticket: Option<Ticket>,
    reset: bool,
    background: bool,
}
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French { fr } else { en }.into()
}
fn groups(snapshot: &Snapshot) -> Vec<&Group> {
    snapshot
        .groups
        .iter()
        .filter(|g| matches!(g.scope, Scope::Collection(_)))
        .collect()
}
fn id(group: &Group) -> Option<i64> {
    if let Scope::Collection(id) = group.scope {
        Some(id.get())
    } else {
        None
    }
}
fn criteria(
    query: &Query,
    snapshot: &Snapshot,
    catalogs: &librairii_app::localization::Catalogs,
) -> String {
    let request = librairii_core::query::QueryRequest {
        criteria: query.clone(),
        ..Default::default()
    };
    let active =
        librairii_ui::filters::active(&request, &snapshot.facets.tags, snapshot.language, catalogs);
    if active.is_empty() {
        words(
            snapshot.language,
            "All active stories",
            "Toutes les histoires actives",
        )
    } else {
        active
            .iter()
            .map(|r| r.label.as_str())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
fn notice(feedback: Option<&Feedback>, language: Language) -> String {
    match feedback {
        None | Some(Feedback::Ready) => String::new(),
        Some(Feedback::Saved(_)) => words(language, "Collection saved.", "Collection enregistrée."),
        Some(Feedback::Deleted) => words(
            language,
            "Collection deleted. Approved Shelf stories were retained.",
            "Collection supprimée. Les histoires approuvées des Étagères sont conservées.",
        ),
        Some(Feedback::Error {
            uncertain: true, ..
        }) => words(
            language,
            "The change could not be confirmed. Recover the library before retrying.",
            "La modification n’a pas pu être confirmée. Récupérez la bibliothèque avant de réessayer.",
        ),
        Some(Feedback::Error { code, .. }) => match code.as_str() {
            "collection.invalid_name" => words(
                language,
                "Enter a name of 1–80 characters.",
                "Saisissez un nom de 1 à 80 caractères.",
            ),
            "collection.duplicate_name" => words(
                language,
                "This Collection name already exists. Choose a different name.",
                "Ce nom de Collection existe déjà. Choisissez un autre nom.",
            ),
            "collection.stale_preview" | "shelf.stale_review" => words(
                language,
                "The preview changed or expired. Cancel and review it again.",
                "L’aperçu a changé ou expiré. Annulez puis examinez-le à nouveau.",
            ),
            "collection.needs_attention" | "collection.criteria_unavailable" => words(
                language,
                "This Collection needs repair. Edit its saved query or explicitly replace it with current filters.",
                "Cette Collection nécessite une réparation. Modifiez sa requête ou remplacez-la explicitement par les filtres actuels.",
            ),
            "collection.invalid_query" => words(
                language,
                "These saved criteria cannot be decoded. Choose current filters, then explicitly replace the saved query.",
                "Ces critères enregistrés ne peuvent pas être décodés. Choisissez les filtres actuels, puis remplacez explicitement la requête.",
            ),
            "settings.busy" => words(
                language,
                "Library is busy. Try again.",
                "La bibliothèque est occupée. Réessayez.",
            ),
            _ => words(
                language,
                "The Collection change could not be completed. Refresh and retry.",
                "La modification de la Collection a échoué. Actualisez puis réessayez.",
            ),
        },
    }
}
impl State {
    pub(super) fn selected(&self) -> Option<i64> {
        self.selected
    }
    pub fn open(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        self.feedback = None;
        self.ticket = None;
        self.background = false;
        self.reset = self
            .selected
            .is_some_and(|selected| !groups(snapshot).iter().any(|g| id(g) == Some(selected)));
        if self.selected.is_none() {
            ui.set_collection_name("".into());
        }
    }
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let Some((revision, kind)) = self.pending else {
            return;
        };
        if snapshot.workflows.collections.revision < revision {
            if snapshot.phase == Phase::Recovery {
                self.pending = None;
                ui.set_collection_busy(false);
                self.feedback = Some(Feedback::Error {
                    code: "storage.runtime_fenced".into(),
                    uncertain: true,
                });
            }
            return;
        }
        self.pending = None;
        ui.set_collection_busy(false);
        self.feedback = snapshot.workflows.collections.feedback.clone();
        match (&self.feedback, kind) {
            (Some(Feedback::Saved(id)), _) => {
                self.selected = Some(*id);
                self.reset = true;
                self.ticket = None;
                if ui.get_dialog() == 14 {
                    ui.invoke_finish_collection_confirmation();
                }
            }
            (Some(Feedback::Deleted), _) => {
                self.selected = None;
                self.reset = true;
                self.ticket = None;
                ui.invoke_finish_collection_confirmation();
            }
            (Some(Feedback::Ready), Pending::Plan) => {
                if let Some(preview) = &snapshot.workflows.collections.preview {
                    self.ticket = Some(preview.ticket.clone());
                    ui.set_collection_replacing(preview.replacing);
                    ui.invoke_show_collection_confirmation();
                }
            }
            (Some(Feedback::Ready), Pending::Open) => {
                if ui.get_dialog() == 13 {
                    ui.invoke_close_dialog();
                }
            }
            (Some(Feedback::Ready), Pending::Edit) => {
                ui.invoke_close_dialog();
                ui.invoke_open_dialog(8);
            }
            _ => {}
        }
    }
    pub fn project(
        &mut self,
        ui: &DesktopWindow,
        snapshot: &Snapshot,
        catalogs: &librairii_app::localization::Catalogs,
        query: &Query,
    ) {
        let choices = groups(snapshot);
        self.union
            .retain(|selected| choices.iter().any(|g| id(g) == Some(*selected)));
        if self
            .selected
            .is_some_and(|selected| !choices.iter().any(|g| id(g) == Some(selected)))
        {
            self.selected = None;
            self.reset = true;
        }
        let index = choices.iter().position(|g| id(g) == self.selected);
        if self.reset {
            self.reset = false;
            ui.set_collection_name(index.map(|i| choices[i].name.as_str()).unwrap_or("").into());
        }
        ui.set_collection_definitions(ModelRc::new(VecModel::from(
            std::iter::once(
                words(snapshot.language, "New Collection…", "Nouvelle Collection…").into(),
            )
            .chain(choices.iter().map(|g| g.name.clone().into()))
            .collect::<Vec<_>>(),
        )));
        ui.set_collection_index(index.map_or(0, |i| (i + 1) as i32));
        ui.set_collection_creating(index.is_none());
        ui.set_collection_union_selected(self.selected.is_some_and(|id| self.union.contains(&id)));
        ui.set_collection_openable(index.is_some_and(|i| choices[i].available));
        ui.set_collection_move_up(index.is_some_and(|i| i > 0));
        ui.set_collection_move_down(index.is_some_and(|i| i + 1 < choices.len()));
        ui.set_collection_criteria(criteria(query, snapshot, catalogs).into());
        let inspection = snapshot
            .workflows
            .collections
            .inspection
            .as_ref()
            .filter(|i| Some(i.collection.id) == self.selected);
        let status = if index.is_none() {
            words(
                snapshot.language,
                "Save a copied query. Archives and approved Shelves stay independent.",
                "Enregistrez une copie des critères. Les archives et les Étagères approuvées restent indépendantes.",
            )
        } else if let Some(reason) = inspection.and_then(|i| i.attention) {
            match reason {
                Attention::MissingCriteria => words(
                    snapshot.language,
                    "Saved tag criteria are unavailable. Explicit repair is required.",
                    "Des critères d’étiquette sont indisponibles. Une réparation explicite est nécessaire.",
                ),
                Attention::UnmigratableQuery => words(
                    snapshot.language,
                    "The saved query is corrupt or unsupported. Its original bytes are retained until explicit replacement.",
                    "La requête est corrompue ou non prise en charge. Ses octets d’origine sont conservés jusqu’au remplacement explicite.",
                ),
            }
        } else if index.is_some_and(|i| !choices[i].available) {
            words(
                snapshot.language,
                "Needs repair. Nothing is silently removed from this saved query.",
                "Réparation nécessaire. Aucun critère n’est retiré silencieusement de cette requête.",
            )
        } else {
            words(
                snapshot.language,
                "Live saved query · membership updates as stories match.",
                "Requête enregistrée dynamique · les correspondances sont actualisées automatiquement.",
            )
        };
        ui.set_collection_status(status.into());
        let mut text = if self.pending.is_some() {
            words(snapshot.language, "Working…", "Traitement…")
        } else {
            notice(self.feedback.as_ref(), snapshot.language)
        };
        if snapshot.workflows.collections.navigation_error {
            text.push_str(&words(snapshot.language," Saved changes are retained, but the view could not open. Reopen it to retry."," Les modifications sont enregistrées, mais la vue n’a pas pu être ouverte. Rouvrez-la pour réessayer."));
        }
        if self.background
            && snapshot.workflows.collections.revision == snapshot.revision
            && matches!(self.feedback, Some(Feedback::Error { .. }))
        {
            ui.set_status(text.clone().into());
        }
        ui.set_collection_notice(text.into());
        ui.set_collection_choices(ModelRc::new(VecModel::from(
            choices
                .iter()
                .enumerate()
                .map(|(index, g)| CollectionChoice {
                    index: index as i32,
                    label: format!(
                        "{} ({}){}",
                        g.name,
                        snapshot.language.number(g.count),
                        if g.available { "" } else { " · ⚠" }
                    )
                    .into(),
                    selected: id(g).is_some_and(|id| self.union.contains(&id)),
                    available: g.available,
                })
                .collect::<Vec<_>>(),
        )));
        let union = snapshot
            .workflows
            .collections
            .union
            .as_ref()
            .filter(|u| u.sources.iter().map(|s| s.0).eq(self.union.iter().copied()));
        ui.set_collection_union_available(union.is_some() && !self.union.is_empty());
        ui.set_collection_union_active(!self.union.is_empty());
        let summary = union
            .map(|u| {
                format!(
                    "{}\n{} {} · {} {}",
                    u.sources
                        .iter()
                        .map(|(_, name, count)| format!(
                            "{name}: {}",
                            snapshot.language.number(*count)
                        ))
                        .collect::<Vec<_>>()
                        .join(" · "),
                    snapshot.language.number(u.unique),
                    if snapshot.language.singular(u.unique as u64) {
                        words(snapshot.language, "unique story", "histoire unique")
                    } else {
                        words(snapshot.language, "unique stories", "histoires uniques")
                    },
                    snapshot.language.number(u.overlap),
                    if snapshot.language.singular(u.overlap as u64) {
                        words(
                            snapshot.language,
                            "overlapping membership collapsed",
                            "correspondance commune dédupliquée",
                        )
                    } else {
                        words(
                            snapshot.language,
                            "overlapping memberships collapsed",
                            "correspondances communes dédupliquées",
                        )
                    }
                )
            })
            .unwrap_or_default();
        ui.set_collection_union_summary(if snapshot.workflows.collections.union_error && !self.union.is_empty() {
            words(snapshot.language,"The combined preview could not be loaded. Repair unavailable Collections or change the selection.","L’aperçu combiné n’a pas pu être chargé. Réparez les Collections indisponibles ou modifiez la sélection.")
        } else {summary}.into());
        if let Some(preview) = &snapshot.workflows.collections.preview
            && self.ticket.as_ref() == Some(&preview.ticket)
        {
            let mut text = if preview.replacing {
                format!(
                    "{} “{}” ?\n{}",
                    words(
                        snapshot.language,
                        "Replace saved query in",
                        "Remplacer la requête de"
                    ),
                    preview.name,
                    preview
                        .replacement
                        .as_ref()
                        .map(|q| criteria(q, snapshot, catalogs))
                        .unwrap_or_default()
                )
            } else {
                format!(
                    "{} “{}” ?\n{} {}\n{}",
                    words(
                        snapshot.language,
                        "Delete Collection",
                        "Supprimer la Collection"
                    ),
                    preview.name,
                    snapshot.language.number(preview.shelf_count as i64),
                    if snapshot.language.singular(preview.shelf_count as u64) {
                        words(snapshot.language, "linked Shelf", "Étagère liée")
                    } else {
                        words(snapshot.language, "linked Shelves", "Étagères liées")
                    },
                    words(
                        snapshot.language,
                        "All approved stories will be kept. Only the Collection and its active source links are removed. Archives are unchanged.",
                        "Toutes les histoires approuvées seront conservées. Seules la Collection et ses sources actives sont retirées. Les archives restent inchangées."
                    )
                )
            };
            for (name, count, incomplete) in &preview.shelves {
                text.push_str(&format!(
                    "\n{name} · {} {}{}",
                    snapshot.language.number(*count as i64),
                    if snapshot.language.singular(*count as u64) {
                        words(snapshot.language, "affected member", "membre concerné")
                    } else {
                        words(snapshot.language, "affected members", "membres concernés")
                    },
                    if *incomplete {
                        words(
                            snapshot.language,
                            " · incomplete coverage; kept",
                            " · couverture incomplète ; conservés",
                        )
                    } else {
                        String::new()
                    }
                ));
            }
            ui.set_collection_impact(text.into());
            ui.set_collection_impact_count(preview.shelf_count as i32);
            ui.set_collection_impact_previous(preview.page > 0);
            ui.set_collection_impact_next(preview.page + 1 < preview.shelf_count);
        }
    }
}
fn send(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
    kind: Pending,
) {
    let mut state = view.borrow_mut();
    if state.collections.pending.is_some() {
        return;
    }
    let Some(generation) = state.snapshot.generation.clone() else {
        return;
    };
    match worker
        .borrow()
        .offer(Command::Collections { generation, action })
    {
        Ok(revision) => {
            state.collections.pending = Some((revision, kind));
            ui.set_collection_busy(true);
        }
        Err(_) => {
            state.collections.feedback = Some(Feedback::Error {
                code: "settings.busy".into(),
                uncertain: false,
            })
        }
    };
    state.project(ui);
}
pub fn open_group(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    id: i64,
) {
    view.borrow_mut().collections.background = true;
    send(view, worker, ui, Action::Open(id), Pending::Open);
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_select_collection(move |index| {
        if let Some(ui) = weak.upgrade() {
            let id = {
                let mut state = state.borrow_mut();
                state.collections.selected = index
                    .checked_sub(1)
                    .and_then(|i| usize::try_from(i).ok())
                    .and_then(|i| groups(&state.snapshot).get(i).and_then(|g| id(g)));
                state.collections.reset = true;
                state.project(&ui);
                state.collections.selected
            };
            if let Some(id) = id {
                send(&state, &pending, &ui, Action::Inspect(id), Pending::Inspect);
            }
        }
    });
    for kind in 0..3 {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                let name = ui.get_collection_name().to_string();
                if name.len() > 320 {
                    let mut state = state.borrow_mut();
                    state.collections.feedback = Some(Feedback::Error {
                        code: "collection.invalid_name".into(),
                        uncertain: false,
                    });
                    state.project(&ui);
                    return;
                }
                let (id, query, union) = {
                    let state = state.borrow();
                    (
                        state.collections.selected,
                        state.filters.request.criteria.clone(),
                        state.collections.union.clone(),
                    )
                };
                let action = match (kind, id) {
                    (0, None) if union.is_empty() => Some(Action::Create {
                        name,
                        criteria: query,
                    }),
                    (0, Some(id)) => Some(Action::Rename { id, name }),
                    (1, Some(id)) => Some(Action::Duplicate { id, name }),
                    _ => {
                        let state = state.borrow();
                        state
                            .snapshot
                            .workflows
                            .collections
                            .union
                            .as_ref()
                            .filter(|u| {
                                u.sources.iter().map(|s| s.0).eq(state
                                    .collections
                                    .union
                                    .iter()
                                    .copied())
                            })
                            .map(|_| Action::SaveUnion { name, ids: union })
                    }
                };
                if let Some(action) = action {
                    send(&state, &pending, &ui, action, Pending::Save);
                }
            }
        };
        match kind {
            0 => ui.on_save_collection(callback),
            1 => ui.on_duplicate_collection(callback),
            _ => ui.on_save_collection_union(callback),
        }
    }
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_move_collection(move |offset| {
        if let Some(ui) = weak.upgrade() {
            let id = state.borrow().collections.selected;
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Move { id, offset },
                    Pending::Save,
                );
            }
        }
    });
    for kind in 0..4 {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                let (id, query) = {
                    let state = state.borrow();
                    (
                        state.collections.selected,
                        state.filters.request.criteria.clone(),
                    )
                };
                if let Some(id) = id {
                    let (action, kind) = match kind {
                        0 => (Action::Open(id), Pending::Open),
                        1 => (
                            Action::PlanReplace {
                                id,
                                criteria: query,
                            },
                            Pending::Plan,
                        ),
                        2 => (Action::PlanDelete(id), Pending::Plan),
                        _ => (Action::EditQuery(id), Pending::Edit),
                    };
                    send(&state, &pending, &ui, action, kind);
                }
            }
        };
        match kind {
            0 => ui.on_open_collection(callback),
            1 => ui.on_replace_collection(callback),
            2 => ui.on_delete_collection(callback),
            _ => ui.on_edit_saved_collection_query(callback),
        }
    }
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_confirm_collection_change(move || {
        if let Some(ui) = weak.upgrade() {
            let ticket = state.borrow().collections.ticket.clone();
            if let Some(ticket) = ticket {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Confirm(ticket),
                    Pending::Confirm,
                );
            }
        }
    });
    let state = view.clone();
    ui.on_cancel_collection_change(move || state.borrow_mut().collections.ticket = None);
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_collection_impact_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let target = {
                let state = state.borrow();
                state.collections.ticket.clone().zip(
                    state
                        .snapshot
                        .workflows
                        .collections
                        .preview
                        .as_ref()
                        .map(|p| p.page.saturating_add_signed(delta as isize)),
                )
            };
            if let Some((ticket, page)) = target {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::PreviewPage { ticket, page },
                    Pending::Page,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_toggle_collection_union(move |index| {
        if let Some(ui) = weak.upgrade() {
            let ids = {
                let mut state = state.borrow_mut();
                let selected = usize::try_from(index)
                    .ok()
                    .and_then(|i| groups(&state.snapshot).get(i).and_then(|g| id(g)));
                if let Some(selected) = selected {
                    if state.collections.union.contains(&selected) {
                        state.collections.union.retain(|id| *id != selected);
                    } else {
                        state.collections.union.push(selected);
                    }
                }
                state.collections.union.clone()
            };
            send(
                &state,
                &pending,
                &ui,
                Action::PreviewUnion(ids),
                Pending::Union,
            );
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_toggle_selected_collection_union(move || {
        if let Some(ui) = weak.upgrade() {
            let index = {
                let state = state.borrow();
                groups(&state.snapshot)
                    .iter()
                    .position(|g| id(g) == state.collections.selected)
            };
            if let Some(index) = index {
                ui.invoke_toggle_collection_union(index as i32);
            }
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_edit_current_collection_query(move || {
        if let Some(ui) = weak.upgrade() {
            state.borrow_mut().collections.feedback = None;
            ui.invoke_close_dialog();
            ui.invoke_open_dialog(8);
        }
    });
    let (state, pending, weak) = (view, worker, ui.as_weak());
    ui.on_open_collection_union(move || {
        if let Some(ui) = weak.upgrade() {
            let ids = state.borrow().collections.union.clone();
            if !ids.is_empty() {
                send(&state, &pending, &ui, Action::OpenUnion(ids), Pending::Open);
            }
        }
    });
}
