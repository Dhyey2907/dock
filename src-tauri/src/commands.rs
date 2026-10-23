//! Everything the webview may call.

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    error::{AppError, AppResult},
    settings::{Settings, SettingsStore},
    shell::{self, ShellState},
};

/// Event sent to every window after settings change.
pub const SETTINGS_CHANGED: &str = "settings-changed";

#[tauri::command]
pub fn settings_get(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
pub fn settings_set(app: AppHandle, settings: Settings) -> AppResult<Settings> {
    let store = app.state::<SettingsStore>();
    let before = store.get();
    store.set(settings.clone())?;
    if before.launch_at_startup != settings.launch_at_startup {
        sync_autostart(&app, settings.launch_at_startup);
    }
    if before.taskbar_mode != settings.taskbar_mode {
        shell::taskbar::apply_system(settings.taskbar_mode);
    }
    if before.auto_hide != settings.auto_hide {
        // Turning auto-hide on tucks the dock away; off brings it out for good.
        shell::set_revealed(&app, false)?;
        app.emit(shell::DOCK_REVEALED, shell::is_revealed(&app))?;
    }
    if before.edge != settings.edge {
        shell::relayout(&app);
    } else {
        shell::place_dock(&app)?;
    }
    app.emit(SETTINGS_CHANGED, &settings)?;
    Ok(settings)
}

/// The cursor arrived on (true) or left (false) an auto-hiding dock.
#[tauri::command]
pub fn dock_reveal(app: AppHandle, revealed: bool) -> AppResult<()> {
    shell::set_revealed(&app, revealed)
}

/// Whether the page should draw the dock rather than the auto-hide sliver.
#[tauri::command]
pub fn dock_is_revealed(app: AppHandle) -> bool {
    shell::is_revealed(&app)
}

/// The page measured the pill's content; resize the dock to fit it.
#[tauri::command]
pub fn dock_set_content_width(app: AppHandle, width: f64) -> AppResult<()> {
    if !width.is_finite() || width < 0.0 {
        return Err(AppError::InvalidInput("content width".into()));
    }
    *app.state::<ShellState>()
        .content_width
        .lock()
        .expect("shell lock") = width;
    shell::place_dock(&app)
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> AppResult<()> {
    shell::open_settings_window(&app)
}

/// Quit the dock; the exit handler puts the Windows taskbar back.
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    app.exit(0);
}

/// Make Windows' "run at sign-in" entry match the setting.
pub fn sync_autostart(app: &AppHandle, enabled: bool) {
    // Dev builds would register the debug exe; only release builds touch sign-in.
    if cfg!(debug_assertions) {
        return;
    }
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    if let Err(error) = result {
        tracing::warn!(%error, enabled, "failed to update launch at startup");
    }
}
