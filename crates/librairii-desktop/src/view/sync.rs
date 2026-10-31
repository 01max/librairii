//! Localized optional metadata observations. Only an explicit shared-runtime
//! Sync command performs networking; panel/status projection stays local.
use super::{View, operations};
use librairii_app::localization::Language;
use librairii_core::{OperationKind, metadata::SyncStatus, operations::is_terminal};
use librairii_desktop::{
    presentation::{Snapshot, Worker},
    workflows::operations::Action,
};
use librairii_ui::native::{DesktopWindow, SyncData};
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French {
        fr.into()
    } else {
        en.into()
    }
}
pub fn project(ui: &DesktopWindow, snapshot: &Snapshot) {
    let data = &snapshot.workflows.operations;
    let language = snapshot.language;
    let Some(catalog) = &data.catalog else {
        ui.set_sync_data(SyncData::default());
        return;
    };
    let job = data.cards.iter().find(|c| {
        c.operation.kind == OperationKind::MetadataSync && !is_terminal(c.operation.state)
    });
    let active = job.is_some() || data.sync_preparing;
    let state = if job.is_some_and(|job| job.operation.cancel_requested) {
        words(
            language,
            "Cancellation requested; committed metadata is preserved.",
            "Annulation demandée ; les métadonnées enregistrées sont conservées.",
        )
    } else if active {
        words(
            language,
            "Refreshing official metadata…",
            "Actualisation des métadonnées officielles…",
        )
    } else {
        match catalog.last_attempt_status {
            Some(SyncStatus::Succeeded) => words(
                language,
                "Metadata is available offline.",
                "Les métadonnées sont disponibles hors ligne.",
            ),
            Some(SyncStatus::Failed) if catalog.activated_at.is_some() => words(
                language,
                "The last sync failed. The previous working metadata is retained.",
                "La dernière synchronisation a échoué. Les métadonnées précédentes sont conservées.",
            ),
            Some(SyncStatus::Cancelled) if catalog.activated_at.is_some() => words(
                language,
                "Sync cancelled. The previous working metadata is retained.",
                "Synchronisation annulée. Les métadonnées précédentes sont conservées.",
            ),
            Some(SyncStatus::Failed) => words(
                language,
                "The last sync failed. Official metadata is unavailable; archive and fallback details remain usable.",
                "La dernière synchronisation a échoué. Les métadonnées officielles sont indisponibles ; les données de l’archive et de secours restent utilisables.",
            ),
            Some(SyncStatus::Cancelled) => words(
                language,
                "Sync cancelled. Official metadata is unavailable; archive and fallback details remain usable.",
                "Synchronisation annulée. Les métadonnées officielles sont indisponibles ; les données de l’archive et de secours restent utilisables.",
            ),
            Some(SyncStatus::Running) => words(
                language,
                "Metadata work is settling…",
                "Finalisation des métadonnées…",
            ),
            None => words(
                language,
                "Optional · no metadata has been synced yet.",
                "Facultatif · aucune synchronisation n’a encore eu lieu.",
            ),
        }
    };
    let last = if let Some(at) = data.last_success_local {
        language.date(at)
    } else if let Some(at) = &catalog.activated_at {
        format!("{at} UTC")
    } else {
        words(language, "Never", "Jamais")
    };
    let mut notice = if data.preference_error {
        words(
            language,
            "Interface preferences could not be saved or read. Session choices are in use; library data is preserved.",
            "Les préférences d’interface n’ont pas pu être lues ou enregistrées. Les choix de session sont utilisés ; la bibliothèque est conservée.",
        )
    } else {
        String::new()
    };
    if let Some(error) = &data.error {
        notice.push_str(&format!(
            "\n{} [{error}]",
            words(
                language,
                "The request could not be completed. Refresh local status and retry.",
                "La demande n’a pas pu aboutir. Actualisez le statut local puis réessayez."
            )
        ));
    }
    let retry = matches!(
        catalog.last_attempt_status,
        Some(SyncStatus::Failed | SyncStatus::Cancelled)
    );
    ui.set_sync_data(SyncData {
        loaded: true,
        never: catalog.activated_at.is_none(),
        hidden: data.sync_hidden,
        active,
        cancellable: job.is_some_and(|job| !job.operation.cancel_requested),
        can_start: !active && snapshot.generation.is_some(),
        title: words(language, "Official metadata · optional", "Métadonnées officielles · facultatif").into(),
        explanation: words(language,
            "Add public titles, descriptions, artwork and age. Your archives stay local. Sync starts only when you request it.",
            "Ajoutez les titres, descriptions, illustrations et âges publics. Vos archives restent locales. La synchronisation démarre à votre demande.").into(),
        last_success: format!("{}: {last}", words(language, "Last successful sync", "Dernière synchronisation réussie")).into(),
        status: state.into(),
        notice: notice.into(),
        start_label: words(language,
            if retry { "Retry metadata sync" } else { "Sync official metadata" },
            if retry { "Réessayer la synchronisation" } else { "Synchroniser les métadonnées" }).into(),
    });
}
pub fn install(ui: &DesktopWindow, state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_start_sync(move || {
        if let Some(ui) = weak.upgrade() {
            operations::send(&s, &w, &ui, Action::Sync);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_hide_sync(move |hidden| {
        if let Some(ui) = weak.upgrade() {
            operations::send(&s, &w, &ui, Action::HideSync(hidden));
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_cancel_sync(move || {
        if let Some(ui) = weak.upgrade() {
            let id = s
                .borrow()
                .snapshot
                .workflows
                .operations
                .cards
                .iter()
                .find(|c| {
                    c.operation.kind == OperationKind::MetadataSync
                        && !is_terminal(c.operation.state)
                })
                .map(|c| c.operation.identity.id.clone());
            if let Some(id) = id {
                operations::send(&s, &w, &ui, Action::Cancel(id));
            }
        }
    });
    let weak = ui.as_weak();
    ui.on_refresh_sync(move || {
        if let Some(ui) = weak.upgrade() {
            operations::send(&state, &worker, &ui, Action::Refresh);
        }
    });
}
