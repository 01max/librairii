//! Main-thread native menu ownership; bounded early command delivery.
use librairii_ui::native::DesktopWindow;
#[derive(Debug, Default)]
pub struct MenuCommands(std::sync::Arc<std::sync::atomic::AtomicU8>);
impl MenuCommands {
    pub fn take(&self) -> u8 {
        self.0.swap(0, std::sync::atomic::Ordering::AcqRel)
    }
}
pub struct Menus {
    pub commands: MenuCommands,
    #[cfg(target_os = "macos")]
    state: std::cell::Cell<Option<(bool, bool, bool)>>,
    #[cfg(target_os = "macos")]
    _bar: muda::Menu,
    #[cfg(target_os = "macos")]
    labels: Vec<muda::MenuItem>,
    #[cfg(target_os = "macos")]
    edit: muda::Submenu,
}
impl Menus {
    pub fn install(
        _target: slint::Weak<DesktopWindow>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let commands = MenuCommands::default();
        #[cfg(target_os = "macos")]
        {
            use muda::accelerator::{Accelerator, Code, Modifiers};
            use muda::{Menu, MenuItem, PredefinedMenuItem as Standard, Submenu};
            let bar = Menu::new();
            let app = Submenu::new("Librairii", true);
            let about = MenuItem::with_id("about-slint", "About Slint…", true, None);
            let settings = MenuItem::with_id(
                "settings",
                "Settings…",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::Comma)),
            );
            let quit = MenuItem::with_id(
                "quit",
                "Quit Librairii",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
            );
            app.append_items(&[
                &about,
                &Standard::separator(),
                &settings,
                &Standard::separator(),
                &Standard::services(None),
                &Standard::hide(None),
                &Standard::hide_others(None),
                &Standard::show_all(None),
                &Standard::separator(),
                &quit,
            ])?;
            let edit = Submenu::new("Edit", true);
            let mut labels = vec![about, settings, quit];
            for (id, label, code, modifiers) in [
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
            ] {
                let item = MenuItem::with_id(
                    id,
                    label,
                    true,
                    Some(Accelerator::new(Some(modifiers), code)),
                );
                edit.append(&item)?;
                labels.push(item);
            }
            bar.append_items(&[&app, &edit])?;
            let pending = commands.0.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                use librairii_ui::native::EditAction;
                let bit = match event.id.as_ref() {
                    "about-slint" => 1,
                    "quit" => 2,
                    "settings" => 4,
                    _ => 0,
                };
                if bit != 0 {
                    pending.fetch_or(bit, std::sync::atomic::Ordering::Release);
                } else {
                    let action = match event.id.as_ref() {
                        "undo" => EditAction::Undo,
                        "redo" => EditAction::Redo,
                        "cut" => EditAction::Cut,
                        "copy" => EditAction::Copy,
                        "paste" => EditAction::Paste,
                        "select-all" => EditAction::SelectAll,
                        _ => return,
                    };
                    if let Some(ui) = _target.upgrade() {
                        ui.invoke_edit_search(action);
                    }
                }
            }));
            bar.init_for_nsapp();
            Ok(Self {
                commands,
                _bar: bar,
                labels,
                edit,
                state: Default::default(),
            })
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Self { commands })
    }
    pub fn update(&self, _french: bool, _modal: bool, _editing: bool) {
        #[cfg(target_os = "macos")]
        {
            if self.state.replace(Some((_french, _modal, _editing)))
                == Some((_french, _modal, _editing))
            {
                return;
            }
            self.edit.set_text(if _french { "Édition" } else { "Edit" });
            let labels = if _french {
                [
                    "À propos de Slint…",
                    "Réglages…",
                    "Quitter Librairii",
                    "Annuler",
                    "Rétablir",
                    "Couper",
                    "Copier",
                    "Coller",
                    "Tout sélectionner",
                ]
            } else {
                [
                    "About Slint…",
                    "Settings…",
                    "Quit Librairii",
                    "Undo",
                    "Redo",
                    "Cut",
                    "Copy",
                    "Paste",
                    "Select All",
                ]
            };
            for (index, (item, label)) in self.labels.iter().zip(labels).enumerate() {
                item.set_text(label);
                item.set_enabled(match index {
                    0 | 1 => !_modal,
                    2 => true,
                    _ => !_modal && _editing,
                });
            }
        }
    }
}
