//! Test-only rendezvous for observing real native dialogs from another process.
use super::Spike;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use slint::{ComponentHandle, Model};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Recorder {
    directory: PathBuf,
    case: String,
    wait_for_start: bool,
}

impl Recorder {
    pub fn from_args() -> Result<Option<Rc<Self>>, Box<dyn std::error::Error>> {
        let Some(directory) =
            std::env::args().find_map(|arg| arg.strip_prefix("--native-probe=").map(PathBuf::from))
        else {
            return Ok(None);
        };
        let case = std::env::args()
            .find_map(|arg| arg.strip_prefix("--native-case=").map(str::to_owned))
            .ok_or("native probe requires an explicit case")?;
        if !matches!(
            case.as_str(),
            "files" | "folder" | "reveal" | "early-settings" | "input"
        ) {
            return Err("unknown native probe case".into());
        }
        if !directory.is_absolute() {
            return Err("native probe needs an absolute fresh evidence directory".into());
        }
        std::fs::create_dir(&directory)?;
        let wait_for_start = std::env::args().any(|arg| arg == "--native-wait-for-start");
        Ok(Some(Rc::new(Self {
            directory,
            case,
            wait_for_start,
        })))
    }

    pub fn write(&self, name: &str, text: &str) {
        let temporary = self.directory.join(format!(".{name}.tmp"));
        std::fs::write(&temporary, text).expect("write owned native probe evidence");
        std::fs::rename(temporary, self.directory.join(name))
            .expect("publish owned native probe evidence");
    }

    pub fn selection(&self, paths: Option<&[PathBuf]>, valid: bool) {
        let outcome = match paths {
            None => "cancelled",
            Some(_) if valid => "selected",
            Some(_) => "rejected",
        };
        self.write("outcome.txt", outcome);
        self.write(
            "selected-paths.txt",
            &paths
                .unwrap_or_default()
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    pub fn retired(&self) {
        self.write("retired.txt", "workers_joined=true; fixture_retired=true");
    }
}

pub struct Probe {
    _timer: slint::Timer,
    completed: Rc<Cell<bool>>,
}

impl Probe {
    pub fn start(ui: &Spike, fixture: &Path, recorder: Rc<Recorder>) -> Self {
        recorder.write("fixture.txt", &fixture.to_string_lossy());
        if recorder.case == "input" {
            use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
            let output = recorder.clone();
            let edits = RefCell::new(String::new());
            let weak = ui.as_weak();
            ui.on_edit_observed(move |action, allowed| {
                use std::fmt::Write;
                let ui = weak.upgrade().expect("observed native edit target");
                let mut trace = edits.borrow_mut();
                writeln!(
                    trace,
                    "{action:?}; allowed={allowed}; search_focus={}; dialog={}; native_dialog={}",
                    ui.get_search_has_focus(),
                    ui.get_dialog(),
                    ui.get_native_dialog_open()
                )
                .expect("format native edit observation");
                assert!(trace.len() <= 65_536, "native edit trace limit");
                output.write("edit-events.txt", &trace);
            });
            let events = RefCell::new(String::new());
            let output = recorder.clone();
            // Observe real OS IME delivery through Slint's versioned public
            // Winit hook. Always propagate; never synthesize composition or
            // modify the text widget to satisfy an external input assertion.
            ui.window().on_winit_window_event(move |_, event| {
                let entry = match event {
                    winit::event::WindowEvent::Ime(ime) => Some(format!("{ime:?}")),
                    winit::event::WindowEvent::Focused(value) => Some(format!("Focused({value})")),
                    winit::event::WindowEvent::KeyboardInput { event, .. } => Some(format!(
                        "KeyboardInput(state={:?}, physical={:?}, key={:?}, repeat={})",
                        event.state, event.physical_key, event.logical_key, event.repeat
                    )),
                    _ => None,
                };
                if let Some(entry) = entry {
                    use std::fmt::Write;
                    let mut trace = events.borrow_mut();
                    writeln!(trace, "{entry}").expect("format native IME event");
                    assert!(trace.len() <= 1_048_576, "native IME trace limit");
                    output.write("ime-events.txt", &trace);
                }
                EventResult::Propagate
            });
        }
        let completed = Rc::new(Cell::new(false));
        let done = completed.clone();
        let weak = ui.as_weak();
        let timer = slint::Timer::default();
        let started = Instant::now();
        let phase = Cell::new(0);
        let load_started = Cell::new(false);
        let action_queued = Cell::new(false);
        let last = RefCell::new(String::new());
        let last_geometry = RefCell::new(String::new());
        let last_capture = RefCell::new(String::new());
        timer.start(
            slint::TimerMode::Repeated,
            Duration::from_millis(20),
            move || {
                assert!(
                    started.elapsed() < Duration::from_secs(45),
                    "native observer deadline"
                );
                let ui = weak.upgrade().expect("native probe component");
                if phase.get() == 0 && started.elapsed() >= Duration::from_millis(300) {
                    // show() and an elapsed timer do not establish native-window
                    // readiness. RFD silently drops unavailable parent handles.
                    let parent = ui.window().window_handle();
                    let (Ok(window), Ok(display)) =
                        (parent.window_handle(), parent.display_handle())
                    else {
                        return;
                    };
                    recorder.write(
                        "parent-handle.txt",
                        &format!("{:?}\n{:?}", window.as_raw(), display.as_raw()),
                    );
                    phase.set(1);
                    if recorder.case != "early-settings" {
                        ui.invoke_focus_search();
                    }
                    recorder.write("ready.txt", "parent shown; native handles observed");
                }
                if phase.get() == 1
                    && !action_queued.get()
                    && (!recorder.wait_for_start
                        || recorder.directory.join("start-native-case").is_file())
                {
                    action_queued.set(true);
                    // AppKit's modal panel pumps the OS loop. Invoke from a
                    // queued event after this Slint timer has returned, as a
                    // real input callback does, rather than re-entering timers.
                    let callback = recorder.as_ref().clone();
                    ui.as_weak()
                        .upgrade_in_event_loop(move |ui| {
                            match callback.case.as_str() {
                                "files" => ui.invoke_pick_files(),
                                "folder" => ui.invoke_pick_folder(),
                                "reveal" => ui.invoke_reveal_fixture(),
                                "early-settings" | "input" => {}
                                _ => unreachable!(),
                            }
                            callback.write("callback-complete.txt", &ui.get_status());
                        })
                        .expect("queue native probe outside timer");
                }
                if phase.get() == 1 {
                    if !load_started.get() && recorder.directory.join("start-load").is_file() {
                        load_started.set(true);
                        ui.invoke_start_work();
                    }
                    let physical = ui.window().size();
                    let scale = ui.window().scale_factor();
                    let logical = physical.to_logical(scale);
                    let contrasts = [
                        contrast(ui.get_observed_foreground().color(), ui.get_observed_background().color(), ui.get_observed_background().color()),
                        contrast(ui.get_observed_control_foreground().color(), ui.get_observed_control_background().color(), ui.get_observed_background().color()),
                        contrast(ui.get_observed_selection_foreground().color(), ui.get_observed_selection_background().color(), ui.get_observed_background().color()),
                    ];
                    if let Ok(capture) = std::fs::read_to_string(recorder.directory.join("capture-request"))
                        && capture != *last_capture.borrow() {
                            assert!(!capture.is_empty() && capture.len() <= 64 && capture.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
                            let temporary = recorder.directory.join(format!(".{capture}.png.tmp"));
                            super::renderer_probe::snapshot(&ui, &temporary);
                            std::fs::rename(temporary, recorder.directory.join(format!("{capture}.png"))).expect("publish complete display frame");
                            *last_capture.borrow_mut() = capture;
                    }
                    let geometry = format!(
                        "{{\"width\":{},\"height\":{},\"logical_width\":{},\"logical_height\":{},\"scale\":{},\"compact\":{},\"wide\":{},\"focus_key\":{},\"selected_id\":{},\"selected_row\":{},\"rows\":{},\"working\":{},\"review_visible\":{},\"contrast_ratios\":{:?}}}",
                        physical.width, physical.height, logical.width, logical.height, scale,
                        ui.get_compact(), ui.get_wide(), ui.get_observed_focus_key(),
                        ui.get_selected_id(), ui.get_selected_row_index(), ui.get_stories().row_count(), ui.get_working(), ui.get_review_visible(), contrasts);
                    if *last_geometry.borrow() != geometry {
                        recorder.write("geometry.json", &geometry);
                        *last_geometry.borrow_mut() = geometry;
                    }
                    let state = format!(
                        "{}\n{}\n{}\n{}\n{}",
                        ui.get_query(),
                        ui.get_stories().row_count(),
                        ui.get_dialog(),
                        ui.get_search_has_focus(),
                        ui.get_close_has_focus()
                    );
                    if *last.borrow() != state {
                        recorder.write("state.txt", &state);
                        *last.borrow_mut() = state;
                    }
                    if recorder.directory.join("finish").is_file() {
                        phase.set(2);
                        done.set(true);
                        slint::quit_event_loop().expect("finish native probe");
                    }
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
            "native probe ended before observer completed"
        );
    }
}

fn contrast(foreground: slint::Color, background: slint::Color, window: slint::Color) -> f64 {
    fn rgb(color: slint::Color) -> [f64; 3] {
        [
            f64::from(color.red()),
            f64::from(color.green()),
            f64::from(color.blue()),
        ]
    }
    fn blend(color: slint::Color, background: [f64; 3]) -> [f64; 3] {
        let alpha = f64::from(color.alpha()) / 255.0;
        let rgb = rgb(color);
        std::array::from_fn(|i| (rgb[i] * alpha + background[i] * (1.0 - alpha)) / 255.0)
    }
    fn luminance(rgb: [f64; 3]) -> f64 {
        let linear = rgb.map(|value| {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        });
        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722
    }
    let background = blend(background, rgb(window));
    let foreground = blend(foreground, background.map(|value| value * 255.0));
    let (first, second) = (luminance(foreground), luminance(background));
    (first.max(second) + 0.05) / (first.min(second) + 0.05)
}
