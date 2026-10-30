//! The native event loop and every window/menu stay on the process main thread.
#![forbid(unsafe_code)]
use librairii_desktop::{menus, presentation::Worker};
use slint::ComponentHandle;
use std::{
    cell::RefCell,
    process::ExitCode,
    rc::Rc,
    time::{Duration, Instant},
};
mod view;

fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let launched = Instant::now();
    let worker = Rc::new(RefCell::new(Worker::start()?));
    eprintln!(
        "native_startup; stage=controller_started; elapsed_ms={:.3}",
        launched.elapsed().as_secs_f64() * 1000.
    );
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
    eprintln!(
        "native_startup; stage=first_snapshot; phase={:?}; elapsed_ms={:.3}",
        first.phase,
        launched.elapsed().as_secs_f64() * 1000.
    );
    let (ui, menus) = menus::create_window()?;
    eprintln!(
        "native_startup; stage=window_constructed; elapsed_ms={:.3}",
        launched.elapsed().as_secs_f64() * 1000.
    );
    ui.set_french(first.language == librairii_app::localization::Language::French);
    let view = view::install(&ui, worker.clone(), first)?;
    eprintln!(
        "native_startup; stage=view_installed; elapsed_ms={:.3}",
        launched.elapsed().as_secs_f64() * 1000.
    );
    let pending = worker.clone();
    let closing_ui = ui.as_weak();
    ui.on_quit(move || {
        eprintln!("native_shutdown; stage=requested");
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
    let mut first_tick = true;
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
                if first_tick {
                    let size = ui.window().size();
                    eprintln!(
                        "native_startup; stage=first_ui_timer; pid={}; phase={:?}; size={}x{}; elapsed_ms={:.3}",
                        std::process::id(),
                        view.borrow().snapshot.phase,
                        size.width,
                        size.height,
                        launched.elapsed().as_secs_f64() * 1000.
                    );
                    first_tick = false;
                }
                if pending.borrow().finished() {
                    let _quit = slint::quit_event_loop();
                }
            }
        },
    );
    let result = ui.run();
    eprintln!(
        "native_shutdown; stage=window_loop_returned; ok={}",
        result.is_ok()
    );
    timer.stop();
    worker.borrow().request_shutdown();
    let closed = worker.borrow_mut().finish();
    eprintln!(
        "native_shutdown; stage=controller_joined; ok={}",
        closed.is_ok()
    );
    result?;
    closed.map_err(Into::into)
}
fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 {
        if arguments[0] == "--version" {
            println!(
                "Librairii {}",
                librairii_desktop::release_info::VERSION.trim()
            );
            return ExitCode::SUCCESS;
        }
        if arguments[0] == "--build-info" {
            println!("{}", librairii_desktop::release_info::build_info());
            return ExitCode::SUCCESS;
        }
    }
    match menus::prepare_process().and_then(|()| launch()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
