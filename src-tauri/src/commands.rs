//! Everything the webview may call.

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;

use crate::{
    error::{AppError, AppResult},
    settings::{Settings, SettingsStore},
    shell,
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
    if before.taskbar_mode != settings.taskbar_mode && !shell::using_windows_taskbar(&app) {
        shell::taskbar::apply_system(settings.taskbar_mode);
    }
    if before.auto_hide != settings.auto_hide {
        // Turning auto-hide on tucks the docks away; off brings them out for good.
        shell::reset_revealed(&app)?;
    }
    let screens_changed = before.screen != settings.screen || before.docks_on != settings.docks_on;
    if screens_changed || before.edge != settings.edge {
        // Docks may move, appear or go (new places, strips, wallpapers and app lists), or
        // claim the other edge. Off this (main) thread, since it may create windows.
        let app = app.clone();
        std::thread::spawn(move || shell::relayout(&app));
    } else {
        shell::place_dock(&app)?;
    }
    crate::widgets::service::settings_changed(&app, &before, &settings);
    crate::system::service::settings_changed(&app);
    app.emit(SETTINGS_CHANGED, &settings)?;
    Ok(settings)
}

/// The cursor arrived on (true) or left (false) the calling dock while it auto-hides.
#[tauri::command]
pub fn dock_reveal(app: AppHandle, window: WebviewWindow, revealed: bool) -> AppResult<()> {
    shell::set_revealed(&app, window.label(), revealed)
}

/// Whether the calling dock's page should draw the dock rather than the auto-hide sliver.
#[tauri::command]
pub fn dock_is_revealed(app: AppHandle, window: WebviewWindow) -> bool {
    shell::is_revealed(&app, window.label())
}

/// The calling dock's page measured its pill's content; resize the dock to fit it.
#[tauri::command]
pub fn dock_set_content_width(app: AppHandle, window: WebviewWindow, width: f64) -> AppResult<()> {
    if !width.is_finite() || width < 0.0 {
        return Err(AppError::InvalidInput("content width".into()));
    }
    shell::set_content_width(&app, window.label(), width)
}

/// Open settings, at `section` if given. Async: creating a window from the main thread can
/// deadlock WebView2.
#[tauri::command(async)]
pub fn open_settings(app: AppHandle, section: Option<String>) -> AppResult<()> {
    shell::open_settings_window(&app, section.as_deref())
}

/// The screens the dock can be put on.
#[tauri::command]
pub fn screens_list(app: AppHandle) -> AppResult<Vec<shell::ScreenInfo>> {
    shell::screens(&app)
}

/// Set the show/hide shortcut; it's registered first, so a taken or invalid one isn't saved.
#[tauri::command]
pub fn shortcut_set(app: AppHandle, shortcut: String) -> AppResult<Settings> {
    let store = app.state::<SettingsStore>();
    if let Err(error) = shell::shortcut::apply(&app, &shortcut) {
        // Put the working one back.
        let _ = shell::shortcut::apply(&app, &store.get().toggle_shortcut);
        return Err(error);
    }
    let mut settings = store.get();
    settings.toggle_shortcut = shortcut.trim().to_string();
    store.set(settings.clone())?;
    app.emit(SETTINGS_CHANGED, &settings)?;
    Ok(store.get())
}

/// The welcome is finished (or skipped): don't show it again.
#[tauri::command(async)]
pub fn onboarding_done(app: AppHandle, open_settings: bool) -> AppResult<()> {
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    settings.onboarded = true;
    store.set(settings.clone())?;
    app.emit(SETTINGS_CHANGED, &settings)?;
    shell::close_welcome_window(&app)?;
    if open_settings {
        shell::open_settings_window(&app, None)?;
    }
    Ok(())
}

/// The apps on the calling dock.
#[tauri::command]
pub fn apps_list(app: AppHandle, window: WebviewWindow) -> Vec<crate::apps::DockApp> {
    crate::apps::service::list(&app, window.label())
}

/// An app's icon as a data URL (none if it has no icon).
#[tauri::command(async)]
pub fn app_icon(app: AppHandle, source: String) -> Option<String> {
    crate::apps::service::icon(&app, &source)
}

#[tauri::command(async)]
pub fn app_click(app: AppHandle, window: WebviewWindow, key: String) -> AppResult<()> {
    crate::apps::service::click(&app, window.label(), &key)
}

#[tauri::command(async)]
pub fn app_launch(app: AppHandle, key: String) -> AppResult<()> {
    crate::apps::service::launch(&app, &key)
}

/// An app's right-click menu, centred on `anchor_x` (screen pixels). Async: the first open
/// creates the popup window, and reading jump lists takes a moment.
#[tauri::command(async)]
pub fn app_menu(app: AppHandle, window: WebviewWindow, key: String, anchor_x: i32) -> AppResult<()> {
    crate::apps::service::show_menu(&app, window.label(), &key, anchor_x)
}

/// A choice in the open app menu.
#[tauri::command(async)]
pub fn app_menu_action(app: AppHandle, action: crate::apps::jump::MenuAction) -> AppResult<()> {
    crate::apps::service::menu_action(&app, action)
}

/// Open the Start menu (as the Windows key does).
#[tauri::command(async)]
pub fn shell_start() {
    #[cfg(windows)]
    crate::apps::win::press(&[windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN]);
}

/// Open Windows search (Win+S).
#[tauri::command(async)]
pub fn shell_search() {
    #[cfg(windows)]
    crate::apps::win::press(&[
        windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN,
        windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(u16::from(b'S')),
    ]);
}

/// Where the calling dock sits on its screen (for drawing the glass).
#[tauri::command]
pub fn dock_geometry(app: AppHandle, window: WebviewWindow) -> Option<shell::DockGeometry> {
    shell::geometry_of(&app, window.label())
}

/// The desktop picture behind the calling dock's glass, if it has been read.
#[tauri::command]
pub fn glass_get(app: AppHandle, window: WebviewWindow) -> Option<crate::glass::GlassFrame> {
    crate::glass::service::current(&app, window.label())
}

/// Open a widget's popup at `anchor_x` (the widget's centre, screen pixels), or close it if
/// it is already open. Async: the first open creates a window.
#[tauri::command(async)]
pub fn panel_toggle(app: AppHandle, kind: crate::settings::WidgetKind, anchor_x: i32) -> AppResult<()> {
    shell::panel::toggle(&app, kind, anchor_x)
}

/// Which widget's popup is open (for the popup page when it first loads).
#[tauri::command]
pub fn panel_current(app: AppHandle) -> Option<shell::panel::PanelContent> {
    shell::panel::current(&app)
}

#[tauri::command]
pub fn panel_close(app: AppHandle) -> AppResult<()> {
    shell::panel::close(&app)
}

/// Everything the widgets show, for a page that just opened.
#[tauri::command]
pub fn widgets_snapshot(app: AppHandle) -> crate::widgets::WidgetSnapshot {
    crate::widgets::service::snapshot(&app)
}

#[tauri::command]
pub fn volume_set(app: AppHandle, level: f32) -> AppResult<()> {
    if !level.is_finite() {
        return Err(AppError::InvalidInput("volume".into()));
    }
    crate::widgets::service::set_volume(&app, level);
    Ok(())
}

/// Play/pause, next, previous or seek. Async: the media app may take a moment to answer.
#[tauri::command(async)]
pub fn media_command(app: AppHandle, command: crate::media::MediaCommand) -> AppResult<()> {
    crate::widgets::service::media_command(&app, command)
}

#[tauri::command]
pub fn volume_toggle_mute(app: AppHandle) {
    crate::widgets::service::toggle_mute(&app);
}

#[tauri::command]
pub fn focus_start(app: AppHandle) {
    crate::widgets::service::focus_update(&app, |s| s.start(chrono::Utc::now()));
}

#[tauri::command]
pub fn focus_pause(app: AppHandle) {
    crate::widgets::service::focus_update(&app, |s| s.pause(chrono::Utc::now()));
}

#[tauri::command]
pub fn focus_reset(app: AppHandle) {
    crate::widgets::service::focus_update(&app, |s| s.reset());
}

/// Open one of the few Windows Settings pages the widgets link to.
#[tauri::command(async)]
pub fn open_windows_settings(page: String) -> AppResult<()> {
    let uri = match page.as_str() {
        "location" => "ms-settings:privacy-location",
        "network" => "ms-settings:network",
        "sound" => "ms-settings:sound",
        "battery" => "ms-settings:batterysaver",
        "bluetooth" => "ms-settings:bluetooth",
        "home" => "ms-settings:",
        "datetime" => "ms-settings:dateandtime",
        "update" => "ms-settings:windowsupdate",
        "display" => "ms-settings:display",
        _ => return Err(AppError::InvalidInput("settings page".into())),
    };
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(uri.into()));
    #[cfg(not(windows))]
    let _ = uri;
    Ok(())
}

/// Wi-Fi, Bluetooth and brightness, for the status popup. Async: the radio and WMI calls
/// block for a moment.
#[tauri::command(async)]
pub fn quick_get() -> crate::quick::QuickState {
    crate::quick::current()
}

#[tauri::command(async)]
pub fn quick_set_radio(kind: crate::quick::RadioKind, on: bool) -> AppResult<crate::quick::QuickState> {
    crate::quick::set_radio(kind, on)?;
    Ok(crate::quick::current())
}

#[tauri::command(async)]
pub fn quick_set_brightness(level: u8) -> AppResult<()> {
    crate::quick::set_brightness(level)
}

/// Windows' own Quick Settings (Win+A), for the toggles only Windows can switch.
#[tauri::command(async)]
pub fn quick_open_windows(app: AppHandle) -> AppResult<()> {
    shell::panel::close(&app)?;
    #[cfg(windows)]
    crate::apps::win::press(&[
        windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN,
        windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(u16::from(b'A')),
    ]);
    Ok(())
}

/// The reminder cards showing now.
#[tauri::command]
pub fn bubbles_get(app: AppHandle) -> Vec<crate::automations::Bubble> {
    crate::automations::service::bubbles(&app)
}

#[tauri::command]
pub fn bubble_dismiss(app: AppHandle, id: String) {
    crate::automations::service::dismiss(&app, &id);
}

/// Claude usage, estimated from this PC's Claude Code transcripts.
#[tauri::command]
pub fn claude_usage(app: AppHandle) -> crate::claude::ClaudeUsage {
    crate::claude::service::current(&app)
}

#[tauri::command]
pub fn calendar_get(app: AppHandle) -> crate::calendar::service::CalendarState {
    crate::calendar::service::current(&app)
}

/// Add a calendar by its secret iCal link; the link is checked first and then kept in Windows
/// Credential Manager.
#[tauri::command]
pub async fn calendar_add(app: AppHandle, name: String, link: String) -> AppResult<()> {
    crate::calendar::service::add(&app, &name, &link).await
}

#[tauri::command(async)]
pub fn calendar_remove(app: AppHandle, id: String) -> AppResult<()> {
    crate::calendar::service::remove(&app, &id)
}

/// Hide the docks and bring back the Windows taskbar; the tray icon or the shortcut switch back.
#[tauri::command]
pub fn use_windows_taskbar(app: AppHandle) {
    shell::use_windows_taskbar(&app, true);
}

/// Quit the dock; the exit handler puts the Windows taskbar back.
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    app.exit(0);
}

// ---------- System widgets ----------

/// The programs using the most processor and memory. Async: the first reading takes half a
/// second.
#[tauri::command(async)]
pub fn top_apps() -> crate::widgets::stats::TopApps {
    crate::widgets::stats::top_apps(6)
}

/// End a program's processes (the popup asks first).
#[tauri::command(async)]
pub fn process_end(pids: Vec<u32>, name: String) -> AppResult<()> {
    crate::widgets::stats::end_task(&pids, &name)
}

#[tauri::command(async)]
pub fn bluetooth_devices() -> Vec<crate::system::bluetooth::BluetoothDevice> {
    crate::system::bluetooth::list()
}

/// Every screen's DDC/CI brightness. Async: each screen takes a moment to answer.
#[tauri::command(async)]
pub fn monitors_brightness() -> Vec<crate::system::brightness::MonitorBrightness> {
    crate::system::brightness::list()
}

#[tauri::command(async)]
pub fn monitor_set_brightness(id: String, percent: u8) -> AppResult<()> {
    crate::system::brightness::set(&id, percent)
}

#[tauri::command(async)]
pub fn wifi_details() -> crate::system::wifi::WifiDetails {
    crate::system::wifi::read()
}

/// The public address, from one request (the Wi-Fi popup asks when it opens).
#[tauri::command]
pub async fn public_ip() -> Option<String> {
    crate::system::wifi::public_ip().await
}

#[tauri::command(async)]
pub fn uptime_get() -> crate::system::uptime::UptimeState {
    crate::system::uptime::read()
}

#[tauri::command(async)]
pub fn dev_servers() -> Vec<crate::system::devservers::DevServer> {
    crate::system::devservers::list()
}

/// Stop the program serving `port` (the popup asks first).
#[tauri::command(async)]
pub fn dev_server_stop(port: u16, pid: u32) -> AppResult<()> {
    crate::system::devservers::stop(port, pid)
}

/// Open a web page (https, or http on this PC) in the default browser.
#[tauri::command(async)]
pub fn open_url(url: String) -> AppResult<()> {
    crate::system::service::open_url(&url)
}

#[tauri::command(async)]
pub fn desktop_icons_get() -> Option<bool> {
    crate::system::desktop::icons_shown()
}

#[tauri::command(async)]
pub fn desktop_icons_set(show: bool) -> AppResult<bool> {
    crate::system::desktop::set_icons_shown(show)
}

/// Turn the screens off; the popup closes first.
#[tauri::command(async)]
pub fn screen_off(app: AppHandle) -> AppResult<()> {
    shell::panel::close(&app)?;
    crate::system::desktop::screens_off();
    Ok(())
}

#[tauri::command(async)]
pub fn wallpaper_get(app: AppHandle) -> crate::system::wallpaper::WallpaperState {
    crate::system::wallpaper::state(&app.state::<SettingsStore>().get().wallpaper_folder)
}

#[tauri::command(async)]
pub fn wallpaper_next(app: AppHandle) -> AppResult<crate::system::wallpaper::WallpaperState> {
    crate::system::service::next_wallpaper(&app)
}

/// Windows' folder picker; the chosen folder, or `None` if cancelled. The page saves it.
#[tauri::command(async)]
pub fn wallpaper_pick_folder(app: AppHandle) -> Option<String> {
    crate::system::wallpaper::pick_folder(&app.state::<SettingsStore>().get().wallpaper_folder)
}

#[tauri::command(async)]
pub fn display_get() -> crate::system::display::DisplayState {
    crate::system::display::read()
}

#[tauri::command(async)]
pub fn display_set(mode: crate::system::display::DisplayMode) -> AppResult<()> {
    crate::system::display::set(mode)
}

#[tauri::command(async)]
pub fn usb_drives() -> Vec<crate::system::usb::UsbDrive> {
    crate::system::usb::list()
}

#[tauri::command(async)]
pub fn usb_eject(letter: String) -> AppResult<crate::system::usb::EjectResult> {
    crate::system::usb::eject(&letter)
}

/// Today's trending GitHub repositories (read at most hourly; the last list when offline).
#[tauri::command]
pub async fn github_trending(app: AppHandle) -> crate::system::trending::Trending {
    crate::system::service::trending(&app).await
}

/// A reminder card's Open button (download cards).
#[tauri::command(async)]
pub fn bubble_open(app: AppHandle, id: String) -> AppResult<()> {
    crate::system::service::open_from_bubble(&app, &id)
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
