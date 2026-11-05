mod apps;
mod automations;
mod calendar;
mod claude;
mod commands;
mod error;
mod glass;
mod logging;
mod media;
mod quick;
mod settings;
mod shell;
mod widgets;

use tauri::Manager;

use crate::{settings::SettingsStore, shell::ShellState};

/// Launch flag the installer uses: quit the running dock (or, if none is running, put back a
/// taskbar a crashed one left hidden) and exit.
const QUIT_FLAG: &str = "--quit";

fn wants_quit<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().skip(1).any(|a| a.as_ref() == QUIT_FLAG)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        // A second launch shows the running docks, or quits them when the installer asks.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if wants_quit(&args) {
                tracing::info!("asked to quit by a second launch");
                app.exit(0);
            } else {
                shell::show_docks(app);
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle();
            let log_guard = logging::init(&handle.path().app_log_dir()?);
            app.manage(log_guard);
            // Before anything touches the taskbar, so a panic from here on restores it.
            shell::taskbar::init(&handle.path().app_data_dir()?);
            if wants_quit(&std::env::args().collect::<Vec<_>>()) {
                // No dock was running (a running one would have taken the flag instead).
                shell::taskbar::restore_system();
                tracing::info!("started with --quit: taskbar restored, exiting");
                std::process::exit(0);
            }

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
            let settings = handle.state::<SettingsStore>().get();
            if let Err(error) = shell::shortcut::apply(handle, &settings.toggle_shortcut) {
                tracing::warn!(%error, "show/hide shortcut not set");
            }
            if !settings.onboarded {
                let handle = handle.clone();
                // Off the main thread: creating a window there can deadlock WebView2.
                std::thread::spawn(move || {
                    if let Err(error) = shell::open_welcome_window(&handle) {
                        tracing::warn!(%error, "could not show the welcome");
                    }
                });
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "dock started");
            Ok(())
        })
        // The dock only quits from the tray or Settings, never from Alt+F4 or WM_CLOSE: quitting
        // must go through the exit path that restores the taskbar.
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } if shell::is_dock_label(window.label()) => {
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
        .invoke_handler(tauri::generate_handler![
            commands::settings_get,
            commands::settings_set,
            commands::dock_set_content_width,
            commands::dock_reveal,
            commands::dock_is_revealed,
            commands::open_settings,
            commands::screens_list,
            commands::shortcut_set,
            commands::onboarding_done,
            commands::apps_list,
            commands::app_icon,
            commands::app_click,
            commands::app_launch,
            commands::app_menu,
            commands::app_menu_action,
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
            commands::use_windows_taskbar,
            commands::quick_get,
            commands::quick_set_radio,
            commands::quick_set_brightness,
            commands::quick_open_windows,
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

#[cfg(test)]
mod tests {
    use super::wants_quit;

    #[test]
    fn quit_flag_is_read_after_the_program_name() {
        assert!(wants_quit(&["dock.exe", "--quit"]));
        assert!(!wants_quit(&["dock.exe"]));
        assert!(!wants_quit(&["--quit"]));
    }
}
