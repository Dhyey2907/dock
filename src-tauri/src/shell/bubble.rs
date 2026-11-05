//! The reminder cards window: a stack of cards just past the dock (above it, or below a top dock),
//! at its right end where the widgets are. It never takes focus — a reminder must not pull you
//! out of what you're typing — and hides when there are no cards.

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use super::placement::Rect;
use crate::{
    automations::Bubble,
    error::AppResult,
    settings::{Edge, SettingsStore},
};

pub const BUBBLE_LABEL: &str = "bubble";
/// Event carrying the current cards to the cards page.
pub const BUBBLES_CHANGED: &str = "bubbles-changed";
/// Card stack width, one card's height and the gap between cards, in logical pixels.
const WIDTH: f64 = 360.0;
const CARD: f64 = 72.0;
const GAP: f64 = 8.0;
/// Space between the dock and the stack.
const OFFSET: f64 = 10.0;
/// Most cards shown at once (the newest).
pub const MAX_CARDS: usize = 3;

/// Where the stack goes for `count` cards. All physical pixels.
pub fn bubble_rect(screen: Rect, dock: Rect, edge: Edge, count: usize, scale: f64) -> Rect {
    let px = |logical: f64| (logical * scale).round() as i32;
    let n = count.clamp(1, MAX_CARDS) as f64;
    let width = px(WIDTH);
    let height = px(n * CARD + (n - 1.0) * GAP + 2.0 * GAP);
    let right = (dock.x + dock.width as i32).min(screen.x + screen.width as i32 - px(8.0));
    let x = (right - width).max(screen.x + px(8.0));
    let y = match edge {
        Edge::Bottom => dock.y - px(OFFSET) - height,
        Edge::Top => dock.y + dock.height as i32 + px(OFFSET),
    };
    Rect {
        x,
        y,
        width: width as u32,
        height: height as u32,
    }
}

fn create(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    Ok(
        WebviewWindowBuilder::new(app, BUBBLE_LABEL, WebviewUrl::App("index.html#bubble".into()))
            .title("Dock reminders")
            .decorations(false)
            .transparent(true)
            .shadow(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .visible(false)
            .build()?,
    )
}

/// Show `bubbles` (or hide the window when there are none) and tell the page.
pub fn sync(app: &AppHandle, bubbles: &[Bubble]) -> AppResult<()> {
    let shown: Vec<Bubble> = bubbles.iter().rev().take(MAX_CARDS).rev().cloned().collect();
    let window = match app.get_webview_window(BUBBLE_LABEL) {
        Some(window) => window,
        None if shown.is_empty() => return Ok(()),
        None => create(app)?,
    };
    app.emit_to(BUBBLE_LABEL, BUBBLES_CHANGED, &shown)?;
    if shown.is_empty() {
        window.hide()?;
        return Ok(());
    }
    let (Some(geometry), Some(dock)) = (super::geometry(app), super::dock_window(app)) else {
        return Ok(());
    };
    let size = dock.outer_size()?;
    let screen = Rect {
        x: geometry.screen_x,
        y: geometry.screen_y,
        width: geometry.screen_width,
        height: geometry.screen_height,
    };
    let dock_rect = Rect {
        x: geometry.x,
        y: geometry.y,
        width: size.width,
        height: size.height,
    };
    let edge = app.state::<SettingsStore>().get().edge;
    let rect = bubble_rect(screen, dock_rect, edge, shown.len(), geometry.scale);
    window.set_size(PhysicalSize::new(rect.width, rect.height))?;
    window.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    show_without_focus(&window)
}

/// Show the window without activating it (Tauri's `show` would take focus).
fn show_without_focus(window: &tauri::WebviewWindow) -> AppResult<()> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, IsWindowVisible, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
            SW_SHOWNOACTIVATE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
        };
        let hwnd = windows::Win32::Foundation::HWND(window.hwnd()?.0);
        // SAFETY: style bits and visibility on our own live top-level window.
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let wanted =
                (style | (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize) & !(WS_EX_APPWINDOW.0 as isize);
            if wanted != style {
                SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted);
            }
            if !IsWindowVisible(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        window.show()?;
        Ok(())
    }
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
        x: 545,
        y: 1504,
        width: 1470,
        height: 84,
    };

    #[test]
    fn the_stack_sits_above_the_docks_right_end() {
        let one = bubble_rect(SCREEN, DOCK, Edge::Bottom, 1, 1.5);
        assert_eq!(one.width, 540);
        assert_eq!(one.x + one.width as i32, 545 + 1470);
        assert_eq!(one.y + one.height as i32, 1504 - 15);
        let three = bubble_rect(SCREEN, DOCK, Edge::Bottom, 3, 1.5);
        assert!(three.height > one.height && three.y < one.y);
        // More than the maximum is the same as the maximum.
        assert_eq!(bubble_rect(SCREEN, DOCK, Edge::Bottom, 9, 1.5), three);
    }

    #[test]
    fn below_a_top_dock_and_kept_on_screen() {
        let top = Rect { y: 12, ..DOCK };
        assert_eq!(bubble_rect(SCREEN, top, Edge::Top, 1, 1.0).y, 12 + 84 + 10);
        let tiny = Rect {
            x: 100,
            width: 200,
            ..DOCK
        };
        assert_eq!(bubble_rect(SCREEN, tiny, Edge::Bottom, 1, 1.0).x, 8);
    }
}
