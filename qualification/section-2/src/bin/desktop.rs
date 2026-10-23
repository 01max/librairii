use librairii_qualification::{REVIEW_COUNT, REVIEW_PAGE, Review, matching};
use slint::{ComponentHandle, Model, ModelRc, SharedPixelBuffer, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[path = "desktop/native_menus.rs"]
mod native_menus;

slint::include_modules!();

fn show_rows(ui: &Spike, query: &str) {
    let rows: Vec<_> = matching(query)
        .into_iter()
        .map(|story| {
            // Original distinct bounded rasters, not image-decode qualification.
            let mut pixels = SharedPixelBuffer::<slint::Rgb8Pixel>::new(48, 64);
            for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
                *pixel = slint::Rgb8Pixel::new(
                    (45 + (story.id * 17 + i / 48) % 180) as u8,
                    (50 + story.id * 29 % 170) as u8,
                    (80 + i % 100) as u8,
                );
            }
            StoryRow {
                id: story.id as i32,
                title: story.title.into(),
                cover: slint::Image::from_rgb8(pixels),
            }
        })
        .collect();
    let grid: Vec<ModelRc<StoryRow>> = rows
        .chunks(3)
        .map(|row| ModelRc::new(VecModel::from(row.to_vec())))
        .collect();
    ui.set_stories(ModelRc::new(VecModel::from(rows)));
    ui.set_grid(ModelRc::new(VecModel::from(grid)));
}

fn update_viewport(ui: &Spike) {
    // Sample actual window geometry instead of feeding layout minimums back into
    // Slint's width constraints. No self-dependent layout binding is introduced.
    let width = ui
        .window()
        .size()
        .to_logical(ui.window().scale_factor())
        .width;
    ui.set_compact(width < 850.0);
    ui.set_wide(width >= 1180.0);
}

fn show_review(ui: &Spike, review: &Review) {
    let first = review.page * REVIEW_PAGE;
    ui.set_review_page(review.page as i32);
    ui.set_review_pages(REVIEW_COUNT.div_ceil(REVIEW_PAGE) as i32);
    ui.set_first_added(review.add[first]);
    ui.set_approvals(review.add.iter().filter(|&&a| a).count() as i32);
    ui.set_decisions(ModelRc::new(VecModel::from(
        (first + 1..first + REVIEW_PAGE)
            .map(|id| DecisionRow {
                id: id as i32,
                added: review.add[id],
            })
            .collect::<Vec<_>>(),
    )));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // No profile, library, network client or filesystem input exists in this spike.
    let ui = Spike::new()?;
    ui.set_macos_menus(cfg!(target_os = "macos"));
    if let Some(size) =
        std::env::args().find_map(|arg| arg.strip_prefix("--size=").map(str::to_owned))
    {
        let (width, height) = size.split_once('x').ok_or("use --size=WIDTHxHEIGHT")?;
        let width: u32 = width.parse()?;
        let height: u32 = height.parse()?;
        if !(560..=3840).contains(&width) || !(400..=2160).contains(&height) {
            return Err("size is outside the qualification range".into());
        }
        ui.window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
    }
    ui.set_french(std::env::args().any(|arg| arg == "--french"));
    let fixture = tempfile::Builder::new()
        .prefix("librairii-qualification-")
        .tempdir()?;
    std::fs::write(
        fixture.path().join("Été 日本語 故事.zip"),
        b"synthetic picker fixture",
    )?;
    std::fs::write(
        fixture.path().join("Deuxième archive 02.pk"),
        b"second synthetic picker fixture",
    )?;
    let root = fixture.path().canonicalize()?;
    let weak = ui.as_weak();
    let files_root = root.clone();
    ui.on_pick_files(move || {
        if let Some(ui) = weak.upgrade() {
            let title = if ui.get_french() {
                "Choisir des archives synthétiques"
            } else {
                "Choose synthetic archives"
            };
            let result = rfd::FileDialog::new()
                .set_title(title)
                .set_directory(&files_root)
                .set_parent(&ui.window().window_handle())
                .add_filter("Archives", &["pk", "zip", "7z"])
                .pick_files();
            let valid = result.as_ref().is_some_and(|paths| {
                paths
                    .iter()
                    .all(|p| p.canonicalize().is_ok_and(|p| p.starts_with(&files_root)))
            });
            ui.set_status(
                match result {
                    None if ui.get_french() => "Annulé",
                    None => "Cancelled",
                    Some(_) if valid && ui.get_french() => "Fichiers synthétiques sélectionnés",
                    Some(_) if valid => "Synthetic files selected",
                    _ if ui.get_french() => "Choix hors du dossier synthétique refusé",
                    _ => "Rejected outside fixture",
                }
                .into(),
            );
        }
    });
    let weak = ui.as_weak();
    let folder_root = root.clone();
    ui.on_pick_folder(move || {
        if let Some(ui) = weak.upgrade() {
            let result = rfd::FileDialog::new()
                .set_title(if ui.get_french() {
                    "Choisir le dossier synthétique"
                } else {
                    "Choose the synthetic folder"
                })
                .set_directory(&folder_root)
                .set_parent(&ui.window().window_handle())
                .pick_folder();
            ui.set_status(
                match result {
                    None if ui.get_french() => "Annulé",
                    None => "Cancelled",
                    Some(p) if p.canonicalize().is_ok_and(|p| p == folder_root) => {
                        if ui.get_french() {
                            "Dossier synthétique sélectionné"
                        } else {
                            "Fixture folder selected"
                        }
                    }
                    _ if ui.get_french() => "Choix hors du dossier synthétique refusé",
                    _ => "Rejected outside fixture",
                }
                .into(),
            );
        }
    });
    let weak = ui.as_weak();
    ui.on_reveal_fixture(move || {
        if let Some(ui) = weak.upgrade() {
            let valid = root.is_dir() && root.canonicalize().is_ok_and(|p| p == root);
            let result = if valid {
                reveal(&root)
            } else {
                Err(std::io::Error::other("fixture changed"))
            };
            ui.set_status(
                match (result.is_ok(), ui.get_french()) {
                    (true, true) => "Affichage demandé ; vérifiez le gestionnaire de fichiers",
                    (true, false) => "Reveal requested; verify file manager",
                    (false, true) => "Échec de l’affichage",
                    (false, false) => "Reveal failed",
                }
                .into(),
            );
        }
    });
    show_rows(&ui, "");
    let weak = ui.as_weak();
    ui.on_search(move |query| {
        if let Some(ui) = weak.upgrade() {
            show_rows(&ui, &query);
        }
    });
    let weak = ui.as_weak();
    ui.on_select_story(move |id| {
        if let Some(ui) = weak.upgrade() {
            ui.set_selected(format!("{id:04} · Étoiles — 日本語 — 故事").into());
        }
    });

    let review = Rc::new(RefCell::new(Review::default()));
    let decisions = review.clone();
    let weak = ui.as_weak();
    ui.on_decide(move |index, add| {
        if let Ok(index) = usize::try_from(index) {
            decisions.borrow_mut().decide(index, add);
        }
        if let Some(ui) = weak.upgrade() {
            show_review(&ui, &decisions.borrow());
        }
    });
    let pages = review.clone();
    let weak = ui.as_weak();
    ui.on_change_review_page(move |page| {
        if let Ok(page) = usize::try_from(page) {
            pages.borrow_mut().set_page(page);
        }
        if let Some(ui) = weak.upgrade() {
            show_review(&ui, &pages.borrow());
        }
    });
    show_review(&ui, &review.borrow());
    let weak = ui.as_weak();
    ui.on_apply_review(move || {
        let applied = review.borrow_mut().apply();
        if let Some(ui) = weak.upgrade() {
            show_review(&ui, &review.borrow());
            ui.set_status(
                if ui.get_french() {
                    format!("{applied} décisions synthétiques")
                } else {
                    format!("{applied} synthetic decisions")
                }
                .into(),
            );
        }
    });

    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(AtomicUsize::new(0));
    let worker = Rc::new(RefCell::new(None::<std::thread::JoinHandle<()>>));
    let worker_slot = worker.clone();
    let stop = cancel.clone();
    let done = progress.clone();
    let weak = ui.as_weak();
    ui.on_start_work(move || {
        if worker_slot
            .borrow()
            .as_ref()
            .is_some_and(|w| !w.is_finished())
        {
            return;
        }
        if let Some(old) = worker_slot.borrow_mut().take() {
            let _ = old.join();
        }
        stop.store(false, Ordering::Release);
        done.store(0, Ordering::Release);
        let stop = stop.clone();
        let done = done.clone();
        *worker_slot.borrow_mut() = Some(std::thread::spawn(move || {
            for step in 1..=100 {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                // Artificial workload, never an archive import or a measured budget.
                std::thread::sleep(Duration::from_millis(20));
                done.store(step, Ordering::Release);
            }
        }));
        if let Some(ui) = weak.upgrade() {
            ui.set_working(true);
        }
    });
    let stop = cancel.clone();
    ui.on_cancel_work(move || stop.store(true, Ordering::Release));
    let weak = ui.as_weak();
    ui.on_simulate_error(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_status(
                if ui.get_french() {
                    "Échec synthétique — réessayez"
                } else {
                    "Synthetic failure — retry"
                }
                .into(),
            );
        }
    });

    // One snapshot slot, polled by an owned UI timer; no unbounded progress queue.
    ui.show()?;
    // Slint's stock macOS app menu has no Settings role. Own a separate public
    // native menu adapter; on Linux the toolkit menu stays in the window.
    let menus = native_menus::Menus::install()?;
    if std::env::args().any(|arg| arg == "--settings-early") {
        menus.commands.settings();
    }
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let work = worker.clone();
    let poll_cancel = cancel.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(30),
        move || {
            if let Some(ui) = weak.upgrade() {
                update_viewport(&ui);
                menus.update(ui.get_french(), ui.get_modal());
                if !ui.get_modal()
                    && let Some(dialog) = menus.commands.next()
                {
                    ui.invoke_open_dialog(dialog, 2);
                }
                ui.set_progress(progress.load(Ordering::Acquire) as f32 / 100.0);
                let working = work.borrow().as_ref().is_some_and(|w| !w.is_finished());
                if ui.get_working() && !working {
                    ui.set_status(
                        match (poll_cancel.load(Ordering::Acquire), ui.get_french()) {
                            (true, true) => "Travail synthétique annulé",
                            (true, false) => "Synthetic work cancelled",
                            (false, true) => "Travail synthétique terminé",
                            (false, false) => "Synthetic work completed",
                        }
                        .into(),
                    );
                }
                ui.set_working(working);
            }
        },
    );
    let exit_timer = slint::Timer::default();
    if std::env::args().any(|arg| arg == "--focus-smoke") {
        ui.show()?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::WindowActiveChanged(true))?;
        ui.set_query("Été 日本語 · unsaved".into());
        ui.invoke_open_dialog(1, 0);
        assert!(ui.get_close_has_focus());
        for _ in 0..4 {
            press_key(ui.window(), slint::platform::Key::Tab.into())?;
            assert!(!ui.get_settings_has_focus());
            assert!(!ui.get_search_has_focus());
        }
        press_key(ui.window(), slint::platform::Key::Escape.into())?;
        assert_eq!(ui.get_dialog(), 0);
        assert!(
            ui.get_settings_has_focus(),
            "Escape must restore the invoker"
        );
        assert_eq!(ui.get_query(), "Été 日本語 · unsaved");
        eprintln!(
            "focus_smoke=passed; initial focus, Tab containment, Escape and invoker restoration"
        );
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--widget-smoke") {
        // Feed Slint's public in-process event API. This does not send OS input
        // or bypass the denied native Computer Use/accessibility permission.
        ui.set_review_visible(true);
        ui.show()?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::WindowActiveChanged(true))?;
        for _ in 0..2 {
            ui.invoke_focus_first_decision();
            press_space(ui.window())?;
            assert!(
                ui.get_first_decision_checked(),
                "actual widget must be checked"
            );
            assert_eq!(ui.get_approvals(), 1, "widget toggled callback must run");
            ui.invoke_focus_apply();
            press_space(ui.window())?;
            assert_eq!(ui.get_approvals(), 0, "actual Apply button must run");
            assert!(!ui.get_first_added());
            assert!(
                !ui.get_first_decision_checked(),
                "actual widget must reset after Apply"
            );
        }
        eprintln!("widget_smoke=passed; in-process Space toggle -> Apply -> unchecked twice");
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--controller-smoke") {
        ui.invoke_search("0042".into());
        assert_eq!(ui.get_stories().row_count(), 1);
        ui.invoke_select_story(42);
        assert!(ui.get_selected().starts_with("0042"));
        ui.invoke_decide(1, true);
        assert_eq!(ui.get_approvals(), 1);
        assert!(ui.get_decisions().row_data(0).is_some_and(|row| row.added));
        ui.invoke_apply_review();
        assert_eq!(ui.get_approvals(), 0);
        assert!(ui.get_decisions().row_data(0).is_some_and(|row| !row.added));
        eprintln!("controller_smoke=passed; public generated component APIs only");
        return Ok(());
    }
    if std::env::args().any(|arg| arg == "--smoke") {
        exit_timer.start(slint::TimerMode::SingleShot, Duration::from_secs(3), || {
            let _ = slint::quit_event_loop();
        });
    }
    eprintln!(
        "constructed_synthetic_model_ms={}",
        start.elapsed().as_millis()
    );
    let result = ui.run();
    cancel.store(true, Ordering::Release);
    if let Some(worker) = worker.borrow_mut().take() {
        worker.join().map_err(|_| "synthetic worker panicked")?;
    }
    result?;
    Ok(())
}

fn press_space(window: &slint::Window) -> Result<(), slint::PlatformError> {
    press_key(window, " ".into())
}

fn press_key(
    window: &slint::Window,
    text: slint::SharedString,
) -> Result<(), slint::PlatformError> {
    window.dispatch_event_with_result(slint::platform::WindowEvent::KeyPressed {
        text: text.clone(),
    })?;
    window.dispatch_event_with_result(slint::platform::WindowEvent::KeyReleased { text })?;
    Ok(())
}

fn reveal(root: &std::path::Path) -> std::io::Result<()> {
    // Separate argv; no shell interpolation and only this run's owned fixture root.
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("/usr/bin/open");
    #[cfg(target_os = "linux")]
    let mut command = std::process::Command::new("xdg-open");
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    let status = command.arg(root).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("file manager rejected request"))
    }
}
