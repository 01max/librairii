//! Actual AppKit menu actions through the production menu owner; no physical HID.
use super::{Result, check, key};
use librairii_desktop::menus::Menus;
use librairii_ui::native::DesktopWindow;
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{NSApplication, NSEvent, NSEventModifierFlags as Flags, NSEventType, NSMenu};
use objc2_foundation::{NSPoint, NSString};
use std::{cell::RefCell, rc::Rc};

fn main_menu() -> Result<Retained<NSMenu>> {
    NSApplication::sharedApplication(MainThreadMarker::new().ok_or("menu requires main thread")?)
        .mainMenu()
        .ok_or_else(|| "native application menu missing".into())
}
fn shortcut(text: &str, flags: Flags, code: u16) -> Result<bool> {
    // AppKit's charactersIgnoringModifiers retains Shift, unlike Command/Option.
    let text = NSString::from_str(&if flags.contains(Flags::Shift) {
        text.to_uppercase()
    } else {
        text.to_owned()
    });
    let event = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown, NSPoint::new(0., 0.), flags, 0., 0, None, &text, &text, false, code,
    ).ok_or("native key event unavailable")?;
    Ok(main_menu()?.performKeyEquivalent(&event))
}
pub fn verify(ui: &DesktopWindow, menus: &Menus) -> Result {
    check(
        menus.owns_native_menu(),
        "Slint replaced the production native menu",
    )?;
    for french in [false, true] {
        ui.set_french(french);
        menus.dispatch(ui);
        let bar = main_menu()?;
        check(
            bar.numberOfItems() == 3,
            "unexpected native menu owner/hierarchy",
        )?;
        let app = bar
            .itemAtIndex(0)
            .and_then(|item| item.submenu())
            .ok_or("application menu missing")?;
        let settings = app
            .itemAtIndex(2)
            .ok_or("application Settings item missing")?;
        check(
            settings.title().to_string()
                == if french {
                    "Réglages…"
                } else {
                    "Settings…"
                },
            "native Settings is not localized in the application menu",
        )?;
        let visibility = if french {
            ["Masquer Librairii", "Masquer les autres", "Tout afficher"]
        } else {
            ["Hide Librairii", "Hide Others", "Show All"]
        };
        for (index, label) in visibility.into_iter().enumerate() {
            check(
                app.itemAtIndex(5 + index as isize)
                    .ok_or("native application visibility item missing")?
                    .title()
                    .to_string()
                    == label,
                "native application visibility action is not localized",
            )?;
        }
        check(
            shortcut(",", Flags::Command, 43)?,
            "Command-comma was not handled natively",
        )?;
        menus.dispatch(ui);
        check(ui.get_dialog() == 2, "native Settings event was lost")?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_modal() && menus.owns_native_menu(),
            "menu ownership/focus changed after Settings",
        )?;
    }
    ui.set_back_enabled(true);
    ui.set_forward_enabled(true);
    menus.dispatch(ui);
    let steps = Rc::new(RefCell::new(Vec::new()));
    let recorded = steps.clone();
    ui.on_history_step(move |step| recorded.borrow_mut().push(step));
    for (text, code) in [("\u{f702}", 123), ("\u{f702}", 123), ("\u{f703}", 124)] {
        check(
            shortcut(text, Flags::Option, code)?,
            "native history accelerator missing",
        )?;
    }
    menus.dispatch(ui);
    check(
        *steps.borrow() == [-1, -1, 1],
        "native history commands coalesced or changed order",
    )?;
    ui.set_back_enabled(false);
    ui.set_forward_enabled(false);
    menus.dispatch(ui);
    let navigation = main_menu()?
        .itemAtIndex(2)
        .and_then(|item| item.submenu())
        .ok_or("native navigation menu missing")?;
    check(
        !navigation
            .itemAtIndex(0)
            .ok_or("native Back missing")?
            .isEnabled(),
        "native Back is not disabled",
    )?;
    // AppKit may consume a disabled key equivalent without activating its item.
    let _consumed = shortcut("\u{f702}", Flags::Option, 123)?;
    menus.dispatch(ui);
    check(
        *steps.borrow() == [-1, -1, 1],
        "disabled native history changed navigation",
    )?;
    ui.invoke_focus_search();
    ui.set_query("Before".into());
    menus.dispatch(ui);
    check(
        shortcut("a", Flags::Command, 0)?,
        "native Select All missing",
    )?;
    key(ui, "After".into())?;
    check(
        ui.get_query() == "After",
        "native Select All did not reach the focused editor",
    )?;
    check(shortcut("z", Flags::Command, 6)?, "native Undo missing")?;
    // The qualified editor records replacement removal and insertion separately.
    check(
        ui.get_query().is_empty(),
        "native Undo did not remove the inserted text",
    )?;
    check(
        shortcut("z", Flags::Command, 6)?,
        "second native Undo missing",
    )?;
    check(
        ui.get_query() == "Before",
        "native Undo did not restore the removed text",
    )?;
    check(
        shortcut("z", Flags::Command | Flags::Shift, 6)?,
        "native Redo missing",
    )?;
    check(
        ui.get_query().is_empty(),
        "native Redo did not remove the restored text",
    )?;
    check(
        shortcut("z", Flags::Command | Flags::Shift, 6)?,
        "second native Redo missing",
    )?;
    check(ui.get_query() == "After", "native Redo event was lost")?;
    ui.set_query("".into());
    ui.invoke_open_dialog(8);
    ui.invoke_focus_filters();
    key(ui, "Before".into())?;
    menus.dispatch(ui);
    check(
        shortcut("a", Flags::Command, 0)?,
        "modal editor Select All missing",
    )?;
    key(ui, "After".into())?;
    check(
        ui.get_filter_search_query() == "After" && ui.get_query().is_empty(),
        "native editing targeted the background search",
    )?;
    check(shortcut("z", Flags::Command, 6)?, "modal Undo missing")?;
    check(
        ui.get_filter_search_query().is_empty(),
        "modal Undo did not remove insertion",
    )?;
    check(
        shortcut("z", Flags::Command, 6)?,
        "second modal Undo missing",
    )?;
    check(
        ui.get_filter_search_query() == "Before",
        "modal Undo did not restore removal",
    )?;
    for _ in 0..2 {
        check(
            shortcut("z", Flags::Command | Flags::Shift, 6)?,
            "modal Redo missing",
        )?;
    }
    check(
        ui.get_filter_search_query() == "After",
        "modal Redo lost the focused editor",
    )?;
    key(ui, slint::platform::Key::Escape.into())?;
    ui.set_filter_search_query("".into());
    check(
        menus.owns_native_menu(),
        "native ownership changed after edit actions",
    )?;
    Ok(())
}
pub fn request_quit(ui: &DesktopWindow, menus: &Menus) -> Result {
    menus.dispatch(ui);
    check(
        shortcut("q", Flags::Command, 12)?,
        "native Quit accelerator missing",
    )?;
    menus.dispatch(ui);
    Ok(())
}
/// Native edit actions must target the focused tag field, never the search.
pub fn bulk_focus(ui: &DesktopWindow, menus: &Menus) -> Result {
    ui.invoke_focus_clear_bulk();
    check(ui.get_clear_bulk_focused(), "Clear selection did not focus")?;
    check(
        shortcut(",", Flags::Command, 43)?,
        "bulk Settings shortcut missing",
    )?;
    menus.dispatch(ui);
    key(ui, slint::platform::Key::Escape.into())?;
    check(
        ui.get_clear_bulk_focused(),
        "Settings lost the Clear selection invoker",
    )?;
    about(ui, menus)?;
    ui.invoke_close_dialog();
    check(
        ui.get_clear_bulk_focused(),
        "About lost the Clear selection invoker",
    )?;
    Ok(())
}
pub fn flag_focus(ui: &DesktopWindow, menus: &Menus) -> Result {
    check(ui.get_panel_flag_focused(), "panel flag was not focused")?;
    check(
        shortcut(",", Flags::Command, 43)?,
        "flag Settings shortcut missing",
    )?;
    menus.dispatch(ui);
    key(ui, slint::platform::Key::Escape.into())?;
    check(
        ui.get_panel_flag_focused(),
        "Settings lost the flag invoker",
    )?;
    about(ui, menus)?;
    ui.invoke_close_dialog();
    check(ui.get_panel_flag_focused(), "About lost the flag invoker")?;
    Ok(())
}
pub fn bulk_flag_undo_focus(ui: &DesktopWindow, menus: &Menus) -> Result {
    check(ui.get_bulk_flag_undo_focused(), "bulk Undo was not focused")?;
    check(
        shortcut(",", Flags::Command, 43)?,
        "bulk Undo Settings shortcut missing",
    )?;
    menus.dispatch(ui);
    key(ui, slint::platform::Key::Escape.into())?;
    check(
        ui.get_bulk_flag_undo_focused(),
        "Settings lost the bulk Undo invoker",
    )?;
    about(ui, menus)?;
    ui.invoke_close_dialog();
    check(
        ui.get_bulk_flag_undo_focused(),
        "About lost the bulk Undo invoker",
    )?;
    Ok(())
}
/// Native edit actions must target the focused tag field, never the search.
pub fn tag_editor(ui: &DesktopWindow, menus: &Menus) -> Result {
    let original = ui.get_tag_label();
    let query = ui.get_query();
    ui.invoke_focus_tag_label();
    menus.dispatch(ui);
    check(shortcut("a", Flags::Command, 0)?, "tag Select All missing")?;
    key(ui, "Edited tag label".into())?;
    check(
        ui.get_tag_label() == "Edited tag label" && ui.get_query() == query,
        "native tag editing targeted the background query",
    )?;
    check(shortcut("z", Flags::Command, 6)?, "tag Undo missing")?;
    check(
        ui.get_tag_label().is_empty(),
        "tag Undo did not remove insertion",
    )?;
    check(shortcut("z", Flags::Command, 6)?, "second tag Undo missing")?;
    check(
        ui.get_tag_label() == original,
        "tag Undo did not restore the label",
    )?;
    for _ in 0..2 {
        check(
            shortcut("z", Flags::Command | Flags::Shift, 6)?,
            "tag Redo missing",
        )?;
    }
    check(
        ui.get_tag_label() == "Edited tag label" && ui.get_query() == query,
        "tag Redo lost the focused editor",
    )?;
    ui.set_tag_label(original);
    Ok(())
}
pub fn about(ui: &DesktopWindow, menus: &Menus) -> Result {
    menus.dispatch(ui);
    let app = main_menu()?
        .itemAtIndex(0)
        .and_then(|item| item.submenu())
        .ok_or("application menu missing")?;
    app.performActionForItemAtIndex(0);
    menus.dispatch(ui);
    check(ui.get_dialog() == 1, "native About Slint event was lost")
}

/// The actual native menus return focus to an inspector control, including when
/// the previous browser/action key differs from the current invoker.
pub fn inspector_focus(ui: &DesktopWindow, menus: &Menus) -> Result {
    check(
        shortcut(",", Flags::Command, 43)?,
        "native Settings shortcut missing",
    )?;
    menus.dispatch(ui);
    check(
        ui.get_dialog() == 2,
        "native Settings did not open from inspector",
    )?;
    key(ui, slint::platform::Key::Escape.into())?;
    check(
        ui.get_inspector_technical_focused(),
        "native Settings lost the inspector invoker",
    )?;
    about(ui, menus)?;
    ui.invoke_close_dialog();
    check(
        ui.get_inspector_technical_focused(),
        "native About lost the inspector invoker",
    )?;
    Ok(())
}

pub fn collection_editor(ui: &DesktopWindow, menus: &Menus) -> Result {
    let original = ui.get_collection_name();
    let query = ui.get_query();
    ui.invoke_focus_collection_name();
    menus.dispatch(ui);
    check(
        shortcut("a", Flags::Command, 0)?,
        "Collection Select All missing",
    )?;
    key(ui, "Edited Collection label".into())?;
    check(
        ui.get_collection_name() == "Edited Collection label" && ui.get_query() == query,
        "native Collection editing targeted the background query",
    )?;
    check(shortcut("z", Flags::Command, 6)?, "Collection Undo missing")?;
    check(
        ui.get_collection_name().is_empty(),
        "Collection Undo did not remove insertion",
    )?;
    check(
        shortcut("z", Flags::Command, 6)?,
        "second Collection Undo missing",
    )?;
    check(
        ui.get_collection_name() == original,
        "Collection Undo did not restore the label",
    )?;
    for _ in 0..2 {
        check(
            shortcut("z", Flags::Command | Flags::Shift, 6)?,
            "Collection Redo missing",
        )?;
    }
    check(
        ui.get_collection_name() == "Edited Collection label" && ui.get_query() == query,
        "Collection Redo lost the focused editor",
    )?;
    ui.set_collection_name(original);
    Ok(())
}

pub fn preview_focus(ui: &DesktopWindow, menus: &Menus) -> Result {
    let invoker = ui.get_focused_key();
    check(invoker < 0, "preview was not focused")?;
    check(
        shortcut(",", Flags::Command, 43)?,
        "preview Settings missing",
    )?;
    menus.dispatch(ui);
    key(ui, slint::platform::Key::Escape.into())?;
    check(
        ui.get_focused_key() == invoker,
        "Settings lost preview invoker",
    )?;
    about(ui, menus)?;
    ui.invoke_close_dialog();
    check(
        ui.get_focused_key() == invoker,
        "About lost preview invoker",
    )
}
