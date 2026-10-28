//! Current production worker/view/menu composition on an owned 1,000-story fixture.
#![forbid(unsafe_code)]
#[path = "scale-native/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../src/view.rs"]
mod view;
use librairii_core::OperationState;
use librairii_desktop::{
    menus,
    presentation::{Command, Phase, Worker},
};
use serde_json::json;
use slint::{ComponentHandle, Model, winit_030::WinitWindowAccessor};
use std::{
    cell::RefCell,
    fs,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn atomic(path: &Path, bytes: impl AsRef<[u8]>) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}
fn run(configuration: &Path, output: PathBuf) -> Result {
    let config = fixture::read(configuration)?;
    if !output.is_dir() || fs::read(output.join(".native-scale"))? != b"native-scale-16\n" {
        return Err("owned observer output required".into());
    }
    let worker = Rc::new(RefCell::new(Worker::start_with(config.clone())?));
    let first = loop {
        if let Some(s) = worker.borrow().take_snapshot() {
            break s;
        }
        if worker.borrow().finished() {
            return Err("startup worker stopped".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let (ui, menus) = menus::create_window()?;
    let view = view::install(&ui, worker.clone(), first)?;
    let closing = worker.clone();
    ui.on_quit(move || closing.borrow().request_shutdown());
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade() {
            ui.invoke_request_quit();
        }
        slint::CloseRequestResponse::KeepWindowShown
    });
    let weak = ui.as_weak();
    let pending = worker.clone();
    let timer = slint::Timer::default();
    let error = Rc::new(RefCell::new(None::<String>));
    let failure = error.clone();
    let mut ready = false;
    let mut imports = 0usize;
    let mut offered_at = Instant::now();
    let mut last_page_command = None;
    let mut max_rows = 0;
    let mut observed_covers = std::collections::BTreeSet::new();
    let mut first_cover = None;
    let mut evicted = false;
    let mut first_cover_reloads = 0usize;
    let started = Instant::now();
    let mut last_capture = 0;
    let mut previous_state = Vec::new();
    let mut geometry_sequence = 0u64;
    timer.start(slint::TimerMode::Repeated,Duration::from_millis(16),move||{
      let result=(||->Result{
        let Some(ui)=weak.upgrade() else{return Err("window disappeared".into())};
        if !ui.window().has_winit_window() {return Ok(())}
        view.borrow_mut().resize(&ui);menus.dispatch(&ui);
        if let Some(s)=pending.borrow().take_snapshot(){view.borrow_mut().accept(&ui,s,pending.borrow().revision());}
        let snapshot=&view.borrow().snapshot;
        if snapshot.phase==Phase::Recovery{return Err(format!("scale observer entered recovery: {}",snapshot.status).into())}
        if !ready && snapshot.phase==Phase::Library && snapshot.page.as_ref().is_some_and(|p|p.total==1000){
            ready=true;ui.invoke_focus_search();atomic(&output.join("ready"),b"ready\n")?;
            println!("scale_ready; pid={}; preparation_excluded=true",std::process::id());
        }
        let page=snapshot.page.as_ref();
        let rows=ui.get_story_groups().iter().map(|g|g.cards.row_count()).sum::<usize>();max_rows=max_rows.max(rows);
        if rows>24 || page.is_some_and(|p|p.stories.len()>24){return Err("unbounded native story model".into())}
        if let Some(p)=page {
            for story in &p.stories {if let Some(t)=&story.thumbnail {
                observed_covers.insert(story.uuid.clone());
                if story.summary.as_ref().is_some_and(|s|s.id==1) {
                    if let Some(old)=&first_cover {if observed_covers.len()>128 && !std::sync::Arc::ptr_eq(old,&t.rgba){evicted=true;first_cover_reloads+=1;first_cover=Some(t.rgba.clone());}}
                    else {first_cover=Some(t.rgba.clone());}
                }
            }}
        }
        let running=snapshot.workflows.operations.cards.iter().any(|c|matches!(c.operation.state,OperationState::Running|OperationState::Queued));
        let loading=output.join("start-load").exists();
        if loading && ready && !running && snapshot.workflows.operations.preparing.is_none() && offered_at.elapsed()>Duration::from_millis(250) && imports<64 && let Some(generation)=snapshot.generation.clone() {pending.borrow().offer(Command::Operations{generation,action:librairii_desktop::workflows::operations::Action::Import(config.imports.clone())})?;imports+=1;offered_at=Instant::now();
        }
        if let Ok(text)=fs::read_to_string(output.join("page")) {
            let requested=text.trim().parse::<i64>()?;
            if !(1..=42).contains(&requested){return Err("bounded qualification page required".into())}
            if last_page_command!=Some(requested) {
                let request=librairii_core::query::QueryRequest{page:requested,..Default::default()};
                pending.borrow().offer(Command::Request(request))?;last_page_command=Some(requested);
            }
        }
        if let Ok(bytes)=fs::read(output.join("geometry")) {
            let request:serde_json::Value=serde_json::from_slice(&bytes)?;
            let sequence=request["sequence"].as_u64().ok_or("geometry sequence")?;
            if sequence>geometry_sequence {
                let width=request["width"].as_u64().filter(|v|(560..=1920).contains(v)).ok_or("geometry width")? as u32;
                let height=request["height"].as_u64().filter(|v|(400..=1080).contains(v)).ok_or("geometry height")? as u32;
                #[cfg(target_os="macos")]
                ui.window().with_winit_window(|w|w.set_resizable(false));
                let scale=ui.window().scale_factor();
                ui.window().set_size(slint::PhysicalSize::new((width as f32*scale).round() as u32,(height as f32*scale).round() as u32));
                geometry_sequence=sequence;
            }
        }
        let sequence=fs::read_to_string(output.join("capture")).ok().and_then(|s|s.trim().parse::<u32>().ok()).unwrap_or(0);
        if sequence>last_capture {
            let pixels=ui.window().take_snapshot()?;use image::ImageEncoder;
            image::codecs::png::PngEncoder::new(fs::File::create(output.join(format!("capture-{sequence}.png")))?).write_image(pixels.as_bytes(),pixels.width(),pixels.height(),image::ExtendedColorType::Rgba8)?;last_capture=sequence;
        }
        let native=ui.window().with_winit_window(|w|json!({"scale":w.scale_factor(),"width":w.inner_size().width,"height":w.inner_size().height,"monitor":w.current_monitor().map(|m|json!({"name":m.name(),"scale":m.scale_factor(),"width":m.size().width,"height":m.size().height}))})).ok_or("native Winit window missing")?;
        let state=json!({"pid":std::process::id(),"slint_scale":ui.window().scale_factor(),"logical_width":ui.window().size().to_logical(ui.window().scale_factor()).width,"logical_height":ui.window().size().to_logical(ui.window().scale_factor()).height,"geometry_sequence":geometry_sequence,"query":ui.get_query().as_str(),"query_focused":ui.get_query_focused(),"focus_key":ui.get_focused_key(),"rows":rows,"max_rows":max_rows,"total":page.map(|p|p.total),"page":page.map(|p|p.page),"pages":page.map(|p|p.pages),"selected":ui.get_selected_story().uuid.as_str(),"selected_index":ui.get_selected_index(),"revision":snapshot.revision,"settled":snapshot.revision==pending.borrow().revision(),"running_import":running,"import_batches":imports,"loaded_covers":observed_covers.len(),"first_cover_evicted":evicted,"first_cover_reloads":first_cover_reloads,"native":native,"capture":last_capture,"browser_height":ui.get_observed_browser_height(),"row_height":ui.get_observed_story_row_height(),"french":ui.get_french()});
        let encoded=serde_json::to_vec(&state)?;if encoded!=previous_state {atomic(&output.join("state.json"),&encoded)?;previous_state=encoded;}
        if output.join("finish").exists(){pending.borrow().request_shutdown();}
        if started.elapsed()>Duration::from_secs(240){return Err("native scale observer deadline".into())}
        if pending.borrow().finished(){slint::quit_event_loop()?;}
        Ok(())
      })();
      if let Err(e)=result{*failure.borrow_mut()=Some(e.to_string());pending.borrow().request_shutdown();let _quit=slint::quit_event_loop();}
    });
    let shown = ui.run();
    timer.stop();
    let joined = worker.borrow_mut().finish();
    shown?;
    joined?;
    if let Some(message) = error.borrow_mut().take() {
        return Err(message.into());
    }
    println!("scale_complete; workers_joined=true");
    Ok(())
}
fn main() -> Result {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [mode, path] if mode == "--prepare" => fixture::prepare(Path::new(path)),
        [mode, configuration, output] if mode == "--run" => {
            menus::prepare_process()?;
            run(Path::new(configuration), PathBuf::from(output))
        }
        _ => Err("usage: scale-native --prepare CONFIG | --run CONFIG OUTPUT".into()),
    }
}
