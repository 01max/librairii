//! The native event loop and every window/menu stay on the process main thread.
#![forbid(unsafe_code)]
use librairii_desktop::{menus, presentation::Worker};
use slint::ComponentHandle;
use std::{cell::RefCell, process::ExitCode, rc::Rc, time::Duration};
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
    let (ui, menus) = menus::create_window()?;
    ui.set_french(first.language == librairii_app::localization::Language::French);
    let view = view::install(&ui, worker.clone(), first)?;
    let pending = worker.clone();
    let closing_ui = ui.as_weak();
    ui.on_quit(move || {
        if let Some(ui) = closing_ui.upgrade() {
            ui.set_closing(true);
            ui.set_pending(true);
            ui.set_indeterminate(true);
            ui.set_status(
                if ui.get_french() {
                    "Fermeture de Librairii…"
                } else {
                    "Closing Librairii…"
                }
                .into(),
            );
        }
        pending.borrow().request_shutdown();
    });
    let weak_close = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak_close.upgrade() {
            ui.invoke_request_quit();
        }
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
                menus.dispatch(&ui);
                if let Some(snapshot) = pending.borrow().take_snapshot()
                    && !ui.get_closing()
                {
                    view.borrow_mut()
                        .accept(&ui, snapshot, pending.borrow().revision());
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
    match menus::prepare_process().and_then(|()| launch()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
