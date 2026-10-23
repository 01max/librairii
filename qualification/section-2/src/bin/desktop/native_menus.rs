//! Qualification-only native menu adapter, using public muda APIs.
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

#[derive(Clone, Default)]
pub struct Commands(Arc<AtomicU8>);
impl Commands {
    pub fn settings(&self) {
        self.0.fetch_or(1, Ordering::Release);
    }
    #[cfg(target_os = "macos")]
    fn about(&self) {
        self.0.fetch_or(2, Ordering::Release);
    }
    pub fn quit(&self) {
        self.0.fetch_or(4, Ordering::Release);
    }
    pub fn take_quit(&self) -> bool {
        self.0.fetch_and(!4, Ordering::AcqRel) & 4 != 0
    }
    pub fn next(&self) -> Option<i32> {
        if self.0.fetch_and(!1, Ordering::AcqRel) & 1 != 0 {
            Some(1)
        } else if self.0.fetch_and(!2, Ordering::AcqRel) & 2 != 0 {
            Some(2)
        } else {
            None
        }
    }
}

pub struct Menus {
    pub commands: Commands,
    #[cfg(target_os = "macos")]
    _bar: muda::Menu,
    #[cfg(target_os = "macos")]
    native_dialog_bar: muda::Menu,
    #[cfg(target_os = "macos")]
    settings: muda::MenuItem,
    #[cfg(target_os = "macos")]
    about: muda::MenuItem,
    #[cfg(target_os = "macos")]
    french: std::cell::Cell<Option<bool>>,
    #[cfg(target_os = "macos")]
    modal: std::cell::Cell<Option<bool>>,
    #[cfg(target_os = "macos")]
    quit: muda::MenuItem,
    #[cfg(target_os = "macos")]
    edits: Vec<muda::MenuItem>,
    #[cfg(target_os = "macos")]
    edit_enabled: std::cell::Cell<Option<bool>>,
}
impl Menus {
    pub fn install(_target: slint::Weak<super::Spike>) -> Result<Self, Box<dyn std::error::Error>> {
        let commands = Commands::default();
        #[cfg(target_os = "macos")]
        {
            use muda::accelerator::{Accelerator, Code, Modifiers};
            use muda::{Menu, MenuItem, PredefinedMenuItem as Standard, Submenu};
            let bar = Menu::new();
            let app = Submenu::new("Librairii", true);
            let about = MenuItem::with_id("qualification.about", "About…", true, None);
            let settings = MenuItem::with_id(
                "qualification.settings",
                "Settings…",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
            );
            let quit = MenuItem::with_id(
                "qualification.quit",
                "Quit Librairii Qualification",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
            );
            app.append_items(&[
                &about,
                &Standard::separator(),
                &settings,
                &Standard::separator(),
                &Standard::services(None),
                &Standard::separator(),
                &Standard::hide(None),
                &Standard::hide_others(None),
                &Standard::show_all(None),
                &Standard::separator(),
                &quit,
            ])?;
            let edit = Submenu::new("Edit", true);
            // Predefined selectors (paste:, copy:, etc.) have no Winit responder.
            // Route both clicks and native accelerators to public widget actions.
            let edits = [
                ("undo", "Undo", Code::KeyZ, Modifiers::META),
                (
                    "redo",
                    "Redo",
                    Code::KeyZ,
                    Modifiers::META | Modifiers::SHIFT,
                ),
                ("cut", "Cut", Code::KeyX, Modifiers::META),
                ("copy", "Copy", Code::KeyC, Modifiers::META),
                ("paste", "Paste", Code::KeyV, Modifiers::META),
                ("select-all", "Select All", Code::KeyA, Modifiers::META),
            ]
            .into_iter()
            .map(|(id, label, code, modifiers)| {
                MenuItem::with_id(
                    format!("qualification.edit.{id}"),
                    label,
                    true,
                    Some(Accelerator::new(Some(modifiers), code)),
                )
            })
            .collect::<Vec<_>>();
            for (index, item) in edits.iter().enumerate() {
                if index == 2 {
                    edit.append(&Standard::separator())?;
                }
                edit.append(item)?;
            }
            bar.append_items(&[&app, &edit])?;
            // Native panels have AppKit text responders and need standard roles.
            let native_dialog_bar = Menu::new();
            let native_edit = Submenu::new("Edit", true);
            native_edit.append_items(&[
                &Standard::undo(None),
                &Standard::redo(None),
                &Standard::separator(),
                &Standard::cut(None),
                &Standard::copy(None),
                &Standard::paste(None),
                &Standard::select_all(None),
            ])?;
            native_dialog_bar.append_items(&[&app, &native_edit])?;
            let pending = commands.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                if event.id == "qualification.settings" {
                    pending.settings();
                } else if event.id == "qualification.about" {
                    pending.about();
                } else if event.id == "qualification.quit" {
                    pending.quit();
                } else {
                    use super::EditAction;
                    let action = match event.id.as_ref() {
                        "qualification.edit.undo" => EditAction::Undo,
                        "qualification.edit.redo" => EditAction::Redo,
                        "qualification.edit.cut" => EditAction::Cut,
                        "qualification.edit.copy" => EditAction::Copy,
                        "qualification.edit.paste" => EditAction::Paste,
                        "qualification.edit.select-all" => EditAction::SelectAll,
                        _ => return,
                    };
                    let _ = _target.upgrade_in_event_loop(move |ui| ui.invoke_edit_search(action));
                }
            }));
            bar.init_for_nsapp();
            Ok(Self {
                commands,
                _bar: bar,
                native_dialog_bar,
                settings,
                about,
                french: std::cell::Cell::new(None),
                modal: std::cell::Cell::new(None),
                quit,
                edits,
                edit_enabled: std::cell::Cell::new(None),
            })
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Self { commands })
    }
    pub fn with_native_dialog<T>(&self, ui: &super::Spike, run: impl FnOnce() -> T) -> T {
        struct Restore<'a> {
            ui: &'a super::Spike,
            #[cfg(target_os = "macos")]
            bar: &'a muda::Menu,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.ui.set_native_dialog_open(false);
                #[cfg(target_os = "macos")]
                self.bar.init_for_nsapp();
            }
        }
        ui.set_native_dialog_open(true);
        #[cfg(target_os = "macos")]
        self.native_dialog_bar.init_for_nsapp();
        let _restore = Restore {
            ui,
            #[cfg(target_os = "macos")]
            bar: &self._bar,
        };
        run()
    }
    pub fn update(&self, french: bool, modal: bool, search_focused: bool) {
        #[cfg(target_os = "macos")]
        {
            if self.french.replace(Some(french)) != Some(french) {
                self.settings.set_text(if french {
                    "Réglages…"
                } else {
                    "Settings…"
                });
                self.about
                    .set_text(if french { "À propos…" } else { "About…" });
                self.quit.set_text(if french {
                    "Quitter Librairii Qualification"
                } else {
                    "Quit Librairii Qualification"
                });
            }
            if self.modal.replace(Some(modal)) != Some(modal) {
                self.settings.set_enabled(!modal);
                self.about.set_enabled(!modal);
            }
            let edit_enabled = !modal && search_focused;
            if self.edit_enabled.replace(Some(edit_enabled)) != Some(edit_enabled) {
                for item in &self.edits {
                    item.set_enabled(edit_enabled);
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (french, modal, search_focused);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_requests_are_retained_before_ready_and_coalesced() {
        let commands = Commands::default();
        let native_handler = commands.clone();
        native_handler.settings();
        native_handler.settings();
        assert_eq!(commands.next(), Some(1));
        assert_eq!(commands.next(), None);
    }
    #[test]
    fn quit_is_independent_of_pending_dialogs_and_drained_once() {
        let commands = Commands::default();
        commands.settings();
        commands.quit();
        commands.quit();
        assert!(commands.take_quit());
        assert!(!commands.take_quit());
        assert_eq!(commands.next(), Some(1));
    }
}
