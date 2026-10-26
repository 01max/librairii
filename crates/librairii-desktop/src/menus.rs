//! Main-thread native menu ownership; bounded early command delivery.
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
    _bar: muda::Menu,
}
impl Menus {
    pub fn install() -> Result<Self, Box<dyn std::error::Error>> {
        let commands = MenuCommands::default();
        #[cfg(target_os = "macos")]
        {
            use muda::accelerator::{Accelerator, Code, Modifiers};
            use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
            let bar = Menu::new();
            let app = Submenu::new("Librairii", true);
            let about = MenuItem::with_id("about-slint", "About Slint…", true, None);
            let quit = MenuItem::with_id(
                "quit",
                "Quit Librairii",
                true,
                Some(Accelerator::new(Some(Modifiers::META), Code::KeyQ)),
            );
            app.append_items(&[&about, &PredefinedMenuItem::separator(), &quit])?;
            bar.append(&app)?;
            let pending = commands.0.clone();
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                let bit = match event.id.as_ref() {
                    "about-slint" => 1,
                    "quit" => 2,
                    _ => 0,
                };
                pending.fetch_or(bit, std::sync::atomic::Ordering::Release);
            }));
            bar.init_for_nsapp();
            Ok(Self {
                commands,
                _bar: bar,
            })
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Self { commands })
    }
}
