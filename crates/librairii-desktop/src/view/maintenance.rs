//! Native settings projection and parented pickers. No storage work runs here.
use super::View;
use librairii_app::{lifecycle::LibraryGeneration, localization::Language};
use librairii_desktop::{
    maintenance::{Action, Control, Mode, Notice, Progress, Stage},
    presentation::{Command, Phase, Snapshot, Worker},
};
use librairii_ui::native::{DesktopWindow, SettingsData};
use slint::{ComponentHandle, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
#[derive(Debug, Default)]
pub struct State {
    control: Option<Control>,
    pending: Option<u64>,
    error: Option<String>,
    progress: Progress,
}
fn words(l: Language, en: &str, fr: &str) -> String {
    if l == Language::French { fr } else { en }.into()
}
impl State {
    fn new(control: Control) -> Self {
        Self {
            control: Some(control),
            ..Default::default()
        }
    }
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        if self.pending.is_some_and(|r| snapshot.revision >= r) {
            self.pending = None;
        }
        let s = &snapshot.workflows.settings;
        let l = snapshot.language;
        let mode = if s.mode == Mode::Unavailable {
            match snapshot.phase {
                Phase::Setup => Mode::Setup,
                Phase::Adoption => Mode::Adoption,
                Phase::Recovery => Mode::Recovery,
                _ => Mode::Library,
            }
        } else {
            s.mode
        };
        let explanation = match mode {
            Mode::Setup => words(
                l,
                "Choose the exact folder for your new local library. Nothing is initialized until you confirm. You can change the interface language first.",
                "Choisissez le dossier exact de votre nouvelle bibliothèque locale. Elle ne sera créée qu’après votre confirmation. Vous pouvez d’abord choisir la langue de l’interface.",
            ),
            Mode::Adoption => words(
                l,
                "Your existing library is preserved. Confirm an empty folder for a verified independent copy. Existing settings and source files remain directed at the source.",
                "Votre bibliothèque existante est conservée. Confirmez un dossier vide pour une copie indépendante vérifiée. Les réglages existants et les fichiers d’origine continuent d’utiliser la source.",
            ),
            Mode::Recovery => words(
                l,
                "The expected library needs recovery. Reconnect its volume and retry, or locate that same library. A different library cannot replace it. Saved language remains independent.",
                "La bibliothèque attendue nécessite une récupération. Reconnectez son volume et réessayez, ou retrouvez cette même bibliothèque. Une autre bibliothèque ne peut pas la remplacer. La langue enregistrée reste indépendante.",
            ),
            Mode::Override => words(
                l,
                "Development root override: production profiles are bypassed, language is session-only, and relocation is disabled. This isolated-fixture route cannot adopt an existing library; normal native setup uses the independent native profile.",
                "Dossier de développement imposé : les profils de production sont ignorés, la langue vaut pour cette session et le déplacement est désactivé. Cette voie de test isolée ne peut pas adopter une bibliothèque existante ; la configuration native normale utilise le profil indépendant.",
            ),
            _ => words(
                l,
                "Your complete library moves together: database, managed archives, organization, catalog, artwork, staging, trash, logs and other root files. Original imports and external exports stay where they are. Review the source, destination and total before starting.",
                "Toute la bibliothèque est déplacée ensemble : base de données, archives gérées, organisation, catalogue, illustrations, préparation, corbeille, journaux et autres fichiers du dossier. Les imports d’origine et les exports externes restent à leur emplacement. Vérifiez la source, la destination et le total avant de commencer.",
            ),
        };
        let notice = match s.notice {
            Some(Notice::Busy) => words(
                l,
                "Library location cannot change while an import, export, metadata sync or library mutation is active. Wait for it to finish, then choose the folder again.",
                "L’emplacement ne peut pas changer pendant un import, un export, une synchronisation des métadonnées ou une modification de la bibliothèque. Attendez sa fin, puis choisissez à nouveau le dossier.",
            ),
            Some(Notice::CleanupPending) => words(
                l,
                "The destination is authoritative and usable. Old-source cleanup remains pending; the retained old location is shown below. Changed or unrelated files are preserved.",
                "La destination est la référence utilisable. La suppression de l’ancienne source reste en attente ; son emplacement conservé figure ci-dessous. Les fichiers modifiés ou étrangers sont préservés.",
            ),
            Some(Notice::Selected) => words(
                l,
                "Folder validated. Confirm to proceed.",
                "Dossier validé. Confirmez pour continuer.",
            ),
            Some(Notice::Unchanged) => words(
                l,
                "This is already the current library. No files were moved.",
                "Ce dossier contient déjà la bibliothèque actuelle. Aucun fichier n’a été déplacé.",
            ),
            Some(Notice::Moved) => words(
                l,
                "The same library is ready at its new location.",
                "La même bibliothèque est prête à son nouvel emplacement.",
            ),
            Some(Notice::MoveFailed) => words(
                l,
                "The move could not complete. The source remains authoritative; retained copies are shown below.",
                "Le déplacement n’a pas abouti. La source reste la référence ; les copies conservées sont indiquées ci-dessous.",
            ),
            Some(Notice::DiagnosticsSaved) => format!(
                "{} {}",
                words(l, "Diagnostic ZIP saved:", "ZIP de diagnostic enregistré :"),
                s.diagnostic_name.as_deref().unwrap_or("")
            ),
            Some(Notice::Cancelled) => words(
                l,
                "Cancellation settled. Already committed effects are preserved.",
                "Annulation terminée. Les effets déjà enregistrés sont conservés.",
            ),
            Some(Notice::Retained) => words(
                l,
                "The authoritative library and unfinished copy were preserved.",
                "La bibliothèque de référence et la copie inachevée ont été conservées.",
            ),
            None => String::new(),
        };
        let path = |p: &Option<PathBuf>| {
            p.as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
                .into()
        };
        let error=self.error.as_ref().or(s.error.as_ref()).map(|e|format!("{} [{e}]",words(l,"The request could not complete. The existing library and retained files are preserved.","La demande n’a pas abouti. La bibliothèque existante et les fichiers conservés sont préservés."))).unwrap_or_default();
        if (!s.can_confirm && ui.get_settings_confirm_focused())
            || (!s.can_retry && ui.get_settings_retry_focused())
        {
            ui.invoke_focus_settings();
        }
        ui.set_settings_data(SettingsData {
            mode: match mode {
                Mode::Setup => 0,
                Mode::Adoption => 1,
                Mode::Library => 2,
                Mode::Recovery => 3,
                _ => 4,
            },
            current: path(&s.current),
            source: path(&s.source),
            proposed: path(&s.proposed),
            summary: s
                .summary
                .map(|s| {
                    format!(
                        "{} {} · {} {}",
                        l.number(s.files as i64),
                        words(l, "files", "fichiers"),
                        l.number(s.bytes as i64),
                        words(l, "bytes total", "octets au total")
                    )
                })
                .unwrap_or_default()
                .into(),
            explanation: explanation.into(),
            notice: notice.into(),
            error: error.into(),
            retained: s
                .retained
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join("\n")
                .into(),
            can_choose: matches!(mode, Mode::Setup | Mode::Adoption | Mode::Library)
                && !s.recovery_required,
            can_confirm: s.can_confirm,
            can_retry: s.can_retry,
            can_locate: s.can_locate,
            can_export: matches!(mode, Mode::Library | Mode::Override)
                && snapshot.generation.is_some(),
            interrupted: s.interrupted_move,
            ..Default::default()
        });
        self.poll(ui, snapshot);
    }
    pub fn poll(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let progress = self
            .control
            .as_ref()
            .map(Control::progress)
            .unwrap_or_default();
        self.progress = progress;
        let busy = progress.busy || self.pending.is_some();
        let mut data = ui.get_settings_data();
        // Move native focus before disabling/hiding the currently focused
        // cancellation control; preserve any later user-selected control.
        if data.cancellable && (!busy || !progress.cancellable) && ui.get_settings_cancel_focused()
        {
            ui.invoke_focus_settings();
        }
        data.busy = busy;
        data.recovering = progress.stage == Stage::WaitingRecovery && self.pending.is_none();
        data.cancellable = progress.busy && progress.cancellable;
        data.progress = if busy {
            let text = match progress.stage {
                Stage::Checking => words(
                    snapshot.language,
                    "Validating the selected folder and counting complete library files…",
                    "Validation du dossier choisi et comptage de tous les fichiers de la bibliothèque…",
                ),
                Stage::Creating => words(
                    snapshot.language,
                    "Creating the confirmed local library…",
                    "Création de la bibliothèque locale confirmée…",
                ),
                Stage::Adopting => words(
                    snapshot.language,
                    "Copying and verifying the independent library… The original is preserved.",
                    "Copie et vérification de la bibliothèque indépendante… L’original est conservé.",
                ),
                Stage::Copying => format!(
                    "{} {}",
                    words(
                        snapshot.language,
                        "Moving the closed library · files processed:",
                        "Déplacement de la bibliothèque fermée · fichiers traités :"
                    ),
                    snapshot.language.number(progress.files as i64)
                ),
                Stage::Verifying => words(
                    snapshot.language,
                    "Verifying copied bytes and database identity before root commitment…",
                    "Vérification des octets copiés et de l’identité de la base avant l’enregistrement du dossier…",
                ),
                Stage::Committing | Stage::Cleanup => words(
                    snapshot.language,
                    "The destination is committed. Finishing verified cleanup; cancellation is unavailable.",
                    "La destination est enregistrée. Fin de la suppression vérifiée ; l’annulation est indisponible.",
                ),
                Stage::Diagnostics => words(
                    snapshot.language,
                    "Writing bounded local diagnostics…",
                    "Enregistrement du diagnostic local limité…",
                ),
                Stage::Recovery | Stage::WaitingRecovery => words(
                    snapshot.language,
                    "Storage ownership is retained while recovery settles.",
                    "Les ressources de stockage sont conservées pendant la récupération.",
                ),
                Stage::Idle => words(
                    snapshot.language,
                    "Preparing the request…",
                    "Préparation de la demande…",
                ),
            };
            text.into()
        } else {
            Default::default()
        };
        ui.set_settings_data(data);
        if busy {
            ui.set_pending(true);
            ui.set_indeterminate(true);
        }
        ui.set_commit_pending(
            snapshot.workflows.operations.critical
                || snapshot.workflows.exports.recovery_required
                || progress.busy && !progress.cancellable,
        );
    }
}
fn send(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
) {
    if !action.bounded() {
        return;
    }
    let generation = state.borrow().snapshot.generation.clone();
    let result = worker
        .borrow()
        .offer(Command::Maintenance { generation, action });
    let mut v = state.borrow_mut();
    v.maintenance.error = None;
    match result {
        Ok(revision) => v.maintenance.pending = Some(revision),
        Err(error) => v.maintenance.error = Some(error.to_string()),
    }
    let snapshot = v.snapshot.clone();
    v.maintenance.project(ui, &snapshot);
}
#[derive(Clone, Copy)]
pub(crate) enum Picker {
    Folder,
    Locate,
    Diagnostics,
}
pub(crate) fn selected(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    generation: Option<LibraryGeneration>,
    kind: Picker,
    path: Option<PathBuf>,
) {
    ui.set_picker_open(false);
    if state.borrow().snapshot.generation != generation {
        return;
    }
    if let Some(path) = path {
        send(
            state,
            worker,
            ui,
            match kind {
                Picker::Folder => Action::Select(path),
                Picker::Locate => Action::Locate(path),
                Picker::Diagnostics => Action::Diagnostics(path),
            },
        );
    }
    match kind {
        Picker::Diagnostics => ui.invoke_focus_settings_diagnostics(),
        _ => ui.invoke_focus_settings_choose(),
    }
}
fn pick(state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>, ui: &DesktopWindow, kind: Picker) {
    if ui.get_picker_open() || ui.get_settings_data().busy {
        return;
    }
    let generation = state.borrow().snapshot.generation.clone();
    let french = ui.get_french();
    let title = match (kind, french) {
        (Picker::Folder, false) => "Choose the exact library folder",
        (Picker::Folder, true) => "Choisir le dossier exact de la bibliothèque",
        (Picker::Locate, false) => "Locate the same library",
        (Picker::Locate, true) => "Retrouver la même bibliothèque",
        (Picker::Diagnostics, false) => "Save diagnostic ZIP",
        (Picker::Diagnostics, true) => "Enregistrer le ZIP de diagnostic",
    };
    let mut dialog = rfd::AsyncFileDialog::new().set_title(title);
    if matches!(kind, Picker::Diagnostics) {
        dialog = dialog
            .add_filter("ZIP", &["zip"])
            .set_file_name("librairii-diagnostics.zip");
    }
    let Some(dialog) = ui.window().with_winit_window(|w| dialog.set_parent(w)) else {
        return;
    };
    ui.set_picker_open(true);
    let weak = ui.as_weak();
    let failure = state.clone();
    if let Err(error) = slint::spawn_local(async move {
        let path = if matches!(kind, Picker::Diagnostics) {
            dialog.save_file().await
        } else {
            dialog.pick_folder().await
        }
        .map(|p| p.path().into());
        if let Some(ui) = weak.upgrade() {
            selected(&state, &worker, &ui, generation, kind, path);
        }
    }) {
        ui.set_picker_open(false);
        failure.borrow_mut().maintenance.error = Some(error.to_string());
    }
}
pub fn install(ui: &DesktopWindow, state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    state.borrow_mut().maintenance = State::new(worker.borrow().maintenance_control());
    for (kind, index) in [
        (Picker::Folder, 0),
        (Picker::Locate, 1),
        (Picker::Diagnostics, 2),
    ] {
        let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                pick(s.clone(), w.clone(), &ui, kind);
            }
        };
        match index {
            0 => ui.on_settings_choose(callback),
            1 => ui.on_settings_locate(callback),
            _ => ui.on_settings_diagnostics(callback),
        }
    }
    for (action, index) in [
        (Action::Confirm, 0),
        (Action::Retry, 1),
        (Action::RetryMove, 2),
        (Action::KeepSource, 3),
    ] {
        let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                send(&s, &w, &ui, action.clone());
            }
        };
        match index {
            0 => ui.on_settings_confirm(callback),
            1 => ui.on_settings_retry(callback),
            2 => ui.on_settings_retry_move(callback),
            _ => ui.on_settings_keep_source(callback),
        }
    }
    let control = worker.borrow().maintenance_control();
    ui.on_settings_cancel(move || {
        control.cancel();
    });
}
