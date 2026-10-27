//! Native presentation fixtures use only explicitly owned synthetic library data.
#[path = "native-foundations/presentation.rs"]
mod callback_checks;
#[cfg(target_os = "macos")]
#[path = "native-foundations/native_menu.rs"]
mod native_menu;
#[allow(dead_code)]
#[path = "../src/view.rs"]
mod view;
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
use slint::{ComponentHandle, ModelRc, VecModel, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::Duration,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn check(ok: bool, message: &str) -> Result {
    if ok { Ok(()) } else { Err(message.into()) }
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
        Story { uuid: summary.uuid.clone(), title, summary: Some(summary), thumbnail: Some(thumbnail(i as usize)), provenance: "Weekend favorites".into(), tags: vec![
            librairii_core::tagging::DisplayTag { source: "user".into(), definition: "Adventure".into(), value: None, color: "#405cf5".into() },
            librairii_core::tagging::DisplayTag { source: "derived".into(), definition: "Age".into(), value: Some("3–5 years".into()), color: "#286350".into() },
        ] }
    }).collect();
    let gate = LifecycleGate::new();
    let generation = gate.enter()?.generation().clone();
    Ok(Snapshot {
        revision: 0,
        restore_navigation: true,
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
            stories,
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
    image::save_buffer(
        path,
        pixels.as_bytes(),
        pixels.width(),
        pixels.height(),
        image::ColorType::Rgba8,
    )?;
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
    callback_checks::paging(&ui)?;
    let initial = snapshot()?;
    let view = Rc::new(RefCell::new(view::View {
        catalogs: librairii_app::localization::Catalogs::embedded()?,
        snapshot: initial,
        selected: Some("f0000000-0000-4000-8000-000000000000".into()),
        columns: 1,
        guard: Default::default(),
    }));
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
    let server = std::env::args().any(|arg| arg == "--ax-server");
    let step = Rc::new(RefCell::new(0usize));
    let errors = Rc::new(RefCell::new(None::<String>));
    let report = Rc::new(RefCell::new(Vec::<serde_json::Value>::new()));
    let timer = slint::Timer::default();
    let weak = ui.as_weak();
    let captured_errors = errors.clone();
    let captured_report = report.clone();
    let destination = output.clone();
    let fixture_step = step.clone();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
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
            let result = (|| -> Result {
                menus.dispatch(&ui);
                #[cfg(target_os = "macos")]
                check(
                    menus.owns_native_menu(),
                    "native application menu owner changed",
                )?;
                view.borrow_mut().resize(&ui);
                if server {
                    return Ok(());
                }
                capture_step(
                    &ui,
                    &view,
                    *fixture_step.borrow(),
                    &destination,
                    &captured_report,
                    &menus,
                )?;
                *fixture_step.borrow_mut() += 1;
                Ok(())
            })();
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
        server || *step.borrow() > 57,
        "native fixture ended before completing its checks",
    )?;
    Ok(())
}

fn capture_step(
    ui: &DesktopWindow,
    view: &Rc<RefCell<view::View>>,
    index: usize,
    destination: &Path,
    report: &RefCell<Vec<serde_json::Value>>,
    _menus: &librairii_desktop::menus::Menus,
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
                    "workspace panes cannot display a full interaction target",
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
        for _ in 0..40 {
            if ui.get_focused_key() >= 220 {
                break;
            }
            key(ui, slint::platform::Key::Tab.into())?;
        }
        let group_invoker = ui.get_focused_key();
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
    } else if (54..=56).contains(&index) {
        let (phase, name, status) = match index {
            54 => (
                Phase::Recovery,
                "recovery",
                "La bibliothèque attendue est indisponible ou nécessite une récupération.",
            ),
            55 => (
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
    } else {
        std::fs::write(
            destination.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": 1, "host": std::env::consts::OS, "architecture": std::env::consts::ARCH,
                "frames": *report.borrow(), "keyboard_checks": "passed",
                "macos_programmatic_fixed_window": cfg!(target_os = "macos"),
                "native_menu_composition": if cfg!(target_os = "macos") { "passed" } else { "not-applicable" },
                "search_paging_callbacks": "passed",
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
    ui.window().set_size(slint::PhysicalSize::new(
        (width * scale).round() as u32,
        (height * scale).round() as u32,
    ));
    Ok(())
}
