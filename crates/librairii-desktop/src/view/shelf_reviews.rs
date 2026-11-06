//! Projection of explicit review decisions. Native rows emit exact UUIDs;
//! mutable row indices and UI booleans grant no storage authority.
use super::{
    View,
    shelves::{plural, words},
};
use librairii_app::{lifecycle::LibraryGeneration, localization::Language};
use librairii_core::{CollectionId, collection_codec::url::Scope};
use librairii_desktop::{
    presentation::{Command, Phase, Snapshot, Worker},
    workflows::shelf_reviews::{Action, Feedback, Kind, Outcome, Preview, Section, Ticket},
};
use librairii_ui::native::{DesktopWindow, ShelfReviewData, ShelfReviewRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[derive(Clone, Copy, Debug)]
enum Pending {
    Plan,
    Page,
    Choice,
    Inspect,
    Acknowledge,
    Apply,
    DismissAck,
}
#[derive(Clone, Copy, Debug)]
enum Focus {
    Refresh,
    Current,
    Browser,
}
#[derive(Debug, Default)]
pub struct State {
    open: bool,
    pending: Option<(u64, Pending)>,
    ticket: Option<Ticket>,
    last_request: Option<Action>,
    feedback: Option<Feedback>,
    generation: Option<LibraryGeneration>,
    focus: Option<Focus>,
    epoch: i32,
    kind: Option<Kind>,
    preference_error: bool,
    navigation_error: bool,
    dismiss: Rc<Cell<bool>>,
    dismiss_ack: Rc<Cell<Option<Result<u64, ()>>>>,
    acknowledgement: Rc<RefCell<Option<(LibraryGeneration, Ticket)>>>,
    seen_entry: Option<Ticket>,
    scope: Option<librairii_core::ShelfId>,
}
fn current_shelf(snapshot: &Snapshot) -> Option<librairii_core::ShelfId> {
    match snapshot.navigation.as_ref()?.current().location.scope {
        Scope::Shelf { id, .. } => Some(id),
        _ => None,
    }
}
fn kind_index(kind: Kind) -> i32 {
    match kind {
        Kind::Updates => 0,
        Kind::Sources => 1,
        Kind::DeleteCollection => 2,
        Kind::DeleteShelf => 3,
    }
}
fn title(kind: Kind, language: Language) -> String {
    match kind {
        Kind::Updates => words(language, "Review updates", "Examiner les mises à jour"),
        Kind::Sources => words(
            language,
            "Review source changes",
            "Examiner les changements de sources",
        ),
        Kind::DeleteCollection => words(
            language,
            "Review Collection deletion",
            "Examiner la suppression de la Collection",
        ),
        Kind::DeleteShelf => words(
            language,
            "Confirm Shelf deletion",
            "Confirmer la suppression de l’Étagère",
        ),
    }
}
fn notice(feedback: Option<&Feedback>, language: Language) -> String {
    match feedback {
        None | Some(Feedback::Ready) => String::new(),
        Some(Feedback::Committed(Outcome::ShelfSaved { changed: false, .. })) => words(
            language,
            "No approved changes. Nothing changed.",
            "Aucune modification approuvée. Rien n’a changé.",
        ),
        Some(Feedback::Committed(Outcome::ShelfSaved { .. })) => words(
            language,
            "Approved changes saved.",
            "Modifications approuvées enregistrées.",
        ),
        Some(Feedback::Committed(Outcome::ShelfDeleted(_))) => words(
            language,
            "Shelf deleted. Library stories and archives were retained.",
            "Étagère supprimée. Les histoires et les archives sont conservées.",
        ),
        Some(Feedback::Committed(Outcome::CollectionDeleted(_))) => words(
            language,
            "Collection deleted. All approved Shelf decisions were saved together.",
            "Collection supprimée. Toutes les décisions approuvées sur les Étagères ont été enregistrées ensemble.",
        ),
        Some(Feedback::Error {
            uncertain: true, ..
        }) => words(
            language,
            "The outcome could not be confirmed. Recover the library before retrying.",
            "Le résultat n’a pas pu être confirmé. Récupérez la bibliothèque avant de réessayer.",
        ),
        Some(Feedback::Error { code, .. }) => match code.as_str() {
            "shelf.stale_review" | "storage.snapshot_changed" => words(
                language,
                "This review changed or expired. Nothing was applied. Refresh and renew all decisions.",
                "La révision a changé ou expiré. Rien n’a été appliqué. Actualisez et renouvelez toutes les décisions.",
            ),
            "shelf.invalid_input" => words(
                language,
                "These decisions cannot be applied. Keep unavailable or uncertain members, or refresh the review.",
                "Ces décisions ne peuvent pas être appliquées. Conservez les membres indisponibles ou incertains, ou actualisez la révision.",
            ),
            _ => words(
                language,
                "The review action could not be completed. Refresh or retry; approved membership is retained.",
                "L’action de révision a échoué. Actualisez ou réessayez ; les membres approuvés sont conservés.",
            ),
        },
    }
}
fn detail(row: &librairii_desktop::workflows::shelf_reviews::Row, language: Language) -> String {
    let mut text = match row.reason {
        "source_match" => words(
            language,
            "New source match",
            "Nouvelle correspondance des sources",
        ),
        "no_source_matches" => words(
            language,
            "Matches no active source · choose Keep or Remove and exclude",
            "Ne correspond à aucune source active · choisissez Conserver ou Retirer et exclure",
        ),
        "source_needs_attention" => words(
            language,
            "Source needs repair · coverage is unknown; source-based removals are suspended",
            "La source nécessite une réparation · couverture inconnue ; les retraits déduits des sources sont suspendus",
        ),
        "story_unavailable" => words(
            language,
            "Approved UUID unavailable · retained; no source removal inferred",
            "UUID approuvé indisponible · conservé ; aucun retrait déduit des sources",
        ),
        "last_source_detached" => words(
            language,
            "Last source is being detached · surviving stories become manual / kept",
            "La dernière source sera détachée · les histoires conservées deviennent manuelles / conservées",
        ),
        "source_coverage_lost" => words(
            language,
            "Source coverage will be lost · an explicit decision is required",
            "La couverture des sources sera perdue · une décision explicite est nécessaire",
        ),
        _ => words(
            language,
            "Review this story explicitly",
            "Examinez explicitement cette histoire",
        ),
    };
    if row.unavailable && row.reason != "story_unavailable" {
        text.push_str(&words(
            language,
            " · Unavailable; Keep is available, removal is disabled",
            " · Indisponible ; conservation possible, retrait désactivé",
        ));
    }
    if row.broken {
        text.push_str(&words(
            language,
            " · ⚠ Broken · excluded from exports",
            " · ⚠ Défectueuse · exclue des exports",
        ));
    }
    if !row.sources.is_empty() {
        text.push_str(&format!(
            "\n{}: {}",
            words(language, "Matching sources", "Sources correspondantes"),
            row.sources.join(", ")
        ));
    }
    if !row.origins.is_empty() {
        text.push_str(&format!(
            "\n{}: {}",
            words(language, "Added from", "Ajoutée depuis"),
            row.origins.join(", ")
        ));
    }
    text
}
fn update_rows(
    current: ModelRc<ShelfReviewRow>,
    rows: Vec<ShelfReviewRow>,
) -> ModelRc<ShelfReviewRow> {
    if let Some(model) = current.as_any().downcast_ref::<VecModel<ShelfReviewRow>>()
        && model.row_count() == rows.len()
        && model
            .iter()
            .zip(&rows)
            .all(|(a, b)| a.uuid == b.uuid && a.warning == b.warning && a.removal == b.removal)
    {
        for (i, row) in rows.into_iter().enumerate() {
            model.set_row_data(i, row);
        }
        return current;
    }
    ModelRc::new(VecModel::from(rows))
}
impl State {
    fn dismiss(&mut self) {
        if self.dismiss.replace(false) {
            self.open = false;
            self.ticket = None;
            self.pending = None;
            self.last_request = None;
            self.feedback = None;
            if let Some(result) = self.dismiss_ack.take() {
                match result {
                    Ok(revision) => self.pending = Some((revision, Pending::DismissAck)),
                    Err(()) => {
                        self.feedback = Some(Feedback::Error {
                            code: "settings.busy".into(),
                            uncertain: false,
                        })
                    }
                }
            }
        }
    }
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        self.dismiss();
        let Some((revision, kind)) = self.pending else {
            return;
        };
        let result = &snapshot.workflows.shelf_reviews;
        if result.revision < revision {
            if snapshot.phase == Phase::Recovery {
                self.pending = None;
                self.feedback = Some(Feedback::Error {
                    code: "storage.runtime_fenced".into(),
                    uncertain: true,
                });
                self.focus = Some(Focus::Refresh);
            }
            return;
        }
        self.pending = None;
        if self
            .ticket
            .as_ref()
            .is_some_and(|ticket| result.preview.as_ref().is_some_and(|p| &p.ticket == ticket))
        {
            self.scope = result.preview.as_ref().and_then(|p| p.shelf_id);
        }
        self.feedback = result.feedback.clone();
        self.preference_error = result.preference_error;
        self.navigation_error = result.navigation_error;
        match (&self.feedback, kind) {
            (Some(Feedback::Ready), Pending::Plan) => {
                if let Some(p) = &result.preview {
                    self.ticket = Some(p.ticket.clone());
                    self.kind = Some(p.kind);
                    self.scope = p.shelf_id;
                    self.open = true;
                    ui.invoke_show_native_review();
                    self.focus = Some(Focus::Refresh);
                }
            }
            (Some(Feedback::Committed(_)), Pending::Apply) => {
                self.ticket = None;
                self.open = false;
                ui.invoke_finish_native_review();
                self.focus = Some(Focus::Browser);
            }
            (Some(Feedback::Error { .. }), Pending::Apply) => {
                self.ticket = None;
                self.focus = Some(Focus::Refresh);
            }
            (_, Pending::DismissAck) => {}
            (Some(Feedback::Ready), Pending::Page | Pending::Acknowledge) => {
                self.focus = Some(Focus::Refresh)
            }
            (Some(Feedback::Ready), _) => self.focus = Some(Focus::Current),
            (Some(Feedback::Error { .. }), _) if self.open => self.focus = Some(Focus::Refresh),
            _ => {}
        }
    }
    pub fn restore_focus(&mut self, ui: &DesktopWindow) {
        match self.focus.take() {
            Some(Focus::Refresh) => ui.invoke_activate_native_review(),
            Some(Focus::Current) => ui.invoke_focus_native_review(),
            Some(Focus::Browser) => ui.invoke_focus_browser(),
            None => {}
        }
    }
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot) {
        self.dismiss();
        self.epoch = self.epoch.wrapping_add(1);
        if self.open
            && (snapshot.generation != self.generation
                || self
                    .scope
                    .is_some_and(|id| Some(id) != current_shelf(snapshot)))
        {
            self.open = false;
            self.ticket = None;
            self.scope = None;
            ui.invoke_finish_native_review();
            self.focus = Some(Focus::Browser);
        }
        if self.pending.is_none()
            && !ui.get_modal()
            && snapshot.phase == Phase::Library
            && let Some(entry) = &snapshot.workflows.shelf_reviews.entry
            && self.seen_entry.as_ref() != Some(&entry.ticket)
            && current_shelf(snapshot) == Some(entry.shelf)
            && snapshot
                .workflows
                .shelf_reviews
                .preview
                .as_ref()
                .is_some_and(|p| p.ticket == entry.ticket)
        {
            self.seen_entry = Some(entry.ticket.clone());
            self.ticket = Some(entry.ticket.clone());
            self.scope = Some(entry.shelf);
            self.generation = snapshot.generation.clone();
            self.last_request = Some(Action::Review(entry.shelf));
            self.kind = Some(Kind::Updates);
            self.feedback = Some(Feedback::Ready);
            self.open = true;
            self.focus = Some(Focus::Refresh);
            ui.invoke_show_native_review();
        }
        let p = snapshot
            .workflows
            .shelf_reviews
            .preview
            .as_ref()
            .filter(|p| self.ticket.as_ref() == Some(&p.ticket));
        *self.acknowledgement.borrow_mut() = p
            .filter(|p| p.mismatch_prompt && self.open)
            .and_then(|p| snapshot.generation.clone().map(|g| (g, p.ticket.clone())));
        let language = snapshot.language;
        let mut text = if self.pending.is_some() {
            words(language, "Working…", "Traitement…")
        } else {
            notice(self.feedback.as_ref(), language)
        };
        if snapshot.workflows.shelf_reviews.entry_error
            && current_shelf(snapshot).is_some()
            && !self.open
        {
            text.push_str(&words(language," Shelf updates could not be checked. Review updates to retry."," Les mises à jour de l’Étagère n’ont pas pu être vérifiées. Examinez les mises à jour pour réessayer."));
        }
        if snapshot.workflows.shelf_reviews.warning_preference_error {
            text.push_str(&words(language," Warning dismissal could not be saved. Refresh the review and dismiss again; it may appear again."," L’avertissement n’a pas pu être masqué durablement. Actualisez la révision puis masquez-le à nouveau ; il peut réapparaître."));
        }
        if self.preference_error {
            text.push_str(&words(language," Saved effects are retained, but the warning preference could not be saved."," Les effets enregistrés sont conservés, mais la préférence d’avertissement n’a pas pu être enregistrée."));
        }
        if self.navigation_error {
            text.push_str(&words(
                language,
                " Saved effects are retained; reopen the intended view.",
                " Les effets enregistrés sont conservés ; rouvrez la vue souhaitée.",
            ));
        }
        let mut data = ShelfReviewData {
            open: self.open && snapshot.phase == Phase::Library,
            busy: self.pending.is_some(),
            committing: self
                .pending
                .is_some_and(|(_, k)| matches!(k, Pending::Apply)),
            stale: snapshot.workflows.shelf_reviews.stale || self.ticket.is_none(),
            kind: kind_index(self.kind.unwrap_or(Kind::Updates)),
            epoch: self.epoch,
            title: title(self.kind.unwrap_or(Kind::Updates), language).into(),
            notice: text.clone().into(),
            ..Default::default()
        };
        if let Some(p) = p {
            project(&mut data, p, snapshot, ui.get_review_data().rows);
        }
        if let Some(story) = &snapshot.workflows.shelf_reviews.story
            && self.ticket.is_some()
        {
            data.preview_uuid = story.uuid.clone().into();
            data.preview_title = story.title.clone().into();
            data.preview_author = story.author.clone().into();
            data.preview_description = story.description.clone().into();
            data.preview_broken = story.broken;
            data.preview_warning = if story.unavailable {
                words(
                    language,
                    "Unavailable approved UUID · retained",
                    "UUID approuvé indisponible · conservé",
                )
            } else if story.broken {
                words(
                    language,
                    "⚠ Broken · excluded from exports",
                    "⚠ Défectueuse · exclue des exports",
                )
            } else {
                String::new()
            }
            .into();
            if let Some(t) = &story.thumbnail {
                data.preview_cover = slint::Image::from_rgba8(
                    slint::SharedPixelBuffer::clone_from_slice(&t.rgba, t.width, t.height),
                );
            }
        }
        ui.set_review_data(data);
        if !self.open && !text.is_empty() {
            if ui.get_dialog() == 14 {
                ui.set_collection_notice(text.clone().into());
            } else if ui.get_shelf_active() || ui.get_dialog() == 15 {
                ui.set_shelf_notice(text.clone().into());
            } else if snapshot.phase == Phase::Library {
                ui.set_status(format!("{text} {}", snapshot.status).trim().into());
            }
        }
    }
}
fn project(
    data: &mut ShelfReviewData,
    p: &Preview,
    snapshot: &Snapshot,
    current: ModelRc<ShelfReviewRow>,
) {
    let language = snapshot.language;
    data.kind = kind_index(p.kind);
    data.title = title(p.kind, language).into();
    data.section = match p.section {
        Section::Additions => 0,
        Section::Removals => 1,
        Section::Warnings => 2,
    };
    data.summary = format!(
        "{}{} · {} {} · {} {} · {} {}",
        p.name,
        if p.kind == Kind::DeleteCollection {
            format!(" · {} · {}/{}", p.shelf_name, p.shelf + 1, p.shelves)
        } else {
            String::new()
        },
        language.number(p.approved),
        plural(
            language,
            p.approved,
            "approved story",
            "approved stories",
            "histoire approuvée",
            "histoires approuvées"
        ),
        language.number(p.unavailable),
        plural(
            language,
            p.unavailable,
            "unavailable",
            "unavailable",
            "indisponible",
            "indisponibles"
        ),
        language.number(p.exclusion_count as i64),
        plural(
            language,
            p.exclusion_count as i64,
            "exclusion",
            "exclusions",
            "exclusion",
            "exclusions"
        )
    )
    .into();
    let status = if data.stale {
        words(
            language,
            "This snapshot may have changed. Refresh; all decisions must be renewed.",
            "Cet aperçu a pu changer. Actualisez ; toutes les décisions devront être renouvelées.",
        )
    } else if p.incomplete {
        words(
            language,
            "Incomplete source coverage · healthy additions remain available; inferred removals are suspended. This review is not Up to date.",
            "Couverture des sources incomplète · les ajouts connus restent disponibles ; les retraits déduits sont suspendus. Cette révision n’est pas à jour.",
        )
    } else if p.up_to_date {
        words(
            language,
            "Up to date · no additions or removals",
            "À jour · aucun ajout ni retrait",
        )
    } else if p.kind == Kind::DeleteShelf {
        words(
            language,
            "Deletes this Shelf, its stored order, source links and exclusions. Library stories, archives, Collections and other Shelves are retained.",
            "Supprime cette Étagère, son ordre, ses liens de sources et ses exclusions. Les histoires, archives, Collections et autres Étagères sont conservées.",
        )
    } else if matches!(p.kind, Kind::Sources | Kind::DeleteCollection) {
        words(
            language,
            "Every affected member needs an explicit Keep or Remove and exclude decision. Not now leaves the source change or deletion unapplied.",
            "Chaque membre concerné nécessite Conserver ou Retirer et exclure. Pas maintenant laisse le changement de sources ou la suppression non appliqué.",
        )
    } else {
        words(
            language,
            "Approved membership is unchanged until Apply approved changes.",
            "Les membres approuvés restent inchangés jusqu’à Appliquer les modifications approuvées.",
        )
    };
    data.status=format!("{}{}",status,if p.mismatch_prompt{words(language,"\nSource matches changed. Dismissing this warning approves no membership changes.","\nLes correspondances des sources ont changé. Masquer cet avertissement n’approuve aucune modification des membres.")}else{String::new()}).into();
    data.source_summary=if matches!(p.kind,Kind::Sources|Kind::DeleteCollection){format!("{}: {}\n{}: {}\n{}",words(language,"Sources after confirmation","Sources après confirmation"),p.sources.join(", "),words(language,"Sources being detached","Sources qui seront détachées"),p.detached.join(", "),words(language,"Linking sources approves no new stories. Retained stories losing coverage become manual / kept.","Lier des sources n’approuve aucune nouvelle histoire. Les histoires conservées qui perdent leur couverture deviennent manuelles / conservées."))}else{format!("{} {}",language.number(p.source_count as i64),plural(language,p.source_count as i64,"active Collection source","active Collection sources","source Collection active","sources Collections actives"))}.into();
    data.counts = format!(
        "{} {} · {} {} · {} {}\n{} {} · {} {} · {}/{}",
        language.number(p.additions as i64),
        plural(
            language,
            p.additions as i64,
            "addition",
            "additions",
            "ajout",
            "ajouts"
        ),
        language.number(p.removals as i64),
        plural(
            language,
            p.removals as i64,
            "removal / affected member",
            "removals / affected members",
            "retrait / membre concerné",
            "retraits / membres concernés"
        ),
        language.number(p.warnings as i64),
        plural(
            language,
            p.warnings as i64,
            "warning",
            "warnings",
            "avertissement",
            "avertissements"
        ),
        language.number(p.decisions as i64),
        plural(
            language,
            p.decisions as i64,
            "explicit decision",
            "explicit decisions",
            "décision explicite",
            "décisions explicites"
        ),
        language.number(p.required as i64),
        plural(
            language,
            p.required as i64,
            "required decision",
            "required decisions",
            "décision requise",
            "décisions requises"
        ),
        p.page + 1,
        p.pages
    )
    .into();
    data.apply_summary = format!(
        "{}: {} · {}: {} · {}: {} · {}: {}",
        words(language, "Approved Add", "Ajouts approuvés"),
        language.number(p.approved_additions as i64),
        words(language, "Approved Exclude", "Exclusions approuvées"),
        language.number(p.approved_exclusions as i64),
        words(language, "Approved Keep", "Conservations approuvées"),
        language.number(p.approved_keeps as i64),
        words(
            language,
            "Approved Remove and exclude",
            "Retraits et exclusions approuvés"
        ),
        language.number(p.approved_removals as i64)
    )
    .into();
    data.can_apply = p.can_apply && !data.stale;
    data.can_keep_all = matches!(p.kind, Kind::Sources | Kind::DeleteCollection) && p.required > 0;
    data.can_bulk_remove = p.bulk_removal_available;
    data.previous = p.page > 0;
    data.next = p.page + 1 < p.pages;
    data.previous_shelf = p.shelf > 0;
    data.next_shelf = p.shelf + 1 < p.shelves;
    data.mismatch = p.mismatch_prompt;
    data.apply_label = match p.kind {
        Kind::DeleteShelf => words(language, "Delete Shelf", "Supprimer l’Étagère"),
        Kind::DeleteCollection => words(
            language,
            "Delete Collection and apply approved Shelf decisions",
            "Supprimer la Collection et appliquer les décisions approuvées",
        ),
        Kind::Sources => words(
            language,
            "Apply source changes and approved decisions",
            "Appliquer les changements de sources et les décisions approuvées",
        ),
        Kind::Updates => format!(
            "{} ({})",
            words(
                language,
                "Apply approved changes",
                "Appliquer les modifications approuvées"
            ),
            language.number(p.decisions as i64)
        ),
    }
    .into();
    data.rows = update_rows(
        current,
        p.rows
            .iter()
            .enumerate()
            .map(|(index, r)| ShelfReviewRow {
                index: index as i32,
                uuid: r.uuid.clone().into(),
                title: r.title.clone().into(),
                detail: detail(r, language).into(),
                choice: r.choice,
                removal: p.section == Section::Removals,
                can_remove: r.can_remove,
                warning: p.section == Section::Warnings,
                broken: r.broken,
            })
            .collect(),
    );
}
fn send(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    action: Action,
    kind: Pending,
) {
    let mut state = view.borrow_mut();
    state.shelf_reviews.dismiss();
    if state
        .shelf_reviews
        .pending
        .is_some_and(|(_, pending)| !matches!(pending, Pending::DismissAck))
    {
        return;
    }
    let Some(generation) = state.snapshot.generation.clone() else {
        return;
    };
    if matches!(kind, Pending::Plan) {
        state.shelf_reviews.ticket = None;
        state.shelf_reviews.last_request = Some(action.clone());
        state.shelf_reviews.generation = Some(generation.clone());
    }
    match worker
        .borrow()
        .offer(Command::ShelfReview { generation, action })
    {
        Ok(revision) => state.shelf_reviews.pending = Some((revision, kind)),
        Err(_) => {
            state.shelf_reviews.feedback = Some(Feedback::Error {
                code: "settings.busy".into(),
                uncertain: false,
            })
        }
    }
    state.project(ui);
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_updates(move || {
        if let Some(ui) = weak.upgrade() {
            let id = current_shelf(&state.borrow().snapshot);
            if let Some(id) = id {
                send(&state, &pending, &ui, Action::Review(id), Pending::Plan);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_preview_shelf_sources(move || {
        if let Some(ui) = weak.upgrade() {
            let draft = state.borrow().shelves.source_draft();
            if let Some((id, sources)) = draft {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Sources { id, sources },
                    Pending::Plan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_delete_shelf(move || {
        if let Some(ui) = weak.upgrade() {
            let id = {
                let s = state.borrow();
                if ui.get_dialog() == 15 {
                    s.shelves.selected()
                } else {
                    current_shelf(&s.snapshot)
                }
            };
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::DeleteShelf(id),
                    Pending::Plan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_delete_collection(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_collection_impact_count() == 0 {
                return;
            }
            let id = state
                .borrow()
                .collections
                .selected()
                .and_then(|id| CollectionId::new(id).ok());
            if let Some(id) = id {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::DeleteCollection(id),
                    Pending::Plan,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            let action = state.borrow().shelf_reviews.last_request.clone();
            if let Some(a) = action {
                send(&state, &pending, &ui, a, Pending::Plan);
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_choose(move |uuid, choice| {
        if let Some(ui) = weak.upgrade() {
            let ticket = state.borrow().shelf_reviews.ticket.clone();
            if let Some(ticket) = ticket {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Choose {
                        ticket,
                        uuid: uuid.to_string(),
                        choice,
                    },
                    Pending::Choice,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_inspect(move |uuid| {
        if let Some(ui) = weak.upgrade() {
            let ticket = state.borrow().shelf_reviews.ticket.clone();
            if let Some(ticket) = ticket {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Inspect {
                        ticket,
                        uuid: uuid.to_string(),
                    },
                    Pending::Inspect,
                );
            }
        }
    });
    let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
    ui.on_review_bulk(move |choice| {
        if let Some(ui) = weak.upgrade() {
            let ticket = state.borrow().shelf_reviews.ticket.clone();
            if let Some(ticket) = ticket {
                send(
                    &state,
                    &pending,
                    &ui,
                    Action::Bulk { ticket, choice },
                    Pending::Choice,
                );
            }
        }
    });
    for kind in 0..3 {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move || {
            if let Some(ui) = weak.upgrade() {
                let ticket = state.borrow().shelf_reviews.ticket.clone();
                if let Some(t) = ticket {
                    let (action, kind) = match kind {
                        0 => (Action::KeepAll(t), Pending::Choice),
                        1 => (Action::Acknowledge(t), Pending::Acknowledge),
                        _ => (Action::Apply(t), Pending::Apply),
                    };
                    send(&state, &pending, &ui, action, kind);
                }
            }
        };
        match kind {
            0 => ui.on_review_keep_all(callback),
            1 => ui.on_review_acknowledge(callback),
            _ => ui.on_review_apply(callback),
        }
    }
    for kind in 0..3 {
        let (state, pending, weak) = (view.clone(), worker.clone(), ui.as_weak());
        let callback = move |delta: i32| {
            if let Some(ui) = weak.upgrade() {
                let action = {
                    let s = state.borrow();
                    s.shelf_reviews
                        .ticket
                        .clone()
                        .zip(s.snapshot.workflows.shelf_reviews.preview.as_ref())
                        .map(|(ticket, p)| Action::Page {
                            ticket,
                            section: if kind == 0 {
                                match delta {
                                    1 => Section::Removals,
                                    2 => Section::Warnings,
                                    _ => Section::Additions,
                                }
                            } else {
                                p.section
                            },
                            page: if kind == 1 {
                                p.page.saturating_add_signed(delta as isize)
                            } else {
                                0
                            },
                            shelf: if kind == 2 {
                                p.shelf.saturating_add_signed(delta as isize)
                            } else {
                                p.shelf
                            },
                        })
                };
                if let Some(a) = action {
                    send(&state, &pending, &ui, a, Pending::Page);
                }
            }
        };
        match kind {
            0 => ui.on_review_section(callback),
            1 => ui.on_review_page(callback),
            _ => ui.on_review_shelf(callback),
        }
    }
    let state = &view.borrow().shelf_reviews;
    let (dismiss, ack_revision, acknowledgement) = (
        state.dismiss.clone(),
        state.dismiss_ack.clone(),
        state.acknowledgement.clone(),
    );
    let weak = ui.as_weak();
    ui.on_review_close(move || {
        if let Some((generation, ticket)) = acknowledgement.borrow_mut().take() {
            ack_revision.set(Some(
                worker
                    .borrow()
                    .offer(Command::ShelfReview {
                        generation,
                        action: Action::Acknowledge(ticket),
                    })
                    .map_err(|_| ()),
            ));
            if ack_revision.get().is_some_and(|result| result.is_ok())
                && let Some(ui) = weak.upgrade()
            {
                let mut data = ui.get_review_data();
                data.busy = true;
                ui.set_review_data(data);
            }
        }
        dismiss.set(true);
    });
}
