//! The dock's own Alt+Tab switcher, in its glass style (on only when chosen in Settings).
//! The keyboard hook (`hooks::set_alt_tab`) hands over Alt+Tab; this keeps the list of
//! windows (front-most first), moves the choice, and switches when Alt is let go. The window
//! never takes the focus, so the app in front stays in front until the switch.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;

use super::{hooks::SwitchKey, layout};
use crate::error::AppResult;

pub const SWITCHER_LABEL: &str = "switcher";
pub const SWITCHER_STATE: &str = "switcher-state";
/// Most windows listed.
const MAX_ITEMS: usize = 21;
const COLUMNS: usize = 7;
const CELL_W: f64 = 112.0;
const CELL_H: f64 = 112.0;
const PADDING: f64 = 18.0;
const TITLE: f64 = 40.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SwitchItem {
    #[ts(type = "number")]
    pub hwnd: isize,
    pub title: String,
    pub app_name: String,
    /// Icon source for `app_icon`.
    pub icon: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SwitcherState {
    pub items: Vec<SwitchItem>,
    pub selected: usize,
}

static STATE: Mutex<Option<SwitcherState>> = Mutex::new(None);

/// Where the choice goes for a key: Alt+Tab opens on the window behind the front one (or
/// moves on), Alt+Shift+Tab goes back; both wrap around.
pub fn step(selected: Option<usize>, len: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    match (selected, forward) {
        (None, true) => 1 % len,
        (None, false) => len - 1,
        (Some(at), true) => (at + 1) % len,
        (Some(at), false) => (at + len - 1) % len,
    }
}

/// The window's logical size for `count` items.
pub fn size(count: usize) -> (f64, f64) {
    let columns = count.clamp(1, COLUMNS);
    let rows = count.div_ceil(COLUMNS).max(1);
    (
        CELL_W * columns as f64 + 2.0 * PADDING,
        CELL_H * rows as f64 + TITLE + 2.0 * PADDING,
    )
}

fn items() -> Vec<SwitchItem> {
    super::win::windows()
        .into_iter()
        .take(MAX_ITEMS)
        .map(|w| SwitchItem {
            icon: match (&w.app_id, &w.exe) {
                (Some(id), _) => format!("aumid:{id}"),
                (None, Some(exe)) => format!("exe:{exe}"),
                (None, None) => String::new(),
            },
            hwnd: w.hwnd,
            title: w.title,
            app_name: w.app_name,
        })
        .collect()
}

fn window(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(SWITCHER_LABEL) {
        return Ok(window);
    }
    let window =
        WebviewWindowBuilder::new(app, SWITCHER_LABEL, WebviewUrl::App("index.html#switcher".into()))
            .title("Dock switcher")
            .decorations(false)
            .transparent(true)
            .shadow(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .focused(false)
            .visible(false)
            .effects(tauri::utils::config::WindowEffectsConfig {
                effects: vec![tauri::window::Effect::Acrylic],
                ..Default::default()
            })
            .build()?;
    crate::shell::panel::round_corners(&window);
    Ok(window)
}

/// Centre the switcher on the screen of the window in front (else the cursor's).
fn show(app: &AppHandle, state: &SwitcherState) -> AppResult<()> {
    let popup = window(app)?;
    app.emit_to(SWITCHER_LABEL, SWITCHER_STATE, state)?;
    let screens = super::win::screens();
    let front = super::win::window_rect(super::win::foreground());
    let point = front.map_or_else(super::win::cursor, |r| {
        (r.x + r.width as i32 / 2, r.y + r.height as i32 / 2)
    });
    let Some(screen) = layout::screen_at(&screens, point) else {
        return Ok(());
    };
    let (width, height) = size(state.items.len());
    popup.set_size(tauri::LogicalSize::new(width, height))?;
    let outer = popup.outer_size()?;
    let work = screen.work;
    popup.set_position(PhysicalPosition::new(
        work.x + (work.width as i32 - outer.width as i32) / 2,
        work.y + (work.height as i32 - outer.height as i32) / 2,
    ))?;
    super::win::show_on_top_quietly(popup.hwnd()?.0 as isize);
    Ok(())
}

fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SWITCHER_LABEL) {
        let _ = window.hide();
        // It was shown straight through Windows, so hide it the same way.
        if let Ok(hwnd) = window.hwnd() {
            super::win::hide_quietly(hwnd.0 as isize);
        }
    }
}

/// What the switcher shows now (for its page when it first loads).
pub fn current() -> Option<SwitcherState> {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A key from the keyboard hook.
pub fn handle(app: &AppHandle, key: SwitchKey) {
    match key {
        SwitchKey::Next | SwitchKey::Previous => {
            let forward = key == SwitchKey::Next;
            let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
            let opening = state.is_none();
            if opening {
                let items = items();
                if items.is_empty() {
                    return;
                }
                let selected = step(None, items.len(), forward);
                *state = Some(SwitcherState { items, selected });
            } else if let Some(s) = state.as_mut() {
                s.selected = step(Some(s.selected), s.items.len(), forward);
            }
            let Some(snapshot) = state.clone() else { return };
            drop(state);
            let result = if opening {
                show(app, &snapshot)
            } else {
                app.emit_to(SWITCHER_LABEL, SWITCHER_STATE, &snapshot)
                    .map_err(Into::into)
            };
            if let Err(error) = result {
                tracing::warn!(%error, "switcher failed");
            }
        }
        SwitchKey::Commit(alt) => {
            // Alt was swallowed by the hook; let it go now, quietly.
            super::win::release_alt_quietly(super::hooks::alt_key(alt));
            let state = STATE.lock().unwrap_or_else(|e| e.into_inner()).take();
            hide(app);
            if let Some(chosen) = state.and_then(|s| s.items.get(s.selected).map(|i| i.hwnd)) {
                super::win::activate(chosen);
            }
        }
        SwitchKey::Cancel => {
            STATE.lock().unwrap_or_else(|e| e.into_inner()).take();
            hide(app);
            // Alt is still held; a harmless key keeps its release from opening a menu bar.
            super::win::press(&[windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0xE8)]);
        }
    }
}

/// An item was clicked: switch to it.
pub fn pick(app: &AppHandle, hwnd: isize) {
    let state = STATE.lock().unwrap_or_else(|e| e.into_inner()).take();
    hide(app);
    if state.is_some_and(|s| s.items.iter().any(|i| i.hwnd == hwnd)) {
        super::win::activate(hwnd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_choice_starts_behind_the_front_window_and_wraps() {
        assert_eq!(step(None, 4, true), 1);
        assert_eq!(step(None, 4, false), 3);
        assert_eq!(step(None, 1, true), 0);
        assert_eq!(step(Some(3), 4, true), 0);
        assert_eq!(step(Some(0), 4, false), 3);
        assert_eq!(step(None, 0, true), 0);
    }

    #[test]
    fn the_window_grows_in_rows_of_seven() {
        let (w1, h1) = size(3);
        let (w7, _) = size(7);
        let (w8, h8) = size(8);
        assert!(w1 < w7);
        assert_eq!(w7, w8);
        assert_eq!(h8, h1 + CELL_H);
    }
}
