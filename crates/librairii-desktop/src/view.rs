//! Main-thread projection of owned bounded worker values into native models.
use librairii_app::localization::Language;
use librairii_core::navigation::Presentation;
use librairii_desktop::{
    browsing::Story,
    presentation::{Command, Snapshot, Worker},
};
use librairii_ui::native::{DesktopWindow, StoryGroup, StoryRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};
pub struct View {
    pub snapshot: Snapshot,
    pub selected: Option<String>,
    pub columns: usize,
}
fn row(index: usize, story: &Story, language: Language) -> StoryRow {
    let (broken, available) = story
        .summary
        .as_ref()
        .map(|s| (s.broken, s.compatibility != "missing"))
        .unwrap_or((false, false));
    let warning = match (broken, available, language) {
        (true, _, Language::English) => "Broken · excluded from exports",
        (true, _, Language::French) => "Défectueuse · exclue des exports",
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
    StoryRow {
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
        provenance: story.provenance.clone().into(),
        cover,
    }
}
impl View {
    pub fn project(&mut self, ui: &DesktopWindow) {
        let french = self.snapshot.language == Language::French;
        ui.set_french(french);
        ui.set_startup_phase(match self.snapshot.phase {
            librairii_desktop::presentation::Phase::Setup => 0,
            librairii_desktop::presentation::Phase::Adoption => 1,
            librairii_desktop::presentation::Phase::Recovery => 2,
            librairii_desktop::presentation::Phase::Library => 3,
        });
        ui.set_status(self.snapshot.status.clone().into());
        let Some(page) = &self.snapshot.page else {
            return;
        };
        let selected = self
            .selected
            .as_ref()
            .and_then(|uuid| page.stories.iter().position(|s| &s.uuid == uuid))
            .or_else(|| {
                self.snapshot.navigation.as_ref().and_then(|n| {
                    page.stories.iter().position(|s| {
                        s.summary
                            .as_ref()
                            .is_some_and(|s| n.current().selection.contains(&s.id))
                    })
                })
            });
        let rows = page
            .stories
            .iter()
            .enumerate()
            .map(|(i, s)| row(i, s, self.snapshot.language))
            .collect::<Vec<_>>();
        let groups = rows
            .chunks(self.columns.max(1))
            .map(|cards| StoryGroup {
                cards: ModelRc::new(VecModel::from(cards.to_vec())),
            })
            .collect::<Vec<_>>();
        ui.set_story_groups(ModelRc::new(VecModel::from(groups)));
        ui.set_story_count(rows.len() as i32);
        ui.set_columns(self.columns as i32);
        ui.set_selected_index(selected.map(|i| i as i32).unwrap_or(-1));
        ui.set_selected_story(
            selected
                .and_then(|i| rows.get(i))
                .cloned()
                .unwrap_or_default(),
        );
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
                self.snapshot.language.number(page.pages)
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
        let columns = if ui.get_mode() == 0 {
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
fn offer(worker: &Worker, command: Command, ui: &DesktopWindow) {
    if worker.commands.try_send(command).is_err() {
        ui.set_status(
            if ui.get_french() {
                "La bibliothèque est occupée. Réessayez."
            } else {
                "Library is busy. Try again."
            }
            .into(),
        );
    }
}
pub fn install(
    ui: &DesktopWindow,
    worker: Rc<RefCell<Worker>>,
    initial: Snapshot,
) -> Rc<RefCell<View>> {
    let view = Rc::new(RefCell::new(View {
        snapshot: initial,
        selected: None,
        columns: 1,
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
    view.borrow_mut().project(ui);
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
                .and_then(|p| p.stories.get(index as usize))
                .cloned();
            if let Some(story) = story {
                state.borrow_mut().selected = Some(story.uuid.clone());
                ui.set_selected_story(row(
                    index as usize,
                    &story,
                    state.borrow().snapshot.language,
                ));
                if let Some(summary) = story.summary {
                    offer(&pending.borrow(), Command::Select(summary.id), &ui);
                }
            }
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_change_page(move |delta| {
        if let Some(ui) = weak.upgrade() {
            if let Some(n) = &state.borrow().snapshot.navigation {
                let mut request = n.current().location.request.clone();
                request.page = (state
                    .borrow()
                    .snapshot
                    .page
                    .as_ref()
                    .map(|p| p.page)
                    .unwrap_or(1)
                    + i64::from(delta))
                .max(1);
                offer(&pending.borrow(), Command::Request(request), &ui);
            }
        }
    });
    let (weak, pending) = (ui.as_weak(), worker.clone());
    ui.on_change_language(move |code| {
        if let (Some(ui), Ok(language)) = (weak.upgrade(), Language::explicit(&code)) {
            offer(&pending.borrow(), Command::Language(language), &ui);
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker);
    ui.on_search(move |text| {
        if let Some(ui) = weak.upgrade() {
            if let Some(n) = &state.borrow().snapshot.navigation {
                let mut request = n.current().location.request.clone();
                request.criteria.name = text.into();
                request.page = 1;
                offer(&pending.borrow(), Command::Request(request), &ui);
            }
        }
    });
    view
}
