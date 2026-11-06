//! Widget popups: one small window, made the first time a popup opens and reused after that.
//! It opens above the dock (below it when the dock is on top), centred on the widget that was
//! clicked, with Windows' acrylic blur behind it. Clicking elsewhere closes it; clicking the
//! same widget again toggles it.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;

use super::placement::Rect;
use crate::{
    apps::jump::AppMenu,
    error::AppResult,
    settings::{Edge, SettingsStore, Theme, WidgetKind},
};

pub const PANEL_LABEL: &str = "panel";
/// Event telling the popup page which widget to show.
pub const PANEL_OPEN: &str = "panel-open";
/// Event telling the dock page the popup closed.
pub const PANEL_CLOSED: &str = "panel-closed";
/// Gap between the dock and its popup, in logical pixels.
const GAP: f64 = 10.0;
/// Closest a popup comes to the screen's sides, in logical pixels.
const SIDE_MARGIN: f64 = 8.0;

/// What the popup shows.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum PanelContent {
    /// A widget's popup.
    Widget { kind: WidgetKind },
    /// An app's right-click menu.
    App { menu: AppMenu },
    /// Quick capture: a note or a to-do, from the shortcut.
    Capture,
}

/// What the popup is showing, while it is open.
#[derive(Default)]
pub struct PanelState {
    open: Mutex<Option<PanelContent>>,
}

/// A popup's size in logical pixels.
pub fn panel_size(kind: WidgetKind) -> (f64, f64) {
    match kind {
        WidgetKind::Clock => (300.0, 330.0),
        WidgetKind::Weather => (320.0, 320.0),
        WidgetKind::Status => (340.0, 252.0),
        WidgetKind::System => (300.0, 230.0),
        WidgetKind::Focus => (280.0, 250.0),
        WidgetKind::Music => (340.0, 190.0),
        WidgetKind::Claude => (320.0, 250.0),
        WidgetKind::Calendar | WidgetKind::Unknown => (340.0, 380.0),
        WidgetKind::Notes => (340.0, 360.0),
        WidgetKind::Todos => (320.0, 380.0),
        WidgetKind::Kanban => (400.0, 400.0),
        WidgetKind::MeetingNotes => (340.0, 360.0),
        WidgetKind::TimeTracker => (320.0, 380.0),
        WidgetKind::QuickLinks => (320.0, 340.0),
        WidgetKind::AppFolders => (320.0, 340.0),
        WidgetKind::Bookmarks => (360.0, 400.0),
        WidgetKind::FileSearch => (380.0, 400.0),
        WidgetKind::PinImage => (300.0, 220.0),
        WidgetKind::ScreenText => (320.0, 260.0),
        WidgetKind::SongsToday => (320.0, 380.0),
        WidgetKind::ClaudeSessions => (360.0, 380.0),
    }
}

/// Where a popup goes: centred on `anchor_x`, kept on screen, just past the dock on the side
/// away from the screen edge. All physical pixels except `size` (logical).
pub fn panel_rect(screen: Rect, dock: Rect, edge: Edge, anchor_x: i32, size: (u32, u32), scale: f64) -> Rect {
    let px = |logical: f64| (logical * scale).round() as i32;
    let (width, height) = (size.0 as i32, size.1 as i32);
    let margin = px(SIDE_MARGIN);
    let left_limit = screen.x + margin;
    let right_limit = (screen.x + screen.width as i32 - margin - width).max(left_limit);
    let x = (anchor_x - width / 2).clamp(left_limit, right_limit);
    let y = match edge {
        Edge::Bottom => dock.y - px(GAP) - height,
        Edge::Top => dock.y + dock.height as i32 + px(GAP),
    };
    Rect {
        x,
        y,
        width: width as u32,
        height: height as u32,
    }
}

/// Open `kind`'s popup at `anchor_x` (the widget's centre, screen pixels), or close it if it is
/// already open. Async-safe: creating the window off the main thread avoids a WebView2 deadlock.
pub fn toggle(app: &AppHandle, kind: WidgetKind, anchor_x: i32) -> AppResult<()> {
    let content = PanelContent::Widget { kind };
    if is_showing(app, |open| *open == content)? {
        return close(app);
    }
    show(app, super::DOCK_LABEL, content, panel_size(kind), anchor_x)
}

/// Size of the quick-capture popup, logical pixels.
const CAPTURE_SIZE: (f64, f64) = (360.0, 200.0);

/// Open quick capture above the middle of the main dock (or close it, if it's open).
pub fn show_capture(app: &AppHandle) -> AppResult<()> {
    if is_showing(app, |open| *open == PanelContent::Capture)? {
        return close(app);
    }
    let Some(geometry) = super::geometry_of(app, super::DOCK_LABEL) else {
        return Ok(());
    };
    let anchor = geometry.screen_x + geometry.screen_width as i32 / 2;
    show(
        app,
        super::DOCK_LABEL,
        PanelContent::Capture,
        CAPTURE_SIZE,
        anchor,
    )
}

/// Open an app's right-click menu above it (or close it, if that menu is already open).
pub fn show_app_menu(app: &AppHandle, menu: AppMenu, anchor_x: i32) -> AppResult<()> {
    let same = |open: &PanelContent| matches!(open, PanelContent::App { menu: m } if m.key == menu.key && m.dock == menu.dock);
    if is_showing(app, same)? {
        return close(app);
    }
    let dock = menu.dock.clone();
    let size = menu.size();
    show(app, &dock, PanelContent::App { menu }, size, anchor_x)
}

/// Whether the popup is on screen showing something `matches` accepts.
fn is_showing(app: &AppHandle, matches: impl Fn(&PanelContent) -> bool) -> AppResult<bool> {
    let open = app.state::<PanelState>().open.lock().expect("panel lock").clone();
    let visible = match app.get_webview_window(PANEL_LABEL) {
        Some(window) => window.is_visible()?,
        None => false,
    };
    Ok(visible && open.as_ref().is_some_and(matches))
}

/// Show `content` in the popup, `size` logical pixels, centred on `anchor_x` past `dock_label`.
fn show(
    app: &AppHandle,
    dock_label: &str,
    content: PanelContent,
    size: (f64, f64),
    anchor_x: i32,
) -> AppResult<()> {
    let state = app.state::<PanelState>();
    let window = match app.get_webview_window(PANEL_LABEL) {
        Some(window) => window,
        None => create(app)?,
    };
    let Some(dock) = app.get_webview_window(dock_label) else {
        return Ok(());
    };
    let Some(geometry) = super::geometry_of(app, dock_label) else {
        return Ok(());
    };
    let screen = Rect {
        x: geometry.screen_x,
        y: geometry.screen_y,
        width: geometry.screen_width,
        height: geometry.screen_height,
    };
    let dock_size = dock.outer_size()?;
    let dock_rect = Rect {
        x: geometry.x,
        y: geometry.y,
        width: dock_size.width,
        height: dock_size.height,
    };
    let settings = app.state::<SettingsStore>().get();

    *state.open.lock().expect("panel lock") = Some(content.clone());
    // Content first, so the popup never flashes the previous widget. (A popup window opened for
    // the first time asks for this itself once its page has loaded.)
    app.emit_to(PANEL_LABEL, PANEL_OPEN, &content)?;
    window.set_theme(Some(if settings.theme == Theme::White {
        tauri::Theme::Light
    } else {
        tauri::Theme::Dark
    }))?;
    let (width, height) = size;
    window.set_size(tauri::LogicalSize::new(width, height))?;
    // Windows makes an undecorated window with a shadow taller than asked; place it by the size
    // it really has, so it never covers the dock.
    let outer = window.outer_size()?;
    let rect = panel_rect(
        screen,
        dock_rect,
        settings.edge,
        anchor_x,
        (outer.width, outer.height),
        geometry.scale,
    );
    window.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    window.show()?;
    #[cfg(windows)]
    super::make_tool_window(&window, false)?;
    window.set_focus()?;
    Ok(())
}

/// What the popup is showing, for a popup page that just loaded.
pub fn current(app: &AppHandle) -> Option<PanelContent> {
    app.state::<PanelState>().open.lock().expect("panel lock").clone()
}

pub fn close(app: &AppHandle) -> AppResult<()> {
    let was_open = app.state::<PanelState>().open.lock().expect("panel lock").take();
    if let Some(window) = app.get_webview_window(PANEL_LABEL) {
        window.hide()?;
    }
    if was_open.is_some() {
        app.emit(PANEL_CLOSED, ())?;
    }
    Ok(())
}

fn create(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, PANEL_LABEL, WebviewUrl::App("index.html#panel".into()))
        .title("Dock popup")
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .effects(tauri::utils::config::WindowEffectsConfig {
            effects: vec![tauri::window::Effect::Acrylic],
            ..Default::default()
        })
        .build()?;
    #[cfg(windows)]
    round_corners(&window);
    Ok(window)
}

/// Windows 11 rounds undecorated windows only when asked.
#[cfg(windows)]
fn round_corners(window: &tauri::WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWM_WINDOW_CORNER_PREFERENCE,
    };
    let Ok(raw) = window.hwnd() else { return };
    let preference = DWMWCP_ROUND;
    // SAFETY: setting a documented attribute on our own live window from a local value.
    let _ = unsafe {
        DwmSetWindowAttribute(
            windows::Win32::Foundation::HWND(raw.0),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&preference as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of_val(&preference) as u32,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 2560,
        height: 1600,
    };
    const DOCK: Rect = Rect {
        x: 760,
        y: 1504,
        width: 1040,
        height: 84,
    };

    #[test]
    fn opens_above_a_bottom_dock_centred_on_the_widget() {
        let rect = panel_rect(SCREEN, DOCK, Edge::Bottom, 1500, (450, 495), 1.5);
        assert_eq!((rect.width, rect.height), (450, 495));
        assert_eq!(rect.x, 1500 - 225);
        assert_eq!(rect.y, 1504 - 15 - 495);
    }

    #[test]
    fn opens_below_a_top_dock() {
        let dock = Rect { y: 12, ..DOCK };
        let rect = panel_rect(SCREEN, dock, Edge::Top, 1500, (450, 495), 1.5);
        assert_eq!(rect.y, 12 + 84 + 15);
    }

    #[test]
    fn stays_on_screen_near_the_sides() {
        let near_right = panel_rect(SCREEN, DOCK, Edge::Bottom, 2550, (450, 495), 1.5);
        assert_eq!(near_right.x, 2560 - 12 - 450);
        let near_left = panel_rect(SCREEN, DOCK, Edge::Bottom, 5, (450, 495), 1.5);
        assert_eq!(near_left.x, 12);
    }

    #[test]
    fn every_widget_has_a_sensible_size() {
        for kind in WidgetKind::ALL {
            let (w, h) = panel_size(kind);
            assert!(
                (240.0..=400.0).contains(&w) && (160.0..=420.0).contains(&h),
                "{kind:?}"
            );
        }
    }
}
