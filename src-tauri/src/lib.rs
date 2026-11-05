mod apps;
mod automations;
mod calendar;
mod claude;
mod commands;
mod error;
mod glass;
mod logging;
mod media;
mod settings;
mod shell;
mod widgets;

use tauri::Manager;

use crate::{settings::SettingsStore, shell::ShellState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
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
            // Before anything touches the taskbar, so a panic from here on restores it.
            shell::taskbar::init(&handle.path().app_data_dir()?);

            let store = SettingsStore::open(&handle.path().app_config_dir()?);
            let launch_at_startup = store.get().launch_at_startup;
            app.manage(store);
            app.manage(ShellState::default());
            app.manage(apps::service::AppsState::default());
            app.manage(glass::service::GlassState::default());
            app.manage(shell::panel::PanelState::default());
            app.manage(widgets::service::WidgetsState::default());
            app.manage(calendar::service::CalendarService::default());
            app.manage(claude::service::ClaudeService::default());
            app.manage(automations::service::AutomationState::default());
            commands::sync_autostart(handle, launch_at_startup);

            shell::init(handle)?;
            apps::service::init(handle);
            glass::service::refresh_soon(handle);
            widgets::service::init(handle);
            calendar::service::init(handle);
            claude::service::init(handle);
            automations::service::init(handle);
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "dock started");
            Ok(())
        })
        // The dock only quits from the tray or Settings, never from Alt+F4 or WM_CLOSE: quitting
        // must go through the exit path that restores the taskbar.
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } if window.label() == shell::DOCK_LABEL => {
                api.prevent_close();
            }
            // A popup closes as soon as you click anywhere else.
            tauri::WindowEvent::Focused(false) if window.label() == shell::panel::PANEL_LABEL => {
                if let Err(error) = shell::panel::close(window.app_handle()) {
                    tracing::warn!(%error, "failed to close the popup");
                }
            }
            _ => {}
        })
        // App menus (right-click on an app); the tray handles its own items.
        .on_menu_event(|app, event| apps::service::on_menu(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set,
            commands::dock_set_content_width,
            commands::dock_reveal,
            commands::dock_is_revealed,
            commands::open_settings,
            commands::apps_list,
            commands::app_icon,
            commands::app_click,
            commands::app_launch,
            commands::app_menu,
            commands::shell_start,
            commands::shell_search,
            commands::dock_geometry,
            commands::glass_get,
            commands::panel_toggle,
            commands::panel_current,
            commands::panel_close,
            commands::widgets_snapshot,
            commands::volume_set,
            commands::volume_toggle_mute,
            commands::media_command,
            commands::calendar_get,
            commands::claude_usage,
            commands::bubbles_get,
            commands::bubble_dismiss,
            commands::calendar_add,
            commands::calendar_remove,
            commands::focus_start,
            commands::focus_pause,
            commands::focus_reset,
            commands::open_windows_settings,
            commands::app_quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app, event| {
        // Quit from the tray, or the last window closing: the taskbar comes back.
        if let tauri::RunEvent::Exit = event {
            shell::taskbar::restore_system();
            tracing::info!("dock stopped; taskbar restored");
        }
    });
}
