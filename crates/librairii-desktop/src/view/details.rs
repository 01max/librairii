//! Main-thread detail presentation and frozen removal confirmation.
use super::{View, offer};
use librairii_app::localization::Language;
use librairii_desktop::{
    browsing::Story,
    presentation::{Command, Snapshot, Worker},
    workflows::StoryTarget,
};
use librairii_ui::native::{DesktopWindow, StoryRow};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};
#[derive(Debug, Default)]
pub struct State {
    frozen: Option<StoryTarget>,
    pending: Option<u64>,
}
fn words(language: Language, english: &str, french: &str) -> String {
    if language == Language::French {
        french
    } else {
        english
    }
    .into()
}
pub fn facts(story: &Story, language: Language) -> String {
    let Some(summary) = &story.summary else {
        return String::new();
    };
    let mut lines = vec![format!(
        "{}: {}",
        words(language, "Imported", "Importée"),
        story.dates[0]
            .map(|date| language.date(date))
            .unwrap_or_else(|| words(language, "Unknown date", "Date inconnue"))
    )];
    if let Some(info) = &summary.official {
        for (en, fr, value) in [
            ("Publisher", "Éditeur", info.publisher.as_str()),
            (
                "Catalog locale",
                "Langue du catalogue",
                info.locale.as_str(),
            ),
            (
                "Story language",
                "Langue de l’histoire",
                info.language.as_str(),
            ),
            (
                "Metadata source",
                "Source des métadonnées",
                info.provenance.as_str(),
            ),
        ] {
            if !value.is_empty() {
                lines.push(format!("{}: {value}", words(language, en, fr)));
            }
        }
        for (index, en, fr) in [
            (1, "Catalog fetched", "Catalogue récupéré"),
            (2, "Catalog activated", "Catalogue activé"),
            (3, "Source updated", "Source actualisée"),
        ] {
            if let Some(date) = story.dates[index] {
                lines.push(format!(
                    "{}: {}",
                    words(language, en, fr),
                    language.date(date)
                ));
            }
        }
        if let Some(seconds) = info.duration_seconds {
            lines.push(format!(
                "{}: {} s",
                words(language, "Duration", "Durée"),
                language.number(seconds)
            ));
        }
        if let Some(age) = info.minimum_age {
            lines.push(format!(
                "{}: {}",
                words(language, "Minimum age", "Âge minimum"),
                language.number(age)
            ));
        }
        if let Some(age) = info.maximum_age {
            lines.push(format!(
                "{}: {}",
                words(language, "Maximum age", "Âge maximum"),
                language.number(age)
            ));
        }
    }
    lines.join("\n")
}
pub fn sources(story: &Story, language: Language) -> String {
    let Some(summary) = &story.summary else {
        return String::new();
    };
    [
        ("Title", "Titre", &summary.sources.title),
        ("Description", "Description", &summary.sources.description),
        ("Author", "Auteur", &summary.sources.author),
        ("Artwork", "Illustration", &summary.sources.artwork),
    ]
    .into_iter()
    .map(|(en, fr, source)| {
        let source = match source.as_str() {
            "official" => words(language, "Official catalog", "Catalogue officiel"),
            "embedded" => words(language, "Archive metadata", "Métadonnées de l’archive"),
            _ => words(language, "Fallback", "Valeur de secours"),
        };
        format!("{} · {source}", words(language, en, fr))
    })
    .collect::<Vec<_>>()
    .join("\n")
}
pub fn project(row: &mut StoryRow, snapshot: &Snapshot) {
    if snapshot.workflows.detail_error {
        row.technical = words(
            snapshot.language,
            "Details could not be loaded. Close and reopen to retry.",
            "Les détails n’ont pas pu être chargés. Fermez puis rouvrez pour réessayer.",
        )
        .into();
    }
    let Some(details) = &snapshot.workflows.details else {
        return;
    };
    if row.uuid != details.target.uuid {
        return;
    }
    let archive = &details.story.archive;
    let language = snapshot.language;
    let filename = if archive.original_filename.is_empty() {
        format!(
            "{} · {:02X?}",
            words(language, "Native filename bytes", "Octets du nom natif"),
            archive.original_filename_bytes
        )
    } else {
        archive.original_filename.clone()
    };
    let verification = match archive.verification.as_str() {
        "compatible" => words(language, "Verified", "Vérifiée"),
        "missing" => words(language, "Archive missing", "Archive manquante"),
        _ => words(language, "Verification failed", "Échec de la vérification"),
    };
    row.technical = format!(
        "{}: {}\n{}: {filename}\n{}: {verification}\nSHA-256: {}",
        words(language, "Story UUID", "UUID de l’histoire"),
        details.target.uuid,
        words(language, "Archive", "Archive"),
        words(language, "Verification", "Vérification"),
        archive.sha256
    )
    .into();
    row.description = details.story.summary.description.clone().into();
}
fn target(view: &View) -> Option<StoryTarget> {
    let selected = view.selected.as_ref()?;
    let story = view
        .snapshot
        .page
        .as_ref()?
        .stories
        .iter()
        .find(|s| &s.uuid == selected)?
        .summary
        .as_ref()?;
    Some(StoryTarget {
        generation: view.snapshot.generation.clone()?,
        query: view.snapshot.navigation.as_ref()?.stamp(),
        id: story.id,
        uuid: story.uuid.clone(),
    })
}
impl State {
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        if snapshot.workflows.removal_recovery {
            ui.set_removal_error(snapshot.status.clone().into());
            return;
        }
        if self
            .pending
            .is_some_and(|revision| snapshot.revision >= revision)
        {
            self.pending = None;
            ui.set_detail_pending(false);
            match &snapshot.workflows.removal {
                Some(Ok(_)) => { self.frozen = None; ui.invoke_complete_removal(); }
                None => ui.set_removal_error(words(snapshot.language,"The removal outcome could not be confirmed. Recover the library before retrying.","Le résultat du retrait n’a pas pu être confirmé. Récupérez la bibliothèque avant de réessayer.").into()),
                Some(Err(_)) => ui.set_removal_error(words(snapshot.language,"The story could not be removed. Its original source file is unchanged. Retry or keep the story.","L’histoire n’a pas pu être retirée. Le fichier d’origine est conservé. Réessayez ou conservez l’histoire.").into()),
            }
        }
    }
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_request_details(move || {
        if let Some(ui) = weak.upgrade()
            && let Some(target) = target(&state.borrow())
        {
            offer(&pending.borrow(), Command::Details(target), &ui);
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_begin_removal(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            if let Some(target) = target(&state) {
                state.details.frozen = Some(target);
                ui.set_removal_title(ui.get_selected_story().title);
                ui.set_removal_error("".into());
                ui.invoke_confirm_removal();
            }
        }
    });
    let (state, weak) = (view, ui.as_weak());
    ui.on_remove_story(move || {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            if state.details.pending.is_some() {
                return;
            }
            if let Some(target) = state.details.frozen.clone() {
                match worker.borrow().offer(Command::Remove(target)) {
                    Ok(revision) => {
                        state.details.pending = Some(revision);
                        ui.set_detail_pending(true);
                    }
                    Err(_) => ui.set_removal_error(
                        words(
                            state.snapshot.language,
                            "Library is busy. Try again.",
                            "La bibliothèque est occupée. Réessayez.",
                        )
                        .into(),
                    ),
                }
            }
        }
    });
}
