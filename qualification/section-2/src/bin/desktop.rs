use librairii_qualification::{REVIEW_COUNT, REVIEW_PAGE, Review, matching};
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[path = "desktop/native_menus.rs"]
mod native_menus;
#[path = "desktop/native_probe.rs"]
mod native_probe;
#[path = "desktop/renderer_probe.rs"]
mod renderer_probe;
#[path = "desktop/thumbnails.rs"]
mod thumbnails;

slint::include_modules!();

fn show_rows(ui: &Spike, query: &str, loader: &Rc<RefCell<thumbnails::Loader>>) {
    let rows: Vec<_> = matching(query)
        .into_iter()
        .map(|story| StoryRow {
            id: story.id as i32,
            title: story.title.into(),
            cover: slint::Image::default(),
        })
        .collect();
    let grid: Vec<ModelRc<StoryRow>> = rows
        .chunks(3)
        .map(|row| thumbnails::Loader::rows(loader, row.to_vec()))
        .collect();
    ui.set_stories(thumbnails::Loader::rows(loader, rows));
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

fn install_pickers(
    ui: &Spike,
    root: &std::path::Path,
    menus: Rc<native_menus::Menus>,
    recorder: Option<Rc<native_probe::Recorder>>,
) {
    let weak = ui.as_weak();
    let files_root = root.to_path_buf();
    let files_menus = menus.clone();
    let files_recorder = recorder.clone();
    ui.on_pick_files(move || {
        if let Some(ui) = weak.upgrade() {
            let title = if ui.get_french() {
                "Choisir des archives synthétiques"
            } else {
                "Choose synthetic archives"
            };
            let result = files_menus.with_native_dialog(&ui, || {
                rfd::FileDialog::new()
                    .set_title(title)
                    .set_directory(&files_root)
                    .set_parent(&ui.window().window_handle())
                    .add_filter("Archives", &["pk", "zip", "7z"])
                    .pick_files()
            })
            // RFD 0.17.2 maps a cancelled Linux portal response to Some([]).
            .filter(|paths| !paths.is_empty());
            let valid = result.as_ref().is_some_and(|paths| {
                !paths.is_empty()
                    && paths
                        .iter()
                        .all(|p| p.canonicalize().is_ok_and(|p| p.starts_with(&files_root)))
            });
            if let Some(recorder) = &files_recorder {
                recorder.selection(result.as_deref(), valid);
            }
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
    let folder_root = root.to_path_buf();
    ui.on_pick_folder(move || {
        if let Some(ui) = weak.upgrade() {
            let result = menus.with_native_dialog(&ui, || {
                rfd::FileDialog::new()
                    .set_title(if ui.get_french() {
                        "Choisir le dossier synthétique"
                    } else {
                        "Choose the synthetic folder"
                    })
                    .set_directory(&folder_root)
                    .set_parent(&ui.window().window_handle())
                    .pick_folder()
            });
            if let Some(recorder) = &recorder {
                let valid = result
                    .as_ref()
                    .is_some_and(|p| p.canonicalize().is_ok_and(|p| p == folder_root));
                recorder.selection(result.as_ref().map(std::slice::from_ref), valid);
            }
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let native_recorder = native_probe::Recorder::from_args()?;
    if std::env::args().any(|arg| arg == "--require-opengl") {
        // The generic Skia graph compiles GL, but its skia-opengl renderer-name
        // alias is feature-gated. Require the API instead of allowing fallback.
        slint::BackendSelector::new()
            .backend_name("winit".into())
            .renderer_name("skia".into())
            .require_opengl()
            .select()?;
    }
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
    let fixture_root = root.clone();
    let reveal_recorder = native_recorder.clone();
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
            if let Some(recorder) = &reveal_recorder {
                recorder.write(
                    "outcome.txt",
                    if result.is_ok() {
                        "requested"
                    } else {
                        "failed"
                    },
                );
            }
        }
    });
    let thumbnails = Rc::new(RefCell::new(thumbnails::Loader::new()));
    show_rows(&ui, "", &thumbnails);
    let weak = ui.as_weak();
    let search_thumbnails = thumbnails.clone();
    ui.on_search(move |query| {
        if let Some(ui) = weak.upgrade() {
            show_rows(&ui, &query, &search_thumbnails);
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
    let menus = Rc::new(native_menus::Menus::install(ui.as_weak())?);
    install_pickers(&ui, &fixture_root, menus.clone(), native_recorder.clone());
    if std::env::args().any(|arg| arg == "--settings-early") {
        menus.commands.settings();
    }
    let requests = menus.commands.clone();
    ui.on_request_quit(move || requests.quit());
    let edit_menus = menus.clone();
    let quit_smoke = std::env::args().any(|arg| arg == "--quit-smoke");
    let quit_during_work = Rc::new(Cell::new(false));
    let observed_quit = quit_during_work.clone();
    let thumbnail_smoke = std::env::args().any(|arg| arg == "--thumbnail-smoke");
    if quit_smoke || thumbnail_smoke {
        ui.invoke_start_work();
    }
    let thumbnail_poll = thumbnails.clone();
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let work = worker.clone();
    let poll_cancel = cancel.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(16),
        move || {
            if let Some(ui) = weak.upgrade() {
                thumbnails::Loader::poll(&thumbnail_poll);
                update_viewport(&ui);
                menus.update(
                    ui.get_french(),
                    ui.get_modal() || ui.get_native_dialog_open(),
                    ui.get_search_has_focus(),
                );
                let completed = progress.load(Ordering::Acquire);
                if quit_smoke && completed > 0 && completed < 100 {
                    observed_quit.set(true);
                    menus.commands.quit();
                }
                if menus.commands.take_quit() {
                    let _ = slint::quit_event_loop();
                }
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
    let measurement_timer = Rc::new(slint::Timer::default());
    let renderer_probe = std::env::args()
        .find_map(|arg| {
            arg.strip_prefix("--renderer-smoke=")
                .map(std::path::PathBuf::from)
        })
        .map(|directory| renderer_probe::Probe::start(&ui, directory));
    let native_probe = native_recorder
        .as_ref()
        .map(|recorder| native_probe::Probe::start(&ui, &fixture_root, recorder.clone()));
    let resource_probe = std::env::args().any(|arg| arg == "--resource-probe");
    let resource_ready_timer = slint::Timer::default();
    let resource_start_timer = slint::Timer::default();
    if thumbnail_smoke {
        start_thumbnail_probe(&ui, &thumbnails, &measurement_timer);
    }
    if resource_probe {
        let weak = ui.as_weak();
        // First owned event-loop timer after show; not a first-presentation signal.
        resource_ready_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_millis(10),
            move || {
                let ui = weak.upgrade().expect("resource probe window");
                let size = ui.window().size();
                eprintln!(
                    "resource_probe_ready; pid={}; scale={}; size={}x{}",
                    std::process::id(),
                    ui.window().scale_factor(),
                    size.width,
                    size.height
                );
            },
        );
        let weak = ui.as_weak();
        let loader = thumbnails.clone();
        let delayed_measurement = measurement_timer.clone();
        // Keep the normal UI polling timer, but do not inject input or start the
        // thumbnail dispatch timer during the ten-second quiet phase.
        resource_start_timer.start(
            slint::TimerMode::SingleShot,
            Duration::from_secs(10),
            move || {
                eprintln!("resource_probe_idle_end");
                let ui = weak.upgrade().expect("resource workload window");
                start_thumbnail_probe(&ui, &loader, &delayed_measurement);
            },
        );
    }
    if std::env::args().any(|arg| arg == "--edit-smoke") {
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(20), move || {
            let ui = weak.upgrade().expect("editing test window");
            ui.window()
                .dispatch_event_with_result(slint::platform::WindowEvent::WindowActiveChanged(true))
                .expect("activate editing test component");
            ui.invoke_focus_search();
            press_key(ui.window(), "0042".into()).expect("enter query through text input");
            assert_eq!(ui.get_query(), "0042");
            assert_eq!(ui.get_stories().row_count(), 1);
            ui.invoke_edit_search(EditAction::SelectAll);
            press_key(ui.window(), "0057".into()).expect("replace actual selection");
            assert_eq!(ui.get_query(), "0057");
            assert_eq!(ui.get_stories().row_data(0).unwrap().id, 57);
            ui.invoke_edit_search(EditAction::Undo);
            assert_ne!(ui.get_query(), "0057", "widget undo must change the query");
            ui.invoke_edit_search(EditAction::Redo);
            assert_eq!(ui.get_query(), "0057");
            edit_menus.with_native_dialog(&ui, || {
                assert!(ui.get_native_dialog_open());
                ui.invoke_edit_search(EditAction::Undo);
                assert_eq!(ui.get_query(), "0057");
            });
            assert!(!ui.get_native_dialog_open());
            // Menu editing must never mutate a background or unfocused input.
            ui.invoke_open_dialog(1, 2);
            ui.invoke_edit_search(EditAction::Undo);
            assert_eq!(ui.get_query(), "0057");
            ui.invoke_close_dialog();
            ui.invoke_open_dialog(1, 0);
            ui.invoke_close_dialog();
            assert!(ui.get_settings_has_focus());
            ui.invoke_edit_search(EditAction::Undo);
            assert_eq!(ui.get_query(), "0057");
            eprintln!(
                "edit_smoke=passed; widget selection, model update, undo/redo and focus/modal guards; native_clipboard=not_measured"
            );
            slint::quit_event_loop().expect("quit editing test loop");
        });
        return Ok(slint::run_event_loop()?);
    }
    if std::env::args().any(|arg| arg == "--focus-smoke") {
        ui.show()?;
        ui.set_review_visible(true);
        let weak = ui.as_weak();
        slint::Timer::single_shot(Duration::from_millis(20), move || {
            let ui = weak.upgrade().expect("test window");
            ui.window()
                .dispatch_event_with_result(slint::platform::WindowEvent::WindowActiveChanged(true))
                .expect("activate");
            ui.set_query("Été 日本語 · unsaved".into());
            ui.invoke_open_dialog(1, 0);
            assert!(ui.get_close_has_focus());
            for _ in 0..4 {
                press_key(ui.window(), slint::platform::Key::Tab.into()).expect("Tab");
                assert!(!ui.get_settings_has_focus());
                assert!(!ui.get_search_has_focus());
            }
            press_key(ui.window(), slint::platform::Key::Escape.into()).expect("Escape");
            assert_eq!(ui.get_dialog(), 0);
            assert!(ui.get_settings_has_focus());
            assert_eq!(ui.get_query(), "Été 日本語 · unsaved");
            ui.invoke_focus_first_decision();
            let weak = ui.as_weak();
            slint::Timer::single_shot(Duration::from_millis(20), move || {
                let ui = weak.upgrade().expect("test window");
                ui.window()
                    .dispatch_event_with_result(slint::platform::WindowEvent::WindowActiveChanged(
                        true,
                    ))
                    .expect("activate test component");
                assert!(ui.get_first_decision_has_focus());
                ui.invoke_open_dialog(1, 2); // The native-menu/accelerator route.
                press_key(ui.window(), slint::platform::Key::Escape.into()).expect("Escape");
                let weak = ui.as_weak();
                slint::Timer::single_shot(Duration::from_millis(20), move || {
                    let ui = weak.upgrade().expect("test window");
                    ui.window()
                        .dispatch_event_with_result(
                            slint::platform::WindowEvent::WindowActiveChanged(true),
                        )
                        .expect("activate test component");
                    assert!(
                        ui.get_first_decision_has_focus(),
                        "menu dialog must restore the actual prior control"
                    );
                    eprintln!(
                        "focus_smoke=passed; containment, Escape, button and native-menu focus restoration"
                    );
                    slint::quit_event_loop().expect("quit test loop");
                });
            });
        });
        return Ok(slint::run_event_loop()?);
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
    timer.stop();
    measurement_timer.stop();
    resource_ready_timer.stop();
    resource_start_timer.stop();
    thumbnails.borrow_mut().close()?;
    let images = thumbnails.borrow();
    eprintln!(
        "thumbnail_probe=png; distinct_decoded={}; peak_cache={}; cache_cap={}; decode_p95_us={}",
        images.decoded.len(),
        images.peak_cache,
        thumbnails::CACHE,
        images.decode_p95_us()
    );
    if let Some(error) = &images.failure {
        return Err(error.clone().into());
    }
    if resource_probe {
        assert_eq!(images.decoded.len(), 1_000, "resource workload must finish");
    }
    drop(images);
    let retired = fixture.close();
    result?;
    retired?;
    if let Some(probe) = renderer_probe {
        probe.verify();
    }
    if let Some(probe) = native_probe {
        probe.verify();
        assert!(worker.borrow().is_none(), "native workers must be joined");
        assert!(!fixture_root.exists(), "native fixture must be retired");
        native_recorder.as_ref().expect("native recorder").retired();
    }
    if resource_probe {
        assert!(worker.borrow().is_none(), "resource worker must be joined");
        assert!(!fixture_root.exists(), "resource fixture must be retired");
        eprintln!("resource_probe_complete; workers_joined=true; fixture_retired=true");
    }
    if quit_smoke {
        assert!(quit_during_work.get());
        assert!(worker.borrow().is_none(), "all workers must be joined");
        assert!(
            !fixture_root.exists(),
            "owned picker fixture must be retired"
        );
        eprintln!("quit_smoke=passed; active worker cancelled/joined and fixture retired");
    }
    Ok(())
}

fn start_thumbnail_probe(
    ui: &Spike,
    thumbnails: &Rc<RefCell<thumbnails::Loader>>,
    measurement_timer: &slint::Timer,
) {
    ui.set_mode(1); // Exercise Image delegates while PNG results arrive.
    let loader = thumbnails.clone();
    let weak = ui.as_weak();
    let started = Instant::now();
    let mut next = 0;
    let mut first_24_ms = None;
    let mut dispatch = std::collections::VecDeque::new();
    let mut completed = false;
    measurement_timer.start(slint::TimerMode::Repeated, Duration::from_millis(10), move || {
        if completed { return; }
        assert!(started.elapsed() < Duration::from_secs(20), "thumbnail workload timed out");
        let ui = weak.upgrade().expect("measurement window");
        if !ui.get_working() { ui.invoke_start_work(); }
        let input = Instant::now();
        press_key(ui.window(), slint::platform::Key::Escape.into()).expect("public component input");
        if dispatch.len() == 512 { dispatch.pop_front(); }
        dispatch.push_back(input.elapsed().as_micros() as u64);
        if next >= 500 { ui.set_mode(2); }
        let mut state = loader.borrow_mut();
        assert!(state.failure.is_none(), "PNG decoder failed: {:?}", state.failure);
        while next < 1_000 && state.request(next) { next += 1; }
        if first_24_ms.is_none() && (0..24).all(|id| state.decoded.contains(&id)) {
            first_24_ms = Some(started.elapsed().as_millis());
        }
        if state.decoded.len() == 1_000 {
            completed = true;
            assert_eq!(state.peak_cache, thumbnails::CACHE);
            let mut samples: Vec<_> = dispatch.iter().copied().collect();
            samples.sort_unstable();
            let p95 = samples[(samples.len() * 95).div_ceil(100) - 1];
            eprintln!("thumbnail_smoke=passed; workload_ms={}; first_24_ms={}; component_dispatch_samples={}; component_dispatch_p95_us={}; physical_input=not_measured",
                started.elapsed().as_millis(), first_24_ms.unwrap(), samples.len(), p95);
            slint::quit_event_loop().expect("quit measurement loop");
        }
    });
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
    let mut command = {
        // xdg-open may remain attached to a newly started file manager. Hand
        // that external GUI to the OS session; wait only for its short launcher.
        let mut launcher = std::process::Command::new("setsid");
        launcher.args(["--fork", "xdg-open"]);
        launcher
    };
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    let status = command.arg(root).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("file manager rejected request"))
    }
}
