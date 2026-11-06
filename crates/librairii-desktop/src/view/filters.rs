//! Main-thread drafts and bounded native filter projection; effects use Worker.
use super::{View, offer};
use librairii_app::localization::Catalogs;
use librairii_core::query::QueryRequest;
use librairii_desktop::presentation::{Command, Snapshot, Worker};
use librairii_ui::{
    filters::{self, Active, BooleanState, Edit, Group, OptionRow, Sort, Target},
    native::{ActiveFilterRow, DesktopWindow, FilterRow},
};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Default)]
pub struct State {
    pub request: QueryRequest,
    search: String,
    options: Vec<OptionRow>,
    active: Vec<Active>,
    collapsed: Vec<Group>,
}
impl State {
    pub fn new(request: QueryRequest) -> Self {
        Self {
            request,
            ..Default::default()
        }
    }
    pub fn project(&mut self, ui: &DesktopWindow, snapshot: &Snapshot, catalogs: &Catalogs) {
        self.options = filters::options(
            &self.request,
            &snapshot.facets.tags,
            &snapshot.facets.languages,
            snapshot.language,
            catalogs,
        );
        self.active = filters::active(
            &self.request,
            &snapshot.facets.tags,
            snapshot.language,
            catalogs,
        );
        let search = self.search.trim().to_lowercase();
        let mut rows = Vec::new();
        let mut group = None;
        for (index, row) in self.options.iter().enumerate().filter(|(_, r)| {
            r.label.to_lowercase().contains(&search) || r.heading.to_lowercase().contains(&search)
        }) {
            let expanded = !self.collapsed.contains(&row.group) || !search.is_empty();
            if group != Some(&row.group) {
                rows.push(FilterRow {
                    index: index as i32,
                    label: row.heading.clone().into(),
                    heading: true,
                    expanded,
                    ..Default::default()
                });
                group = Some(&row.group);
            }
            if expanded {
                rows.push(FilterRow {
                    index: index as i32,
                    label: row.label.clone().into(),
                    boolean: matches!(row.target, Target::Boolean(_)),
                    state: row.state,
                    ..Default::default()
                });
            }
        }
        ui.set_filter_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_active_filter_rows(ModelRc::new(VecModel::from(
            self.active
                .iter()
                .enumerate()
                .map(|(index, row)| ActiveFilterRow {
                    index: index as i32,
                    label: row.label.clone().into(),
                })
                .collect::<Vec<_>>(),
        )));
        ui.set_active_filter_summary(
            self.active
                .iter()
                .map(|r| r.label.as_str())
                .collect::<Vec<_>>()
                .join(" · ")
                .into(),
        );
        ui.set_filter_label(
            format!(
                "{} ({})",
                if ui.get_french() {
                    "Filtres"
                } else {
                    "Filters"
                },
                snapshot.language.number(self.active.len() as i64)
            )
            .into(),
        );
        let shelf = snapshot
            .navigation
            .as_ref()
            .and_then(|n| match n.current().location.scope {
                librairii_core::collection_codec::url::Scope::Shelf { manual_order, .. } => {
                    Some(manual_order)
                }
                _ => None,
            });
        ui.set_sort_manual_available(shelf.is_some());
        ui.set_sort_index(if shelf == Some(true) {
            2
        } else {
            i32::from(self.request.sort != "imported_desc")
        });
    }
}
fn apply(
    view: &Rc<RefCell<View>>,
    worker: &Rc<RefCell<Worker>>,
    ui: &DesktopWindow,
    edit: Edit,
) -> bool {
    let mut borrowed = view.borrow_mut();
    let view = &mut *borrowed;
    let sorting = matches!(edit, Edit::Sort(_));
    let Ok(request) = filters::edit(&view.filters.request, edit) else {
        ui.set_status(
            if ui.get_french() {
                "Ces critères ne sont pas valides."
            } else {
                "These criteria are not valid."
            }
            .into(),
        );
        return false;
    };
    let command = if sorting {
        Command::Sort {
            request: request.clone(),
            manual_order: false,
        }
    } else {
        Command::Request(request.clone())
    };
    let accepted = offer(&worker.borrow(), command, ui);
    if accepted {
        view.filters.request = request;
    }
    view.filters.project(ui, &view.snapshot, &view.catalogs);
    accepted
}
pub fn install(ui: &DesktopWindow, view: Rc<RefCell<View>>, worker: Rc<RefCell<Worker>>) {
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_search(move |text| {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            state.filters.request.criteria.name = text.into();
            state.filters.request.page = 1;
            offer(
                &pending.borrow(),
                Command::Request(state.filters.request.clone()),
                &ui,
            );
            let state = &mut *state;
            state.filters.project(&ui, &state.snapshot, &state.catalogs);
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_filter_search(move |text| {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            let state = &mut *state;
            state.filters.search = text.into();
            state.filters.project(&ui, &state.snapshot, &state.catalogs);
        }
    });
    let (state, weak) = (view.clone(), ui.as_weak());
    ui.on_filter_collapse(move |index| {
        if let Some(ui) = weak.upgrade() {
            let mut state = state.borrow_mut();
            let state = &mut *state;
            if let Some(group) = usize::try_from(index)
                .ok()
                .and_then(|i| state.filters.options.get(i))
                .map(|r| r.group.clone())
            {
                if state.filters.collapsed.contains(&group) {
                    state.filters.collapsed.retain(|g| g != &group);
                } else {
                    state.filters.collapsed.push(group);
                }
                state.filters.project(&ui, &state.snapshot, &state.catalogs);
            }
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_filter_edit(move |index, value| {
        if let Some(ui) = weak.upgrade() {
            let target = usize::try_from(index).ok().and_then(|i| {
                state
                    .borrow()
                    .filters
                    .options
                    .get(i)
                    .map(|r| r.target.clone())
            });
            if let Some(target) = target {
                let edit = if let Target::Boolean(definition) = target {
                    Edit::Boolean {
                        definition,
                        state: match value {
                            1 => BooleanState::Tagged,
                            2 => BooleanState::NotTagged,
                            _ => BooleanState::Any,
                        },
                    }
                } else {
                    Edit::Toggle(target)
                };
                apply(&state, &pending, &ui, edit);
            }
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_filter_remove(move |index| {
        if let Some(ui) = weak.upgrade() {
            let target = usize::try_from(index).ok().and_then(|i| {
                state
                    .borrow()
                    .filters
                    .active
                    .get(i)
                    .map(|r| r.target.clone())
            });
            if let Some(target) = target {
                let name = target == Target::Name;
                if apply(&state, &pending, &ui, Edit::Remove(target)) && name {
                    ui.set_query("".into());
                }
                ui.invoke_focus_filters();
            }
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_filter_clear(move || {
        if let Some(ui) = weak.upgrade()
            && apply(&state, &pending, &ui, Edit::Clear)
        {
            ui.set_query("".into());
            ui.invoke_close_dialog();
            ui.invoke_focus_search();
        }
    });
    let (state, weak, pending) = (view.clone(), ui.as_weak(), worker.clone());
    ui.on_filter_clear_tags(move || {
        if let Some(ui) = weak.upgrade() {
            apply(&state, &pending, &ui, Edit::ClearTags);
        }
    });
    let (state, weak, pending) = (view, ui.as_weak(), worker);
    ui.on_sort_selected(move |index| {
        if let Some(ui) = weak.upgrade() {
            if index == 2 {
                let request = state.borrow().filters.request.clone();
                offer(
                    &pending.borrow(),
                    Command::Sort {
                        request,
                        manual_order: true,
                    },
                    &ui,
                );
                return;
            }
            apply(
                &state,
                &pending,
                &ui,
                Edit::Sort(if index == 1 {
                    Sort::Name
                } else {
                    Sort::RecentlyAdded
                }),
            );
        }
    });
}
