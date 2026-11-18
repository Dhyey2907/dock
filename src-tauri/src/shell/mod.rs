//! The dock window, the settings window and the tray icon.

#[cfg(windows)]
mod appbar;
pub mod bubble;
pub mod floating;
pub mod panel;
pub mod placement;
pub mod screens;
pub mod shortcut;
pub mod taskbar;
pub mod tray;
#[cfg(windows)]
mod watch;
#[cfg(windows)]
mod workarea;
pub mod winkeys;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};

use serde::Serialize;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::{
    error::AppResult,
    settings::{Edge, SettingsStore},
};
use placement::Rect;
use screens::ScreenId;

pub use screens::{is_dock_label, MAIN_LABEL as DOCK_LABEL};
const SETTINGS_LABEL: &str = "settings";
/// Event telling a dock page whether it is showing or waiting as an auto-hide sliver.
pub const DOCK_REVEALED: &str = "dock-revealed";
/// Event telling a dock page where its window sits on its screen (for the glass).
pub const DOCK_GEOMETRY: &str = "dock-geometry";

/// Where a dock window is on its screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DockGeometry {
    pub screen_x: i32,
    pub screen_y: i32,
    pub screen_width: u32,
    pub screen_height: u32,
    pub x: i32,
    pub y: i32,
    /// Physical pixels per CSS pixel (1.5 at 150%).
    pub scale: f64,
}

/// One dock's live state, beyond the saved settings.
#[derive(Debug, Clone)]
struct DockState {
    /// The device name of the screen it is on.
    screen: String,
    /// Pill content width in logical pixels, as the page measured it.
    content_width: f64,
    /// With auto-hide on: whether the dock is out (the cursor is on it).
    revealed: bool,
    /// A fullscreen app covers the dock's screen.
    fullscreen: bool,
    /// The strip last claimed from Windows, so unchanged layouts don't re-claim it (each claim
    /// changes the work area, which would come back as another layout change).
    reserved: Option<(Rect, Edge)>,
    /// The last strip handed to Windows was empty. Kept through `relayout_all`: with the
    /// taskbar hidden on this screen only, re-claiming nothing would make Explorer recompute
    /// the work area each time, undoing the one the dock sets and starting another relayout.
    claims_nothing: bool,
    /// Where the dock last went, as sent to the page.
    geometry: Option<DockGeometry>,
}

impl DockState {
    fn new(screen: String) -> Self {
        Self {
            screen,
            content_width: placement::MIN_PILL_WIDTH,
            revealed: false,
            fullscreen: false,
            reserved: None,
            claims_nothing: false,
            geometry: None,
        }
    }
}

/// The docks' live state, by window label.
pub struct ShellState {
    docks: Mutex<HashMap<String, DockState>>,
    /// Hidden from the tray or the shortcut (all docks together): a fullscreen app closing
    /// must not bring them back, and a newly plugged-in screen's dock stays hidden too.
    hidden_by_user: AtomicBool,
    /// One pass of adding and removing docks at a time.
    syncing: Mutex<()>,
}

impl Default for ShellState {
    fn default() -> Self {
        let mut docks = HashMap::new();
        docks.insert(DOCK_LABEL.to_string(), DockState::new(String::new()));
        Self {
            docks: Mutex::new(docks),
            hidden_by_user: AtomicBool::new(false),
            syncing: Mutex::new(()),
        }
    }
}

impl ShellState {
    fn docks(&self) -> std::sync::MutexGuard<'_, HashMap<String, DockState>> {
        self.docks.lock().expect("shell lock")
    }

    fn labels(&self) -> Vec<String> {
        let mut labels: Vec<String> = self.docks().keys().cloned().collect();
        labels.sort_by_key(|l| (l != DOCK_LABEL, l.clone()));
        labels
    }
}

/// The main dock: pinned apps, widgets, and what popups and reminder cards line up with.
pub fn dock_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK_LABEL)
}

/// Every dock window, the main one first.
pub fn dock_windows(app: &AppHandle) -> Vec<WebviewWindow> {
    app.state::<ShellState>()
        .labels()
        .into_iter()
        .filter_map(|l| app.get_webview_window(&l))
        .collect()
}

/// Each dock's label and the full rectangle of its screen in physical pixels, main dock first.
pub fn dock_screens(app: &AppHandle) -> Vec<(String, Rect)> {
    let Ok(monitors) = monitors(app) else {
        return Vec::new();
    };
    let shell = app.state::<ShellState>();
    let docks = shell.docks().clone();
    shell
        .labels()
        .into_iter()
        .filter_map(|label| {
            let monitor = find_monitor(&monitors, &docks.get(&label)?.screen, &label)?;
            Some((label, monitor.rect))
        })
        .collect()
}

/// Where the main dock sits on its screen.
pub fn geometry(app: &AppHandle) -> Option<DockGeometry> {
    geometry_of(app, DOCK_LABEL)
}

pub fn geometry_of(app: &AppHandle, label: &str) -> Option<DockGeometry> {
    app.state::<ShellState>().docks().get(label)?.geometry
}

/// A screen the dock can be put on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScreenInfo {
    /// Windows' device name ("\\.\DISPLAY2"), as saved in settings.
    pub name: String,
    /// "Screen 2 · 1920×1080".
    pub label: String,
    pub primary: bool,
}

/// A connected screen, in physical pixels.
#[derive(Debug, Clone)]
struct Monitor {
    name: String,
    rect: Rect,
    scale: f64,
    primary: bool,
}

/// The screens connected now, main screen first.
fn monitors(app: &AppHandle) -> AppResult<Vec<Monitor>> {
    let Some(window) = dock_window(app) else {
        return Ok(Vec::new());
    };
    let primary = window.primary_monitor()?.and_then(|m| m.name().cloned());
    let mut list: Vec<Monitor> = window
        .available_monitors()?
        .into_iter()
        .filter_map(|m| {
            let name = m.name()?.clone();
            Some(Monitor {
                primary: primary.as_ref() == Some(&name),
                rect: Rect {
                    x: m.position().x,
                    y: m.position().y,
                    width: m.size().width,
                    height: m.size().height,
                },
                scale: m.scale_factor(),
                name,
            })
        })
        .collect();
    list.sort_by(|a, b| b.primary.cmp(&a.primary).then_with(|| a.name.cmp(&b.name)));
    Ok(list)
}

fn screen_ids(monitors: &[Monitor]) -> Vec<ScreenId> {
    monitors
        .iter()
        .map(|m| ScreenId {
            name: m.name.clone(),
            primary: m.primary,
        })
        .collect()
}

/// A dock's screen. The main dock falls back to the main screen if its own is gone; a
/// secondary dock whose screen is gone has none (it is about to be removed).
fn find_monitor<'a>(monitors: &'a [Monitor], screen: &str, label: &str) -> Option<&'a Monitor> {
    monitors.iter().find(|m| m.name == screen).or_else(|| {
        (label == DOCK_LABEL)
            .then(|| monitors.iter().find(|m| m.primary).or(monitors.first()))
            .flatten()
    })
}

/// The screens connected now, main screen first, for Settings.
pub fn screens(app: &AppHandle) -> AppResult<Vec<ScreenInfo>> {
    Ok(monitors(app)?
        .into_iter()
        .map(|m| ScreenInfo {
            label: format!(
                "Screen {} · {}×{}",
                screens::screen_number(&m.name).unwrap_or("?"),
                m.rect.width,
                m.rect.height
            ),
            primary: m.primary,
            name: m.name,
        })
        .collect())
}

/// Make the set of docks match the screens and settings: the main dock onto its screen, a dock
/// added for each screen that needs one and removed from each screen that's gone. Then place
/// them all. Creates windows, so never call it on the main thread (WebView2 can deadlock).
pub fn sync_docks(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    let _one_at_a_time = shell.syncing.lock().unwrap_or_else(|e| e.into_inner());
    let settings = app.state::<SettingsStore>().get();
    let monitors = match monitors(app) {
        Ok(monitors) => monitors,
        Err(error) => {
            tracing::warn!(%error, "could not list screens");
            return;
        }
    };
    let slots = screens::plan(&screen_ids(&monitors), &settings.screen, settings.docks_on);
    let mut changed = false;

    // Docks no longer wanted (or whose screen changed) go first, giving their strips back.
    let existing: Vec<(String, String)> = shell
        .docks()
        .iter()
        .map(|(label, dock)| (label.clone(), dock.screen.clone()))
        .collect();
    for (label, screen) in existing {
        let wanted = slots.iter().find(|s| s.label == label);
        if label == DOCK_LABEL {
            // The main dock is never removed; it moves.
            if let Some(slot) = wanted.filter(|s| s.screen != screen) {
                if let Some(dock) = shell.docks().get_mut(&label) {
                    dock.screen = slot.screen.clone();
                    dock.fullscreen = false;
                }
                changed = true;
            }
            continue;
        }
        if wanted.is_some_and(|s| s.screen == screen) {
            continue;
        }
        shell.docks().remove(&label);
        if let Some(window) = app.get_webview_window(&label) {
            // Destroy skips the close request docks refuse; the window procedure gives the
            // app-bar strip back as the window goes.
            if let Err(error) = window.destroy() {
                tracing::warn!(%error, label, "could not remove a dock");
            }
        }
        tracing::info!(label, "removed a dock");
        changed = true;
    }

    for slot in slots.iter().filter(|s| s.label != DOCK_LABEL) {
        if shell.docks().contains_key(&slot.label) {
            continue;
        }
        match create_dock(app, &slot.label) {
            Ok(window) => {
                shell
                    .docks()
                    .insert(slot.label.clone(), DockState::new(slot.screen.clone()));
                tracing::info!(label = slot.label, screen = slot.screen, "added a dock");
                let shown = place_one(app, &slot.label).and_then(|()| {
                    if shell.hidden_by_user.load(Ordering::SeqCst) {
                        Ok(())
                    } else {
                        show_dock(&window)
                    }
                });
                if let Err(error) = shown {
                    tracing::warn!(%error, label = slot.label, "could not show a new dock");
                }
                changed = true;
            }
            Err(error) => tracing::warn!(%error, label = slot.label, "could not add a dock"),
        }
    }

    relayout_all(app);
    if changed {
        crate::glass::service::refresh_soon(app);
        crate::apps::service::docks_changed(app);
        #[cfg(windows)]
        watch::refresh_fullscreen(app);
    }
}

/// A secondary dock window, set up like the main one in the app config.
fn create_dock(app: &AppHandle, label: &str) -> AppResult<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title("Dock")
        .inner_size(640.0, 64.0)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .build()?;
    // The subclass must be installed on the thread that owns the window.
    #[cfg(windows)]
    {
        let attached = window.clone();
        app.run_on_main_thread(move || watch::attach(&attached))?;
    }
    Ok(window)
}

/// Size and move every dock for the current settings, and claim (or give back) their strips.
pub fn place_dock(app: &AppHandle) -> AppResult<()> {
    for label in app.state::<ShellState>().labels() {
        place_one(app, &label)?;
    }
    Ok(())
}

/// Size and move one dock, and claim (or give back) its strip of its screen.
pub fn place_one(app: &AppHandle, label: &str) -> AppResult<()> {
    let Some(window) = app.get_webview_window(label) else {
        return Ok(());
    };
    let shell = app.state::<ShellState>();
    let Some(dock) = shell.docks().get(label).cloned() else {
        return Ok(());
    };
    let monitors = monitors(app)?;
    let Some(monitor) = find_monitor(&monitors, &dock.screen, label) else {
        return Ok(());
    };
    let (screen, scale) = (monitor.rect, monitor.scale);
    let settings = app.state::<SettingsStore>().get();
    let full = placement::dock_rect(screen, scale, &settings, dock.content_width);
    let tucked_away = settings.auto_hide && !dock.revealed;
    let rect = if tucked_away {
        placement::sliver_rect(screen, settings.edge)
    } else {
        full
    };
    window.set_size(PhysicalSize::new(rect.width, rect.height))?;
    window.set_position(PhysicalPosition::new(rect.x, rect.y))?;

    let geometry = DockGeometry {
        screen_x: screen.x,
        screen_y: screen.y,
        screen_width: screen.width,
        screen_height: screen.height,
        x: rect.x,
        y: rect.y,
        scale,
    };
    // An auto-hiding or user-hidden dock claims nothing: windows may use the whole screen.
    let claims = !settings.auto_hide && !shell.hidden_by_user.load(Ordering::SeqCst);
    let strip = if claims {
        placement::reserved_rect(screen, full, settings.edge)
    } else {
        Rect {
            height: 0,
            ..placement::sliver_rect(screen, settings.edge)
        }
    };
    // With the taskbar hidden on this screen only (the others stay pinned), the hidden taskbar
    // still holds its strip, so the dock sets the screen's work area itself and its app bar
    // claims nothing (Windows would put a claim beside the taskbar's strip, not over it).
    let sets_work_area = !shell.hidden_by_user.load(Ordering::SeqCst)
        && taskbar::pins_other_screens(settings.taskbar_mode);
    let claim = if sets_work_area {
        Rect { height: 0, ..strip }
    } else {
        strip
    };
    let (moved, reserve) = {
        let mut docks = shell.docks();
        let Some(state) = docks.get_mut(label) else {
            return Ok(());
        };
        let moved = state.geometry.replace(geometry) != Some(geometry);
        let reserve = if sets_work_area {
            !state.claims_nothing
        } else {
            state.reserved != Some((claim, settings.edge))
        };
        state.reserved = Some((claim, settings.edge));
        if reserve {
            state.claims_nothing = claim.height == 0;
        }
        (moved, reserve)
    };
    if moved {
        app.emit_to(label, DOCK_GEOMETRY, geometry)?;
    }
    if reserve {
        tracing::debug!(label, ?claim, "claiming the dock's strip");
        #[cfg(windows)]
        appbar::reserve(
            windows::Win32::Foundation::HWND(window.hwnd()?.0),
            claim,
            settings.edge,
        );
    }
    #[cfg(windows)]
    if sets_work_area {
        workarea::hold(screen, placement::work_area(screen, strip, settings.edge));
    } else {
        workarea::release();
    }
    Ok(())
}

/// Re-place every dock, re-claiming strips even if they look unchanged (another app bar or the
/// taskbar may have taken part of one).
fn relayout_all(app: &AppHandle) {
    for dock in app.state::<ShellState>().docks().values_mut() {
        dock.reserved = None;
    }
    if let Err(error) = place_dock(app) {
        tracing::warn!(%error, "failed to re-place the docks");
    }
}

/// After a layout or settings change: add or remove docks for the screens there are now, and
/// re-place them all. Creates windows, so never call it on the main thread.
pub fn relayout(app: &AppHandle) {
    sync_docks(app);
    // Screens may have come or gone: windows on them get rearranged.
    crate::apps::service::docks_changed(app);
}

/// A dock's page says the cursor arrived on (or left) it while it auto-hides.
pub fn set_revealed(app: &AppHandle, label: &str, revealed: bool) -> AppResult<()> {
    let shell = app.state::<ShellState>();
    let was = {
        let mut docks = shell.docks();
        let Some(dock) = docks.get_mut(label) else {
            return Ok(());
        };
        std::mem::replace(&mut dock.revealed, revealed)
    };
    if was != revealed {
        place_one(app, label)?;
        app.emit_to(label, DOCK_REVEALED, revealed)?;
    }
    Ok(())
}

/// Auto-hide was switched: tuck every dock away (on) or bring them out for good (off).
pub fn reset_revealed(app: &AppHandle) -> AppResult<()> {
    for label in app.state::<ShellState>().labels() {
        set_revealed(app, &label, false)?;
        app.emit_to(label.as_str(), DOCK_REVEALED, is_revealed(app, &label))?;
    }
    Ok(())
}

/// Whether a dock's page should draw the dock (not just the auto-hide sliver).
pub fn is_revealed(app: &AppHandle, label: &str) -> bool {
    let auto_hide = app.state::<SettingsStore>().get().auto_hide;
    !auto_hide
        || app
            .state::<ShellState>()
            .docks()
            .get(label)
            .is_some_and(|d| d.revealed)
}

/// A dock's page measured its pill's content; resize that dock to fit it.
pub fn set_content_width(app: &AppHandle, label: &str, width: f64) -> AppResult<()> {
    match app.state::<ShellState>().docks().get_mut(label) {
        Some(dock) => dock.content_width = width,
        None => return Ok(()),
    }
    place_one(app, label)
}

/// A drag of a dock ended: every dock snaps to whichever edge it was dropped nearer.
#[cfg_attr(not(windows), allow(dead_code))]
fn snap_after_drag(app: &AppHandle, label: &str) {
    let result = (|| -> AppResult<()> {
        let Some(window) = app.get_webview_window(label) else {
            return Ok(());
        };
        let Some((_, screen)) = dock_screens(app).into_iter().find(|(l, _)| l == label) else {
            return Ok(());
        };
        let (position, size) = (window.outer_position()?, window.outer_size()?);
        let dropped = Rect {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        };
        let store = app.state::<SettingsStore>();
        let mut settings = store.get();
        let edge = placement::edge_for_drop(screen, dropped);
        if edge != settings.edge {
            settings.edge = edge;
            store.set(settings.clone())?;
            app.emit(crate::commands::SETTINGS_CHANGED, &settings)?;
        }
        // Back into place either way: a drag along the same edge just springs back.
        relayout_all(app);
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(%error, "failed to snap the dock after a drag");
    }
}

/// A fullscreen app started or stopped covering a dock's screen.
#[cfg_attr(not(windows), allow(dead_code))]
fn set_fullscreen(app: &AppHandle, label: &str, active: bool) {
    let shell = app.state::<ShellState>();
    match shell.docks().get_mut(label) {
        Some(dock) if dock.fullscreen != active => dock.fullscreen = active,
        _ => return,
    }
    tracing::debug!(label, active, "fullscreen app changed");
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let result = if active {
        window.hide().map_err(Into::into)
    } else if shell.hidden_by_user.load(Ordering::SeqCst) {
        Ok(())
    } else {
        show_dock(&window)
    };
    if let Err(error) = result {
        tracing::warn!(%error, "failed to follow a fullscreen app");
    }
}

/// Explorer restarted: it forgot the docks' app bars and shows its taskbar again.
#[cfg_attr(not(windows), allow(dead_code))]
fn explorer_restarted(app: &AppHandle) {
    tracing::info!("explorer restarted; setting up the docks again");
    if !using_windows_taskbar(app) {
        taskbar::apply_system(app.state::<SettingsStore>().get().taskbar_mode);
    }
    #[cfg(windows)]
    for window in dock_windows(app) {
        if let Ok(raw) = window.hwnd() {
            let hwnd = windows::Win32::Foundation::HWND(raw.0);
            appbar::unregister(hwnd);
            appbar::register(hwnd);
        }
    }
    relayout_all(app);
}

/// Show the dock and keep it out of Alt+Tab. Use this instead of `window.show()`.
pub fn show_dock(window: &WebviewWindow) -> AppResult<()> {
    // Showing an already-visible window makes Tauri re-apply its styles, which would force
    // the hide/show below and steal focus.
    if window.is_visible()? {
        return Ok(());
    }
    window.show()?;
    #[cfg(windows)]
    make_tool_window(window, true)?;
    Ok(())
}

/// Give the window `WS_EX_TOOLWINDOW` so it has no Alt+Tab entry; Tauri's `skipTaskbar` only
/// removes the taskbar button. Tauri re-applies its own styles on every `show()`, so this runs
/// after each show. Windows only re-reads the Alt+Tab list when a window is shown, hence the
/// raw hide/show (which bypasses Tauri) after changing the style.
///
/// The dock also gets `WS_EX_NOACTIVATE` (`no_activate`): clicking it must leave the app you
/// were using in front, or "click the active app to minimize it" could never work. Bringing a
/// window forward still works, because the dock received the click. Popups take focus instead,
/// which is how they know to close when you click elsewhere.
#[cfg(windows)]
pub(crate) fn make_tool_window(window: &WebviewWindow, no_activate: bool) -> AppResult<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE,
        WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    // Tauri may link a different `windows` version; both HWNDs wrap the same raw handle.
    let hwnd = windows::Win32::Foundation::HWND(window.hwnd()?.0);
    let add = if no_activate {
        WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0
    } else {
        WS_EX_TOOLWINDOW.0
    };
    // SAFETY: `hwnd` is a live top-level window owned by this process; we only toggle
    // extended-style bits and visibility on it.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let tool_style = (style | add as isize) & !(WS_EX_APPWINDOW.0 as isize);
        if tool_style != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, tool_style);
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = ShowWindow(hwnd, if no_activate { SW_SHOWNOACTIVATE } else { SW_SHOW });
        }
    }
    Ok(())
}

/// Event telling an open settings window which section to show.
pub const SETTINGS_SECTION: &str = "settings-section";
const WELCOME_LABEL: &str = "welcome";

fn mica() -> tauri::utils::config::WindowEffectsConfig {
    tauri::utils::config::WindowEffectsConfig {
        effects: vec![tauri::window::Effect::Mica],
        ..Default::default()
    }
}

/// Open (or focus) the settings window, at `section` if given ("widgets", "calendars", …).
pub fn open_settings_window(app: &AppHandle, section: Option<&str>) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        if let Some(section) = section {
            app.emit_to(SETTINGS_LABEL, SETTINGS_SECTION, section)?;
        }
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }
    let url = match section {
        Some(section) => format!("index.html#settings/{section}"),
        None => "index.html#settings".into(),
    };
    WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App(url.into()))
        .title("Dock settings")
        .inner_size(840.0, 640.0)
        .min_inner_size(640.0, 480.0)
        .transparent(true)
        .effects(mica())
        .center()
        .build()?;
    Ok(())
}

/// The first-run welcome.
pub fn open_welcome_window(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(WELCOME_LABEL) {
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, WELCOME_LABEL, WebviewUrl::App("index.html#welcome".into()))
        .title("Welcome to Dock")
        .inner_size(620.0, 560.0)
        .resizable(false)
        .maximizable(false)
        .transparent(true)
        .effects(mica())
        .center()
        .build()?;
    Ok(())
}

pub fn close_welcome_window(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(WELCOME_LABEL) {
        window.close()?;
    }
    Ok(())
}

/// Switch between the docks and the Windows taskbar (dock button, tray and shortcut).
pub fn toggle_dock(app: &AppHandle) {
    // Hide if any dock shows. A dock hidden for a fullscreen app doesn't count as showing,
    // and stays hidden when the others come back.
    let hide = dock_windows(app).iter().any(|w| w.is_visible().unwrap_or(false));
    use_windows_taskbar(app, hide);
}

/// Whether the user switched to the Windows taskbar.
pub fn using_windows_taskbar(app: &AppHandle) -> bool {
    app.state::<ShellState>().hidden_by_user.load(Ordering::SeqCst)
}

/// Hide every dock and put the Windows taskbar back as the user had it (`true`), or hide the
/// taskbar again and bring the docks back (`false`).
pub fn use_windows_taskbar(app: &AppHandle, windows_taskbar: bool) {
    let hide = windows_taskbar;
    let windows = dock_windows(app);
    let shell = app.state::<ShellState>();
    shell.hidden_by_user.store(hide, Ordering::SeqCst);
    if !hide {
        // The taskbar goes first, so its strip is free for the docks to claim.
        taskbar::apply_system(app.state::<SettingsStore>().get().taskbar_mode);
    }
    let fullscreen: Vec<String> = shell
        .docks()
        .iter()
        .filter(|(_, d)| d.fullscreen)
        .map(|(l, _)| l.clone())
        .collect();
    for window in windows {
        let result = if hide {
            window.hide().map_err(Into::into)
        } else if fullscreen.iter().any(|l| l == window.label()) {
            Ok(())
        } else {
            show_dock(&window)
        };
        if let Err(error) = result {
            tracing::warn!(%error, label = window.label(), "failed to toggle a dock");
        }
    }
    // A hidden dock gives its strip of the screen back.
    if let Err(error) = place_dock(app) {
        tracing::warn!(%error, "failed to toggle dock");
    }
    if hide {
        taskbar::restore_system();
    }
    tracing::info!(windows_taskbar = hide, "switched taskbars");
}

/// Bring every dock back (a second launch of the app does this).
pub fn show_docks(app: &AppHandle) {
    let shell = app.state::<ShellState>();
    if shell.hidden_by_user.load(Ordering::SeqCst) {
        toggle_dock(app);
    }
}
fn setup_tray(app: &AppHandle) -> AppResult<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Switch Dock / Windows taskbar", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Dock", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &settings, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Dock")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_dock(app),
            "settings" => {
                if let Err(error) = open_settings_window(app, None) {
                    tracing::warn!(%error, "failed to open settings window");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_dock(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn init(app: &AppHandle) -> AppResult<()> {
    setup_tray(app)?;
    // The taskbar goes first: once it auto-hides, its strip is free for the dock to claim.
    let settings = app.state::<SettingsStore>().get();
    taskbar::set_dock_screen(&settings);
    taskbar::apply_system(settings.taskbar_mode);
    let Some(window) = dock_window(app) else {
        return Ok(());
    };
    #[cfg(windows)]
    {
        watch::install(app);
        watch::attach(&window);
    }
    // The main dock's screen, before it is first placed.
    let settings = app.state::<SettingsStore>().get();
    let slots = screens::plan(&screen_ids(&monitors(app)?), &settings.screen, settings.docks_on);
    if let Some(main) = slots.first() {
        if let Some(dock) = app.state::<ShellState>().docks().get_mut(DOCK_LABEL) {
            dock.screen = main.screen.clone();
        }
    }
    place_one(app, DOCK_LABEL)?;
    show_dock(&window)?;
    #[cfg(windows)]
    watch::refresh_fullscreen(app);
    // Docks for the other screens, off the main thread: creating windows there can deadlock.
    if slots.len() > 1 {
        let app = app.clone();
        std::thread::spawn(move || sync_docks(&app));
    }
    Ok(())
}
