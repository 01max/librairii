//! Native Shelf drafts and bounded, explicitly approved creation/curation plans.
use super::View;
use librairii_app::localization::Language;
use librairii_core::{
    CollectionId, ShelfId,
    collection_codec::{Query, url::Scope},
    query::QueryRequest,
};
use librairii_desktop::{
    browsing::Group,
    presentation::{Command, Phase, Snapshot, Worker},
    workflows::{
        Targets,
        shelves::{Action, Feedback, Kind, Ticket},
    },
};
use librairii_ui::native::{DesktopWindow, ShelfPreviewRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, Copy, Debug)]
enum Pending {
    Read,
    Save,
    Plan,
    EditPlan,
    Confirm,
    Move,
}
#[derive(Debug, Default)]
pub struct State {
    selected: Option<ShelfId>,
    sources: Vec<CollectionId>,
    source_choices: Vec<CollectionId>,
    source_loaded: bool,
    linked_sources: Vec<librairii_core::shelves::Source>,
    manual: Option<Targets>,
    pending: Option<(u64, Pending)>,
    feedback: Option<Feedback>,
    ticket: Option<Ticket>,
    plan: Option<Action>,
    reset: bool,
    focus: bool,
    exclusion_focus: bool,
    refresh_failed: bool,
    navigation_error: bool,
    dismiss: Rc<Cell<bool>>,
}
pub(super) fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French { fr } else { en }.into()
}
pub(super) fn plural(
    language: Language,
    n: i64,
    en: &str,
    ens: &str,
    fr: &str,
    frs: &str,
) -> String {
    let singular = language.singular(n.max(0) as u64);
    words(
        language,
        if singular { en } else { ens },
        if singular { fr } else { frs },
    )
}
fn definitions(snapshot: &Snapshot) -> Vec<(ShelfId, &Group)> {
    snapshot
        .groups
        .iter()
        .filter_map(|g| {
            if let Scope::Shelf { id, .. } = g.scope {
                Some((id, g))
            } else {
                None
            }
        })
        .collect()
}
fn current(snapshot: &Snapshot) -> Option<ShelfId> {
    match snapshot.navigation.as_ref()?.current().location.scope {
        Scope::Shelf { id, .. } => Some(id),
        _ => None,
    }
}
fn members(view: &View) -> Vec<String> {
    if view.tags.selection.is_empty() {
        view.selected.clone().into_iter().collect()
    } else {
        view.tags.selection.clone()
    }
}
fn notice(feedback: Option<&Feedback>, language: Language) -> String {
    match feedback {
        None | Some(Feedback::Ready) => String::new(),
        Some(Feedback::Saved(_)) => words(language, "Shelf saved.", "Étagère enregistrée."),
        Some(Feedback::Error {
            uncertain: true, ..
        }) => words(
            language,
            "The change could not be confirmed. Recover the library before retrying.",
            "La modification n’a pas pu être confirmée. Récupérez la bibliothèque avant de réessayer.",
        ),
        Some(Feedback::Error { code, .. }) => match code.as_str() {
            "shelf.invalid_name" => words(
                language,
                "Enter a name of 1–80 characters.",
                "Saisissez un nom de 1 à 80 caractères.",
            ),
            "shelf.duplicate_name" => words(
                language,
                "This Shelf name already exists. Choose another name.",
                "Ce nom d’Étagère existe déjà. Choisissez un autre nom.",
            ),
            "shelf.stale_review" | "storage.snapshot_changed" => words(
                language,
                "This preview changed or expired. Refresh it and renew your choices. Nothing was applied.",
                "L’aperçu a changé ou expiré. Actualisez-le et renouvelez vos choix. Rien n’a été appliqué.",
            ),
            "shelf.not_found" => words(
                language,
                "This Shelf no longer exists. Reopen the manager.",
                "Cette Étagère n’existe plus. Rouvrez le gestionnaire.",
            ),
            _ => words(
                language,
                "The Shelf operation failed. Refresh and retry.",
                "L’opération sur l’Étagère a échoué. Actualisez puis réessayez.",
            ),
        },
    }
}
impl State {
    pub(super) fn source_draft(&self) -> Option<(ShelfId, Vec<CollectionId>)> {
        self.selected
            .filter(|_| self.source_loaded)
            .map(|id| (id, self.sources.clone()))
    }
    pub(super) fn selected(&self) -> Option<ShelfId> {
        self.selected
    }
    pub fn open(
        &mut self,
        ui: &DesktopWindow,
        snapshot: &Snapshot,
        manual: Option<Targets>,
        bulk: bool,
    ) {
        self.dismiss.set(false);
        self.feedback = None;
        self.ticket = None;
        self.plan = None;
        self.selected = current(snapshot);
        self.reset = true;
        self.manual = manual;
        self.sources = snapshot
            .page
            .as_ref()
            .and_then(|p| p.shelf.as_ref())
            .filter(|c| Some(c.shelf.id) == self.selected)
            .map(|c| {
                c.sources
                    .iter()
                    .filter_map(|(s, _)| s.collection_id)
                    .collect()
            })
            .unwrap_or_default();
        self.linked_sources = snapshot
            .page
            .as_ref()
            .and_then(|p| p.shelf.as_ref())
            .filter(|c| Some(c.shelf.id) == self.selected)
            .map(|c| c.sources.iter().map(|(source, _)| source.clone()).collect())
            .unwrap_or_default();
        self.source_loaded = self.selected.is_none()
            || snapshot
                .page
                .as_ref()
                .and_then(|p| p.shelf.as_ref())
                .is_some_and(|c| Some(c.shelf.id) == self.selected);
        ui.set_shelf_source_index(0);
        ui.set_shelf_include_manual(bulk && self.manual.is_some());
        self.refresh_failed = false;
        self.navigation_error = false;
    }
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) -> Option<String> {
        let (revision, kind) = self.pending?;
        let result = &snapshot.workflows.shelves;
        if result.revision < revision {
            if snapshot.phase == Phase::Recovery {
                self.pending = None;
                ui.set_shelf_busy(false);
                self.feedback = Some(Feedback::Error {
                    code: "storage.runtime_fenced".into(),
                    uncertain: true,
                });
            }
            return None;
        }
        self.pending = None;
        ui.set_shelf_busy(false);
        self.feedback = result.feedback.clone();
        if ui.get_dialog() == 18
            && matches!(self.feedback, Some(Feedback::Ready | Feedback::Saved(_)))
        {
            self.exclusion_focus = true;
        }
        self.refresh_failed = result.refresh_failed;
        self.navigation_error = result.navigation_error;
        if matches!(kind, Pending::Read)
            && matches!(self.feedback, Some(Feedback::Ready))
            && result
                .inspection
                .as_ref()
                .is_some_and(|s| Some(s.id) == self.selected)
            && ui.get_dialog() == 15
        {
            self.linked_sources = result.sources.clone();
            self.sources = result
                .sources
                .iter()
                .filter_map(|s| s.collection_id)
                .collect();
            self.source_loaded = true;
        }
        match (&self.feedback, kind) {
            (Some(Feedback::Saved(id)), _) => {
                self.selected = Some(*id);
                self.reset = true;
                if matches!(kind, Pending::Confirm) {
                    self.ticket = None;
                    self.plan = None;
                    ui.invoke_finish_shelf_preview();
                    self.focus = true;
                }
                if ui.get_dialog() == 18 {
                    self.exclusion_focus = true;
                }
                if matches!(kind, Pending::Move) {
                    self.focus = true;
                }
            }
            (Some(Feedback::Ready), Pending::Plan) => {
                if let Some(p) = &result.preview {
                    self.ticket = Some(p.ticket.clone());
                    ui.invoke_show_shelf_preview(kind_index(p.kind));
                }
            }
            (Some(Feedback::Error { .. }), Pending::Confirm) => {
                self.ticket = None;
            }
            _ => {}
        }
        result.focus_member.clone()
    }
    pub fn restore_focus(&mut self, ui: &DesktopWindow) {
        if self.exclusion_focus {
            self.exclusion_focus = false;
            ui.invoke_focus_shelf_exclusions();
        }
        if self.focus {
            self.focus = false;
            ui.invoke_focus_browser();
        }
    }
    pub fn project(
        &mut self,
        ui: &DesktopWindow,
        snapshot: &Snapshot,
        request: &QueryRequest,
        selected: Option<&str>,
        selection: &[String],
    ) {
        if self.dismiss.replace(false) {
            self.feedback = None;
            self.ticket = None;
            self.plan = None;
            self.refresh_failed = false;
            self.navigation_error = false;
        }
        let language = snapshot.language;
        let choices = definitions(snapshot);
        if self
            .selected
            .is_some_and(|id| !choices.iter().any(|(i, _)| *i == id))
        {
            self.selected = None;
            self.reset = true;
        }
        let index = choices
            .iter()
            .position(|(id, _)| Some(*id) == self.selected);
        if self.reset {
            self.reset = false;
            ui.set_shelf_name(
                index
                    .map(|i| choices[i].1.name.as_str())
                    .unwrap_or("")
                    .into(),
            );
        }
        ui.set_shelf_definitions(ModelRc::new(VecModel::from(
            std::iter::once(words(language, "New Shelf…", "Nouvelle Étagère…").into())
                .chain(choices.iter().map(|(_, g)| g.name.clone().into()))
                .collect::<Vec<_>>(),
        )));
        ui.set_shelf_index(index.map_or(0, |i| i as i32 + 1));
        ui.set_shelf_creating(index.is_none());
        ui.set_shelf_move_up(index.is_some_and(|i| i > 0));
        ui.set_shelf_move_down(index.is_some_and(|i| i + 1 < choices.len()));
        let mut sources = snapshot
            .groups
            .iter()
            .filter_map(|g| {
                if let Scope::Collection(id) = g.scope {
                    Some((id, g.name.clone(), g.available))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        for id in &self.sources {
            if !sources.iter().any(|s| s.0 == *id) {
                sources.push((
                    *id,
                    words(language, "Missing Collection", "Collection manquante"),
                    false,
                ));
            }
        }
        self.source_choices = sources.iter().map(|s| s.0).collect();
        ui.set_shelf_sources(ModelRc::new(VecModel::from(
            sources
                .iter()
                .map(|(id, name, available)| {
                    format!(
                        "{}{}{}",
                        if self.sources.contains(id) {
                            "✓ "
                        } else {
                            ""
                        },
                        name,
                        if *available { "" } else { " · ⚠" }
                    )
                    .into()
                })
                .collect::<Vec<_>>(),
        )));
        let source_index = ui.get_shelf_source_index().max(0) as usize;
        ui.set_shelf_source_selected(
            self.source_choices
                .get(source_index)
                .is_some_and(|id| self.sources.contains(id)),
        );
        ui.set_shelf_manual_available(self.manual.is_some());
        ui.set_shelf_source_editable(self.source_loaded);
        let n = self.manual.as_ref().map_or(0, |t| t.stories.len());
        ui.set_shelf_manual_summary(format!("{} {}",language.number(n as i64),plural(language,n as i64,"frozen available story selected · mixed unavailable selections cannot be added","frozen available stories selected · mixed unavailable selections cannot be added","histoire disponible sélectionnée et figée · une sélection contenant des histoires indisponibles ne peut pas être ajoutée","histoires disponibles sélectionnées et figées · une sélection contenant des histoires indisponibles ne peut pas être ajoutée")).into());
        ui.set_shelf_source_summary(
            format!(
                "{} {}: {}\n{}: {}",
                language.number(self.sources.len() as i64),
                plural(
                    language,
                    self.sources.len() as i64,
                    "selected source (ANY)",
                    "selected sources (ANY)",
                    "source sélectionnée (OU)",
                    "sources sélectionnées (OU)"
                ),
                sources
                    .iter()
                    .filter(|s| self.sources.contains(&s.0))
                    .map(|s| s.1.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                words(language, "Currently linked", "Actuellement liées"),
                self.linked_sources
                    .iter()
                    .map(|s| format!(
                        "{}{}",
                        s.name,
                        if s.collection_id.is_none() {
                            words(language, " · needs attention", " · à réparer")
                        } else {
                            String::new()
                        }
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into(),
        );
        let inspection = snapshot
            .workflows
            .shelves
            .inspection
            .as_ref()
            .filter(|s| Some(s.id) == self.selected)
            .or_else(|| {
                snapshot
                    .page
                    .as_ref()
                    .and_then(|p| p.shelf.as_ref())
                    .map(|c| &c.shelf)
                    .filter(|s| Some(s.id) == self.selected)
            });
        ui.set_shelf_summary(inspection.map(|s|format!("{} · {} {} · {} {}",s.name,language.number(s.count),plural(language,s.count,"approved","approved","approuvée","approuvées"),language.number(s.unavailable_count),plural(language,s.unavailable_count,"unavailable","unavailable","indisponible","indisponibles"))).unwrap_or_else(||words(language,"Create a named Shelf with copied approved membership and a stored order. No candidate is approved by default.","Créez une Étagère nommée avec des membres approuvés et un ordre enregistré. Aucune candidate n’est approuvée par défaut.")).into());
        let active = current(snapshot).is_some();
        ui.set_shelf_active(active);
        ui.set_workspace(active);
        if active {
            ui.set_mode(0);
            let context = snapshot.page.as_ref().and_then(|p| p.shelf.as_ref());
            let refined = request.criteria != Query::default()
                || snapshot.navigation.as_ref().is_some_and(|n| {
                    matches!(
                        n.current().location.scope,
                        Scope::Shelf {
                            manual_order: false,
                            ..
                        }
                    )
                });
            ui.set_shelf_refined(refined);
            let member = snapshot
                .page
                .as_ref()
                .and_then(|p| p.stories.iter().find(|s| Some(s.uuid.as_str()) == selected))
                .and_then(|s| s.membership.as_ref());
            let single = selection.is_empty()
                || selection.len() == 1 && selected.is_some_and(|u| selection[0] == u);
            ui.set_shelf_member_up(!refined && single && member.is_some_and(|m| m.position > 0));
            ui.set_shelf_member_down(
                !refined
                    && single
                    && member
                        .zip(context)
                        .is_some_and(|(m, c)| m.position + 1 < c.shelf.count),
            );
            ui.set_shelf_member_removable(member.is_some() || !selection.is_empty());
            let summary = context
                .map(|c| {
                    format!(
                        "{} · {} {} · {} {} · {} {}{}",
                        c.shelf.name,
                        language.number(c.shelf.count),
                        plural(
                            language,
                            c.shelf.count,
                            "approved",
                            "approved",
                            "approuvée",
                            "approuvées"
                        ),
                        language.number(c.shelf.unavailable_count),
                        plural(
                            language,
                            c.shelf.unavailable_count,
                            "unavailable",
                            "unavailable",
                            "indisponible",
                            "indisponibles"
                        ),
                        language.number(c.exclusion_count as i64),
                        plural(
                            language,
                            c.exclusion_count as i64,
                            "excluded",
                            "excluded",
                            "exclue",
                            "exclues"
                        ),
                        if c.incomplete {
                            words(
                                language,
                                " · source coverage incomplete",
                                " · couverture des sources incomplète",
                            )
                        } else {
                            String::new()
                        }
                    )
                })
                .unwrap_or_default();
            ui.set_shelf_view_summary(
                format!(
                    "{} · {}",
                    summary,
                    words(
                        language,
                        "Search, filters and sort change only this view",
                        "La recherche, les filtres et le tri modifient seulement cette vue"
                    )
                )
                .into(),
            );
            ui.set_sources(ModelRc::new(VecModel::from(
                context
                    .map(|c| {
                        c.sources
                            .iter()
                            .map(|(s, attention)| {
                                format!("{}{}", s.name, if *attention { " · ⚠" } else { "" }).into()
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            )));
        }
        let mut text = if self.pending.is_some() {
            words(language, "Working…", "Traitement…")
        } else {
            notice(self.feedback.as_ref(), language)
        };
        if self.refresh_failed {
            text.push_str(&words(
                language,
                " Saved changes are retained; refresh the exclusions to reload.",
                " Modifications enregistrées ; actualisez les exclusions pour les recharger.",
            ));
        }
        if self.navigation_error {
            text.push_str(&words(
                language,
                " Saved changes are retained; reopen the Shelf.",
                " Modifications enregistrées ; rouvrez l’Étagère.",
            ));
        }
        ui.set_shelf_notice(text.into());
        ui.set_shelf_confirm_enabled(self.pending.is_none() && self.ticket.is_some());
        if let Some(p) = snapshot
            .workflows
            .shelves
            .preview
            .as_ref()
            .filter(|p| self.ticket.as_ref() == Some(&p.ticket))
        {
            ui.set_shelf_preview_kind(kind_index(p.kind));
            ui.set_shelf_preview_warnings(p.warnings);
            ui.set_shelf_preview_has_warnings(p.warning_count > 0);
            ui.set_shelf_preview_previous(p.page > 0);
            ui.set_shelf_preview_next(p.page + 1 < p.pages);
            let verb = match p.kind {
                Kind::Create => words(language, "Create", "Créer"),
                Kind::Duplicate => words(language, "Duplicate", "Dupliquer"),
                Kind::Remove => words(language, "Remove", "Retirer"),
                Kind::RemoveAndExclude => {
                    words(language, "Remove and exclude", "Retirer et exclure")
                }
            };
            ui.set_shelf_confirm_label(
                format!(
                    "{} {}",
                    verb,
                    if p.kind == Kind::Create {
                        language.number(p.selected as i64)
                    } else {
                        String::new()
                    }
                )
                .trim()
                .into(),
            );
            let mut explanation = match p.kind {
                Kind::Create => words(
                    language,
                    "Select candidates explicitly. Linking sources alone adds no stories. Refresh resets all approvals.",
                    "Sélectionnez explicitement les candidates. Lier des sources n’ajoute aucune histoire. L’actualisation réinitialise les approbations.",
                ),
                Kind::Duplicate => words(
                    language,
                    "Copies full approved order, manual state, exclusions and provenance, including unavailable UUIDs. It accepts no new matches.",
                    "Copie l’ordre approuvé, l’état manuel, les exclusions et la provenance, y compris les UUID indisponibles. Aucune nouvelle correspondance n’est acceptée.",
                ),
                Kind::Remove => words(
                    language,
                    "Removes approved membership only. Future source matches may be suggested again. Archives are retained.",
                    "Retire uniquement les membres approuvés. Les sources pourront les suggérer à nouveau. Les archives sont conservées.",
                ),
                Kind::RemoveAndExclude => words(
                    language,
                    "Removes membership and excludes these UUIDs from future suggestions. Archives are retained.",
                    "Retire les membres et exclut ces UUID des futures suggestions. Les archives sont conservées.",
                ),
            };
            if p.kind == Kind::Duplicate {
                explanation.push_str(&format!(
                    "\n{} {} · {} {}",
                    language.number(p.source_count),
                    plural(
                        language,
                        p.source_count,
                        "source",
                        "sources",
                        "source",
                        "sources"
                    ),
                    language.number(p.unavailable_count),
                    plural(
                        language,
                        p.unavailable_count,
                        "unavailable member retained",
                        "unavailable members retained",
                        "membre indisponible conservé",
                        "membres indisponibles conservés"
                    )
                ));
            }
            ui.set_shelf_preview_summary(
                format!(
                    "{} · {} {} · {} {} · {} {} · {}/{}{}\n{}",
                    p.name,
                    language.number(p.total as i64),
                    if p.kind == Kind::Create {
                        plural(
                            language,
                            p.total as i64,
                            "candidate",
                            "candidates",
                            "candidate",
                            "candidates",
                        )
                    } else {
                        plural(
                            language,
                            p.total as i64,
                            "member",
                            "members",
                            "membre",
                            "membres",
                        )
                    },
                    language.number(p.selected as i64),
                    plural(
                        language,
                        p.selected as i64,
                        "selected",
                        "selected",
                        "sélectionnée",
                        "sélectionnées"
                    ),
                    language.number(p.warning_count as i64),
                    plural(
                        language,
                        p.warning_count as i64,
                        "warning",
                        "warnings",
                        "avertissement",
                        "avertissements"
                    ),
                    p.page + 1,
                    p.pages,
                    if p.incomplete {
                        words(
                            language,
                            " · incomplete source coverage",
                            " · couverture des sources incomplète",
                        )
                    } else {
                        String::new()
                    },
                    explanation
                )
                .into(),
            );
            let rows=p.rows.iter().map(|r|ShelfPreviewRow{index:r.index as i32,uuid:r.uuid.clone().into(),label:r.title.clone().into(),selected:r.selected,selectable:p.kind==Kind::Create && !p.warnings,detail:if r.warning.is_some(){words(language,"Unavailable candidate or source · skipped; source coverage may be incomplete","Candidate ou source indisponible · ignorée ; la couverture peut être incomplète")}else{format!("{}{}",if r.manual {words(language,"Manual selection · ","Sélection manuelle · ")}else{String::new()},r.sources.join(", "))}.into()}).collect();
            update_rows(ui, rows);
        } else {
            ui.set_shelf_preview_previous(false);
            ui.set_shelf_preview_next(false);
        }
        if let Some(e) = &snapshot.workflows.shelves.exclusions {
            ui.set_shelf_exclusion_summary(
                format!(
                    "{} {} · {}/{}",
                    language.number(e.total as i64),
                    plural(
                        language,
                        e.total as i64,
                        "excluded UUID",
                        "excluded UUIDs",
                        "UUID exclu",
                        "UUID exclus"
                    ),
                    e.page + 1,
                    e.total.div_ceil(25).max(1)
                )
                .into(),
            );
            ui.set_shelf_exclusion_previous(e.page > 0);
            ui.set_shelf_exclusion_next(e.page + 1 < e.total.div_ceil(25));
            ui.set_shelf_exclusion_rows(ModelRc::new(VecModel::from(
                e.rows
                    .iter()
                    .enumerate()
                    .map(|(index, (uuid, title))| ShelfPreviewRow {
                        index: index as i32,
                        uuid: uuid.clone().into(),
                        label: title.clone().into(),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            )));
        }
    }
}
fn kind_index(kind: Kind) -> i32 {
    match kind {
        Kind::Create => 0,
        Kind::Duplicate => 1,
        Kind::Remove => 2,
        Kind::RemoveAndExclude => 3,
    }
}
fn update_rows(ui: &DesktopWindow, next: Vec<ShelfPreviewRow>) {
    let current = ui.get_shelf_preview_rows();
    if let Some(model) = current.as_any().downcast_ref::<VecModel<ShelfPreviewRow>>()
        && model.row_count() == next.len()
        && model
            .iter()
            .zip(&next)
            .all(|(a, b)| a.uuid == b.uuid && a.selectable == b.selectable)
    {
        for (i, row) in next.into_iter().enumerate() {
            model.set_row_data(i, row);
        }
        return;
    }
    ui.set_shelf_preview_rows(ModelRc::new(VecModel::from(next)));
}
fn send(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
    kind: Pending,
) {
    let mut state = view.borrow_mut();
    if state.shelves.pending.is_some() {
        return;
    }
    let Some(generation) = state.snapshot.generation.clone() else {
        return;
    };
    if matches!(kind, Pending::Plan) {
        state.shelves.plan = Some(action.clone());
        state.shelves.ticket = None;
    }
    state.shelves.dismiss.set(false);
    match worker
        .borrow()
        .offer(Command::Shelves { generation, action })
    {
        Ok(revision) => {
            state.shelves.pending = Some((revision, kind));
            ui.set_shelf_busy(true);
        }
        Err(_) => {
            state.shelves.feedback = Some(Feedback::Error {
                code: "settings.busy".into(),
                uncertain: false,
            })
        }
    }
    state.project(ui);
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_select_shelf(move |index| {
        if let Some(ui) = weak.upgrade() {
            let id = {
                let mut s = state.borrow_mut();
                s.shelves.selected = usize::try_from(index - 1)
                    .ok()
                    .and_then(|i| definitions(&s.snapshot).get(i).map(|x| x.0));
                s.shelves.reset = true;
                s.shelves.sources.clear();
                s.shelves.linked_sources.clear();
                s.shelves.source_loaded = s.shelves.selected.is_none();
                s.shelves.ticket = None;
                s.shelves.feedback = None;
                s.project(&ui);
                s.shelves.selected
            };
            if let Some(id) = id {
                send(&state, &pending, &ui, Action::Inspect(id), Pending::Read);
            }
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_select_shelf_source(move |_| {
        if let Some(ui) = weak.upgrade() {
            state.borrow_mut().project(&ui);
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_toggle_shelf_source(move || {
        if let Some(ui) = weak.upgrade() {
            let mut s = state.borrow_mut();
            let id = s
                .shelves
                .source_choices
                .get(ui.get_shelf_source_index().max(0) as usize)
                .copied();
            if let Some(id) = id {
                if s.shelves.sources.contains(&id) {
                    s.shelves.sources.retain(|i| *i != id);
                } else {
                    s.shelves.sources.push(id);
                }
            }
            s.project(&ui);
        }
    });
    for duplicate in [false, true] {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                let id = state.borrow().shelves.selected;
                if let Some(id) = id {
                    let name = ui.get_shelf_name().to_string();
                    let action = if duplicate {
                        Action::PlanDuplicate { id, name }
                    } else {
                        Action::Rename { id, name }
                    };
                    send(
                        &state,
                        &pending,
                        &ui,
                        action,
                        if duplicate {
                            Pending::Plan
                        } else {
                            Pending::Save
                        },
                    );
                }
            }
        };
        if duplicate {
            ui.on_duplicate_shelf(callback);
        } else {
            ui.on_rename_shelf(callback);
        }
    }
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_move_shelf(move |offset| {
        if let Some(ui) = weak.upgrade() {
            let id = state.borrow().shelves.selected;
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::MoveShelf { id, offset },
                    Pending::Save,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_preview_shelf_creation(move || {
        if let Some(ui) = weak.upgrade() {
            let action = {
                let s = state.borrow();
                Action::PreviewCreate {
                    name: ui.get_shelf_name().to_string(),
                    sources: s.shelves.sources.clone(),
                    manual: if ui.get_shelf_include_manual() {
                        s.shelves.manual.clone()
                    } else {
                        None
                    },
                }
            };
            send(&state, &pending, &ui, action, Pending::Plan);
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_add_manual_shelf(move || {
        if let Some(ui) = weak.upgrade() {
            let action = {
                let s = state.borrow();
                s.shelves
                    .selected
                    .zip(s.shelves.manual.clone())
                    .map(|(id, targets)| Action::AddManual { id, targets })
            };
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Save);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_remove_members(move |exclude| {
        if let Some(ui) = weak.upgrade() {
            let action = {
                let s = state.borrow();
                current(&s.snapshot).map(|id| Action::PlanMembers {
                    id,
                    uuids: members(&s),
                    exclude,
                })
            };
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Plan);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_move_member(move |offset| {
        if let Some(ui) = weak.upgrade() {
            let action = {
                let s = state.borrow();
                if s.filters.request.criteria != Query::default() {
                    None
                } else {
                    current(&s.snapshot)
                        .zip(s.selected.clone())
                        .map(|(id, uuid)| Action::MoveMember { id, uuid, offset })
                }
            };
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Move);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_clear_shelf_refinement(move || {
        if let Some(ui) = weak.upgrade() {
            let id = current(&state.borrow().snapshot);
            if id.is_some() {
                let mut request = state.borrow().filters.request.clone();
                request.criteria = Default::default();
                request.sort.clear();
                request.page = 1;
                if super::offer(
                    &pending.borrow(),
                    Command::Sort {
                        request: request.clone(),
                        manual_order: true,
                    },
                    &ui,
                ) {
                    state.borrow_mut().filters.request = request;
                    ui.set_query("".into());
                    state.borrow_mut().project(&ui);
                }
            }
        }
    });
    for refresh in [false, true] {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                let id = {
                    let s = state.borrow();
                    if ui.get_dialog() == 15 {
                        s.shelves.selected
                    } else {
                        current(&s.snapshot).or(s.shelves.selected)
                    }
                };
                if let Some(id) = id {
                    send(
                        &state,
                        &pending,
                        &ui,
                        Action::Exclusions { id, page: 0 },
                        Pending::Read,
                    );
                    if !refresh {
                        ui.invoke_show_shelf_exclusion_list();
                    }
                }
            }
        };
        if refresh {
            ui.on_refresh_shelf_exclusions(callback);
        } else {
            ui.on_show_shelf_exclusions(callback);
        }
    }
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_exclusion_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let action = state
                .borrow()
                .snapshot
                .workflows
                .shelves
                .exclusions
                .as_ref()
                .map(|e| Action::Exclusions {
                    id: e.id,
                    page: e.page.saturating_add_signed(delta as isize),
                });
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Read);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_allow_shelf_suggestions(move |index| {
        if let Some(ui) = weak.upgrade() {
            let action = state
                .borrow()
                .snapshot
                .workflows
                .shelves
                .exclusions
                .as_ref()
                .and_then(|e| {
                    e.rows
                        .get(index as usize)
                        .map(|r| Action::AllowSuggestions {
                            id: e.id,
                            uuid: r.0.clone(),
                            page: e.page,
                        })
                });
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Save);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_refresh_shelf_preview(move || {
        if let Some(ui) = weak.upgrade() {
            let a = state.borrow().shelves.plan.clone();
            if let Some(a) = a {
                send(&state, &pending, &ui, a, Pending::Plan);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_preview_toggle(move |index| {
        if let Some(ui) = weak.upgrade() {
            let t = state.borrow().shelves.ticket.clone();
            if let Some(ticket) = t {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::ToggleSeed {
                        ticket,
                        index: index as usize,
                    },
                    Pending::EditPlan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_preview_all(move |selected| {
        if let Some(ui) = weak.upgrade() {
            let t = state.borrow().shelves.ticket.clone();
            if let Some(ticket) = t {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::SeedAll { ticket, selected },
                    Pending::EditPlan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_preview_show_warnings(move |warnings| {
        if let Some(ui) = weak.upgrade() {
            let t = state.borrow().shelves.ticket.clone();
            if let Some(ticket) = t {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::PreviewWarnings { ticket, warnings },
                    Pending::EditPlan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_shelf_preview_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            let a = {
                let s = state.borrow();
                s.shelves
                    .ticket
                    .clone()
                    .zip(s.snapshot.workflows.shelves.preview.as_ref())
                    .map(|(ticket, p)| Action::PreviewPage {
                        ticket,
                        page: p.page.saturating_add_signed(delta as isize),
                    })
            };
            if let Some(a) = a {
                send(&state, &pending, &ui, a, Pending::EditPlan);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker, ui.as_weak());
    ui.on_confirm_shelf_preview(move || {
        if let Some(ui) = weak.upgrade() {
            let t = state.borrow().shelves.ticket.clone();
            if let Some(ticket) = t {
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
    let dismiss = view.borrow().shelves.dismiss.clone();
    ui.on_cancel_shelf_preview(move || {
        dismiss.set(true);
    });
    let dismiss = view.borrow().shelves.dismiss.clone();
    ui.on_dismiss_shelf_feedback(move || {
        dismiss.set(true);
    });
}
