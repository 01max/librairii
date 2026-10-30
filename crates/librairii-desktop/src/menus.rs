//! Main-thread native menu ownership; bounded early command delivery.
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Configure Slint's documented menu fallback before any thread or toolkit exists.
/// Replacing this process preserves PID/arguments without unsafe global env writes.
pub fn prepare_process() -> Result {
    #[cfg(target_os = "macos")]
    if std::env::var_os("SLINT_NO_MUDA").is_none() {
        use std::os::unix::process::CommandExt;
        return Err(std::process::Command::new(std::env::current_exe()?)
            .args(std::env::args_os().skip(1))
            .env("SLINT_NO_MUDA", "1")
            .exec()
            .into());
    }
    Ok(())
}

/// The production and fixture paths use exactly the same toolkit/menu composition.
pub fn create_window() -> Result<(DesktopWindow, Menus)> {
    #[cfg(target_os = "macos")]
    {
        if std::env::var_os("SLINT_NO_MUDA").is_none() {
            return Err("native menu process policy was not initialized".into());
        }
        let selected = std::env::var("SLINT_BACKEND").unwrap_or_else(|_| "winit-skia".into());
        let renderer = selected.strip_prefix("winit-").ok_or("Winit is required")?;
        if !matches!(renderer, "skia" | "skia-software") {
            return Err("the qualified Skia renderer is required".into());
        }
        slint::platform::set_platform(Box::new(
            i_slint_backend_winit::Backend::builder()
                .with_renderer_name(renderer)
                .with_default_menu_bar(false)
                .build()?,
        ))?;
    }
    let ui = DesktopWindow::new()?;
    ui.set_platform_menu(cfg!(target_os = "macos"));
    let menus = Menus::install(ui.as_weak())?;
    Ok((ui, menus))
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    french: bool,
    modal: bool,
    editing: bool,
    native_editing: bool,
    back: bool,
    forward: bool,
    ready: bool,
    library: bool,
}
#[derive(Clone, Copy, Debug)]
pub enum NativeCommand {
    About,
    Settings,
    Quit,
    Back,
    Forward,
    History,
    Location,
    Diagnostics,
}
#[derive(Clone, Debug, Default)]
pub struct MenuCommands {
    queue: std::sync::Arc<std::sync::Mutex<std::collections::VecDeque<NativeCommand>>>,
    quit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
}
impl MenuCommands {
    pub fn enqueue(&self, command: NativeCommand) {
        if matches!(command, NativeCommand::Quit) {
            self.quit.store(true, std::sync::atomic::Ordering::Release);
        } else if let Ok(mut queue) = self.queue.lock() {
            if queue.len() < 16 {
                queue.push_back(command);
            } else {
                self.overflow
                    .store(true, std::sync::atomic::Ordering::Release);
            }
        } else {
            self.overflow
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
    fn next(&self, mut ready: impl FnMut(NativeCommand) -> bool) -> Option<NativeCommand> {
        let mut queue = self.queue.lock().ok()?;
        // Preserve FIFO among commands that can run. A stale Library-only
        // command must not prevent opening Settings during recovery.
        let index = queue.iter().position(|command| ready(*command))?;
        queue.remove(index)
    }
}
pub struct Menus {
    commands: MenuCommands,
    #[cfg(target_os = "macos")]
    state: std::cell::Cell<Option<State>>,
    #[cfg(target_os = "macos")]
    _bar: muda::Menu,
    #[cfg(target_os = "macos")]
    labels: Vec<muda::MenuItem>,
    #[cfg(target_os = "macos")]
    edit: muda::Submenu,
    #[cfg(target_os = "macos")]
    native_edit: muda::Submenu,
    #[cfg(target_os = "macos")]
    native_actions: [muda::PredefinedMenuItem; 6],
    #[cfg(target_os = "macos")]
    navigation: muda::Submenu,
    #[cfg(target_os = "macos")]
    visibility: [muda::PredefinedMenuItem; 3],
}
impl std::fmt::Debug for Menus {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Menus")
            .field("commands", &self.commands)
            .finish_non_exhaustive()
    }
}
#[cfg(target_os = "macos")]
fn native_text_editing() -> bool {
    use objc2::{ClassType, runtime::NSObjectProtocol};
    let Some(thread) = objc2::MainThreadMarker::new() else {
        return false;
    };
    objc2_app_kit::NSApplication::sharedApplication(thread)
        .keyWindow()
        .and_then(|window| window.firstResponder())
        .is_some_and(|responder| responder.isKindOfClass(objc2_app_kit::NSText::class()))
}
impl Menus {
    fn install(_target: slint::Weak<DesktopWindow>) -> Result<Self> {
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
            let visibility = [
                Standard::hide(None),
                Standard::hide_others(None),
                Standard::show_all(None),
            ];
            let additions = [
                MenuItem::with_id("history", "History…", false, None),
                MenuItem::with_id("location", "Library Location…", true, None),
                MenuItem::with_id("diagnostics", "Save Diagnostic ZIP…", false, None),
            ];
            app.append_items(&[
                &about,
                &Standard::separator(),
                &settings,
                &Standard::separator(),
                &Standard::services(None),
                &visibility[0],
                &visibility[1],
                &visibility[2],
                &Standard::separator(),
                &additions[0],
                &additions[1],
                &additions[2],
                &Standard::separator(),
                &quit,
            ])?;
            let edit = Submenu::new("Edit", true);
            // AppKit's native panel field editor requires responder-chain
            // actions. Slint text fields retain their qualified explicit route.
            let native_edit = Submenu::new("Edit", true);
            let native_actions = [
                Standard::undo(None),
                Standard::redo(None),
                Standard::cut(None),
                Standard::copy(None),
                Standard::paste(None),
                Standard::select_all(None),
            ];
            for action in &native_actions {
                native_edit.append(action)?;
            }
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
                if id == "redo" {
                    // AppKit's uppercase equivalent encodes Command-Shift-Z.
                    // This also matches real shifted events with this Muda version.
                    item.set_key_accelerator(Some(muda::accelerator::KeyAccelerator::new(
                        Some(Modifiers::META),
                        muda::accelerator::Key::Character("Z".into()),
                    )))?;
                }
                edit.append(&item)?;
                labels.push(item);
            }
            let navigation = Submenu::new("Navigation", true);
            for (id, label, code) in [
                ("back", "Back", Code::ArrowLeft),
                ("forward", "Forward", Code::ArrowRight),
            ] {
                let item = MenuItem::with_id(
                    id,
                    label,
                    false,
                    Some(Accelerator::new(Some(Modifiers::ALT), code)),
                );
                navigation.append(&item)?;
                labels.push(item);
            }
            bar.append_items(&[&app, &edit, &navigation])?;
            labels.extend(additions);
            let pending = commands.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                use librairii_ui::native::EditAction;
                let command = match event.id.as_ref() {
                    "about-slint" => Some(NativeCommand::About),
                    "quit" => Some(NativeCommand::Quit),
                    "settings" => Some(NativeCommand::Settings),
                    "back" => Some(NativeCommand::Back),
                    "forward" => Some(NativeCommand::Forward),
                    "history" => Some(NativeCommand::History),
                    "location" => Some(NativeCommand::Location),
                    "diagnostics" => Some(NativeCommand::Diagnostics),
                    _ => None,
                };
                if let Some(command) = command {
                    pending.enqueue(command);
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
                native_edit,
                native_actions,
                navigation,
                visibility,
                state: Default::default(),
            })
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Self { commands })
    }
    pub fn dispatch(&self, ui: &DesktopWindow) {
        self.update(ui);
        if self
            .commands
            .quit
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            ui.invoke_request_quit();
            return;
        }
        // Keep early or temporarily blocked commands in their bounded FIFO.
        // Delivery occurs only when the exact native panel can accept focus.
        while let Some(command) = self.commands.next(|command| {
            if ui.get_closing() || ui.get_startup_phase() == 4 || ui.get_picker_open() {
                return false;
            }
            match command {
                NativeCommand::Settings | NativeCommand::Location => {
                    ui.get_settings_command_ready()
                }
                NativeCommand::Diagnostics => {
                    ui.get_settings_command_ready() && ui.get_startup_phase() == 3
                }
                NativeCommand::About => !ui.get_modal(),
                _ => !ui.get_modal() && ui.get_startup_phase() == 3,
            }
        }) {
            match command {
                NativeCommand::About => ui.invoke_open_dialog(1),
                NativeCommand::Settings => ui.invoke_show_panel(5),
                NativeCommand::Location => {
                    ui.invoke_show_panel(5);
                    ui.invoke_focus_settings_choose();
                }
                NativeCommand::Diagnostics => {
                    ui.invoke_show_panel(5);
                    ui.invoke_focus_settings_diagnostics();
                }
                NativeCommand::History => ui.invoke_show_panel(4),
                NativeCommand::Back | NativeCommand::Forward
                    if !ui.get_modal() && !ui.get_closing() =>
                {
                    let backwards = matches!(command, NativeCommand::Back);
                    if if backwards {
                        ui.get_back_enabled()
                    } else {
                        ui.get_forward_enabled()
                    } {
                        ui.invoke_history_step(if backwards { -1 } else { 1 });
                    }
                }
                _ => {}
            }
        }
        if self
            .commands
            .overflow
            .swap(false, std::sync::atomic::Ordering::AcqRel)
        {
            ui.set_status(
                if ui.get_french() {
                    "Trop de commandes. Réessayez."
                } else {
                    "Too many commands. Please try again."
                }
                .into(),
            );
        }
    }
    fn update(&self, _ui: &DesktopWindow) {
        #[cfg(target_os = "macos")]
        {
            let state = State {
                french: _ui.get_french(),
                modal: _ui.get_modal() || _ui.get_closing() || _ui.get_review_data().committing,
                editing: _ui.get_text_edit_focused(),
                native_editing: native_text_editing(),
                back: _ui.get_back_enabled(),
                forward: _ui.get_forward_enabled(),
                ready: _ui.get_settings_command_ready(),
                library: _ui.get_startup_phase() == 3,
            };
            let previous = self.state.get();
            if previous == Some(state) {
                return;
            }
            if previous.is_some_and(|s| s.native_editing) != state.native_editing {
                let (old, next) = if state.native_editing {
                    (&self.edit, &self.native_edit)
                } else {
                    (&self.native_edit, &self.edit)
                };
                if self._bar.remove(old).is_err() {
                    _ui.set_status(
                        if state.french {
                            "Le menu d’édition natif n’a pas pu être mis à jour."
                        } else {
                            "Native edit menu could not be updated."
                        }
                        .into(),
                    );
                    return;
                }
                if self._bar.insert(next, 1).is_err() {
                    let restored = self._bar.insert(old, 1);
                    _ui.set_status(
                        if state.french && restored.is_ok() {
                            "Le menu d’édition natif n’a pas pu être mis à jour."
                        } else if state.french {
                            "Le menu d’édition natif n’a pas pu être restauré. Redémarrez Librairii."
                        } else if restored.is_ok() {
                            "Native edit menu could not be updated."
                        } else {
                            "Native edit menu could not be restored. Restart Librairii."
                        }
                        .into(),
                    );
                    return;
                }
            }
            self.state.set(Some(state));
            self.edit
                .set_text(if state.french { "Édition" } else { "Edit" });
            self.native_edit
                .set_text(if state.french { "Édition" } else { "Edit" });
            self.navigation.set_text("Navigation");
            let visibility_labels = if state.french {
                ["Masquer Librairii", "Masquer les autres", "Tout afficher"]
            } else {
                ["Hide Librairii", "Hide Others", "Show All"]
            };
            for (item, label) in self.visibility.iter().zip(visibility_labels) {
                item.set_text(label);
            }
            let labels = if state.french {
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
                    "Retour",
                    "Suivant",
                    "Historique…",
                    "Emplacement de la bibliothèque…",
                    "Enregistrer le ZIP de diagnostic…",
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
                    "Back",
                    "Forward",
                    "History…",
                    "Library Location…",
                    "Save Diagnostic ZIP…",
                ]
            };
            for (item, label) in self.native_actions.iter().zip(&labels[3..9]) {
                item.set_text(*label);
            }
            for (index, (item, label)) in self.labels.iter().zip(labels).enumerate() {
                item.set_text(label);
                item.set_enabled(match index {
                    0 => !state.modal,
                    1 | 12 => state.ready,
                    2 => true,
                    9 => !state.modal && state.back,
                    10 => !state.modal && state.forward,
                    11 => !state.modal && state.library,
                    13 => state.ready && state.library,
                    _ => state.editing,
                });
            }
        }
    }
    pub fn commands(&self) -> MenuCommands {
        self.commands.clone()
    }
    #[cfg(target_os = "macos")]
    pub fn owns_native_menu(&self) -> bool {
        use muda::ContextMenu;
        let Some(thread) = objc2::MainThreadMarker::new() else {
            return false;
        };
        objc2_app_kit::NSApplication::sharedApplication(thread)
            .mainMenu()
            .is_some_and(|menu| {
                std::ptr::from_ref::<objc2_app_kit::NSMenu>(&menu)
                    .cast_mut()
                    .cast::<std::ffi::c_void>()
                    == self._bar.ns_menu()
            })
    }
}
