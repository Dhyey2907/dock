mod commands;
mod error;
mod logging;
mod settings;
mod shell;

use tauri::Manager;

use crate::{settings::SettingsStore, shell::ShellState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // A second launch just shows the running dock.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = shell::dock_window(app) {
                let _ = shell::show_dock(&window);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let handle = app.handle();
            let log_guard = logging::init(&handle.path().app_log_dir()?);
            app.manage(log_guard);

            let store = SettingsStore::open(&handle.path().app_config_dir()?);
            let launch_at_startup = store.get().launch_at_startup;
            app.manage(store);
            app.manage(ShellState::default());
            commands::sync_autostart(handle, launch_at_startup);

            shell::init(handle)?;
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "dock started");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set,
            commands::dock_set_content_width,
            commands::open_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
