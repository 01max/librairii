//! Native presentation fixtures use only explicitly owned synthetic library data.
#[path = "native-foundations/presentation.rs"]
mod callback_checks;
#[path = "native-foundations/cover_projection.rs"]
mod cover_projection_checks;
#[cfg(target_os = "macos")]
#[path = "native-foundations/native_menu.rs"]
mod native_menu;
#[allow(dead_code)]
#[path = "../src/view.rs"]
mod view;
use image::ImageEncoder;
use librairii_adapters::thumbnails::Thumbnail;
use librairii_app::{lifecycle::LifecycleGate, localization::Language};
use librairii_core::{
    browsing::{DisplaySources, StorySummary},
    collection_codec::url::{Location, Scope},
    navigation::Navigation,
};
use librairii_desktop::{
    browsing::{Group, Page, Story},
    presentation::{Phase, Snapshot},
};
use librairii_ui::native::{DesktopWindow, ProposalRow};
use slint::{ComponentHandle, Model, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};
std::thread_local! {
    static REQUESTED_SIZE: std::cell::Cell<Option<(u32,u32)>> = const {std::cell::Cell::new(None)};
}
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn check(ok: bool, message: &str) -> Result {
    if ok {
        Ok(())
    } else {
        eprintln!("native check failed: {message}");
        Err(message.into())
    }
}
fn thumbnail(index: usize) -> Thumbnail {
    let (width, height) = match index % 3 {
        0 => (160, 256),
        1 => (256, 160),
        _ => (224, 224),
    };
    let mut rgba = Vec::new();
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&[
                30 + (x * 100 / width) as u8,
                65 + (y * 100 / height) as u8,
                100 + (index % 8 * 12) as u8,
                255,
            ]);
        }
    }
    Thumbnail {
        width,
        height,
        rgba: Arc::from(rgba),
    }
}
fn snapshot() -> Result<Snapshot> {
    let stories = (0..24).map(|i| {
        let title = match i % 4 {
            0 => "The Clockwork Forest",
            1 => "La lune et les étoiles",
            2 => "Les aventures de Zoé",
            _ => "Une histoire — 日本語 — 故事",
        }.to_owned();
        let title = if i >= 4 { format!("{title} · {:02}", i + 1) } else { title };
        let summary = StorySummary {
            broken: i == 1, id: i + 1,
            uuid: format!("f0000000-0000-4000-8000-{i:012}"), title: title.clone(),
            description: "An independently scrolling story description. Une description longue, lisible et conservée sans interprétation de balisage.".into(),
            author: "Studio des histoires".into(), artwork_id: String::new(),
            sources: DisplaySources { title: "embedded".into(), description: "embedded".into(), author: "embedded".into(), artwork: "embedded".into() },
            detected_format: "studio_zip".into(),
            compatibility: if i == 2 { "missing" } else { "compatible" }.into(),
            compatibility_reason: String::new(), byte_size: 12000,
            imported_at: "2026-10-20T12:00:00Z".into(), official: None,
        };
        Story { membership:None,current_sources:Vec::new(),coverage_incomplete:false,dates: Default::default(), uuid: summary.uuid.clone(), title, summary: Some(summary), thumbnail: Some(thumbnail(i as usize)), provenance: "Weekend favorites".into(), tags: vec![
            librairii_core::tagging::DisplayTag { source: "user".into(), definition: "Adventure".into(), value: None, color: "#405cf5".into() },
            librairii_core::tagging::DisplayTag { source: "derived".into(), definition: "Age".into(), value: Some("3–5 years".into()), color: "#286350".into() },
        ] }
    }).collect();
    let gate = LifecycleGate::new();
    let generation = gate.enter()?.generation().clone();
    Ok(Snapshot {
        workflows: Default::default(),
        facets: Default::default(),
        revision: 0,
        restore_revision: 0,
        generation: Some(generation),
        phase: Phase::Library,
        language: Language::English,
        status: String::new(),
        groups: vec![
            Group {
                scope: Scope::Collection(librairii_core::CollectionId::new(1)?),
                name: "Weekend favorites".into(),
                count: 18,
                available: true,
            },
            Group {
                scope: Scope::Shelf {
                    id: librairii_core::ShelfId::new(1)?,
                    manual_order: true,
                },
                name: "Bedtime stories".into(),
                count: 24,
                available: true,
            },
        ],
        navigation: Some(Navigation::new(
            "e0000000-0000-4000-8000-000000000014".parse()?,
            1,
            Location {
                scope: Scope::Library,
                request: Default::default(),
            },
        )?),
        page: Some(Page {
            shelf: None,
            stories,
            previews: Vec::new(),
            preview_page: 0,
            preview_pages: 0,
            page: 1,
            pages: 56,
            total: 1324,
        }),
    })
}
fn key(ui: &DesktopWindow, key: slint::SharedString) -> Result {
    ui.window()
        .dispatch_event_with_result(slint::platform::WindowEvent::KeyPressed {
            text: key.clone(),
        })?;
    ui.window()
        .dispatch_event_with_result(slint::platform::WindowEvent::KeyReleased { text: key })?;
    Ok(())
}
// Same alpha-aware palette measurement used by the qualified toolkit spike.
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
fn capture(ui: &DesktopWindow, path: &Path) -> Result {
    let ratios = [
        contrast(
            ui.get_observed_foreground().color(),
            ui.get_observed_background().color(),
            ui.get_observed_background().color(),
        ),
        contrast(
            ui.get_observed_control_foreground().color(),
            ui.get_observed_control_background().color(),
            ui.get_observed_background().color(),
        ),
        contrast(
            ui.get_observed_selection_foreground().color(),
            ui.get_observed_selection_background().color(),
            ui.get_observed_background().color(),
        ),
    ];
    check(
        ratios.iter().all(|ratio| *ratio >= 4.5),
        "native text palette contrast is below 4.5",
    )?;
    let pixels = ui.window().take_snapshot()?;
    check(
        pixels.width() >= 560 && pixels.height() >= 400,
        "native frame smaller than supported minimum",
    )?;
    check(
        pixels
            .as_bytes()
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p != &pixels.as_bytes()[..4]),
        "native frame is flat",
    )?;
    // Store the exact RGBA pixels without debug-build filtering/deflate work.
    // The provider compresses the evidence archive after the bounded native run.
    // Preserve every frame, assertion and the same per-scale deadline.
    let mut output = std::io::BufWriter::new(std::fs::File::create(path)?);
    image::codecs::png::PngEncoder::new_with_quality(
        &mut output,
        image::codecs::png::CompressionType::Uncompressed,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    std::io::Write::flush(&mut output)?;
    Ok(())
}
fn main() -> Result {
    librairii_desktop::menus::prepare_process()?;
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("explicit fixture output required")?;
    check(output.is_absolute(), "fixture output must be absolute")?;
    check(
        std::fs::read(output.join(".librairii-native-fixture"))? == b"native-foundations-14\n",
        "isolated fixture marker missing",
    )?;
    let (ui, menus) = librairii_desktop::menus::create_window()?;
    if std::env::args().any(|arg| arg == "--sync-callback-checks") {
        return callback_checks::sync(&ui);
    }
    if std::env::args().any(|arg| arg == "--operation-callback-checks") {
        return callback_checks::operations(&ui);
    }
    if let Some(case) =
        std::env::args().find_map(|a| a.strip_prefix("--workflow-case=").map(str::to_owned))
    {
        let language = std::env::args()
            .find_map(|a| a.strip_prefix("--workflow-language=").map(str::to_owned))
            .ok_or("explicit workflow language required")?;
        return callback_checks::speech(&ui, &menus, &output, &case, &language);
    }
    let server = std::env::args().any(|arg| arg == "--ax-server");
    // The parent verifies every callback at both scales before launching these
    // semantic observers. Publish their window within the original AX deadline
    // instead of rerunning the whole backend suite before registration.
    if !server {
        eprintln!("native callbacks: paging");
        callback_checks::paging(&ui)?;
        eprintln!("native callbacks: history");
        callback_checks::history(&ui)?;
        eprintln!("native callbacks: failed_query");
        callback_checks::failed_query(&ui)?;
        callback_checks::filters(&ui, None)?;
        eprintln!("native callbacks: details");
        callback_checks::details(&ui)?;
        eprintln!("native callbacks: tags");
        callback_checks::tags(&ui)?;
        eprintln!("native callbacks: flags");
        callback_checks::flags(&ui)?;
        eprintln!("native callbacks: collections");
        callback_checks::collections(&ui)?;
        eprintln!("native callbacks: shelves");
        callback_checks::shelves(&ui)?;
        eprintln!("native callbacks: shelf_reviews");
        callback_checks::shelf_reviews(&ui)?;
        eprintln!("native callbacks: operations");
        callback_checks::operations(&ui)?;
        eprintln!("native callbacks: sync");
        callback_checks::sync(&ui)?;
        eprintln!("native callbacks: exports");
        callback_checks::exports(&ui)?;
        eprintln!("native callbacks: activity_history");
        callback_checks::activity_history(&ui)?;
        eprintln!("native callbacks: maintenance");
        callback_checks::maintenance(&ui)?;
    }
    while ui.get_dialog() != 0 {
        ui.invoke_close_dialog();
    }
    ui.set_startup_phase(4);
    menus
        .commands()
        .enqueue(librairii_desktop::menus::NativeCommand::History);
    menus
        .commands()
        .enqueue(librairii_desktop::menus::NativeCommand::Settings);
    menus.dispatch(&ui);
    check(!ui.get_modal(), "startup menu command ran before readiness")?;
    ui.set_startup_phase(0);
    menus.dispatch(&ui);
    check(
        ui.get_dialog() == 2,
        "buffered native Settings command was discarded",
    )?;
    ui.invoke_close_dialog();
    ui.set_startup_phase(3);
    menus.dispatch(&ui);
    ui.invoke_close_dialog();
    let initial = snapshot()?;
    let view = Rc::new(RefCell::new(view::View {
        covers: Default::default(),
        maintenance: Default::default(),
        activation: initial.workflows.settings.activation,
        operations: Default::default(),
        exports: Default::default(),
        shelf_reviews: Default::default(),
        shelves: Default::default(),
        collections: Default::default(),
        flags: Default::default(),
        tags: Default::default(),
        details: Default::default(),
        filters: view::filters::State::new(
            initial
                .navigation
                .as_ref()
                .map(|n| n.current().location.request.clone())
                .unwrap_or_default(),
        ),
        catalogs: librairii_app::localization::Catalogs::embedded()?,
        snapshot: initial,
        selected: Some("f0000000-0000-4000-8000-000000000000".into()),
        columns: 1,
        guard: Default::default(),
    }));
    let surfaces = Rc::new(Surfaces::default());
    view.borrow_mut().project(&ui);
    ui.set_sources(ModelRc::new(VecModel::from(vec![
        "Weekend favorites".into(),
        "French stories".into(),
    ])));
    proposals(&ui, false);
    let weak = ui.as_weak();
    ui.on_quit(move || {
        if let Some(ui) = weak.upgrade() {
            let _hidden = ui.hide();
        }
        let _quit = slint::quit_event_loop();
    });
    let state = view.clone();
    let weak = ui.as_weak();
    ui.on_change_language(move |code| {
        if let (Some(ui), Ok(language)) = (weak.upgrade(), Language::explicit(&code)) {
            state.borrow_mut().snapshot.language = language;
            state.borrow_mut().project(&ui);
            proposals(&ui, language == Language::French);
        }
    });
    let state = view.clone();
    let weak = ui.as_weak();
    ui.on_select_story(move |index| {
        if let Some(ui) = weak.upgrade() {
            let uuid = state
                .borrow()
                .snapshot
                .page
                .as_ref()
                .and_then(|p| p.stories.get(index as usize))
                .map(|s| s.uuid.clone());
            state.borrow_mut().selected = uuid;
            state.borrow_mut().project(&ui);
        }
    });
    cover_projection_checks::checks(&ui, &view)?;
    let state = view.clone();
    let weak = ui.as_weak();
    ui.on_mode_selected(move |_| {
        if let Some(ui) = weak.upgrade() {
            state.borrow_mut().resize(&ui);
            state.borrow_mut().project(&ui);
        }
    });
    if std::env::args().any(|arg| arg == "--workspace") {
        ui.set_workspace(true);
        ui.set_mode(0);
    }
    let step = Rc::new(RefCell::new(0usize));
    let errors = Rc::new(RefCell::new(None::<String>));
    let report = Rc::new(RefCell::new(Vec::<serde_json::Value>::new()));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let captured_errors = errors.clone();
    let captured_report = report.clone();
    let destination = output.clone();
    let fixture_step = step.clone();
    // Native menu dispatch can run a nested event loop. A fixture step owns
    // its command/response pair until it returns; do not execute it twice.
    let ticking = std::cell::Cell::new(false);
    let geometry_wait = std::cell::Cell::new(None);
    timer.start(
        slint::TimerMode::Repeated,
        // Steps already await actual worker responses and requested geometry.
        // Avoid adding a tenth of a second of idle time to every fixture step.
        Duration::from_millis(16),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            // A timer may fire before Winit resumes and creates the OS window.
            // Keep the same fixture step until that observable lifecycle event;
            // the parent driver's deadline still bounds a missing window.
            if !ui.window().has_winit_window() {
                return;
            }
            // A native resize is asynchronous. Keep the next existing step
            // pending until its actual render-space geometry is observed;
            // the unchanged parent deadline still bounds missing OS delivery.
            if REQUESTED_SIZE.with(|size| {
                size.get().is_some_and(|(w, h)| {
                    ui.window().size().width != w || ui.window().size().height != h
                })
            }) {
                let step = *fixture_step.borrow();
                if geometry_wait.replace(Some(step)) != Some(step) {
                    eprintln!(
                        "native geometry pending at step {step}: actual={:?}, requested={:?}",
                        ui.window().size(),
                        REQUESTED_SIZE.with(|size| size.get())
                    );
                }
                return;
            }
            if captured_errors.borrow().is_some() || ticking.replace(true) {
                return;
            }
            let result = (|| -> Result {
                menus.dispatch(&ui);
                #[cfg(target_os = "macos")]
                check(
                    menus.owns_native_menu(),
                    "native application menu owner changed",
                )?;
                if let Some(surface) = surfaces.filters.borrow().as_ref() {
                    surface.poll(&ui)?;
                } else if let Some(surface) = surfaces.details.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.tags.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.flags.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.shelves.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.reviews.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.collections.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.operations.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.sync.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.exports.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.history.borrow().as_ref() {
                    surface.poll(&ui);
                } else if let Some(surface) = surfaces.maintenance.borrow().as_ref() {
                    surface.poll(&ui);
                } else {
                    view.borrow_mut().resize(&ui);
                }
                if server {
                    return Ok(());
                }
                eprintln!("native frame step: {}", *fixture_step.borrow());
                capture_step(
                    &ui,
                    &view,
                    *fixture_step.borrow(),
                    &destination,
                    &captured_report,
                    &menus,
                    &surfaces,
                )?;
                *fixture_step.borrow_mut() += 1;
                Ok(())
            })();
            ticking.set(false);
            if let Err(error) = result {
                *captured_errors.borrow_mut() = Some(error.to_string());
                let _quit = slint::quit_event_loop();
            }
        },
    );
    ui.run()?;
    timer.stop();
    if let Some(error) = errors.borrow_mut().take() {
        return Err(error.into());
    }
    check(
        server || *step.borrow() > 136,
        "native fixture ended before completing its checks",
    )?;
    Ok(())
}

#[derive(Debug, Default)]
struct Surfaces {
    maintenance: RefCell<Option<callback_checks::MaintenanceSurface>>,
    sync: RefCell<Option<callback_checks::SyncSurface>>,
    exports: RefCell<Option<callback_checks::ExportSurface>>,
    history: RefCell<Option<callback_checks::HistorySurface>>,
    operations: RefCell<Option<callback_checks::OperationSurface>>,
    collections: RefCell<Option<callback_checks::CollectionSurface>>,
    shelves: RefCell<Option<callback_checks::ShelfSurface>>,
    reviews: RefCell<Option<callback_checks::ReviewSurface>>,
    flags: RefCell<Option<callback_checks::FlagSurface>>,
    filters: RefCell<Option<callback_checks::FilterSurface>>,
    details: RefCell<Option<callback_checks::DetailSurface>>,
    tags: RefCell<Option<callback_checks::TagSurface>>,
}

fn capture_step(
    ui: &DesktopWindow,
    view: &Rc<RefCell<view::View>>,
    index: usize,
    destination: &Path,
    report: &RefCell<Vec<serde_json::Value>>,
    _menus: &librairii_desktop::menus::Menus,
    surfaces: &Surfaces,
) -> Result {
    if index < 48 {
        let workspace = index >= 36;
        let case = if workspace {
            (index - 36) / 2
        } else {
            index / 2
        };
        let mode = if workspace { 0 } else { (case % 3) as i32 };
        let density = if workspace { case % 3 } else { (case / 3) % 3 };
        let french = case >= if workspace { 3 } else { 9 };
        if index.is_multiple_of(2) {
            let (width, height) = match density {
                0 => (1180., 760.),
                1 => (980., 600.),
                _ => (560., 400.),
            };
            ui.set_workspace(workspace);
            ui.set_mode(mode);
            let mut state = view.borrow_mut();
            if workspace {
                if let Some(nav) = &mut state.snapshot.navigation {
                    nav.replace(librairii_core::navigation::Frame {
                        location: Location {
                            scope: Scope::Shelf {
                                id: librairii_core::ShelfId::new(1)?,
                                manual_order: false,
                            },
                            request: Default::default(),
                        },
                        selection: Vec::new(),
                    })?;
                }
                if let Some(page) = &mut state.snapshot.page {
                    page.total = 24;
                    page.pages = 1;
                }
            }
            resize(ui, width, height)?;
            state.snapshot.language = if french {
                Language::French
            } else {
                Language::English
            };
            state.resize(ui);
            state.project(ui);
            ui.set_shelf_active(false);
            ui.set_workspace(workspace);
            proposals(ui, french);
        } else {
            let name = if workspace {
                format!(
                    "workspace-{density}-{}.png",
                    if french { "fr" } else { "en" }
                )
            } else {
                format!(
                    "story-{density}-{mode}-{}.png",
                    if french { "fr" } else { "en" }
                )
            };
            if workspace {
                check(
                    ui.get_observed_approved_height() >= 76.0
                        && ui.get_observed_review_height() >= 80.0,
                    &format!(
                        "workspace panes cannot display a full interaction target: approved={} review={} controls_y={} controls_height={} compact={} pending={} startup={} window={:?}",
                        ui.get_observed_approved_height(),
                        ui.get_observed_review_height(),
                        ui.get_controls_viewport_y(),
                        ui.get_controls_viewport_height(),
                        ui.get_compact(),
                        ui.get_pending(),
                        ui.get_startup_phase(),
                        ui.window().size()
                    ),
                )?;
            } else {
                check(
                    ui.get_observed_browser_height() + 0.5 >= ui.get_observed_story_row_height(),
                    "story viewport cannot display a full row with its external captions",
                )?;
            }
            capture(ui, &destination.join(name))?;
            if workspace && density == 2 && french {
                ui.invoke_focus_review();
                capture(ui, &destination.join("workspace-review-compact-fr.png"))?;
            }
            let size = ui.window().size();
            let native_window = native_window_state(ui)?;
            report.borrow_mut().push(serde_json::json!({
                "concept": if workspace { "Desktop workspace" } else { "Story shelves" },
                "density": density, "mode": mode, "french": french, "width": size.width, "height": size.height,
                "scale": ui.window().scale_factor(), "items": ui.get_story_count(), "columns": ui.get_columns(),
                "compact": ui.get_compact(), "wide": ui.get_wide(), "decisions": ui.get_decisions(),
                "story_viewport_height": ui.get_observed_browser_height(), "story_row_height": ui.get_observed_story_row_height(),
                "approved_viewport_height": ui.get_observed_approved_height(), "review_viewport_height": ui.get_observed_review_height(),
                "controls_viewport_y": ui.get_controls_viewport_y(), "controls_viewport_height": ui.get_controls_viewport_height(),
                "native_window": native_window,
            }));
        }
    } else if index == 48 {
        ui.set_workspace(false);
        let mut state = view.borrow_mut();
        if let Some(nav) = &mut state.snapshot.navigation {
            nav.replace(librairii_core::navigation::Frame {
                location: Location {
                    scope: Scope::Library,
                    request: Default::default(),
                },
                selection: Vec::new(),
            })?;
        }
        if let Some(page) = &mut state.snapshot.page {
            page.total = 1324;
            page.pages = 56;
        }
        resize(ui, 560., 400.)?;
        state.snapshot.language = Language::French;
        state.project(ui);
        #[cfg(target_os = "macos")]
        native_menu::verify(ui, _menus)?;
    } else if index == 49 {
        ui.invoke_focus_search();
        key(ui, "Étoile".into())?;
        check(ui.get_query() == "Étoile", "literal UTF8 input lost")?;
        ui.invoke_open_dialog(2);
        check(ui.get_modal(), "settings did not open")?;
        for _ in 0..10 {
            key(ui, slint::platform::Key::Tab.into())?;
            check(!ui.get_query_focused(), "modal focus escaped to background")?;
        }
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_modal() && ui.get_query_focused(),
            "dialog did not restore search focus",
        )?;
    } else if index == 50 {
        ui.invoke_open_sidebar();
        check(ui.get_sidebar_open(), "compact sidebar did not open")?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Shift.into(),
            })?;
        key(ui, slint::platform::Key::Tab.into())?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::KeyReleased {
                text: slint::platform::Key::Shift.into(),
            })?;
        check(
            ui.get_focused_key() == 205,
            "reverse sidebar wrap lost the owned Settings focus",
        )?;
        key(ui, " ".into())?;
        check(
            ui.get_dialog() == 2,
            "keyboard Settings endpoint did not open",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            ui.get_focused_key() == 205,
            "Settings did not restore its exact sidebar invoker",
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_focused_key() == 201,
            "forward sidebar wrap lost the owned Close focus",
        )?;
        for _ in 0..40 {
            if ui.get_focused_key() >= 220 {
                break;
            }
            key(ui, slint::platform::Key::Tab.into())?;
        }
        let group_invoker = ui.get_focused_key();
        if group_invoker < 220 {
            capture(ui, &destination.join("sidebar-failure.png"))?;
        }
        check(
            group_invoker >= 220,
            "dynamic sidebar group is not keyboard reachable",
        )?;
        ui.invoke_show_panel(5);
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            ui.get_focused_key() == group_invoker,
            "dialog did not restore dynamic group invoker",
        )?;
        check(
            ui.get_sidebar_open() && !ui.get_modal(),
            "dialog discarded underlying sidebar or remained open",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_sidebar_open(),
            &format!(
                "nested Escape failed: compact={}, focused key={}",
                ui.get_compact(),
                ui.get_focused_key()
            ),
        )?;
        ui.invoke_focus_browser();
        key(ui, slint::platform::Key::End.into())?;
        check(
            ui.get_selected_index() == 23,
            "offscreen keyboard seek failed",
        )?;
    } else if index == 51 {
        capture(ui, &destination.join("keyboard-end-fr.png"))?;
        ui.set_status("Opération en cours".into());
        ui.set_pending(true);
        ui.set_indeterminate(true);
        ui.set_commit_pending(true);
        #[cfg(target_os = "macos")]
        native_menu::request_quit(ui, _menus)?;
        #[cfg(not(target_os = "macos"))]
        ui.invoke_request_quit();
        check(
            ui.get_dialog() == 7,
            "pending close did not offer safe choice",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_modal() && ui.get_pending(),
            "dismissal cancelled pending work",
        )?;
        capture(ui, &destination.join("motion-first.png"))?;
    } else if index == 52 {
        capture(ui, &destination.join("motion-second.png"))?;
        let first = image::open(destination.join("motion-first.png"))?.into_rgba8();
        let second = image::open(destination.join("motion-second.png"))?.into_rgba8();
        check(
            first.dimensions() == second.dimensions(),
            "progress frame dimensions changed",
        )?;
        let band = (6.0 * ui.window().scale_factor()).ceil() as u32;
        let start = ((first.height() - band.min(first.height())) * first.width() * 4) as usize;
        check(
            first.as_raw()[start..] == second.as_raw()[start..],
            "application progress introduces nonessential motion",
        )?;
        ui.set_pending(false);
        ui.set_commit_pending(false);
        #[cfg(target_os = "macos")]
        native_menu::about(ui, _menus)?;
        #[cfg(not(target_os = "macos"))]
        ui.invoke_open_dialog(1);
    } else if index == 53 {
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_attribution_focused(),
            "attribution website is not keyboard reachable",
        )?;
        capture(ui, &destination.join("about-slint-fr.png"))?;
        key(ui, slint::platform::Key::Escape.into())?;
    } else if index == 54 {
        *surfaces.filters.borrow_mut() = Some(callback_checks::FilterSurface::open(ui)?);
    } else if index == 55 {
        capture(ui, &destination.join("filters-compact-en.png"))?;
        surfaces
            .filters
            .borrow()
            .as_ref()
            .ok_or("filter surface missing")?
            .bottom(ui)?;
    } else if index == 56 {
        capture(ui, &destination.join("filters-actions-compact-en.png"))?;
        surfaces
            .filters
            .borrow()
            .as_ref()
            .ok_or("filter surface missing")?
            .french(ui)?;
    } else if index == 57 {
        capture(ui, &destination.join("filters-compact-fr.png"))?;
        surfaces
            .filters
            .borrow()
            .as_ref()
            .ok_or("filter surface missing")?
            .bottom(ui)?;
    } else if index == 58 {
        capture(ui, &destination.join("filters-actions-compact-fr.png"))?;
        ui.invoke_close_dialog();
        surfaces.filters.borrow_mut().take();
    } else if index == 59 {
        resize(ui, 1180.0, 760.0)?;
        *surfaces.details.borrow_mut() = Some(callback_checks::DetailSurface::open(ui)?);
    } else if index == 60 {
        surfaces
            .details
            .borrow()
            .as_ref()
            .ok_or("detail surface missing")?
            .first_disclosure(ui)?;
        #[cfg(target_os = "macos")]
        native_menu::inspector_focus(ui, _menus)?;
    } else if index == 61 {
        capture(ui, &destination.join("details-initial-wide-en.png"))?;
        surfaces
            .details
            .borrow()
            .as_ref()
            .ok_or("detail surface missing")?
            .begin_details(ui)?;
        resize(ui, 560.0, 400.0)?;
    } else if index == 62 {
        capture(ui, &destination.join("details-compact-en.png"))?;
        surfaces
            .details
            .borrow()
            .as_ref()
            .ok_or("detail surface missing")?
            .technical(ui)?;
    } else if index == 63 {
        capture(ui, &destination.join("details-technical-compact-en.png"))?;
        surfaces
            .details
            .borrow()
            .as_ref()
            .ok_or("detail surface missing")?
            .french_reopen(ui)?;
    } else if index == 64 {
        capture(ui, &destination.join("details-compact-fr.png"))?;
        surfaces
            .details
            .borrow()
            .as_ref()
            .ok_or("detail surface missing")?
            .technical(ui)?;
    } else if index == 65 {
        capture(ui, &destination.join("details-technical-compact-fr.png"))?;
        ui.invoke_begin_removal();
    } else if index == 66 {
        capture(ui, &destination.join("removal-confirmation-compact-fr.png"))?;
        check(
            ui.get_removal_keep_focused(),
            "confirmation did not retain Keep focus",
        )?;
        ui.window()
            .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Escape.into(),
            });
        check(
            ui.get_dialog() == 3,
            "Escape dismissed story details along with removal confirmation",
        )?;
        ui.invoke_close_dialog();
        surfaces.details.borrow_mut().take();
    } else if index == 67 {
        *surfaces.tags.borrow_mut() = Some(callback_checks::TagSurface::open(ui, _menus)?);
    } else if index == 68 {
        #[cfg(target_os = "macos")]
        native_menu::tag_editor(ui, _menus)?;
        capture(ui, &destination.join("tag-manager-compact-en.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .bottom(ui)?;
    } else if index == 69 {
        capture(ui, &destination.join("tag-manager-actions-compact-en.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .french(ui)?;
    } else if index == 70 {
        capture(ui, &destination.join("tag-manager-compact-fr.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .bottom(ui)?;
    } else if index == 71 {
        capture(ui, &destination.join("tag-manager-actions-compact-fr.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .assignment(ui, "en")?;
    } else if index == 72 {
        capture(ui, &destination.join("tag-assignment-mixed-compact-en.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .assignment(ui, "fr")?;
    } else if index == 73 {
        capture(ui, &destination.join("tag-assignment-mixed-compact-fr.png"))?;
        surfaces
            .tags
            .borrow()
            .as_ref()
            .ok_or("tag surface missing")?
            .confirm(ui)?;
    } else if index == 74 {
        capture(ui, &destination.join("tag-impact-compact-fr.png"))?;
        ui.invoke_close_dialog();
        check(
            ui.get_tag_delete_invoker_focused(),
            "tag confirmation Escape lost its invoker",
        )?;
        ui.invoke_close_dialog();
        surfaces.tags.borrow_mut().take();
    } else if index == 75 {
        *surfaces.flags.borrow_mut() = Some(callback_checks::FlagSurface::open(ui)?);
    } else if index == 76 {
        surfaces
            .flags
            .borrow()
            .as_ref()
            .ok_or("flag surface missing")?
            .toggle(ui)?;
    } else if index == 77 {
        capture(ui, &destination.join("broken-detail-compact-en.png"))?;
        surfaces
            .flags
            .borrow()
            .as_ref()
            .ok_or("flag surface missing")?
            .french_bulk(ui)?;
    } else if index == 78 {
        check(
            ui.get_observed_browser_height() + 0.5 >= ui.get_observed_story_row_height(),
            &format!(
                "active bulk controls hide the story viewport: viewport={} row={} notice={:?} error={:?}",
                ui.get_observed_browser_height(),
                ui.get_observed_story_row_height(),
                ui.get_terminal_notice(),
                ui.get_terminal_notice_error()
            ),
        )?;
        surfaces
            .flags
            .borrow()
            .as_ref()
            .ok_or("flag surface missing")?
            .focus_undo(ui)?;
        #[cfg(target_os = "macos")]
        native_menu::bulk_flag_undo_focus(ui, _menus)?;
        capture(ui, &destination.join("broken-bulk-compact-fr.png"))?;
        surfaces
            .flags
            .borrow()
            .as_ref()
            .ok_or("flag surface missing")?
            .undo(ui)?;
        capture(ui, &destination.join("broken-undo-compact-fr.png"))?;
        resize(ui, 1180., 760.)?;
    } else if index == 79 {
        surfaces
            .flags
            .borrow()
            .as_ref()
            .ok_or("flag surface missing")?
            .panel(ui)?;
    } else if index == 80 {
        #[cfg(target_os = "macos")]
        native_menu::flag_focus(ui, _menus)?;
        capture(ui, &destination.join("broken-panel-wide-fr.png"))?;
        surfaces.flags.borrow_mut().take();
        resize(ui, 560., 400.)?;
    } else if index == 81 {
        *surfaces.collections.borrow_mut() = Some(callback_checks::CollectionSurface::open(ui)?);
    } else if index == 82 {
        #[cfg(target_os = "macos")]
        native_menu::collection_editor(ui, _menus)?;
        capture(ui, &destination.join("collections-compact-en.png"))?;
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .french(ui)?;
    } else if index == 83 {
        capture(ui, &destination.join("collections-compact-fr.png"))?;
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .union(ui)?;
    } else if index == 84 {
        check(
            ui.get_collection_union_focused()
                && ui.get_collection_union_window_y() >= ui.get_dialog_viewport_y() - 0.5
                && ui.get_collection_union_window_y() + ui.get_collection_union_height()
                    <= ui.get_dialog_viewport_y() + ui.get_dialog_viewport_height() + 0.5,
            "focused union action was clipped",
        )?;
        capture(ui, &destination.join("collections-union-fr.png"))?;
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .impact(ui)?;
    } else if index == 85 {
        capture(ui, &destination.join("collection-replacement-fr.png"))?;
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .overview(ui)?;
    } else if index == 86 {
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .bounded(ui)?;
    } else if index == 87 {
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .focus_preview(ui)?;
    } else if index == 88 {
        capture(ui, &destination.join("collection-preview-focus-fr.png"))?;
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .preview(ui)?;
    } else if index == 89 {
        check(
            ui.get_preview_focused_key() == ui.get_focused_key()
                && ui.get_preview_focused_key() < 0,
            "preview inspection lost actual native focus after model refresh",
        )?;
        capture(ui, &destination.join("collection-previews-fr.png"))?;
        #[cfg(target_os = "macos")]
        native_menu::preview_focus(ui, _menus)?;
    } else if index == 90 {
        surfaces
            .collections
            .borrow()
            .as_ref()
            .ok_or("Collection surface missing")?
            .bulk(ui)?;
    } else if index == 91 {
        capture(ui, &destination.join("collection-preview-bulk-fr.png"))?;
        surfaces.collections.borrow_mut().take();
    } else if (92..=94).contains(&index) {
        let (phase, name, status) = match index {
            92 => (
                Phase::Recovery,
                "recovery",
                "La bibliothèque attendue est indisponible ou nécessite une récupération.",
            ),
            93 => (
                Phase::Setup,
                "setup",
                "Choisissez où conserver votre bibliothèque.",
            ),
            _ => (Phase::Opening, "opening", "Ouverture de la bibliothèque…"),
        };
        let mut state = view.borrow_mut();
        state.snapshot.phase = phase;
        state.snapshot.status = status.into();
        state.snapshot.page = None;
        state.snapshot.navigation = None;
        state.snapshot.groups.clear();
        state.project(ui);
        check(
            ui.get_story_count() == 0 && ui.get_selected_index() == -1,
            "first-run/recovery retained library content",
        )?;
        capture(ui, &destination.join(format!("{name}-compact-fr.png")))?;
    } else if index == 95 {
        resize(ui, 1180., 760.)?;
        *surfaces.shelves.borrow_mut() = Some(callback_checks::ShelfSurface::open(ui)?);
    } else if index == 96 {
        capture(ui, &destination.join("shelf-approved-wide-en.png"))?;
        #[cfg(target_os = "macos")]
        native_menu::shelf_focus(ui, _menus)?;
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .french(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 97 {
        capture(ui, &destination.join("shelf-approved-compact-fr.png"))?;
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .bulk(ui)?;
        capture(ui, &destination.join("shelf-approved-bulk-compact-fr.png"))?;
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .clear_bulk(ui);
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .creation(ui)?;
    } else if index == 98 {
        capture(ui, &destination.join("shelf-creation-compact-fr.png"))?;
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .name(ui)?;
    } else if index == 99 {
        #[cfg(target_os = "macos")]
        native_menu::shelf_editor(ui, _menus)?;
        capture(ui, &destination.join("shelf-manager-edit-compact-fr.png"))?;
        surfaces
            .shelves
            .borrow()
            .as_ref()
            .ok_or("Shelf surface missing")?
            .exclusions(ui)?;
    } else if index == 100 {
        check(
            ui.get_shelf_exclusion_refresh_focused(),
            "exclusion opening lost initial Refresh focus",
        )?;
        capture(ui, &destination.join("shelf-exclusions-compact-fr.png"))?;
        key(ui, slint::platform::Key::Tab.into())?;
    } else if index == 101 {
        check(
            !ui.get_shelf_exclusion_focused_uuid().is_empty(),
            "native Tab did not reach Allow suggestions",
        )?;
        capture(ui, &destination.join("shelf-exclusions-keyboard-fr.png"))?;
        surfaces.shelves.borrow_mut().take();
    } else if index == 102 {
        resize(ui, 1180., 760.)?;
        *surfaces.reviews.borrow_mut() = Some(callback_checks::ReviewSurface::open(ui)?);
    } else if index == 103 {
        capture(ui, &destination.join("shelf-review-additions-wide-en.png"))?;
        surfaces
            .reviews
            .borrow()
            .as_ref()
            .ok_or("review surface missing")?
            .french_removals(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 104 {
        capture(
            ui,
            &destination.join("shelf-review-removals-compact-fr.png"),
        )?;
        check(
            ui.get_review_refresh_focused(),
            "review section lost stable Refresh focus",
        )?;
        surfaces
            .reviews
            .borrow()
            .as_ref()
            .ok_or("review surface missing")?
            .native_choice(ui)?;
    } else if index == 105 {
        capture(ui, &destination.join("shelf-review-keep-keyboard-fr.png"))?;
        check(
            !ui.get_modal() && !ui.get_review_focused_uuid().is_empty(),
            "review decision lost exact native focus",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
    } else if index == 106 {
        check(
            !ui.get_review_shown(),
            "native Escape did not defer the review",
        )?;
        surfaces
            .reviews
            .borrow()
            .as_ref()
            .ok_or("review surface missing")?
            .collection_deletion(ui)?;
        capture(
            ui,
            &destination.join("shelf-linked-deletion-keyboard-fr.png"),
        )?;
        surfaces.reviews.borrow_mut().take();
    } else if index == 107 {
        resize(ui, 1180., 760.)?;
        *surfaces.operations.borrow_mut() = Some(callback_checks::OperationSurface::open(ui)?);
    } else if index == 108 {
        capture(ui, &destination.join("imports-results-wide-en.png"))?;
        surfaces
            .operations
            .borrow()
            .as_ref()
            .ok_or("operation surface missing")?
            .french(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 109 {
        check(
            ui.get_operation_displayed_choice()
                .starts_with("Import · Terminée"),
            "operation choice did not re-localize its native displayed value",
        )?;
        check(
            ui.get_operation_first_focused(),
            "import picker action lost native focus",
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_operation_selector_focused(),
            "native Tab did not reach operation selection",
        )?;
        capture(ui, &destination.join("imports-results-compact-fr.png"))?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(!ui.get_modal(), "import Escape did not return to browsing")?;
        surfaces.operations.borrow_mut().take();
    } else if index == 110 {
        resize(ui, 1180., 760.)?;
        *surfaces.sync.borrow_mut() = Some(callback_checks::SyncSurface::open(ui)?);
    } else if index == 111 {
        capture(ui, &destination.join("metadata-sync-prompt-wide-en.png"))?;
        surfaces
            .sync
            .borrow()
            .as_ref()
            .ok_or("sync surface missing")?
            .success(ui)?;
    } else if index == 112 {
        capture(ui, &destination.join("metadata-sync-success-wide-en.png"))?;
        surfaces
            .sync
            .borrow()
            .as_ref()
            .ok_or("sync surface missing")?
            .french_failure(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 113 {
        ui.invoke_open_sidebar();
        ui.invoke_focus_sync();
    } else if index == 114 {
        capture(
            ui,
            &destination.join("metadata-sync-failure-compact-fr.png"),
        )?;
        check(
            ui.get_sync_start_focused(),
            "sync Retry lost native keyboard focus",
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_sync_refresh_focused(),
            "native Tab did not reach local sync status refresh",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_sidebar_open(),
            "sync sidebar Escape did not return to browsing",
        )?;
        surfaces.sync.borrow_mut().take();
    } else if index == 115 {
        resize(ui, 1180., 760.)?;
        *surfaces.exports.borrow_mut() = Some(callback_checks::ExportSurface::open(ui)?);
    } else if index == 116 {
        check(
            ui.get_export_first_focused(),
            "export scope lost native focus",
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_export_picker_focused(),
            "native Tab did not reach export destination picker",
        )?;
        capture(ui, &destination.join("export-preview-wide-en.png"))?;
        surfaces
            .exports
            .borrow()
            .as_ref()
            .ok_or("export surface missing")?
            .finish_export(ui)?;
    } else if index == 117 {
        capture(ui, &destination.join("export-report-wide-en.png"))?;
        surfaces
            .exports
            .borrow()
            .as_ref()
            .ok_or("export surface missing")?
            .french_zero(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 118 {
        ui.invoke_focus_export_start();
    } else if index == 119 {
        capture(ui, &destination.join("export-zero-ready-compact-fr.png"))?;
        check(
            ui.get_export_picker_focused(),
            "zero-ready Start did not offer enabled destination reselection",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(!ui.get_modal(), "export Escape did not return to browsing")?;
        surfaces.exports.borrow_mut().take();
    } else if index == 120 {
        resize(ui, 1180., 760.)?;
        *surfaces.history.borrow_mut() = Some(callback_checks::HistorySurface::open(ui)?);
    } else if index == 121 {
        check(
            ui.get_history_first_focused(),
            "History initial focus was lost",
        )?;
        capture(ui, &destination.join("history-wide-en.png"))?;
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .report(ui)?;
    } else if index == 122 {
        capture(ui, &destination.join("history-export-report-en.png"))?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            ui.get_dialog() == 21 && ui.get_history_entry_focused() == 1,
            "History report Escape did not restore the exact entry",
        )?;
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .report(ui)?;
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .french_error(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 123 {
        check(
            ui.get_dialog() == 21 && ui.get_history_retry_focused(),
            &format!(
                "History Retry changed during compact resize: dialog={} retry={} entry={}",
                ui.get_dialog(),
                ui.get_history_retry_focused(),
                ui.get_history_entry_focused()
            ),
        )?;
    } else if index == 124 {
        check(
            ui.get_dialog() == 21 && ui.get_history_retry_focused(),
            &format!(
                "History Retry changed after the settled frame: dialog={} retry={} entry={}",
                ui.get_dialog(),
                ui.get_history_retry_focused(),
                ui.get_history_entry_focused()
            ),
        )?;
        capture(ui, &destination.join("history-retry-compact-fr.png"))?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::KeyPressed {
                text: slint::platform::Key::Shift.into(),
            })?;
        key(ui, slint::platform::Key::Tab.into())?;
        ui.window()
            .dispatch_event_with_result(slint::platform::WindowEvent::KeyReleased {
                text: slint::platform::Key::Shift.into(),
            })?;
        check(
            ui.get_history_entry_focused() == ui.get_history_entries().row_count() as i32 - 1
                && !ui.get_close_focused()
                && !ui.get_history_first_focused()
                && !ui.get_history_retry_focused(),
            &format!(
                "History reverse Tab from Retry missed the preceding entry: entry={} refresh={} retry={} close={}",
                ui.get_history_entry_focused(),
                ui.get_history_first_focused(),
                ui.get_history_retry_focused(),
                ui.get_close_focused(),
            ),
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_history_retry_focused()
                && !ui.get_history_first_focused()
                && !ui.get_close_focused(),
            "History Tab did not return from the preceding entry to Retry",
        )?;
        key(ui, " ".into())?;
        check(
            ui.get_dialog() == 21,
            "History Retry Space activated a different dialog action",
        )?;
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .settled(ui)?;
        check(
            ui.get_history_error().is_empty() && ui.get_history_first_focused(),
            "successful native History Retry lost its visible focus anchor",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(!ui.get_modal(), "History Escape did not restore browsing")?;
    } else if index == 125 {
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .another_error(ui)?;
    } else if index == 126 {
        check(
            ui.get_history_retry_focused(),
            "History repeated Retry lost focus",
        )?;
        key(ui, " ".into())?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_close_focused(),
            "History Tab did not move from Retry to Close",
        )?;
        surfaces
            .history
            .borrow()
            .as_ref()
            .ok_or("History surface")?
            .settled(ui)?;
        check(
            ui.get_history_error().is_empty() && ui.get_close_focused(),
            "History Retry completion stole the user's later Close focus",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(
            !ui.get_modal(),
            "History late-focus Escape did not restore browsing",
        )?;
        surfaces.history.borrow_mut().take();
    } else if index == 127 {
        resize(ui, 1180., 760.)?;
        *surfaces.maintenance.borrow_mut() = Some(callback_checks::MaintenanceSurface::open(ui)?);
    } else if index == 128 {
        check(
            ui.get_dialog() == 4 && ui.get_settings_first_focused(),
            "setup did not focus first native setting",
        )?;
        capture(ui, &destination.join("setup-wide-en.png"))?;
        key(ui, slint::platform::Key::Tab.into())?;
        key(ui, " ".into())?;
        surfaces
            .maintenance
            .borrow()
            .as_ref()
            .ok_or("settings surface")?
            .french(ui)?;
        resize(ui, 560., 400.)?;
    } else if index == 129 {
        capture(ui, &destination.join("setup-compact-fr.png"))?;
        surfaces
            .maintenance
            .borrow_mut()
            .as_mut()
            .ok_or("settings surface")?
            .confirmed(ui)?;
        resize(ui, 1180., 760.)?;
    } else if index == 130 {
        check(
            ui.get_dialog() == 2 && ui.get_settings_first_focused(),
            "setup completion left focus on a hidden confirmation",
        )?;
        capture(ui, &destination.join("settings-wide-fr.png"))?;
        surfaces
            .maintenance
            .borrow()
            .as_ref()
            .ok_or("settings surface")?
            .preview(ui)?;
    } else if index == 131 {
        capture(ui, &destination.join("location-preview-wide-fr.png"))?;
        ui.invoke_focus_settings_confirm();
        check(
            ui.get_settings_confirm_focused(),
            "move confirmation could not receive native focus",
        )?;
        key(ui, slint::platform::Key::Escape.into())?;
        check(!ui.get_modal(), "Settings Escape did not return to library")?;
        ui.invoke_show_panel(5);
        resize(ui, 560., 400.)?;
    } else if index == 132 {
        capture(ui, &destination.join("location-preview-compact-fr.png"))?;
        surfaces
            .maintenance
            .borrow_mut()
            .as_mut()
            .ok_or("settings surface")?
            .recover(ui)?;
        resize(ui, 1180., 760.)?;
    } else if index == 133 {
        capture(ui, &destination.join("recovery-wide-fr.png"))?;
        check(
            ui.get_settings_first_focused(),
            "Recovery did not begin on its first setting",
        )?;
        key(ui, slint::platform::Key::Tab.into())?;
        key(ui, slint::platform::Key::Tab.into())?;
        check(
            ui.get_settings_retry_focused(),
            "native Tab did not reach Recovery Retry from the language controls",
        )?;
        key(ui, " ".into())?;
    } else if index == 134 {
        capture(ui, &destination.join("recovery-retry-wide-fr.png"))?;
        resize(ui, 560., 400.)?;
    } else if index == 135 {
        capture(ui, &destination.join("recovery-compact-fr.png"))?;
    } else if index == 136 {
        surfaces.maintenance.borrow_mut().take();
    } else {
        std::fs::write(
            destination.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": 1, "host": std::env::consts::OS, "architecture": std::env::consts::ARCH,
                "frames": *report.borrow(), "keyboard_checks": "passed",
                "macos_programmatic_fixed_window": cfg!(target_os = "macos"),
                "native_menu_composition": if cfg!(target_os = "macos") { "passed" } else { "not-applicable" },
                "search_paging_callbacks": "passed",
                "history_restoration_callbacks": "passed",
                "failed_query_history_callbacks": "passed",
                "filter_callbacks": "passed",
                "details_removal_callbacks": "passed",
                "tag_callbacks": "passed",
                "flag_callbacks": "passed",
                "collection_callbacks": "passed",
                "shelf_callbacks": "passed",
                "shelf_review_callbacks": "passed",
                "operation_callbacks":"passed",
                "sync_callbacks":"passed",
                "export_callbacks":"passed",
                "activity_history_callbacks":"passed",
                "maintenance_callbacks":"passed",
                "scope": "shown native component/public window events and Skia capture; not physical HID or packaged workflow qualification",
            }))?,
        )?;
        let _quit = slint::quit_event_loop();
    }
    Ok(())
}

fn native_window_state(ui: &DesktopWindow) -> Result<serde_json::Value> {
    let (resizable, size, monitor) = ui
        .window()
        .with_winit_window(|window| {
            (
                window.is_resizable(),
                window.inner_size(),
                window
                    .current_monitor()
                    .map(|monitor| (monitor.size(), monitor.scale_factor())),
            )
        })
        .ok_or("native fixture has no Winit window")?;
    let (monitor_size, monitor_scale) = monitor.ok_or("native fixture has no current monitor")?;
    check(
        !cfg!(target_os = "macos") || !resizable,
        "macOS content fixture unexpectedly permits pointer resizing",
    )?;
    Ok(serde_json::json!({
        "resizable": resizable, "width": size.width, "height": size.height,
        "monitor_width": monitor_size.width, "monitor_height": monitor_size.height,
        "monitor_scale": monitor_scale,
    }))
}
fn proposals(ui: &DesktopWindow, french: bool) {
    ui.set_proposals(ModelRc::new(VecModel::from(vec![
        ProposalRow {
            title: "The next adventure".into(),
            warning: if french {
                "Source : Collection"
            } else {
                "Collection source"
            }
            .into(),
            decision: 0,
        },
        ProposalRow {
            title: "A changed source".into(),
            warning: if french {
                "Aucun retrait sans approbation"
            } else {
                "Nothing removed without approval"
            }
            .into(),
            decision: 0,
        },
    ])));
}

fn resize(ui: &DesktopWindow, width: f32, height: f32) -> Result {
    #[cfg(target_os = "macos")]
    {
        // AppKit shrinks resizable titled windows to the CI display's height.
        // This content-snapshot fixture uses programmatic resizing, so retain
        // the title bar but allow its native backing surface beyond the screen.
        ui.window()
            .with_winit_window(|window| window.set_resizable(false))
            .ok_or("native fixture has no Winit window")?;
    }
    // Winit logical sizing follows the OS backing scale, while an explicit
    // SLINT_SCALE_FACTOR changes rendering. Set the observed physical size so
    // the fixture's intended render-space dimensions are truthful on both.
    let scale = ui.window().scale_factor();
    let requested = slint::PhysicalSize::new(
        (width * scale).round() as u32,
        (height * scale).round() as u32,
    );
    REQUESTED_SIZE.with(|size| size.set(Some((requested.width, requested.height))));
    ui.window().set_size(requested);
    Ok(())
}
