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
    store.set(settings)?;
    applied(&app, before, store.get())
}

/// Put back the widget layout and look from before the last change.
#[tauri::command]
pub fn settings_undo_layout(app: AppHandle) -> AppResult<Settings> {
    let store = app.state::<SettingsStore>();
    let before = store.get();
    match store.undo_layout()? {
        Some(settings) => applied(&app, before, settings),
        None => Ok(before),
    }
}

/// Open or close the widget gallery, centred on `anchor_x` (screen pixels).
#[tauri::command]
pub async fn overflow_toggle(
    app: AppHandle,
    kinds: Vec<crate::settings::WidgetKind>,
    anchor_x: i32,
) -> AppResult<()> {
    let kinds = kinds.into_iter().take(60).collect();
    shell::panel::toggle_overflow(&app, kinds, anchor_x)
}

/// Open or close the widget gallery above the dock's › button.
#[tauri::command]
pub async fn gallery_toggle(app: AppHandle, anchor_x: i32) -> AppResult<()> {
    shell::panel::toggle_gallery(&app, anchor_x)
}

/// Turn the open widget popup into a window that stays on top.
#[tauri::command]
pub async fn panel_detach(app: AppHandle) -> AppResult<()> {
    shell::floating::detach_popup(&app)
}

/// Show a widget in its own window, on top of other windows or on the desktop.
#[tauri::command]
pub async fn floating_show(app: AppHandle, kind: crate::settings::WidgetKind, on_top: bool) -> AppResult<()> {
    shell::floating::show(&app, kind, on_top)
}

/// A floating widget was dragged somewhere new (physical pixels).
#[tauri::command]
pub fn floating_moved(app: AppHandle, kind: crate::settings::WidgetKind, x: i32, y: i32) -> AppResult<()> {
    shell::floating::moved(&app, kind, x, y)
}

#[tauri::command]
pub fn floating_close(app: AppHandle, kind: crate::settings::WidgetKind) -> AppResult<()> {
    shell::floating::close(&app, kind)
}

#[tauri::command]
pub async fn dashboard_open(app: AppHandle) -> AppResult<()> {
    shell::floating::open_dashboard(&app)
}

/// Whether "Undo layout" has anything to undo.
#[tauri::command]
pub fn settings_can_undo(store: State<'_, SettingsStore>) -> bool {
    store.can_undo()
}

/// Act on a settings change that has been saved.
fn applied(app: &AppHandle, before: Settings, settings: Settings) -> AppResult<Settings> {
    let app = app.clone();
    if before.launch_at_startup != settings.launch_at_startup {
        sync_autostart(&app, settings.launch_at_startup);
    }
    let screens_changed = before.docks_on != settings.docks_on || before.screen != settings.screen;
    if screens_changed {
        shell::taskbar::set_dock_screen(&settings);
    }
    if (before.taskbar_mode != settings.taskbar_mode || screens_changed)
        && !shell::using_windows_taskbar(&app)
    {
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
    crate::apps::service::settings_changed(&app);
    if before.accounts != settings.accounts || before.widgets != settings.widgets {
        crate::accounts::github::refresh_soon(&app);
    }
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
    crate::actions::hotkey_set(app, shell::shortcut::HotkeyAction::Toggle, shortcut)
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

/// A left click on an app; `anchor_x` (screen pixels) is where a pinned folder's popup goes.
#[tauri::command(async)]
pub fn app_click(app: AppHandle, window: WebviewWindow, key: String, anchor_x: i32) -> AppResult<()> {
    crate::apps::service::click(&app, window.label(), &key, anchor_x)
}

/// A file dragged over an app's icon for a moment: bring its window forward.
#[tauri::command(async)]
pub fn app_raise(app: AppHandle, window: WebviewWindow, key: String) -> AppResult<()> {
    crate::apps::service::raise(&app, window.label(), &key)
}

/// An app's icon is being dragged off the dock (its window snaps where it is let go).
#[tauri::command(async)]
pub fn app_drag_out(app: AppHandle, window: WebviewWindow, key: String) -> AppResult<()> {
    crate::apps::service::drag_out(&app, window.label(), &key)
}

/// Where the calling dock's app buttons are (for windows shrinking into them).
#[tauri::command]
pub fn app_rects(
    app: AppHandle,
    window: WebviewWindow,
    rects: Vec<crate::apps::service::ButtonRect>,
) -> AppResult<()> {
    crate::apps::service::set_button_rects(&app, window.label(), rects)
}

/// Every dock's apps, for Settings.
#[tauri::command]
pub fn apps_all(app: AppHandle) -> Vec<crate::apps::DockApp> {
    crate::apps::service::all(&app)
}

/// Hovering an app: show its windows above it. Async: the first time creates a window.
#[tauri::command(async)]
pub fn app_previews(app: AppHandle, window: WebviewWindow, key: String, anchor_x: i32) -> AppResult<()> {
    if !app.state::<crate::settings::SettingsStore>().get().apps.previews {
        return Ok(());
    }
    let dock_app = crate::apps::service::list(&app, window.label())
        .into_iter()
        .find(|a| a.key == key)
        .ok_or_else(|| AppError::InvalidInput("unknown app".into()))?;
    #[cfg(windows)]
    crate::apps::preview::show(&app, window.label(), &dock_app, anchor_x)?;
    #[cfg(not(windows))]
    let _ = (dock_app, anchor_x);
    Ok(())
}

/// A label over a dock button, like the taskbar's tooltips. Async: the first time creates a
/// window.
#[tauri::command(async)]
pub fn tooltip_show(app: AppHandle, window: WebviewWindow, text: String, anchor_x: i32) -> AppResult<()> {
    #[cfg(windows)]
    crate::apps::preview::show_tooltip(&app, window.label(), &text, anchor_x)?;
    #[cfg(not(windows))]
    let _ = (app, window, text, anchor_x);
    Ok(())
}

/// Hide the window previews after `delay_ms`, unless the cursor reaches them.
#[tauri::command]
pub fn preview_hide(app: AppHandle, delay_ms: u64) {
    #[cfg(windows)]
    crate::apps::preview::hide_later(&app, std::time::Duration::from_millis(delay_ms.min(5_000)));
    #[cfg(not(windows))]
    let _ = (app, delay_ms);
}

#[tauri::command]
pub fn preview_hover(app: AppHandle, hovered: bool) {
    #[cfg(windows)]
    crate::apps::preview::set_hovered(&app, hovered);
    #[cfg(not(windows))]
    let _ = (app, hovered);
}

#[cfg(windows)]
#[tauri::command]
pub fn preview_current() -> Option<crate::apps::preview::PreviewContent> {
    crate::apps::preview::current()
}

#[cfg(windows)]
#[tauri::command(async)]
pub fn preview_activate(app: AppHandle, hwnd: isize) -> AppResult<()> {
    crate::apps::preview::activate(&app, hwnd)
}

#[cfg(windows)]
#[tauri::command(async)]
pub fn preview_close_window(app: AppHandle, hwnd: isize) -> AppResult<()> {
    crate::apps::preview::close_window(&app, hwnd)
}

/// Open a file (or the folder) from the open stack.
#[tauri::command(async)]
pub fn stack_open(app: AppHandle, path: String) -> AppResult<()> {
    crate::apps::service::stack_open(&app, &path)
}

/// The dock's own menu, from a right-click on an empty part of it.
#[tauri::command(async)]
pub fn dock_menu(app: AppHandle, window: WebviewWindow, anchor_x: i32) -> AppResult<()> {
    crate::apps::service::dock_menu(&app, window.label(), anchor_x)
}

#[tauri::command(async)]
pub fn dock_menu_action(app: AppHandle, action: crate::apps::jump::DockMenuAction) -> AppResult<()> {
    crate::apps::service::dock_menu_action(&app, action)
}

/// The mouse went down on an empty part of the calling dock.
#[tauri::command(async)]
pub fn dock_press(app: AppHandle, window: WebviewWindow) -> AppResult<()> {
    crate::apps::service::press(&app, window.label())
}

/// A moment after that press: still held there?
#[tauri::command(async)]
pub fn dock_hold(app: AppHandle, window: WebviewWindow) -> AppResult<()> {
    crate::apps::service::hold(&app, window.label())
}

#[tauri::command(async)]
pub fn show_desktop(app: AppHandle) {
    crate::apps::service::show_desktop(&app);
}

#[tauri::command(async)]
pub fn desktop_peek(app: AppHandle, window: WebviewWindow, on: bool) {
    crate::apps::service::peek(&app, window.label(), on);
}

/// Ask for an image file (`folder` false) or a folder. Async: waits for the dialog.
#[tauri::command(async)]
pub fn pick_path(folder: bool) -> Option<String> {
    #[cfg(windows)]
    return crate::apps::win::pick(folder);
    #[cfg(not(windows))]
    {
        let _ = folder;
        None
    }
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

/// Open or close a dock folder's popup, centred on `anchor_x` (screen pixels).
#[tauri::command(async)]
pub fn folder_open(app: AppHandle, window: WebviewWindow, id: String, anchor_x: i32) -> AppResult<()> {
    crate::apps::service::show_folder(&app, window.label(), &id, anchor_x)
}

/// Hovering a dock folder: open its popup until the cursor leaves the folder and the popup.
#[tauri::command(async)]
pub fn folder_peek(app: AppHandle, window: WebviewWindow, id: String, anchor_x: i32) -> AppResult<()> {
    crate::apps::service::peek_folder(&app, window.label(), &id, anchor_x)
}

/// Save the dock's folders, leaving every other setting as it is now.
#[tauri::command]
pub fn dock_folders_set(app: AppHandle, folders: Vec<crate::settings::DockFolder>) -> AppResult<Settings> {
    let store = app.state::<SettingsStore>();
    let before = store.get();
    let mut settings = before.clone();
    settings.apps.folders = folders;
    store.set(settings)?;
    applied(&app, before, store.get())
}

/// An app in a folder's popup was clicked.
#[tauri::command(async)]
pub fn folder_app_click(app: AppHandle, dock: String, key: String) -> AppResult<()> {
    crate::apps::service::folder_click(&app, &dock, &key)
}

/// A right click on an app in a folder's popup: that app's menu, in the popup's place.
#[tauri::command(async)]
pub fn folder_app_menu(app: AppHandle, dock: String, key: String, anchor_x: i32) -> AppResult<()> {
    if !crate::shell::is_dock_label(&dock) {
        return Err(AppError::InvalidInput("unknown dock".into()));
    }
    crate::apps::service::show_menu(&app, &dock, &key, anchor_x)
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

/// Open Task View (as Windows + Tab does).
#[tauri::command(async)]
pub fn shell_task_view() {
    #[cfg(windows)]
    crate::apps::win::press(&[
        windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN,
        windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB,
    ]);
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

// ---------- Account card widgets ----------

/// Which account services have a key saved (never the keys).
#[tauri::command(async)]
pub fn service_connected() -> Vec<crate::accounts::services::Service> {
    crate::accounts::services::connected()
}

/// Save (or, empty, remove) a service's key, in Windows Credential Manager.
#[tauri::command(async)]
pub fn service_key_set(service: crate::accounts::services::Service, key: String) -> AppResult<()> {
    crate::accounts::services::set_key(service, &key)
}

#[tauri::command]
pub async fn service_revenue(
    app: AppHandle,
    service: crate::accounts::services::Service,
) -> Result<crate::accounts::services::Revenue, AppError> {
    crate::accounts::services::revenue(&app, service).await
}

#[tauri::command]
pub async fn service_revenue_all(
    app: AppHandle,
) -> Result<Vec<crate::accounts::services::Revenue>, AppError> {
    Ok(crate::accounts::services::all_revenue(&app).await)
}

#[tauri::command]
pub async fn service_traffic(
    app: AppHandle,
    service: crate::accounts::services::Service,
) -> Result<crate::accounts::services::Traffic, AppError> {
    crate::accounts::services::traffic(&app, service).await
}

#[tauri::command]
pub async fn service_count(
    app: AppHandle,
    service: crate::accounts::services::Service,
) -> Result<crate::accounts::services::Count, AppError> {
    crate::accounts::services::counts(&app, service).await
}

/// An exchange rate (for Total Revenue across stores in different currencies).
#[tauri::command]
pub async fn fx_rate(from: String, to: String) -> Result<f64, AppError> {
    Ok(crate::cards::online::rate(&from, &to).await?.rate)
}

#[tauri::command]
pub async fn github_overview() -> Result<crate::accounts::services::GithubOverview, AppError> {
    crate::accounts::services::github_overview().await
}

#[tauri::command]
pub async fn gmail_unread(app: AppHandle) -> Result<u32, AppError> {
    crate::accounts::services::gmail_unread(&app).await
}

// ---------- Card widgets ----------

/// Today's screen time, per hour and per app.
#[tauri::command]
pub fn screen_time() -> crate::cards::screentime::ScreenTime {
    crate::cards::screentime::today()
}

/// Your ChatGPT plan's Codex limits, from Codex's own files; `None` when it isn't used here.
#[tauri::command(async)]
pub fn codex_usage() -> Option<crate::cards::local::CodexUsage> {
    crate::cards::local::codex_usage()
}

/// Network speed since the last call.
#[tauri::command]
pub fn net_speed() -> crate::cards::local::NetSpeed {
    crate::cards::local::net_speed()
}

/// The PC's temperature in °C, when Windows reports one.
#[tauri::command(async)]
pub fn cpu_temperature() -> Option<f64> {
    crate::cards::local::temperature()
}

#[tauri::command(async)]
pub fn drives() -> Vec<crate::cards::local::Drive> {
    crate::cards::local::drives()
}

#[tauri::command(async)]
pub fn recycle_bin() -> crate::cards::local::RecycleBin {
    crate::cards::local::recycle_bin()
}

/// Empty the Recycle Bin for good (the card asked first).
#[tauri::command(async)]
pub fn recycle_bin_empty() -> AppResult<()> {
    crate::cards::local::empty_recycle_bin()
}

#[tauri::command(async)]
pub fn recent_files() -> Vec<crate::cards::local::RecentFile> {
    crate::cards::local::recent_files()
}

#[tauri::command(async)]
pub fn downloads_list() -> Vec<crate::cards::local::RecentFile> {
    crate::cards::local::downloads()
}

#[tauri::command(async)]
pub fn screenshots_list() -> Vec<crate::cards::local::RecentFile> {
    crate::cards::local::screenshots()
}

/// Open a file one of the cards listed.
#[tauri::command(async)]
pub fn open_listed_file(path: String) -> AppResult<()> {
    crate::cards::local::open_listed(&path)
}

#[tauri::command]
pub async fn stock_quotes(app: AppHandle) -> Result<Vec<crate::cards::online::Quote>, AppError> {
    let symbols = app.state::<crate::settings::SettingsStore>().get().cards.stocks;
    Ok(crate::cards::online::quotes(&symbols).await)
}

#[tauri::command]
pub async fn coin_price(app: AppHandle) -> Result<crate::cards::online::CoinPrice, AppError> {
    let id = app.state::<crate::settings::SettingsStore>().get().cards.metric;
    crate::cards::online::coin(&id).await
}

#[tauri::command]
pub async fn currency_rate(app: AppHandle) -> Result<crate::cards::online::Rate, AppError> {
    let cards = app.state::<crate::settings::SettingsStore>().get().cards;
    crate::cards::online::rate(&cards.currency_from, &cards.currency_to).await
}

#[tauri::command]
pub async fn github_contributions(app: AppHandle) -> Result<crate::cards::online::Contributions, AppError> {
    let user = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .cards
        .github_user;
    crate::cards::online::contributions(&user).await
}

#[tauri::command]
pub async fn leetcode_stats(app: AppHandle) -> Result<crate::cards::online::LeetcodeStats, AppError> {
    let user = app
        .state::<crate::settings::SettingsStore>()
        .get()
        .cards
        .leetcode_user;
    crate::cards::online::leetcode(&user).await
}

/// The Search card: a folder path opens in File Explorer; anything else is searched on the web.
#[tauri::command(async)]
pub fn web_search(query: String) -> AppResult<()> {
    let query = query.trim();
    if query.is_empty() || query.len() > 500 {
        return Err(AppError::InvalidInput("nothing to search".into()));
    }
    let path = std::path::Path::new(query);
    #[cfg(windows)]
    if path.is_absolute() && path.is_dir() {
        crate::apps::win::open(query);
        return Ok(());
    }
    let encoded: String = query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    #[cfg(windows)]
    crate::apps::win::open(&format!("https://www.google.com/search?q={encoded}"));
    #[cfg(not(windows))]
    let _ = (path, encoded);
    Ok(())
}

/// Windows' own screen capture (the Snipping Tool's bar, as Windows + Shift + S opens).
#[tauri::command(async)]
pub fn screen_capture() {
    #[cfg(windows)]
    crate::apps::win::open("ms-screenclip:");
}

/// Whether Windows is in dark mode.
#[tauri::command]
pub fn windows_theme() -> Option<crate::automations::ThemeChoice> {
    #[cfg(windows)]
    {
        crate::automations::theme::current()
    }
    #[cfg(not(windows))]
    None
}

/// Switch Windows to light or dark (the Theme Toggle card). Only logged under DOCK_DRY_RUN=1.
#[tauri::command(async)]
pub fn windows_theme_set(choice: crate::automations::ThemeChoice) -> AppResult<()> {
    if std::env::var("DOCK_DRY_RUN").is_ok_and(|v| v == "1") {
        tracing::info!(?choice, "dry run: would switch the Windows theme");
        return Ok(());
    }
    #[cfg(windows)]
    {
        crate::automations::theme::apply(choice)
    }
    #[cfg(not(windows))]
    Ok(())
}

/// Press one of the Windows taskbar corner's buttons (it opens Windows' own flyout).
#[tauri::command]
pub fn tray_press(button: shell::tray::TrayButton) {
    shell::tray::press(button);
}

/// Windows' accent colour and taskbar mode, for the Windows 11 look.
#[tauri::command]
pub fn system_look() -> shell::tray::SystemLook {
    shell::tray::system_look()
}

/// The time and date in Windows' own short formats, as the taskbar shows them.
#[tauri::command]
pub fn system_clock() -> shell::tray::SystemClock {
    shell::tray::system_clock()
}

/// The input language of the window in front, as the taskbar shows it.
#[tauri::command]
pub fn input_language() -> shell::tray::InputLanguage {
    shell::tray::input_language()
}

/// Hovering a widget: preview its popup without taking the focus. Async: the first open
/// creates a window.
#[tauri::command(async)]
pub fn widget_peek(app: AppHandle, kind: crate::settings::WidgetKind, anchor_x: i32) -> AppResult<()> {
    shell::panel::peek(&app, kind, anchor_x)
}

/// The cursor left a widget: close its preview soon, unless the cursor reaches it.
#[tauri::command]
pub fn widget_unpeek(app: AppHandle, delay_ms: u64) {
    shell::panel::unpeek_later(&app, std::time::Duration::from_millis(delay_ms.min(5_000)));
}

/// The open menu's real height, measured by its page: the popup fits it.
#[tauri::command(async)]
pub fn panel_fit(app: AppHandle, height: f64) -> AppResult<()> {
    shell::panel::fit_height(&app, height)
}

/// A click inside a widget preview keeps it open, with the focus.
#[tauri::command(async)]
pub fn panel_pin(app: AppHandle) -> AppResult<()> {
    shell::panel::pin(&app)
}

/// The cursor arrived on (or left) the popup.
#[tauri::command]
pub fn panel_hover(app: AppHandle, hovered: bool) {
    shell::panel::set_hovered(&app, hovered);
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
