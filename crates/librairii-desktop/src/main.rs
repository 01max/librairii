//! The native event loop and every window/menu stay on the process main thread.
#![forbid(unsafe_code)]
use librairii_desktop::presentation::Worker;
use librairii_ui::native::DesktopWindow;
use slint::ComponentHandle;
use std::{cell::RefCell, process::ExitCode, rc::Rc, time::Duration};
mod menus;
mod view;

fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let worker = Rc::new(RefCell::new(Worker::start()?));
    // Rejected overrides never construct a window or open a production profile.
    let first = loop {
        if let Some(snapshot) = worker.borrow().take_snapshot() {
            break snapshot;
        }
        if worker.borrow().finished() {
            return Err(worker
                .borrow_mut()
                .finish()
                .err()
                .unwrap_or_else(|| "native controller stopped".into())
                .into());
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let ui = DesktopWindow::new()?;
    let menus = menus::Menus::install(ui.as_weak())?;
    ui.set_french(first.language == librairii_app::localization::Language::French);
    let view = view::install(&ui, worker.clone(), first);
    let pending = worker.clone();
    ui.on_quit(move || pending.borrow().request_shutdown());
    let pending = worker.clone();
    ui.window().on_close_requested(move || {
        pending.borrow().request_shutdown();
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = ui.as_weak();
    let pending = worker.clone();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            if let Some(ui) = weak.upgrade() {
                view.borrow_mut().resize(&ui);
                menus.update(ui.get_french(), false, ui.get_query_focused());
                let command = menus.commands.take();
                if command & 1 != 0 {
                    ui.set_about_open(true);
                }
                if command & 4 != 0 {
                    ui.invoke_show_panel(5);
                }
                if command & 2 != 0 {
                    pending.borrow().request_shutdown();
                }
                if let Some(snapshot) = pending.borrow().take_snapshot() {
                    view.borrow_mut().snapshot = snapshot;
                    view.borrow_mut().project(&ui);
                }
                if pending.borrow().finished() {
                    let _quit = slint::quit_event_loop();
                }
            }
        },
    );
    let result = ui.run();
    timer.stop();
    worker.borrow().request_shutdown();
    let closed = worker.borrow_mut().finish();
    result?;
    closed.map_err(Into::into)
}
fn main() -> ExitCode {
    match launch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
