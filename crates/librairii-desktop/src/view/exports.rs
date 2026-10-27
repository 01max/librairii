//! Native scope/picker composition. All preflight hashing and copy authority
//! remain on the controller and shared job workers.
use super::View;
use librairii_app::localization::Language;
use librairii_core::{
    browsing::StoryIdentity,
    collection_codec::url::Scope,
    export::{Disposition, Issue},
};
use librairii_desktop::{
    presentation::{Command, Snapshot, Worker},
    workflows::exports::{Action, Mode},
};
use librairii_ui::native::{CollectionChoice, DesktopWindow, ExportData};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
#[derive(Debug, Default)]
pub struct State {
    pending: Option<u64>,
    handled: u64,
    group_page: usize,
    keys: Vec<i64>,
    local_error: Option<String>,
}
fn words(language: Language, en: &str, fr: &str) -> String {
    if language == Language::French { fr } else { en }.into()
}
fn mode(value: i32) -> Mode {
    match value {
        1 => Mode::Selection,
        2 => Mode::Collections,
        3 => Mode::Shelves,
        _ => Mode::Results,
    }
}
fn index(mode: Mode) -> i32 {
    match mode {
        Mode::Results => 0,
        Mode::Selection => 1,
        Mode::Collections => 2,
        Mode::Shelves => 3,
    }
}
pub fn issue(issue: Issue, language: Language) -> String {
    words(
        language,
        match issue {
            Issue::StoryMarkedBroken => {
                "Marked Broken; kept in the library and excluded from export."
            }
            Issue::StoryUnavailable => "Approved story is unavailable in the library.",
            Issue::EmptyScope => {
                "This scope has no members. Select stories or choose another scope."
            }
            Issue::NoExportableStories => {
                "No eligible archives. Review the reasons below or choose another scope."
            }
            Issue::ArchiveMissing => "Managed archive is missing.",
            Issue::ArchiveInvalid => "Managed archive verification failed.",
            Issue::ArchiveChanged => "Managed archive bytes changed.",
            Issue::UnsupportedExtension => "Archive extension is unsupported by Lunii.QT.",
            Issue::FilenameConflict => {
                "A destination filename already exists; it will be preserved."
            }
        },
        match issue {
            Issue::StoryMarkedBroken => {
                "Défectueuse ; conservée dans la bibliothèque et exclue de l’export."
            }
            Issue::StoryUnavailable => "Histoire approuvée indisponible dans la bibliothèque.",
            Issue::EmptyScope => {
                "Cette portée ne contient aucun membre. Sélectionnez des histoires ou une autre portée."
            }
            Issue::NoExportableStories => {
                "Aucune archive éligible. Consultez les motifs ou choisissez une autre portée."
            }
            Issue::ArchiveMissing => "Archive gérée manquante.",
            Issue::ArchiveInvalid => "Échec de vérification de l’archive gérée.",
            Issue::ArchiveChanged => "Les octets de l’archive gérée ont changé.",
            Issue::UnsupportedExtension => "Extension non prise en charge par Lunii.QT.",
            Issue::FilenameConflict => {
                "Un fichier de même nom existe déjà dans la destination ; il sera conservé."
            }
        },
    )
}
fn error(code: &str, language: Language) -> String {
    let (en, fr) = match code {
        "export.prepared_stale" | "storage.snapshot_changed" => (
            "The preview changed. Refresh it before exporting.",
            "L’aperçu a changé. Actualisez-le avant d’exporter.",
        ),
        "export.invalid_scope" => (
            "Select stories or valid groups. A Collection that needs attention must be repaired before export.",
            "Sélectionnez des histoires ou des groupes valides. Réparez les Collections signalées avant l’export.",
        ),
        _ => (
            "The export request or destination is unavailable. Refresh the preview or choose another destination.",
            "L’export ou sa destination est indisponible. Actualisez l’aperçu ou choisissez une autre destination.",
        ),
    };
    format!("{} [{code}]", words(language, en, fr))
}
fn replace<T: Clone + PartialEq + 'static>(current: ModelRc<T>, rows: Vec<T>) -> ModelRc<T> {
    if let Some(m) = current
        .as_any()
        .downcast_ref::<VecModel<T>>()
        .filter(|m| m.row_count() == rows.len())
    {
        for (i, row) in rows.into_iter().enumerate() {
            if m.row_data(i).as_ref() != Some(&row) {
                m.set_row_data(i, row);
            }
        }
        current
    } else {
        ModelRc::new(VecModel::from(rows))
    }
}
impl State {
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let s = &snapshot.workflows.exports;
        if self.pending.is_some_and(|r| s.revision >= r) {
            self.pending = None;
        }
        if s.revision > self.handled {
            self.handled = s.revision;
            if s.started.is_some() {
                ui.invoke_show_export_report();
            }
        }
    }
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        let s = &snapshot.workflows.exports;
        let l = snapshot.language;
        ui.set_export_busy(self.pending.is_some());
        ui.set_export_mode(index(s.mode));
        ui.set_export_modes(replace(
            ui.get_export_modes(),
            vec![
                words(
                    l,
                    "Current results · all pages",
                    "Résultats actuels · toutes les pages",
                )
                .into(),
                words(l, "Selected stories", "Histoires sélectionnées").into(),
                words(l, "One or more Collections", "Une ou plusieurs Collections").into(),
                words(
                    l,
                    "One or more approved Shelves",
                    "Une ou plusieurs Étagères approuvées",
                )
                .into(),
            ],
        ));
        ui.invoke_sync_export_choice();
        let groups = snapshot
            .groups
            .iter()
            .filter(|g| {
                matches!(
                    (&s.mode, &g.scope),
                    (Mode::Collections, Scope::Collection(_))
                        | (Mode::Shelves, Scope::Shelf { .. })
                )
            })
            .collect::<Vec<_>>();
        let pages = groups.len().div_ceil(25).max(1);
        self.group_page = self.group_page.min(pages - 1);
        self.keys.clear();
        let rows = groups
            .iter()
            .skip(self.group_page * 25)
            .take(25)
            .enumerate()
            .map(|(i, g)| {
                let id = match g.scope {
                    Scope::Collection(id) => id.get(),
                    Scope::Shelf { id, .. } => id.get(),
                    _ => 0,
                };
                self.keys.push(id);
                CollectionChoice {
                    index: i as i32,
                    label: format!(
                        "{} · {}{}",
                        g.name,
                        l.number(g.count),
                        if g.available {
                            ""
                        } else if l == Language::French {
                            " · réparation nécessaire"
                        } else {
                            " · needs repair"
                        }
                    )
                    .into(),
                    selected: s.groups.contains(&id),
                    available: g.available || s.mode == Mode::Shelves,
                }
            })
            .collect();
        ui.set_export_groups(replace(ui.get_export_groups(), rows));
        ui.set_export_selection_epoch(ui.get_export_selection_epoch().wrapping_add(1));
        let mut data = ExportData {
            has_destination: s.has_destination,
            preparing: s.preparing,
            group_previous: self.group_page > 0,
            group_next: self.group_page + 1 < pages,
            ..Default::default()
        };
        data.scope_summary=match s.mode {
            Mode::Results=>words(l,"Snapshots every result across all pages. Shelf results include the current temporary refinement.","Capture tous les résultats, toutes pages confondues. Les résultats d’une Étagère incluent son affinement temporaire."),
            Mode::Selection=>if s.has_selection{words(l,"Snapshots the explicit selection. Later browsing changes do not expand it.","Capture la sélection explicite. La navigation ultérieure ne l’élargit pas.")}else{words(l,"No explicit story selection. Close and select stories, or choose another scope.","Aucune histoire sélectionnée. Fermez pour sélectionner des histoires ou choisissez une autre portée.")},
            Mode::Collections=>format!("{} · {} / {}",words(l,"Reevaluate the selected Collections and deduplicate their union. Invalid Collections require repair.","Réévalue les Collections sélectionnées et déduplique leur union. Les Collections invalides nécessitent une réparation."),l.number(s.groups.len() as i64),l.number(groups.len() as i64)),
            Mode::Shelves=>format!("{} · {} / {}",words(l,"All approved members, independent of temporary filters and pending proposals. Source problems do not replace approved membership.","Tous les membres approuvés, indépendamment des filtres temporaires et propositions en attente. Les sources invalides ne remplacent pas les membres approuvés."),l.number(s.groups.len() as i64),l.number(groups.len() as i64)),
        }.into();
        let mut notice = s.error.as_deref().map(|e| error(e, l)).unwrap_or_default();
        if let Some(local) = &self.local_error {
            notice = format!(
                "{} [{local}]",
                words(
                    l,
                    "The request is busy or the native folder picker is unavailable. Try again.",
                    "La demande est occupée ou le sélecteur de dossiers indisponible. Réessayez."
                )
            );
        }
        let mut items = Vec::new();
        if let Some(p) = &s.preview {
            data.can_start = p.ticket.is_some() && p.ready > 0 && s.error.is_none();
            data.previous = p.page > 0;
            data.next = p.page + 1 < p.pages;
            let skipped = p.intended.saturating_sub(p.ready);
            data.summary = format!(
                "{}: {}\n{}: {} · {}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {} / {}",
                words(l, "Intended members", "Membres prévus"),
                l.number(p.intended as i64),
                words(l, "Ready files", "Fichiers prêts"),
                l.number(p.ready as i64),
                words(l, "Eligible bytes", "Octets éligibles"),
                l.number(p.bytes),
                words(l, "Excluded or conflicted", "Exclus ou en conflit"),
                l.number(skipped as i64),
                words(l, "Collapsed overlap", "Chevauchements dédupliqués"),
                l.number(p.overlap as i64),
                words(l, "Destination / formats", "Destination / formats"),
                format_args!("{} · {}", p.destination, p.formats.join(", ")),
                words(l, "Members page", "Page des membres"),
                l.number(p.page as i64 + 1),
                l.number(p.pages as i64)
            )
            .into();
            if let Some(i) = p.issue {
                notice.push_str(&format!("\n{}", issue(i, l)));
            }
            for item in &p.items {
                let why = match item.disposition {
                    Disposition::Ready => words(l, "Ready", "Prête"),
                    Disposition::Conflicted => issue(Issue::FilenameConflict, l),
                    Disposition::Skipped(i) => issue(i, l),
                };
                items.push(
                    format!(
                        "{} · {}\n{} · {} · {} {}\n{}",
                        item.title,
                        item.uuid,
                        item.output_name,
                        item.format,
                        l.number(item.bytes),
                        words(l, "bytes", "octets"),
                        why
                    )
                    .into(),
                );
            }
        } else {
            data.summary=words(l,"Choose a destination to inspect eligibility, original filenames and every exclusion before starting.","Choisissez une destination pour examiner l’éligibilité, les noms d’origine et chaque exclusion avant de démarrer.").into();
        }
        data.notice = notice.into();
        ui.set_export_data(data);
        ui.set_export_items(replace(ui.get_export_items(), items));
        ui.set_operation_reveal_notice(
            if !snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .is_some_and(|d| d.card.operation.kind == librairii_core::OperationKind::Export)
            {
                String::new()
            } else if s.revealed {
                words(
                    l,
                    "Destination handed to the native file manager.",
                    "Destination transmise au gestionnaire de fichiers.",
                )
            } else {
                s.error.as_deref().map(|e| error(e, l)).unwrap_or_default()
            }
            .into(),
        );
    }
}
fn send(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
) {
    let mut view = state.borrow_mut();
    let Some(generation) = view.snapshot.generation.clone() else {
        return;
    };
    if view.exports.pending.is_some() && !matches!(action, Action::Close) {
        view.exports.local_error = Some("settings.busy".into());
        let snapshot = view.snapshot.clone();
        view.exports.project(ui, &snapshot);
        return;
    }
    view.exports.local_error = None;
    match worker
        .borrow()
        .offer(Command::Exports { generation, action })
    {
        Ok(r) => {
            view.exports.pending = Some(r);
            ui.set_export_busy(true);
        }
        Err(e) => {
            view.exports.local_error = Some(e.to_string());
            let snapshot = view.snapshot.clone();
            view.exports.project(ui, &snapshot);
        }
    }
}
pub fn selected(
    state: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    generation: librairii_app::lifecycle::LibraryGeneration,
    path: Option<PathBuf>,
) {
    ui.set_picker_open(false);
    if state.borrow().snapshot.generation.as_ref() != Some(&generation) {
        return;
    }
    if let Some(path) = path {
        send(state, worker, ui, Action::Destination(path));
    }
}
fn pick(state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>, ui: &DesktopWindow) {
    if ui.get_picker_open() || ui.get_export_busy() {
        return;
    }
    let Some(generation) = state.borrow().snapshot.generation.clone() else {
        return;
    };
    let dialog = rfd::AsyncFileDialog::new().set_title(if ui.get_french() {
        "Choisir la destination de l’export"
    } else {
        "Choose export destination"
    });
    let Some(dialog) = ui.window().with_winit_window(|w| dialog.set_parent(w)) else {
        return;
    };
    ui.set_picker_open(true);
    let weak = ui.as_weak();
    let failure = state.clone();
    if let Err(e) = slint::spawn_local(async move {
        let path = dialog.pick_folder().await.map(|p| p.path().to_path_buf());
        if let Some(ui) = weak.upgrade() {
            selected(&state, &worker, &ui, generation, path);
        }
    }) {
        ui.set_picker_open(false);
        failure.borrow_mut().exports.local_error = Some(e.to_string());
    }
}
pub fn install(ui: &DesktopWindow, state: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_open_export(move || {
        if let Some(ui) = weak.upgrade() {
            let selection = {
                let v = s.borrow();
                let ids = if v.tags.selection.is_empty() {
                    v.selected.iter().cloned().collect()
                } else {
                    v.tags.selection.clone()
                };
                ids.into_iter()
                    .filter_map(|uuid| {
                        v.snapshot
                            .page
                            .as_ref()?
                            .all_stories()
                            .find(|row| row.uuid == uuid)
                            .map(|row| StoryIdentity {
                                id: row.summary.as_ref().map_or(0, |r| r.id),
                                uuid,
                            })
                    })
                    .collect()
            };
            s.borrow_mut().exports.group_page = 0;
            send(&s, &w, &ui, Action::Open(selection));
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_export_scope(move |i| {
        if let Some(ui) = weak.upgrade() {
            let m = mode(i);
            s.borrow_mut().exports.group_page = 0;
            let groups = match (
                &m,
                s.borrow()
                    .snapshot
                    .navigation
                    .as_ref()
                    .map(|n| n.current().location.scope.clone()),
            ) {
                (Mode::Collections, Some(Scope::Collection(id))) => vec![id.get()],
                (Mode::Shelves, Some(Scope::Shelf { id, .. })) => vec![id.get()],
                _ => Vec::new(),
            };
            send(&s, &w, &ui, Action::Select { mode: m, groups });
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_export_toggle(move |i| {
        if let Some(ui) = weak.upgrade() {
            let target = usize::try_from(i)
                .ok()
                .and_then(|i| s.borrow().exports.keys.get(i).copied());
            if let Some(id) = target {
                let snap = s.borrow().snapshot.workflows.exports.clone();
                let mut groups = snap.groups;
                if groups.contains(&id) {
                    groups.retain(|g| *g != id);
                } else {
                    groups.push(id);
                }
                send(
                    &s,
                    &w,
                    &ui,
                    Action::Select {
                        mode: snap.mode,
                        groups,
                    },
                );
            }
        }
    });
    let (s, weak) = (state.clone(), ui.as_weak());
    ui.on_export_group_page(move |d| {
        if let Some(ui) = weak.upgrade() {
            let mut v = s.borrow_mut();
            v.exports.group_page = v.exports.group_page.saturating_add_signed(d as isize);
            let snap = v.snapshot.clone();
            v.exports.project(&ui, &snap);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_pick_export_destination(move || {
        if let Some(ui) = weak.upgrade() {
            pick(s.clone(), w.clone(), &ui);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_refresh_export(move || {
        if let Some(ui) = weak.upgrade() {
            send(&s, &w, &ui, Action::Refresh);
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_export_page(move |d| {
        if let Some(ui) = weak.upgrade() {
            send(&s, &w, &ui, Action::Page(d));
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_start_export(move || {
        if let Some(ui) = weak.upgrade() {
            let ticket = s
                .borrow()
                .snapshot
                .workflows
                .exports
                .preview
                .as_ref()
                .and_then(|p| p.ticket.clone());
            if let Some(t) = ticket {
                send(&s, &w, &ui, Action::Start(t));
            }
        }
    });
    let (s, w, weak) = (state.clone(), worker.clone(), ui.as_weak());
    ui.on_close_export(move || {
        if let Some(ui) = weak.upgrade() {
            send(&s, &w, &ui, Action::Close);
        }
    });
    let weak = ui.as_weak();
    ui.on_reveal_export(move || {
        if let Some(ui) = weak.upgrade() {
            let id = state
                .borrow()
                .snapshot
                .workflows
                .operations
                .details
                .as_ref()
                .map(|d| d.card.operation.identity.id.clone());
            if let Some(id) = id {
                send(&state, &worker, &ui, Action::Reveal(id));
            }
        }
    });
}
