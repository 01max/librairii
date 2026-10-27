//! Independent detail/panel/bulk flag controls; presentation never owns restore data.
use super::View;
use librairii_app::localization::Language;
use librairii_core::browsing::StoryIdentity;
use librairii_desktop::{
    presentation::{Command, Phase, Snapshot, Worker},
    workflows::{
        Targets,
        flags::{Action, Failure, Ticket},
    },
};
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Debug, Default)]
pub struct State {
    dismiss_requested: Rc<Cell<bool>>,
    pending: Option<u64>,
    dismissed: u64,
    feedback: Option<librairii_desktop::workflows::flags::Snapshot>,
    admission_error: bool,
}
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French { fr } else { en }.into()
}
fn error(failure: &Failure, language: Language) -> String {
    if failure.uncertain {
        return words(
            language,
            "The flag change could not be confirmed. Recover the library before retrying.",
            "La modification du statut n’a pas pu être confirmée. Récupérez la bibliothèque avant de réessayer.",
        );
    }
    match failure.code.as_str() {
        "library.flag_restore_stale" | "storage.snapshot_changed" => words(
            language,
            "Flags or story availability changed. Nothing was restored; select the stories and try again.",
            "Le statut ou la disponibilité des histoires a changé. Aucune restauration ; sélectionnez les histoires puis réessayez.",
        ),
        _ => words(
            language,
            "Broken flags could not be updated. Try again.",
            "Le statut Défectueuse n’a pas pu être modifié. Réessayez.",
        ),
    }
}
pub fn targets(view: &View, bulk: bool) -> Option<Targets> {
    let uuids = if bulk {
        view.tags
            .selection
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    } else {
        view.selected.as_deref().into_iter().collect()
    };
    if uuids.is_empty() {
        return None;
    }
    let page = view.snapshot.page.as_ref()?;
    let stories = uuids
        .into_iter()
        .map(|uuid| {
            page.stories
                .iter()
                .find(|s| s.uuid == uuid)?
                .summary
                .as_ref()
                .map(|s| StoryIdentity {
                    id: s.id,
                    uuid: s.uuid.clone(),
                })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Targets {
        generation: view.snapshot.generation.clone()?,
        stories,
    })
}
impl State {
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        self.apply_dismiss(snapshot);
        let Some(revision) = self.pending else {
            return;
        };
        if snapshot.workflows.flags.revision >= revision {
            self.pending = None;
            self.feedback = (revision > self.dismissed).then(|| snapshot.workflows.flags.clone());
        } else if snapshot.phase == Phase::Recovery {
            self.pending = None;
            self.feedback = Some(librairii_desktop::workflows::flags::Snapshot {
                result: Some(Err(Failure {
                    code: "storage.runtime_fenced".into(),
                    uncertain: true,
                })),
                ..Default::default()
            });
        }
        ui.set_flag_pending(self.pending.is_some());
    }
    pub fn dismiss(&mut self, snapshot: &Snapshot) {
        self.dismissed = self
            .pending
            .unwrap_or(snapshot.workflows.flags.revision)
            .max(self.dismissed);
        self.feedback = None;
        self.admission_error = false;
    }
    fn apply_dismiss(&mut self, snapshot: &Snapshot) {
        if self.dismiss_requested.replace(false) {
            self.dismiss(snapshot);
        }
    }
    fn undo(&self) -> Option<Ticket> {
        self.feedback.as_ref()?.undo.clone()
    }
    pub fn project(
        &mut self,
        ui: &DesktopWindow,
        snapshot: &Snapshot,
        selected: Option<&str>,
        selection: &[String],
    ) {
        self.apply_dismiss(snapshot);
        let page = snapshot.page.as_ref();
        let summary = |ids: &[&str]| {
            let flagged = ids
                .iter()
                .filter(|id| {
                    page.is_some_and(|p| {
                        p.stories.iter().any(|s| {
                            s.uuid.as_str() == **id && s.summary.as_ref().is_some_and(|s| s.broken)
                        })
                    })
                })
                .count();
            let available = !ids.is_empty()
                && ids.iter().all(|id| {
                    page.is_some_and(|p| {
                        p.stories
                            .iter()
                            .any(|s| s.uuid.as_str() == *id && s.summary.is_some())
                    })
                });
            (flagged, available)
        };
        let single = selected.into_iter().collect::<Vec<_>>();
        let bulk = selection.iter().map(String::as_str).collect::<Vec<_>>();
        let (single_flagged, single_available) = summary(&single);
        let (bulk_flagged, bulk_available) = summary(&bulk);
        let panel_bulk = selected.is_some_and(|s| selection.iter().any(|id| id == s));
        ui.set_detail_flag_checked(single_flagged == 1);
        ui.set_detail_flag_enabled(single_available);
        ui.set_panel_flag_checked(if panel_bulk {
            !bulk.is_empty() && bulk_flagged == bulk.len()
        } else {
            single_flagged == 1
        });
        ui.set_panel_flag_enabled(if panel_bulk {
            bulk_available
        } else {
            single_available
        });
        ui.set_bulk_flag_checked(!bulk.is_empty() && bulk_flagged == bulk.len());
        ui.set_bulk_flag_enabled(bulk_available);
        let language = snapshot.language;
        ui.set_panel_flag_target_label(
            if panel_bulk {
                format!(
                    "{} · {}",
                    words(language, "Selected stories", "Histoires sélectionnées"),
                    language.number(bulk.len() as i64)
                )
            } else {
                String::new()
            }
            .into(),
        );
        let counts = |flagged: usize, total: usize| {
            format!(
                "{} / {} {}",
                language.number(flagged as i64),
                language.number(total as i64),
                words(
                    language,
                    "flagged",
                    if language.singular(flagged as u64) {
                        "défectueuse"
                    } else {
                        "défectueuses"
                    }
                )
            )
        };
        ui.set_panel_flag_summary(
            if panel_bulk {
                counts(bulk_flagged, bulk.len())
            } else {
                String::new()
            }
            .into(),
        );
        ui.set_bulk_flag_summary(
            if bulk_available {
                counts(bulk_flagged, bulk.len())
            } else {
                words(
                    language,
                    "Unavailable stories cannot be flagged.",
                    "Les histoires indisponibles ne peuvent pas être marquées.",
                )
            }
            .into(),
        );
        ui.set_flag_pending(self.pending.is_some());
        ui.set_flag_undo_enabled(self.pending.is_none() && self.undo().is_some());
        let notice = if self
            .pending
            .is_some_and(|revision| revision > self.dismissed)
        {
            words(
                language,
                "Updating Broken flags…",
                "Modification du statut Défectueuse…",
            )
        } else if self.admission_error {
            words(
                language,
                "Library is busy. Try again.",
                "La bibliothèque est occupée. Réessayez.",
            )
        } else if let Some(feedback) = &self.feedback {
            match &feedback.result {
                Some(Ok(result)) if result.changed_stories == 0 => words(
                    language,
                    "No Broken flags changed.",
                    "Aucun statut Défectueuse modifié.",
                ),
                Some(Ok(_)) if feedback.restored => words(
                    language,
                    "Previous Broken flags restored.",
                    "Statuts Défectueuse précédents restaurés.",
                ),
                Some(Ok(result)) => format!(
                    "{} / {} {}",
                    language.number(result.changed_stories as i64),
                    language.number(result.requested_stories as i64),
                    if language.singular(result.changed_stories as u64) {
                        words(language, "story updated.", "histoire modifiée.")
                    } else {
                        words(language, "stories updated.", "histoires modifiées.")
                    }
                ),
                Some(Err(failure)) => error(failure, language),
                None => String::new(),
            }
        } else {
            String::new()
        };
        ui.set_flag_notice(notice.into());
    }
}
fn send(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
) {
    let mut view = view.borrow_mut();
    {
        let View {
            flags, snapshot, ..
        } = &mut *view;
        flags.apply_dismiss(snapshot);
    }
    if view.flags.pending.is_some() {
        return;
    }
    let Some(generation) = view.snapshot.generation.clone() else {
        return;
    };
    view.flags.admission_error = false;
    match worker.borrow().offer(Command::Flags { generation, action }) {
        Ok(revision) => {
            view.flags.pending = Some(revision);
            view.flags.feedback = None;
        }
        Err(_) => {
            view.flags.admission_error = true;
        }
    }
    view.project(ui);
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    for kind in 0..3 {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move |broken| {
            if let Some(ui) = weak.upgrade() {
                let targets = {
                    let view = state.borrow();
                    let bulk = kind == 2
                        || (kind == 1
                            && view
                                .selected
                                .as_ref()
                                .is_some_and(|id| view.tags.selection.contains(id)));
                    targets(&view, bulk)
                };
                if let Some(targets) = targets {
                    send(&state, &pending, &ui, Action::Set { targets, broken });
                }
            }
        };
        match kind {
            0 => ui.on_detail_flag_change(callback),
            1 => ui.on_panel_flag_change(callback),
            _ => ui.on_bulk_flag_change(callback),
        }
    }
    let (state, pending, weak) = (view.clone(), worker, ui.as_weak());
    ui.on_undo_flags(move || {
        if let Some(ui) = weak.upgrade() {
            let ticket = state.borrow().flags.undo();
            if let Some(ticket) = ticket {
                send(&state, &pending, &ui, Action::Undo(ticket));
            }
        }
    });
    let (dismiss, weak) = (view.borrow().flags.dismiss_requested.clone(), ui.as_weak());
    ui.on_dismiss_flag_feedback(move || {
        // Closing can be driven by another workflow while View is projecting.
        // Record the intent without borrowing View again inside that callback.
        dismiss.set(true);
        if let Some(ui) = weak.upgrade() {
            ui.set_flag_notice("".into());
            ui.set_flag_undo_enabled(false);
        }
    });
}
