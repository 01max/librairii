//! Selection must not discard native image buffers for unchanged decoded covers.
use super::{Result, check, view};
use librairii_adapters::thumbnails::Thumbnail;
use librairii_ui::native::DesktopWindow;
use slint::Model;
use std::{cell::RefCell, rc::Rc, sync::Arc};

fn covers(ui: &DesktopWindow) -> Vec<slint::Image> {
    ui.get_story_groups()
        .iter()
        .flat_map(|group| {
            group
                .cards
                .iter()
                .map(|card| card.cover)
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn checks(ui: &DesktopWindow, state: &Rc<RefCell<view::View>>) -> Result {
    let before = covers(ui);
    check(
        before.len() == 24,
        "cover projection fixture needs 24 visible rows",
    )?;
    check(
        before.iter().all(|image| image.size().width > 0),
        "cover fixture is not decoded",
    )?;
    for index in (0..64).map(|i| [0, 1, 23, 0][i % 4]) {
        ui.invoke_select_story(index);
        check(
            covers(ui) == before,
            "selection replaced unchanged native cover buffers",
        )?;
    }

    let original = state
        .borrow()
        .snapshot
        .page
        .as_ref()
        .ok_or("cover page")?
        .stories[0]
        .thumbnail
        .clone();
    let pixels: Arc<[u8]> = Arc::from([200, 20, 40, 255, 10, 120, 30, 255]);
    let retired = Arc::downgrade(&pixels);
    state
        .borrow_mut()
        .snapshot
        .page
        .as_mut()
        .ok_or("cover page")?
        .stories[0]
        .thumbnail = Some(Thumbnail {
        width: 2,
        height: 1,
        rgba: pixels.clone(),
    });
    state.borrow_mut().project(ui);
    let replaced = covers(ui);
    check(
        replaced[0] != before[0],
        "new decoded pixels reused a stale native image",
    )?;
    check(
        replaced[0].to_rgba8().ok_or("RGBA cover")?.as_bytes()
            == [200, 20, 40, 255, 10, 120, 30, 255],
        "replacement cover pixels differ",
    )?;
    check(
        replaced[1..] == before[1..],
        "one changed cover replaced other native images",
    )?;
    {
        let mut state = state.borrow_mut();
        let thumbnail = state.snapshot.page.as_mut().ok_or("cover page")?.stories[0]
            .thumbnail
            .as_mut()
            .ok_or("cover thumbnail")?;
        thumbnail.width = 1;
        thumbnail.height = 2;
    }
    state.borrow_mut().project(ui);
    let resized = covers(ui);
    check(
        resized[0] != replaced[0] && resized[0].size().width == 1 && resized[0].size().height == 2,
        "changed cover dimensions reused a stale native image",
    )?;

    state
        .borrow_mut()
        .snapshot
        .page
        .as_mut()
        .ok_or("cover page")?
        .stories[0]
        .thumbnail = None;
    state.borrow_mut().project(ui);
    check(
        covers(ui)[0].size().width == 0,
        "removed cover remained visible",
    )?;
    check(
        Arc::strong_count(&pixels) == 1 && Arc::weak_count(&pixels) == 1,
        "projection retained an invisible cover entry",
    )?;
    drop(pixels);
    check(
        retired.strong_count() == 0,
        "projection retained retired decoded cover pixels",
    )?;
    state
        .borrow_mut()
        .snapshot
        .page
        .as_mut()
        .ok_or("cover page")?
        .stories[0]
        .thumbnail = original;
    state.borrow_mut().project(ui);
    ui.invoke_select_story(0);
    Ok(())
}
