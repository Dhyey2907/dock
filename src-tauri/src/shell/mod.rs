//! The dock window, the settings window and the tray icon.

pub mod placement;

use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use crate::{error::AppResult, settings::SettingsStore};
use placement::Rect;

pub const DOCK_LABEL: &str = "dock";
const SETTINGS_LABEL: &str = "settings";

/// What the page last reported about the dock's content.
pub struct ShellState {
    /// Pill content width in logical pixels.
    pub content_width: Mutex<f64>,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            content_width: Mutex::new(placement::MIN_PILL_WIDTH),
        }
    }
}

pub fn dock_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK_LABEL)
}

/// Size and move the dock for the current settings on the primary monitor.
pub fn place_dock(app: &AppHandle) -> AppResult<()> {
    let Some(window) = dock_window(app) else {
        return Ok(());
    };
    let Some(monitor) = window.primary_monitor()?.or(window.current_monitor()?) else {
        return Ok(());
    };
    let screen = Rect {
        x: monitor.position().x,
        y: monitor.position().y,
        width: monitor.size().width,
        height: monitor.size().height,
    };
    let settings = app.state::<SettingsStore>().get();
    let content_width = *app
        .state::<ShellState>()
        .content_width
        .lock()
        .expect("shell lock");
    let rect = placement::dock_rect(screen, monitor.scale_factor(), &settings, content_width);
    window.set_size(PhysicalSize::new(rect.width, rect.height))?;
    window.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    Ok(())
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
    make_tool_window(window)?;
    Ok(())
}

/// Give the window `WS_EX_TOOLWINDOW` so it has no Alt+Tab entry; Tauri's `skipTaskbar` only
/// removes the taskbar button. Tauri re-applies its own styles on every `show()`, so this runs
/// after each show. Windows only re-reads the Alt+Tab list when a window is shown, hence the
/// raw hide/show (which bypasses Tauri) after changing the style.
#[cfg(windows)]
fn make_tool_window(window: &WebviewWindow) -> AppResult<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOWNOACTIVATE,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    // Tauri may link a different `windows` version; both HWNDs wrap the same raw handle.
    let hwnd = windows::Win32::Foundation::HWND(window.hwnd()?.0);
    // SAFETY: `hwnd` is a live top-level window owned by this process; we only toggle
    // extended-style bits and visibility on it.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let tool_style = (style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        if tool_style != style {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, tool_style);
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
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
    let result = if window.is_visible().unwrap_or(false) {
        window.hide().map_err(Into::into)
    } else {
        show_dock(&window)
    };
    if let Err(error) = result {
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
    place_dock(app)?;
    if let Some(window) = dock_window(app) {
        show_dock(&window)?;
    }
    Ok(())
}
