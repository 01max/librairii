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
