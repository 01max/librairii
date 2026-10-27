//! Main-thread projection of owned bounded worker values into native models.
#[path = "view/collections.rs"]
pub(crate) mod collections;
#[path = "view/details.rs"]
pub(crate) mod details;
#[path = "view/filters.rs"]
pub(crate) mod filters;
#[path = "view/flags.rs"]
pub(crate) mod flags;
#[path = "view/shelf_reviews.rs"]
pub(crate) mod shelf_reviews;
#[path = "view/shelves.rs"]
pub(crate) mod shelves;
#[path = "view/tags.rs"]
pub(crate) mod tags;
use librairii_app::localization::{Catalogs, Language, LocalizationError};
use librairii_core::navigation::Presentation;
use librairii_desktop::{
    browsing::Story,
    presentation::{Command, Snapshot, Worker},
};
use librairii_ui::native::{
    CollectionPreview, DesktopWindow, NavigationRow, StoryGroup, StoryRow, TagRow,
};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
/// Keep native preview items mounted while a snapshot changes their data. A
/// fresh ModelRc would discard the focused Button/CheckBox on every response.
fn update_previews(ui: &DesktopWindow, mut next: Vec<CollectionPreview>) -> bool {
    let current = ui.get_collection_previews();
    if let Some(model) = current
        .as_any()
        .downcast_ref::<VecModel<CollectionPreview>>()
    {
        let previous = (0..model.row_count())
            .map(|i| model.row_data(i))
            .collect::<Option<Vec<_>>>();
        if let Some(previous) = previous {
            let stable = previous.len() == next.len()
                && previous.iter().zip(&next).all(|(old, row)| {
                    old.index == row.index
                        && old.stories.as_any().is::<VecModel<StoryRow>>()
                        && old.stories.row_count() == row.stories.row_count()
                        && old
                            .stories
                            .iter()
                            .zip(row.stories.iter())
                            .all(|(a, b)| a.uuid == b.uuid)
                });
            if stable {
                for (i, (row, old)) in next.iter_mut().zip(previous).enumerate() {
                    if let Some(stories) = old.stories.as_any().downcast_ref::<VecModel<StoryRow>>()
                    {
                        for (j, story) in row.stories.iter().enumerate() {
                            stories.set_row_data(j, story);
                        }
                        row.stories = old.stories;
                    }
                    model.set_row_data(i, row.clone());
                }
                return true;
            }
        }
    }
    ui.set_collection_previews(ModelRc::new(VecModel::from(next)));
    false
}
pub struct View {
    pub shelf_reviews: shelf_reviews::State,
    pub shelves: shelves::State,
    pub collections: collections::State,
    pub flags: flags::State,
    pub tags: tags::State,
    pub details: details::State,
    pub filters: filters::State,
    pub catalogs: Catalogs,
    pub snapshot: Snapshot,
    pub selected: Option<String>,
    pub columns: usize,
    pub guard: librairii_ui::responses::ResponseGuard,
}
fn row(index: usize, story: &Story, language: Language, catalogs: &Catalogs) -> StoryRow {
    let (broken, available) = story
        .summary
        .as_ref()
        .map(|s| (s.broken, s.compatibility != "missing"))
        .unwrap_or((false, false));
    let warning = match (broken, available, language) {
        (true, _, Language::English) => "⚠ Broken · excluded from exports",
        (true, _, Language::French) => "⚠ Défectueuse · exclue des exports",
        (_, false, Language::English) => "Unavailable · excluded from exports",
        (_, false, Language::French) => "Indisponible · exclue des exports",
        _ => "",
    };
    let cover = story
        .thumbnail
        .as_ref()
        .map(|t| {
            slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
                &t.rgba, t.width, t.height,
            ))
        })
        .unwrap_or_default();
    let tags = story
        .tags
        .iter()
        .map(|tag| {
            let color = tag
                .color
                .strip_prefix('#')
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .map(|rgb| {
                    slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
                })
                .unwrap_or_else(|| slint::Color::from_rgb_u8(80, 80, 80));
            TagRow {
                label: catalogs.tag_label(language, tag).into(),
                color,
            }
        })
        .collect::<Vec<_>>();
    let metadata = story
        .summary
        .as_ref()
        .map(|s| {
            format!(
                "{} · {} {}",
                match s.detected_format.as_str() {
                    "studio_zip" => "STUdio ZIP",
                    "seven_zip" => "7z",
                    "zip" => "ZIP",
                    "v1_pk" => "PK v1",
                    "v2_pk" => "PK v2",
                    _ => "PK",
                },
                language.number(s.byte_size),
                if language == Language::French {
                    if language.singular(s.byte_size as u64) {
                        "octet"
                    } else {
                        "octets"
                    }
                } else if s.byte_size == 1 {
                    "byte"
                } else {
                    "bytes"
                }
            )
        })
        .unwrap_or_default();
    let words = |en: &str, fr: &str| {
        if language == Language::French {
            fr.to_owned()
        } else {
            en.to_owned()
        }
    };
    let (member_status, acceptance, matches) = story
        .membership
        .as_ref()
        .map(|member| {
            let mut status = if !available {
                format!(
                    "{} · {}",
                    words(
                        "Unavailable · approved UUID retained",
                        "Indisponible · UUID approuvé conservé"
                    ),
                    member.uuid
                )
            } else if member.manually_retained {
                words("Manual / kept", "Manuelle / conservée")
            } else {
                words("From Collections", "Issue des Collections")
            };
            if broken {
                status.push_str(&format!(" · {warning}"));
            }
            let origins = member
                .origins
                .iter()
                .map(|s| {
                    format!(
                        "{}{}",
                        s.name,
                        if s.active {
                            String::new()
                        } else {
                            words(" (detached)", " (détachée)")
                        }
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let acceptance = format!(
                "{}: {}",
                words("Added from", "Ajoutée depuis"),
                if origins.is_empty() {
                    words("manual selection", "sélection manuelle")
                } else {
                    origins
                }
            );
            let current = story
                .current_sources
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            let matches = format!(
                "{}: {}{}",
                words("Current matches", "Correspondances actuelles"),
                if !available {
                    words("unavailable; unknown", "indisponibles ; inconnues")
                } else if current.is_empty() && story.coverage_incomplete {
                    words("unknown", "inconnues")
                } else if current.is_empty() {
                    words("none", "aucune")
                } else {
                    current
                },
                if story.coverage_incomplete {
                    words(" · incomplete coverage", " · couverture incomplète")
                } else {
                    String::new()
                }
            );
            (status, acceptance, matches)
        })
        .unwrap_or_default();
    StoryRow {
        member_status: member_status.into(),
        acceptance: acceptance.into(),
        matches: matches.into(),
        broken,
        bulk: false,
        uuid: story.uuid.clone().into(),
        facts: details::facts(story, language).into(),
        display_sources: details::sources(story, language).into(),
        technical: if language == Language::French {
            "Chargement des détails techniques…"
        } else {
            "Loading technical details…"
        }
        .into(),
        tags: ModelRc::new(VecModel::from(tags)),
        metadata: metadata.into(),
        index: index as i32,
        title: story.title.clone().into(),
        author: story
            .summary
            .as_ref()
            .map(|s| s.author.as_str())
            .unwrap_or_default()
            .into(),
        description: story
            .summary
            .as_ref()
            .map(|s| s.description.as_str())
            .unwrap_or_default()
            .into(),
        warning: warning.into(),
        provenance: if story.provenance.is_empty() {
            String::new()
        } else {
            format!(
                "{}: {}",
                if language == Language::French {
                    "Origine"
                } else {
                    "Source"
                },
                story.provenance
            )
        }
        .into(),
        cover,
    }
}
impl View {
    pub fn accept(&mut self, ui: &DesktopWindow, snapshot: Snapshot, revision: u64) -> bool {
        if let Some(generation) = &snapshot.generation {
            if self.snapshot.generation.is_none() {
                self.guard.activate(generation.clone());
            }
            self.guard.expect(revision);
            if !self.guard.accepts(generation, snapshot.revision) {
                return false;
            }
        }
        if let Some(n) = &snapshot.navigation {
            ui.set_mode(match n.presentation() {
                Presentation::List => 0,
                Presentation::SmallCovers => 1,
                Presentation::LargeCovers => 2,
            });
            if self.snapshot.navigation.is_none()
                || snapshot.restore_revision != self.snapshot.restore_revision
            {
                ui.set_query(n.current().location.request.criteria.name.clone().into());
                self.filters.request = n.current().location.request.clone();
                self.selected = None;
            }
        }
        self.snapshot = snapshot;
        self.details.accept(ui, &self.snapshot);
        self.tags.accept(ui, &self.snapshot);
        self.flags.accept(ui, &self.snapshot);
        self.collections.accept(ui, &self.snapshot);
        if let Some(uuid) = self.shelves.accept(ui, &self.snapshot) {
            self.selected = Some(uuid);
        }
        self.shelf_reviews.accept(ui, &self.snapshot);
        self.resize(ui);
        self.project(ui);
        self.shelves.restore_focus(ui);
        self.shelf_reviews.restore_focus(ui);
        true
    }
    pub fn project(&mut self, ui: &DesktopWindow) {
        self.tags.retain_visible(&self.snapshot);
        self.flags.project(
            ui,
            &self.snapshot,
            self.selected.as_deref(),
            &self.tags.selection,
        );
        let french = self.snapshot.language == Language::French;
        ui.set_french(french);
        self.filters.project(ui, &self.snapshot, &self.catalogs);
        ui.set_startup_phase(match self.snapshot.phase {
            librairii_desktop::presentation::Phase::Opening => 4,
            librairii_desktop::presentation::Phase::Setup => 0,
            librairii_desktop::presentation::Phase::Adoption => 1,
            librairii_desktop::presentation::Phase::Recovery => 2,
            librairii_desktop::presentation::Phase::Library => 3,
        });
        ui.set_status(self.snapshot.status.clone().into());
        self.collections.project(
            ui,
            &self.snapshot,
            &self.catalogs,
            &self.filters.request.criteria,
        );
        self.shelves.project(
            ui,
            &self.snapshot,
            &self.filters.request,
            self.selected.as_deref(),
            &self.tags.selection,
        );
        self.shelf_reviews.project(ui, &self.snapshot);
        ui.set_back_enabled(
            self.snapshot
                .navigation
                .as_ref()
                .is_some_and(|navigation| navigation.can_back()),
        );
        ui.set_forward_enabled(
            self.snapshot
                .navigation
                .as_ref()
                .is_some_and(|navigation| navigation.can_forward()),
        );
        let nav_rows = |shelves: bool| {
            self.snapshot
                .groups
                .iter()
                .enumerate()
                .filter(|(_, g)| {
                    matches!(
                        g.scope,
                        librairii_core::collection_codec::url::Scope::Shelf { .. }
                    ) == shelves
                })
                .map(|(index, g)| NavigationRow {
                    index: index as i32,
                    label: format!("{} ({})", g.name, self.snapshot.language.number(g.count))
                        .into(),
                    enabled: g.available,
                })
                .collect::<Vec<_>>()
        };
        ui.set_collection_rows(ModelRc::new(VecModel::from(nav_rows(false))));
        ui.set_shelf_rows(ModelRc::new(VecModel::from(nav_rows(true))));
        let title = self
            .snapshot
            .navigation
            .as_ref()
            .and_then(|n| {
                self.snapshot.groups.iter().find(|g| {
                    match (&g.scope, &n.current().location.scope) {
                        (
                            librairii_core::collection_codec::url::Scope::Collection(a),
                            librairii_core::collection_codec::url::Scope::Collection(b),
                        ) => a == b,
                        (
                            librairii_core::collection_codec::url::Scope::Shelf { id: a, .. },
                            librairii_core::collection_codec::url::Scope::Shelf { id: b, .. },
                        ) => a == b,
                        _ => false,
                    }
                })
            })
            .map(|g| g.name.as_str())
            .unwrap_or(if french { "Bibliothèque" } else { "Library" });
        ui.set_scope_title(title.into());
        let Some(page) = &self.snapshot.page else {
            ui.set_story_groups(ModelRc::new(VecModel::from(Vec::<StoryGroup>::new())));
            ui.set_story_count(0);
            ui.set_collection_previews(ModelRc::default());
            ui.set_selected_index(-1);
            ui.set_selected_story(StoryRow::default());
            ui.set_counts("".into());
            ui.set_previous_enabled(false);
            ui.set_next_enabled(false);
            self.tags
                .project(ui, &self.snapshot, &self.catalogs, self.selected.as_deref());
            return;
        };
        let selected = self
            .selected
            .as_ref()
            .and_then(|uuid| page.all_stories().position(|s| &s.uuid == uuid))
            .or_else(|| {
                self.snapshot.navigation.as_ref().and_then(|n| {
                    page.all_stories().position(|s| {
                        s.summary
                            .as_ref()
                            .is_some_and(|s| n.current().selection.contains(&s.id))
                    })
                })
            })
            .or_else(|| (!page.stories.is_empty()).then_some(0));
        self.selected =
            selected.and_then(|index| page.all_stories().nth(index).map(|s| s.uuid.clone()));
        let rows = page
            .all_stories()
            .enumerate()
            .map(|(i, s)| {
                let mut row = row(i, s, self.snapshot.language, &self.catalogs);
                row.bulk = self.tags.selection.contains(&s.uuid);
                row
            })
            .collect::<Vec<_>>();
        let groups = rows[..page.stories.len()]
            .chunks(self.columns.max(1))
            .map(|cards| StoryGroup {
                cards: ModelRc::new(VecModel::from(cards.to_vec())),
            })
            .collect::<Vec<_>>();
        ui.set_story_groups(ModelRc::new(VecModel::from(groups)));
        ui.set_story_count(page.stories.len() as i32);
        let mut offset = page.stories.len();
        let previews = page
            .previews
            .iter()
            .map(|preview| {
                let stories = rows[offset..offset + preview.stories.len()].to_vec();
                offset += preview.stories.len();
                CollectionPreview {
                    index: preview.group_index as i32,
                    title: preview.name.clone().into(),
                    caption: if preview.failed {
                        if french {
                            "L’aperçu n’a pas pu être chargé. Actualisez pour réessayer.".into()
                        } else {
                            "Preview could not be loaded. Refresh to retry.".into()
                        }
                    } else if preview.unavailable {
                        if french {
                            "Réparation nécessaire".into()
                        } else {
                            "Needs repair".into()
                        }
                    } else {
                        format!(
                            "{} {}",
                            self.snapshot.language.number(preview.total),
                            if french {
                                if self.snapshot.language.singular(preview.total as u64) {
                                    "histoire"
                                } else {
                                    "histoires"
                                }
                            } else if self.snapshot.language.singular(preview.total as u64) {
                                "story"
                            } else {
                                "stories"
                            }
                        )
                        .into()
                    },
                    available: !preview.unavailable && !preview.failed,
                    stories: ModelRc::new(VecModel::from(stories)),
                }
            })
            .collect::<Vec<_>>();
        let preview_structure_retained = update_previews(ui, previews);
        ui.set_group_previous(page.preview_page > 0);
        ui.set_group_next(page.preview_page + 1 < page.preview_pages);
        let focused = ui.get_focused_key();
        if focused < 0 && !ui.get_modal() {
            let mounted = page
                .previews
                .iter()
                .any(|p| focused == -1000 - p.group_index as i32 * 3)
                || (page.stories.len()..rows.len())
                    .any(|i| focused == -1001 - i as i32 * 3 || focused == -1002 - i as i32 * 3);
            if !mounted || !preview_structure_retained {
                ui.invoke_focus_browser();
            }
        }
        ui.set_columns(self.columns as i32);
        ui.set_selected_index(selected.map(|i| i as i32).unwrap_or(-1));
        let mut selected_row = selected
            .and_then(|i| rows.get(i))
            .cloned()
            .unwrap_or_default();
        details::project(&mut selected_row, &self.snapshot);
        ui.set_selected_story(selected_row);
        self.flags.project(
            ui,
            &self.snapshot,
            self.selected.as_deref(),
            &self.tags.selection,
        );
        self.tags
            .project(ui, &self.snapshot, &self.catalogs, self.selected.as_deref());
        self.shelves.project(
            ui,
            &self.snapshot,
            &self.filters.request,
            self.selected.as_deref(),
            &self.tags.selection,
        );
        self.shelf_reviews.project(ui, &self.snapshot);
        ui.set_previous_enabled(page.page > 1);
        ui.set_next_enabled(page.page < page.pages);
        ui.set_counts(
            format!(
                "{} {} · {} / {}",
                self.snapshot.language.number(page.total),
                if french {
                    if self.snapshot.language.singular(page.total as u64) {
                        "histoire"
                    } else {
                        "histoires"
                    }
                } else if page.total == 1 {
                    "story"
                } else {
                    "stories"
                },
                self.snapshot.language.number(page.page),
                self.snapshot.language.number(page.pages.max(1))
            )
            .into(),
        );
    }
    pub fn resize(&mut self, ui: &DesktopWindow) {
        let size = ui.window().size().to_logical(ui.window().scale_factor());
        ui.set_compact(size.width < 850.0);
        ui.set_wide(size.width >= 1180.0);
        let content = size.width
            - if size.width >= 850.0 { 216.0 } else { 0.0 }
            - if size.width >= 1180.0 {
                264.0 + 12.0
            } else {
                0.0
            }
            - 40.0;
        let columns = if ui.get_mode() == 0
            || self.snapshot.navigation.as_ref().is_some_and(|n| {
                matches!(
                    n.current().location.scope,
                    librairii_core::collection_codec::url::Scope::Shelf { .. }
                )
            }) {
            1
        } else {
            ((content / if ui.get_mode() == 1 { 170.0 } else { 240.0 }).floor() as usize)
                .clamp(1, 8)
        };
        if self.columns != columns {
            self.columns = columns;
            self.project(ui);
        }
    }
}
fn offer(worker: &Worker, command: Command, ui: &DesktopWindow) -> bool {
    if worker.offer(command).is_err() {
        ui.set_status(
            if ui.get_french() {
                "La bibliothèque est occupée. Réessayez."
            } else {
                "Library is busy. Try again."
            }
            .into(),
        );
        false
    } else {
        true
    }
}
pub fn install(
    ui: &DesktopWindow,
    worker: Rc<RefCell<Worker>>,
    initial: Snapshot,
) -> Result<Rc<RefCell<View>>, LocalizationError> {
    let view = Rc::new(RefCell::new(View {
        shelf_reviews: Default::default(),
        shelves: Default::default(),
        collections: Default::default(),
        flags: Default::default(),
        tags: Default::default(),
        details: Default::default(),
        filters: filters::State::new(
            initial
                .navigation
                .as_ref()
                .map(|n| n.current().location.request.clone())
                .unwrap_or_default(),
        ),
        catalogs: Catalogs::embedded()?,
        snapshot: initial,
        selected: None,
        columns: 1,
        guard: Default::default(),
    }));
    if let Some(navigation) = &view.borrow().snapshot.navigation {
        ui.set_mode(match navigation.presentation() {
            Presentation::List => 0,
            Presentation::SmallCovers => 1,
            Presentation::LargeCovers => 2,
        });
        ui.set_query(
            navigation
                .current()
                .location
                .request
                .criteria
                .name
                .clone()
                .into(),
        );
    }
    let generation = view.borrow().snapshot.generation.clone();
    if let Some(generation) = generation {
        view.borrow_mut().guard.activate(generation);
    }
    view.borrow_mut().project(ui);
    view.borrow_mut().shelf_reviews.restore_focus(ui);
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_mode_selected(move |mode| {
        if let Some(ui) = weak.upgrade() {
            let mode = match mode {
                1 => Presentation::SmallCovers,
                2 => Presentation::LargeCovers,
                _ => Presentation::List,
            };
            state.borrow_mut().resize(&ui);
            offer(&pending.borrow(), Command::Presentation(mode), &ui);
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_select_story(move |index| {
        if let Some(ui) = weak.upgrade() {
            let story = state
                .borrow()
                .snapshot
                .page
                .as_ref()
                .and_then(|p| p.all_stories().nth(index as usize))
                .cloned();
            if let Some(story) = story {
                state.borrow_mut().selected = Some(story.uuid.clone());
                state.borrow_mut().project(&ui);
                if let Some(n) = state.borrow().snapshot.navigation.as_ref() {
                    let command = if let Some(summary) = story.summary {
                        Command::Select {
                            id: summary.id,
                            query: n.stamp(),
                        }
                    } else {
                        Command::SelectUnavailable {
                            uuid: story.uuid,
                            query: n.stamp(),
                        }
                    };
                    offer(&pending.borrow(), command, &ui);
                }
            }
        }
    });
    let (weak, pending) = (ui.as_weak(), worker.clone());
    ui.on_change_group_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            offer(&pending.borrow(), Command::GroupPage(delta), &ui);
        }
    });
    let (weak, pending) = (ui.as_weak(), worker.clone());
    ui.on_change_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            offer(&pending.borrow(), Command::ChangePage(delta), &ui);
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_navigate_group(move |index| {
        if let Some(ui) = weak.upgrade() {
            let scope = usize::try_from(index).ok().and_then(|index| {
                state
                    .borrow()
                    .snapshot
                    .groups
                    .get(index)
                    .map(|g| g.scope.clone())
            });
            if let Some(scope) = scope {
                match scope {
                    librairii_core::collection_codec::url::Scope::Collection(id) => {
                        collections::open_group(&state, &pending, &ui, id.get())
                    }
                    scope => {
                        offer(&pending.borrow(), Command::Navigate(scope), &ui);
                    }
                }
            }
        }
    });
    let (weak, pending) = (ui.as_weak(), worker.clone());
    ui.on_history_step(move |step| {
        if let Some(ui) = weak.upgrade() {
            offer(
                &pending.borrow(),
                if step < 0 {
                    Command::Back
                } else {
                    Command::Forward
                },
                &ui,
            );
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_request_panel(move |panel| {
        if panel == 10
            && let Some(ui) = weak.upgrade()
        {
            let mut state = state.borrow_mut();
            let manual = flags::targets(&state, !state.tags.selection.is_empty());
            let bulk = !state.tags.selection.is_empty();
            let state = &mut *state;
            state.shelves.open(&ui, &state.snapshot, manual, bulk);
            state.project(&ui);
        }
        if panel == 9
            && let Some(ui) = weak.upgrade()
        {
            let mut state = state.borrow_mut();
            let state = &mut *state;
            state.collections.open(&ui, &state.snapshot);
            state.project(&ui);
        }
        if panel == 1
            && let Some(ui) = weak.upgrade()
        {
            let mut state = state.borrow_mut();
            let state = &mut *state;
            state.tags.open(&ui, &state.snapshot);
            state.tags.project(
                &ui,
                &state.snapshot,
                &state.catalogs,
                state.selected.as_deref(),
            );
        }
        if panel == 0
            && let Some(ui) = weak.upgrade()
        {
            offer(
                &pending.borrow(),
                Command::Navigate(librairii_core::collection_codec::url::Scope::Library),
                &ui,
            );
        }
    });
    let (weak, pending) = (ui.as_weak(), worker.clone());
    ui.on_change_language(move |code| {
        if let (Some(ui), Ok(language)) = (weak.upgrade(), Language::explicit(&code)) {
            offer(&pending.borrow(), Command::Language(language), &ui);
        }
    });
    collections::install(ui, view.clone(), worker.clone());
    shelves::install(ui, view.clone(), worker.clone());
    shelf_reviews::install(ui, view.clone(), worker.clone());
    flags::install(ui, view.clone(), worker.clone());
    tags::install(ui, view.clone(), worker.clone());
    details::install(ui, view.clone(), worker.clone());
    filters::install(ui, view.clone(), worker);
    Ok(view)
}
