//! Native tag forms and distinct bulk selection. Frozen targets never follow a
//! later search/selection; the writer rechecks every numeric ID and UUID.
use super::View;
use librairii_app::localization::{Catalogs, Language};
use librairii_core::{
    browsing::StoryIdentity,
    tagging::{CatalogEntry, DisplayTag, Kind},
};
use librairii_desktop::{
    presentation::{Command, Snapshot, Worker},
    workflows::tags::{Action, Feedback, Targets, Ticket},
};
use librairii_storage::tagging::DeleteTarget;
use librairii_ui::native::{AssignmentRow, DesktopWindow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone, Copy, Debug)]
enum Pending {
    Save,
    Plan,
    Delete,
    Workspace,
    Assign,
}
#[derive(Clone, Debug)]
struct Assignment {
    definition: i64,
    value: Option<i64>,
}
#[derive(Debug, Default)]
pub struct State {
    pub selection: Vec<String>,
    definition: Option<i64>,
    value: Option<i64>,
    frozen: Option<Targets>,
    origin: Option<String>,
    active_ticket: Option<Ticket>,
    rows: Vec<Assignment>,
    pending: Option<(u64, Pending)>,
    feedback: Option<Feedback>,
    reset: bool,
}
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French { fr } else { en }.into()
}
fn count_words(
    language: Language,
    count: i64,
    en_one: &str,
    en_other: &str,
    fr_one: &str,
    fr_other: &str,
) -> String {
    let one = language.singular(count.max(0) as u64);
    words(
        language,
        if one { en_one } else { en_other },
        if one { fr_one } else { fr_other },
    )
}
fn allowed(entry: &&CatalogEntry) -> bool {
    entry.definition.source != "builtin" || entry.definition.key != "broken"
}
fn label(
    entry: &CatalogEntry,
    value: Option<&str>,
    language: Language,
    catalogs: &Catalogs,
) -> String {
    catalogs.tag_label(
        language,
        &DisplayTag {
            source: entry.definition.source.clone(),
            definition: entry.definition.label.clone(),
            value: value.map(str::to_owned),
            color: entry.definition.color.clone(),
        },
    )
}
fn notice(feedback: Option<&Feedback>, language: Language) -> String {
    match feedback {
        None | Some(Feedback::Ready) => String::new(),
        Some(Feedback::Saved) => words(
            language,
            "Tag changes saved.",
            "Modifications des étiquettes enregistrées.",
        ),
        Some(Feedback::Deleted) => words(
            language,
            "Saved metadata deleted. Affected Collections now need attention.",
            "Métadonnées supprimées. Les Collections concernées nécessitent une réparation.",
        ),
        Some(Feedback::Assignment(result)) if result.changed_stories == 0 => words(
            language,
            "No stories changed; this tag already matched the targets.",
            "Aucune histoire modifiée ; cette étiquette correspondait déjà aux cibles.",
        ),
        Some(Feedback::Assignment(result)) => format!(
            "{} / {} {}",
            language.number(result.changed_stories as i64),
            language.number(result.requested_stories as i64),
            count_words(
                language,
                result.changed_stories as i64,
                "story updated.",
                "stories updated.",
                "histoire modifiée.",
                "histoires modifiées."
            )
        ),
        Some(Feedback::Error {
            uncertain: true, ..
        }) => words(
            language,
            "The change could not be confirmed. Recover the library before retrying.",
            "La modification n’a pas pu être confirmée. Récupérez la bibliothèque avant de réessayer.",
        ),
        Some(Feedback::Error { code, .. }) => match code.as_str() {
            "tagging.duplicate_definition" | "tagging.duplicate_value" => words(
                language,
                "This key already exists. Choose a unique key.",
                "Cette clé existe déjà. Choisissez une clé unique.",
            ),
            "tagging.invalid_order" => words(
                language,
                "This item cannot move farther in that direction. Refresh and retry.",
                "Cet élément ne peut pas aller plus loin dans cette direction. Actualisez puis réessayez.",
            ),
            "tagging.delete_plan_stale" => words(
                language,
                "The impact changed. Cancel and review it again.",
                "L’impact a changé. Annulez puis examinez-le à nouveau.",
            ),
            "tagging.protected_definition" | "tagging.derived_assignment" => words(
                language,
                "This identity, value or assignment is read-only. Tag colors remain editable.",
                "Cette identité, valeur ou attribution est en lecture seule. Les couleurs restent modifiables.",
            ),
            "tagging.invalid_definition" | "tagging.invalid_value" => words(
                language,
                "Use a valid key (64 characters), label (80 characters) and #RRGGBB color with enough contrast.",
                "Utilisez une clé valide (64 caractères), un nom (80 caractères) et une couleur #RRGGBB suffisamment contrastée.",
            ),
            "settings.busy" => words(
                language,
                "Library is busy. Try again.",
                "La bibliothèque est occupée. Réessayez.",
            ),
            _ => words(
                language,
                "The tag change could not be completed. Reopen the editor and retry with the original targets.",
                "La modification a échoué. Rouvrez l’éditeur puis réessayez avec les cibles d’origine.",
            ),
        },
    }
}
impl State {
    fn entry<'a>(&self, snapshot: &'a Snapshot) -> Option<&'a CatalogEntry> {
        snapshot
            .facets
            .tags
            .iter()
            .find(|e| Some(e.definition.id) == self.definition)
    }
    fn fields(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let entry = self.entry(snapshot);
        ui.set_tag_key(
            entry
                .map(|e| e.definition.key.as_str())
                .unwrap_or("")
                .into(),
        );
        ui.set_tag_label(
            entry
                .map(|e| e.definition.label.as_str())
                .unwrap_or("")
                .into(),
        );
        ui.set_tag_color(
            entry
                .map(|e| e.definition.color.as_str())
                .unwrap_or("#25603A")
                .into(),
        );
        ui.set_tag_kind(i32::from(
            entry.is_some_and(|e| e.definition.kind == Kind::Choice),
        ));
        self.value_fields(ui, snapshot);
    }
    fn value_fields(&self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let value = self
            .entry(snapshot)
            .and_then(|e| e.values.iter().find(|v| Some(v.id) == self.value));
        ui.set_tag_value_key(value.map(|v| v.key.as_str()).unwrap_or("").into());
        ui.set_tag_value_label(value.map(|v| v.label.as_str()).unwrap_or("").into());
    }
    pub fn open(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        self.definition = None;
        self.value = None;
        self.active_ticket = None;
        self.feedback = None;
        self.fields(ui, snapshot);
    }
    pub fn retain_visible(&mut self, snapshot: &Snapshot) {
        self.selection.retain(|uuid| {
            snapshot
                .page
                .as_ref()
                .is_some_and(|p| p.stories.iter().any(|s| &s.uuid == uuid))
        });
    }
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let Some((revision, kind)) = self.pending else {
            return;
        };
        if snapshot.workflows.tags.revision < revision {
            if snapshot.phase == librairii_desktop::presentation::Phase::Recovery {
                self.pending = None;
                ui.set_tag_busy(false);
                self.feedback = Some(Feedback::Error {
                    code: "storage.runtime_fenced".into(),
                    uncertain: true,
                });
            }
            return;
        }
        self.pending = None;
        ui.set_tag_busy(false);
        self.feedback = snapshot.workflows.tags.feedback.clone();
        match (&self.feedback, kind) {
            (Some(Feedback::Ready), Pending::Plan) => {
                if let Some(preview) = &snapshot.workflows.tags.preview {
                    self.active_ticket = Some(preview.ticket.clone());
                    ui.set_tag_delete_value(preview.value);
                    ui.invoke_show_tag_confirmation();
                }
            }
            (Some(Feedback::Deleted), Pending::Delete) => {
                self.active_ticket = None;
                self.reset = true;
                ui.invoke_finish_tag_confirmation();
            }
            (Some(Feedback::Saved), Pending::Save) => self.reset = true,
            _ => {}
        }
    }
    pub fn project(
        &mut self,
        ui: &DesktopWindow,
        snapshot: &Snapshot,
        catalogs: &Catalogs,
        inspected: Option<&str>,
    ) {
        if let Some(preview) = &snapshot.workflows.tags.preview
            && self.active_ticket.as_ref() == Some(&preview.ticket)
        {
            let language = snapshot.language;
            ui.set_tag_impact(format!("{} “{}” ?\n{} {} · {} {} · {} {}\n{}",
                words(language,"Delete","Supprimer"), preview.label,
                language.number(preview.assignments), count_words(language,preview.assignments,"assignment","assignments","attribution","attributions"),
                language.number(preview.values), count_words(language,preview.values,"value","values","valeur","valeurs"),
                language.number(preview.collections as i64), count_words(language,preview.collections as i64,"affected Collection","affected Collections","Collection concernée","Collections concernées"),
                words(language,"This permanently removes saved metadata. Collection criteria are retained for explicit repair.","Les métadonnées seront supprimées définitivement. Les critères des Collections seront conservés pour une réparation explicite.")
            ).into());
        }
        let catalog = snapshot
            .facets
            .tags
            .iter()
            .filter(allowed)
            .collect::<Vec<_>>();
        if self.definition.is_some() && self.entry(snapshot).is_none() {
            self.definition = None;
            self.value = None;
            self.reset = true;
        }
        if self.reset {
            self.reset = false;
            if self
                .entry(snapshot)
                .is_some_and(|e| !e.values.iter().any(|v| Some(v.id) == self.value))
            {
                self.value = None;
            }
            self.fields(ui, snapshot);
        }
        ui.set_tag_definitions(ModelRc::new(VecModel::from(
            std::iter::once(words(snapshot.language, "New tag…", "Nouvelle étiquette…").into())
                .chain(
                    catalog
                        .iter()
                        .map(|e| label(e, None, snapshot.language, catalogs).into()),
                )
                .collect::<Vec<_>>(),
        )));
        ui.set_tag_definition_index(
            catalog
                .iter()
                .position(|e| Some(e.definition.id) == self.definition)
                .map(|i| (i + 1) as i32)
                .unwrap_or(0),
        );
        let entry = self.entry(snapshot);
        ui.set_tag_creating(entry.is_none());
        ui.set_tag_editable(
            entry.is_some_and(|e| e.definition.source == "user" && !e.definition.protected),
        );
        ui.set_tag_choice(entry.is_some_and(|e| e.definition.kind == Kind::Choice));
        let custom = catalog
            .iter()
            .filter(|e| e.definition.source == "user")
            .collect::<Vec<_>>();
        let position = custom
            .iter()
            .position(|e| Some(e.definition.id) == self.definition);
        ui.set_tag_move_up(position.is_some_and(|i| i > 0));
        ui.set_tag_move_down(position.is_some_and(|i| i + 1 < custom.len()));
        let position = entry.and_then(|e| e.values.iter().position(|v| Some(v.id) == self.value));
        ui.set_tag_value_move_up(position.is_some_and(|i| i > 0));
        ui.set_tag_value_move_down(
            position.is_some_and(|i| entry.is_some_and(|e| i + 1 < e.values.len())),
        );
        ui.set_tag_source(entry.map(|e|words(snapshot.language,match e.definition.source.as_str(){"derived"=>"System-derived · identity and values are read-only; color is editable.","builtin"=>"Built-in · identity is read-only; color is editable.",_=>"Custom tag · keys remain immutable."},match e.definition.source.as_str(){"derived"=>"Automatique · identité et valeurs en lecture seule ; couleur modifiable.","builtin"=>"Intégrée · identité en lecture seule ; couleur modifiable.",_=>"Étiquette personnalisée · les clés restent immuables."})).unwrap_or_default().into());
        ui.set_tag_values(ModelRc::new(VecModel::from(
            std::iter::once(words(snapshot.language, "New value…", "Nouvelle valeur…").into())
                .chain(entry.into_iter().flat_map(|e| {
                    e.values
                        .iter()
                        .map(|v| label(e, Some(&v.label), snapshot.language, catalogs).into())
                }))
                .collect::<Vec<_>>(),
        )));
        ui.set_tag_value_index(
            entry
                .and_then(|e| e.values.iter().position(|v| Some(v.id) == self.value))
                .map(|i| (i + 1) as i32)
                .unwrap_or(0),
        );
        ui.set_tag_notice(
            if let Some((_, kind)) = self.pending {
                words(
                    snapshot.language,
                    if matches!(kind, Pending::Workspace | Pending::Plan) {
                        "Loading tags…"
                    } else {
                        "Saving changes…"
                    },
                    if matches!(kind, Pending::Workspace | Pending::Plan) {
                        "Chargement des étiquettes…"
                    } else {
                        "Enregistrement…"
                    },
                )
            } else {
                let mut text = notice(self.feedback.as_ref(), snapshot.language);
                if snapshot.workflows.tags.refresh_failed {
                    text.push_str(&words(snapshot.language,
                        " Saved changes are retained, but the tag display could not refresh. Use Refresh tags before another edit.",
                        " Les modifications enregistrées sont conservées, mais l’affichage n’a pas pu être actualisé. Actualisez les étiquettes avant une autre modification."));
                }
                text
            }
            .into(),
        );
        ui.set_bulk_summary(
            if self.selection.is_empty() {
                String::new()
            } else {
                format!(
                    "{} {}",
                    snapshot.language.number(self.selection.len() as i64),
                    count_words(
                        snapshot.language,
                        self.selection.len() as i64,
                        "selected",
                        "selected",
                        "sélectionnée",
                        "sélectionnées"
                    )
                )
            }
            .into(),
        );
        let count = if inspected.is_some_and(|id| self.selection.iter().any(|s| s == id)) {
            self.selection.len().max(1)
        } else {
            1
        };
        ui.set_edit_tags_label(
            if count > 1 {
                format!(
                    "{} · {} {}",
                    words(snapshot.language, "Edit tags", "Modifier les étiquettes"),
                    snapshot.language.number(count as i64),
                    words(snapshot.language, "stories", "histoires")
                )
            } else {
                words(snapshot.language, "Edit tags", "Modifier les étiquettes")
            }
            .into(),
        );
        let target_uuids = if inspected.is_some_and(|uuid| self.selection.iter().any(|s| s == uuid))
        {
            self.selection
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        } else {
            inspected.into_iter().collect()
        };
        ui.set_edit_tags_enabled(
            !target_uuids.is_empty()
                && target_uuids.iter().all(|uuid| {
                    snapshot.page.as_ref().is_some_and(|p| {
                        p.stories
                            .iter()
                            .any(|s| s.uuid == *uuid && s.summary.is_some())
                    })
                }),
        );
        if let Some(entry) = entry
            && entry.definition.source != "user"
        {
            ui.set_tag_label(label(entry, None, snapshot.language, catalogs).into());
            if let Some(value) = entry.values.iter().find(|v| Some(v.id) == self.value) {
                ui.set_tag_value_label(
                    catalogs
                        .text(snapshot.language, &value.label)
                        .unwrap_or(&value.label)
                        .into(),
                );
            }
        }
        ui.set_assignment_return_details(self.origin.as_ref().is_some_and(|uuid| {
            snapshot
                .page
                .as_ref()
                .is_some_and(|p| p.stories.iter().any(|s| &s.uuid == uuid))
        }));
        self.rows.clear();
        let mut rows = Vec::new();
        if !self
            .pending
            .is_some_and(|(_, kind)| matches!(kind, Pending::Workspace))
            && self.frozen.as_ref() == snapshot.workflows.tags.targets.as_ref()
            && let Some(workspace) = &snapshot.workflows.tags.workspace
        {
            for entry in workspace.catalog.iter().filter(allowed) {
                let state = workspace
                    .states
                    .iter()
                    .find(|s| s.definition_id == entry.definition.id);
                let values = if entry.definition.kind == Kind::Boolean {
                    vec![(
                        None,
                        state.map_or(0, |s| s.assigned_stories),
                        label(entry, None, snapshot.language, catalogs),
                    )]
                } else {
                    entry
                        .values
                        .iter()
                        .map(|v| {
                            (
                                Some(v.id),
                                state
                                    .and_then(|s| s.values.iter().find(|s| s.value_id == v.id))
                                    .map_or(0, |s| s.assigned_stories),
                                label(entry, Some(&v.label), snapshot.language, catalogs),
                            )
                        })
                        .collect()
                };
                for (value, count, label) in values {
                    rows.push(AssignmentRow {
                        index: self.rows.len() as i32,
                        label: format!(
                            "{label} · {} / {}",
                            snapshot.language.number(count),
                            snapshot.language.number(workspace.requested_stories as i64)
                        )
                        .into(),
                        read_only: entry.definition.source == "derived",
                        state: if count == 0 {
                            0
                        } else if count == workspace.requested_stories as i64 {
                            1
                        } else {
                            2
                        },
                    });
                    self.rows.push(Assignment {
                        definition: entry.definition.id,
                        value,
                    });
                }
            }
        }
        ui.set_assignment_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_assignment_summary(
            self.frozen
                .as_ref()
                .map(|t| {
                    format!(
                        "{} {}",
                        snapshot.language.number(t.stories.len() as i64),
                        count_words(
                            snapshot.language,
                            t.stories.len() as i64,
                            "frozen story target",
                            "frozen story targets",
                            "histoire cible figée",
                            "histoires cibles figées"
                        )
                    )
                })
                .unwrap_or_default()
                .into(),
        );
    }
}
fn send(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
    kind: Pending,
) {
    let mut borrowed = view.borrow_mut();
    let view = &mut *borrowed;
    if view
        .tags
        .pending
        .is_some_and(|(_, kind)| !matches!(kind, Pending::Workspace))
    {
        return;
    }
    let Some(generation) = view.snapshot.generation.clone() else {
        return;
    };
    match worker.borrow().offer(Command::Tags { generation, action }) {
        Ok(revision) => {
            view.tags.pending = Some((revision, kind));
            ui.set_tag_busy(!matches!(kind, Pending::Workspace));
            ui.set_tag_notice(
                words(
                    view.snapshot.language,
                    if matches!(kind, Pending::Workspace | Pending::Plan) {
                        "Loading tags…"
                    } else {
                        "Saving changes…"
                    },
                    if matches!(kind, Pending::Workspace | Pending::Plan) {
                        "Chargement des étiquettes…"
                    } else {
                        "Enregistrement…"
                    },
                )
                .into(),
            );
        }
        Err(_) => {
            view.tags.feedback = Some(Feedback::Error {
                code: "settings.busy".into(),
                uncertain: false,
            });
            ui.set_tag_notice(notice(view.tags.feedback.as_ref(), view.snapshot.language).into());
        }
    }
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_toggle_bulk(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            let uuid = state
                .snapshot
                .page
                .as_ref()
                .and_then(|p| usize::try_from(index).ok().and_then(|i| p.stories.get(i)))
                .map(|s| s.uuid.clone());
            if let Some(uuid) = uuid {
                if state.tags.selection.contains(&uuid) {
                    state.tags.selection.retain(|s| s != &uuid);
                } else if state.tags.selection.len() < 1000 {
                    state.tags.selection.push(uuid);
                }
                state.project(&ui);
                ui.invoke_focus_bulk_story(index);
            }
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_clear_bulk(move || {
        if let Some(ui) = weak.upgrade() {
            state.borrow_mut().tags.selection.clear();
            state.borrow_mut().project(&ui);
            ui.invoke_focus_browser();
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_select_tag_definition(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut b = state.borrow_mut();
            let s = &mut *b;
            s.tags.definition = index
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| s.snapshot.facets.tags.iter().filter(allowed).nth(i))
                .map(|e| e.definition.id);
            s.tags.value = None;
            s.tags.feedback = None;
            s.tags.fields(&ui, &s.snapshot);
            s.tags
                .project(&ui, &s.snapshot, &s.catalogs, s.selected.as_deref());
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_select_tag_value(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut b = state.borrow_mut();
            let s = &mut *b;
            s.tags.value = index
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| s.tags.entry(&s.snapshot).and_then(|e| e.values.get(i)))
                .map(|v| v.id);
            s.tags.value_fields(&ui, &s.snapshot);
            s.tags
                .project(&ui, &s.snapshot, &s.catalogs, s.selected.as_deref());
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_save_tag_definition(move || {
        if let Some(ui) = weak.upgrade() {
            let (key, label, color) = (
                ui.get_tag_key().to_string(),
                ui.get_tag_label().to_string(),
                ui.get_tag_color().to_string(),
            );
            if key.len() > 256 || label.len() > 320 || color.len() > 7 {
                let mut state = state.borrow_mut();
                state.tags.feedback = Some(Feedback::Error {
                    code: "tagging.invalid_definition".into(),
                    uncertain: false,
                });
                ui.set_tag_notice(
                    notice(state.tags.feedback.as_ref(), state.snapshot.language).into(),
                );
                return;
            }
            let id = state.borrow().tags.definition;
            send(
                &state,
                &pending,
                &ui,
                Action::SaveDefinition {
                    id,
                    key,
                    label,
                    color,
                    kind: if ui.get_tag_kind() == 1 {
                        Kind::Choice
                    } else {
                        Kind::Boolean
                    },
                },
                Pending::Save,
            );
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_save_tag_value(move || {
        if let Some(ui) = weak.upgrade() {
            let (key, label) = (
                ui.get_tag_value_key().to_string(),
                ui.get_tag_value_label().to_string(),
            );
            let ids = {
                let s = state.borrow();
                s.tags
                    .definition
                    .map(|definition| (definition, s.tags.value))
            };
            if let Some((definition, id)) = ids {
                if key.len() > 256 || label.len() > 320 {
                    let mut state = state.borrow_mut();
                    state.tags.feedback = Some(Feedback::Error {
                        code: "tagging.invalid_value".into(),
                        uncertain: false,
                    });
                    ui.set_tag_notice(
                        notice(state.tags.feedback.as_ref(), state.snapshot.language).into(),
                    );
                    return;
                }
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::SaveValue {
                        definition,
                        id,
                        key,
                        label,
                    },
                    Pending::Save,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_move_tag_definition(move |offset| {
        if let Some(ui) = weak.upgrade() {
            let id = state.borrow().tags.definition;
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::MoveDefinition { id, offset },
                    Pending::Save,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_move_tag_value(move |offset| {
        if let Some(ui) = weak.upgrade() {
            let ids = {
                let s = state.borrow();
                s.tags.definition.zip(s.tags.value)
            };
            if let Some((definition, id)) = ids {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::MoveValue {
                        definition,
                        id,
                        offset,
                    },
                    Pending::Save,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_delete_tag_definition(move || {
        if let Some(ui) = weak.upgrade() {
            let id = state.borrow().tags.definition;
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Plan(DeleteTarget::Definition(id)),
                    Pending::Plan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_delete_tag_value(move || {
        if let Some(ui) = weak.upgrade() {
            let id = state.borrow().tags.value;
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Plan(DeleteTarget::Value(id)),
                    Pending::Plan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_confirm_tag_delete(move || {
        if let Some(ui) = weak.upgrade()
            && ui.get_dialog() == 12
        {
            let ticket = state.borrow().tags.active_ticket.clone();
            if let Some(ticket) = ticket {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Confirm(ticket),
                    Pending::Delete,
                );
            }
        }
    });
    let state = view.clone();
    ui.on_cancel_tag_delete(move || state.borrow_mut().tags.active_ticket = None);
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_edit_tags(move || {
        if let Some(ui) = weak.upgrade() {
            let targets = {
                let s = state.borrow();
                s.snapshot.generation.clone().and_then(|generation| {
                    let ids = if s
                        .selected
                        .as_ref()
                        .is_some_and(|uuid| s.tags.selection.contains(uuid))
                    {
                        s.tags.selection.clone()
                    } else {
                        s.selected.clone().into_iter().collect()
                    };
                    let stories = ids
                        .iter()
                        .map(|uuid| {
                            s.snapshot
                                .page
                                .as_ref()?
                                .stories
                                .iter()
                                .find(|story| &story.uuid == uuid)?
                                .summary
                                .as_ref()
                                .map(|story| StoryIdentity {
                                    id: story.id,
                                    uuid: story.uuid.clone(),
                                })
                        })
                        .collect::<Option<Vec<_>>>()?;
                    (!stories.is_empty()).then_some(Targets {
                        generation,
                        stories,
                    })
                })
            };
            if let Some(targets) = targets {
                let mut s = state.borrow_mut();
                s.tags.frozen = Some(targets.clone());
                s.tags.origin = s.selected.clone();
                s.tags.feedback = None;
                s.tags.rows.clear();
                ui.set_assignment_rows(ModelRc::new(VecModel::default()));
                ui.invoke_show_assignments();
                drop(s);
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Workspace(targets),
                    Pending::Workspace,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_refresh_assignments(move || {
        if let Some(ui) = weak.upgrade() {
            let targets = state.borrow().tags.frozen.clone();
            if let Some(targets) = targets {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Workspace(targets),
                    Pending::Workspace,
                );
            }
        }
    });
    let (state, pending, weak) = (view, worker, ui.as_weak());
    ui.on_assignment_edit(move |index, value| {
        if let Some(ui) = weak.upgrade() {
            if value == 2 {
                return;
            }
            let request = {
                let s = state.borrow();
                usize::try_from(index)
                    .ok()
                    .and_then(|i| s.tags.rows.get(i))
                    .cloned()
                    .zip(s.tags.frozen.clone())
            };
            if let Some((row, targets)) = request {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Assign {
                        targets,
                        definition: row.definition,
                        value: row.value,
                        assigned: value == 1,
                    },
                    Pending::Assign,
                );
            }
        }
    });
}
