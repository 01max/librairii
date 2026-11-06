//! Search feedback must not move the results viewport on every edit.
use super::{Result, check, view};
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
use std::{cell::RefCell, rc::Rc};

pub fn checks(ui: &DesktopWindow, state: &Rc<RefCell<view::View>>) -> Result {
    let original_query = ui.get_query();
    let original_name = state.borrow().filters.request.criteria.name.clone();
    let project = |text: &str| {
        ui.set_query(text.into());
        let mut state = state.borrow_mut();
        state.filters.request.criteria.name = text.into();
        let state = &mut *state;
        state.filters.project(ui, &state.snapshot, &state.catalogs);
    };
    let result = (|| -> Result {
        project("");
        let initial = ui.window().take_snapshot()?;
        let height = ui.get_observed_browser_height();
        check(
            height >= ui.get_observed_story_row_height(),
            "typing fixture needs a full story row",
        )?;
        for text in (0..64).map(|index| if index % 2 == 0 { "x" } else { "" }) {
            project(text);
            let frame = ui.window().take_snapshot()?;
            check(
                frame.width() == initial.width() && frame.height() == initial.height(),
                "typing resized the native window",
            )?;
            check(
                (ui.get_observed_browser_height() - height).abs() < 0.1,
                "search feedback moved the results viewport during typing",
            )?;
            check(
                ui.get_active_filter_summary().is_empty() == text.is_empty(),
                "stable typing geometry lost active-filter feedback",
            )?;
        }
        Ok(())
    })();
    project(&original_name);
    ui.set_query(original_query);
    result
}
