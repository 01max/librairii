use librairii_qualification::{Review, matching};
use slint::{ComponentHandle, ModelRc, SharedPixelBuffer, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

slint::include_modules!();

fn show_rows(ui: &Spike, query: &str) {
    let mut pixels = SharedPixelBuffer::<slint::Rgb8Pixel>::new(64, 80);
    for (i, pixel) in pixels.make_mut_slice().iter_mut().enumerate() {
        *pixel = slint::Rgb8Pixel::new(45 + (i % 40) as u8, 108, 112);
    }
    let cover = slint::Image::from_rgb8(pixels);
    let rows: Vec<_> = matching(query)
        .into_iter()
        .map(|story| StoryRow {
            id: story.id as i32,
            title: story.title.into(),
            cover: cover.clone(),
        })
        .collect();
    let grid: Vec<ModelRc<StoryRow>> = rows
        .chunks(3)
        .map(|row| ModelRc::new(VecModel::from(row.to_vec())))
        .collect();
    ui.set_stories(ModelRc::new(VecModel::from(rows)));
    ui.set_grid(ModelRc::new(VecModel::from(grid)));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    // No profile, library, network client or filesystem input exists in this spike.
    let ui = Spike::new()?;
    let fixture = tempfile::Builder::new()
        .prefix("librairii-qualification-")
        .tempdir()?;
    std::fs::write(
        fixture.path().join("Été 日本語 故事.zip"),
        b"synthetic picker fixture",
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
                    None => "Cancelled",
                    Some(_) if valid => "Synthetic files selected",
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
                .set_directory(&folder_root)
                .set_parent(&ui.window().window_handle())
                .pick_folder();
            ui.set_status(
                match result {
                    None => "Cancelled",
                    Some(p) if p.canonicalize().is_ok_and(|p| p == folder_root) => {
                        "Fixture folder selected"
                    }
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
                if result.is_ok() {
                    "Reveal requested; verify file manager"
                } else {
                    "Reveal failed"
                }
                .into(),
            );
        }
    });
    if std::env::args().any(|arg| arg == "--settings-early") {
        ui.set_settings_visible(true);
    }
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
        if let Ok(index) = usize::try_from(index)
            && let Some(decision) = decisions.borrow_mut().add.get_mut(index)
        {
            *decision = add;
        }
        if let Some(ui) = weak.upgrade() {
            ui.set_approvals(decisions.borrow().add.iter().filter(|&&a| a).count() as i32);
        }
    });
    let weak = ui.as_weak();
    ui.on_apply_review(move || {
        let applied = review.borrow_mut().apply();
        if let Some(ui) = weak.upgrade() {
            ui.set_approvals(0);
            ui.set_status(format!("{applied} synthetic decisions").into());
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

    // One snapshot slot, polled by an owned UI timer; no unbounded progress queue.
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let work = worker.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(30),
        move || {
            if let Some(ui) = weak.upgrade() {
                ui.set_progress(progress.load(Ordering::Acquire) as f32 / 100.0);
                ui.set_working(work.borrow().as_ref().is_some_and(|w| !w.is_finished()));
            }
        },
    );
    let exit_timer = slint::Timer::default();
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
