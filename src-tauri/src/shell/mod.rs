//! The dock window, the settings window and the tray icon.

#[cfg(windows)]
mod appbar;
pub mod panel;
pub mod placement;
pub mod taskbar;
#[cfg(windows)]
mod watch;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

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

pub const DOCK_LABEL: &str = "dock";
const SETTINGS_LABEL: &str = "settings";
/// Event telling the dock page whether it is showing or waiting as an auto-hide sliver.
pub const DOCK_REVEALED: &str = "dock-revealed";
/// Event telling the dock page where its window sits on its screen (for the glass).
pub const DOCK_GEOMETRY: &str = "dock-geometry";

/// Where the dock window is on its screen, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, ts_rs::TS)]
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

/// The dock's live state, beyond the saved settings.
pub struct ShellState {
    /// Pill content width in logical pixels, as the page measured it.
    pub content_width: Mutex<f64>,
    /// With auto-hide on: whether the dock is out (the cursor is on it).
    revealed: AtomicBool,
    /// Hidden from the tray: a fullscreen app closing must not bring it back.
    hidden_by_user: AtomicBool,
    /// A fullscreen app covers the dock's screen.
    fullscreen: AtomicBool,
    /// The strip last claimed from Windows, so unchanged layouts don't re-claim it (each claim
    /// changes the work area, which would come back as another layout change).
    reserved: Mutex<Option<(Rect, Edge)>>,
    /// Where the dock last went, as sent to the page.
    geometry: Mutex<Option<DockGeometry>>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            content_width: Mutex::new(placement::MIN_PILL_WIDTH),
            revealed: AtomicBool::new(false),
            hidden_by_user: AtomicBool::new(false),
            fullscreen: AtomicBool::new(false),
            reserved: Mutex::new(None),
            geometry: Mutex::new(None),
        }
    }
}

pub fn dock_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK_LABEL)
}

/// The full rectangle of the dock's screen, in physical pixels.
pub fn dock_screen(app: &AppHandle) -> Option<Rect> {
    let window = dock_window(app)?;
    monitor_rect(&window).ok().flatten().map(|(rect, _)| rect)
}

pub fn geometry(app: &AppHandle) -> Option<DockGeometry> {
    *app.state::<ShellState>().geometry.lock().expect("shell lock")
}

fn monitor_rect(window: &WebviewWindow) -> AppResult<Option<(Rect, f64)>> {
    let Some(monitor) = window.primary_monitor()?.or(window.current_monitor()?) else {
        return Ok(None);
    };
    let rect = Rect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    Ok(Some((rect, monitor.scale_factor())))
}

/// Size and move the dock for the current settings on the primary monitor, and claim (or give
/// back) its strip of the screen.
pub fn place_dock(app: &AppHandle) -> AppResult<()> {
    let Some(window) = dock_window(app) else {
        return Ok(());
    };
    let Some((screen, scale)) = monitor_rect(&window)? else {
        return Ok(());
    };
    let settings = app.state::<SettingsStore>().get();
    let shell = app.state::<ShellState>();
    let content_width = *shell.content_width.lock().expect("shell lock");
    let dock = placement::dock_rect(screen, scale, &settings, content_width);
    let tucked_away = settings.auto_hide && !shell.revealed.load(Ordering::SeqCst);
    let rect = if tucked_away {
        placement::sliver_rect(screen, settings.edge)
    } else {
        dock
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
    let moved = shell.geometry.lock().expect("shell lock").replace(geometry) != Some(geometry);
    if moved {
        app.emit(DOCK_GEOMETRY, geometry)?;
    }

    // An auto-hiding or user-hidden dock claims nothing: windows may use the whole screen.
    let claims = !settings.auto_hide && !shell.hidden_by_user.load(Ordering::SeqCst);
    let strip = if claims {
        placement::reserved_rect(screen, dock, settings.edge)
    } else {
        Rect {
            height: 0,
            ..placement::sliver_rect(screen, settings.edge)
        }
    };
    let mut reserved = shell.reserved.lock().expect("shell lock");
    if *reserved != Some((strip, settings.edge)) {
        *reserved = Some((strip, settings.edge));
        #[cfg(windows)]
        appbar::reserve(
            windows::Win32::Foundation::HWND(window.hwnd()?.0),
            strip,
            settings.edge,
        );
    }
    Ok(())
}

/// Re-place after a layout change, re-claiming the strip even if it looks unchanged (another
/// app bar or the taskbar may have taken part of it).
pub fn relayout(app: &AppHandle) {
    *app.state::<ShellState>().reserved.lock().expect("shell lock") = None;
    if let Err(error) = place_dock(app) {
        tracing::warn!(%error, "failed to re-place the dock");
    }
}

/// The page says the cursor arrived on (or left) an auto-hiding dock.
pub fn set_revealed(app: &AppHandle, revealed: bool) -> AppResult<()> {
    let shell = app.state::<ShellState>();
    if shell.revealed.swap(revealed, Ordering::SeqCst) != revealed {
        place_dock(app)?;
        app.emit(DOCK_REVEALED, revealed)?;
    }
    Ok(())
}

/// Whether the page should draw the dock (not just the auto-hide sliver).
pub fn is_revealed(app: &AppHandle) -> bool {
    let auto_hide = app.state::<SettingsStore>().get().auto_hide;
    !auto_hide || app.state::<ShellState>().revealed.load(Ordering::SeqCst)
}

/// A drag of the dock ended: snap to whichever edge it was dropped nearer.
#[cfg_attr(not(windows), allow(dead_code))]
fn snap_after_drag(app: &AppHandle) {
    let result = (|| -> AppResult<()> {
        let Some(window) = dock_window(app) else {
            return Ok(());
        };
        let Some((screen, _)) = monitor_rect(&window)? else {
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
        relayout(app);
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(%error, "failed to snap the dock after a drag");
    }
}

/// A fullscreen app started or stopped covering the dock's screen.
#[cfg_attr(not(windows), allow(dead_code))]
fn set_fullscreen(app: &AppHandle, active: bool) {
    let shell = app.state::<ShellState>();
    if shell.fullscreen.swap(active, Ordering::SeqCst) == active {
        return;
    }
    tracing::debug!(active, "fullscreen app changed");
    let Some(window) = dock_window(app) else { return };
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

/// Explorer restarted: it forgot the dock's app bar and shows its taskbar again.
#[cfg_attr(not(windows), allow(dead_code))]
fn explorer_restarted(app: &AppHandle) {
    tracing::info!("explorer restarted; setting up the dock again");
    taskbar::apply_system(app.state::<SettingsStore>().get().taskbar_mode);
    #[cfg(windows)]
    if let Some(Ok(raw)) = dock_window(app).map(|w| w.hwnd()) {
        let hwnd = windows::Win32::Foundation::HWND(raw.0);
        appbar::unregister(hwnd);
        appbar::register(hwnd);
    }
    relayout(app);
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
fn make_tool_window(window: &WebviewWindow, no_activate: bool) -> AppResult<()> {
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

/// Open (or focus) the settings window.
pub fn open_settings_window(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.unminimize()?;
        window.set_focus()?;
        return Ok(());
    }
    WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("index.html#settings".into()))
        .title("Dock settings")
        .inner_size(560.0, 640.0)
        .min_inner_size(480.0, 480.0)
        .center()
        .build()?;
    Ok(())
}

fn toggle_dock(app: &AppHandle) {
    let Some(window) = dock_window(app) else { return };
    let hide = window.is_visible().unwrap_or(false);
    app.state::<ShellState>()
        .hidden_by_user
        .store(hide, Ordering::SeqCst);
    let result = if hide {
        window.hide().map_err(Into::into)
    } else {
        show_dock(&window)
    };
    // A hidden dock gives its strip of the screen back.
    if let Err(error) = result.and_then(|()| place_dock(app)) {
        tracing::warn!(%error, "failed to toggle dock");
    }
}

fn setup_tray(app: &AppHandle) -> AppResult<()> {
    let toggle = MenuItem::with_id(app, "toggle", "Show / hide dock", true, None::<&str>)?;
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
                if let Err(error) = open_settings_window(app) {
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
    taskbar::apply_system(app.state::<SettingsStore>().get().taskbar_mode);
    let Some(window) = dock_window(app) else {
        return Ok(());
    };
    #[cfg(windows)]
    watch::install(&window, app);
    place_dock(app)?;
    show_dock(&window)?;
    #[cfg(windows)]
    watch::refresh_fullscreen(app);
    Ok(())
}
