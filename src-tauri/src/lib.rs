mod backend;

use std::time::Duration;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application = tauri::Builder::default()
        .setup(|app| {
            let runtime = backend::RuntimeConfig::new(app.path().app_data_dir()?)?;

            #[cfg(debug_assertions)]
            let backend = backend::BackendProcess::launch_development(runtime.clone())?;

            #[cfg(not(debug_assertions))]
            let backend = backend::BackendProcess::launch_packaged(
                runtime.clone(),
                &app.path().resource_dir()?,
            )?;

            backend.wait_until_healthy(Duration::from_secs(20))?;

            let backend_port = runtime.port;
            tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::External(runtime.launch_url().parse()?),
            )
            .title("Librairii")
            .inner_size(1440.0, 900.0)
            .min_inner_size(1100.0, 720.0)
            .center()
            .on_navigation(move |url| {
                url.scheme() == "http"
                    && url.host_str() == Some("127.0.0.1")
                    && url.port() == Some(backend_port)
            })
            .build()?;

            app.manage(backend);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Tauri application");

    application.run(|app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
        ) {
            app_handle.state::<backend::BackendProcess>().stop();
        }
    });
}
