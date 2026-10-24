//! Public-API rendered output and component input, separate from physical input/IME.
use super::{Spike, press_key};
use image::ImageEncoder;
use slint::{ComponentHandle, Model};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

pub struct Probe {
    _timer: slint::Timer,
    completed: Rc<Cell<bool>>,
}

impl Probe {
    pub fn start(ui: &Spike, directory: PathBuf) -> Self {
        let completed = Rc::new(Cell::new(false));
        let observed = completed.clone();
        let weak = ui.as_weak();
        let timer = slint::Timer::default();
        let mut before = None;
        ui.set_french(true);
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(400),
            move || {
                if observed.get() {
                    return;
                }
                let ui = weak.upgrade().expect("renderer probe window");
                if let Some(before) = before.take() {
                    assert_eq!(ui.get_query(), "0042");
                    assert_eq!(ui.get_stories().row_count(), 1);
                    assert_eq!(ui.get_stories().row_data(0).unwrap().id, 42);
                    assert!(ui.get_selected().contains("日本語 — 故事"));
                    let after = snapshot(&ui, &directory.join("filtered.png"));
                    assert_ne!(before, after, "input must change rendered scene pixels");
                    observed.set(true);
                    eprintln!(
                        "renderer_smoke=passed; snapshots=2; glyph_visual_review=required; component_input=passed; physical_input_ime=not_measured"
                    );
                    slint::quit_event_loop().expect("quit renderer probe");
                } else {
                    assert_eq!(ui.get_stories().row_count(), 1_000);
                    before = Some(snapshot(&ui, &directory.join("multilingual.png")));
                    ui.window()
                        .dispatch_event_with_result(
                            slint::platform::WindowEvent::WindowActiveChanged(true),
                        )
                        .expect("activate renderer probe component");
                    ui.invoke_focus_search();
                    press_key(ui.window(), "0042".into()).expect("renderer probe input");
                    ui.invoke_select_story(42);
                }
            },
        );
        Self {
            _timer: timer,
            completed,
        }
    }

    pub fn verify(&self) {
        assert!(
            self.completed.get(),
            "renderer probe ended before completion"
        );
    }
}

fn snapshot(ui: &Spike, path: &Path) -> Vec<u8> {
    // Slint re-renders into CPU pixels here. The separate backend performance
    // trace, not this snapshot, identifies the actual on-screen surface.
    let pixels = ui.window().take_snapshot().expect("Skia scene snapshot");
    assert!(pixels.width() >= 560 && pixels.height() >= 400);
    let bytes = pixels.as_bytes();
    let (rgba, _) = bytes.as_chunks::<4>();
    assert!(
        rgba.iter().any(|pixel| pixel != &rgba[0]),
        "renderer returned a flat frame"
    );
    image::codecs::png::PngEncoder::new(std::fs::File::create_new(path).expect("new snapshot"))
        .write_image(
            bytes,
            pixels.width(),
            pixels.height(),
            image::ExtendedColorType::Rgba8,
        )
        .expect("encode snapshot");
    bytes.to_vec()
}
