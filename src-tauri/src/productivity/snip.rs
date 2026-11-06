//! Choosing an area of the screen, then pinning it or reading its text.
//!
//! The screen under the cursor is captured first; a borderless window covering that screen
//! shows the capture, and the area is dragged out on it (so what's chosen is exactly what was
//! on screen, popups and all). Pinned pictures float in small always-on-top windows of their own.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU32, Ordering},
        Mutex,
    },
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;

use super::capture::{fraction_rect, Shot};
use crate::error::{AppError, AppResult};

pub const SNIP_LABEL: &str = "snip";
pub const PIN_PREFIX: &str = "pin-";
/// Event telling the area picker to load a new capture.
pub const SNIP_OPEN: &str = "snip-open";
pub const PINS_CHANGED: &str = "pins-changed";
pub const SCREEN_TEXT: &str = "screen-text";
/// A pinned picture covers at most this much of its screen when it first appears.
const PIN_MAX_SHARE: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SnipMode {
    /// Pin the area in a floating window.
    Pin,
    /// Read the area's text and copy it.
    Text,
}

/// What the area picker shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SnipBackdrop {
    pub mode: SnipMode,
    /// The captured screen as a data URL.
    pub image: String,
}

/// The last text read from the screen, or why it couldn't be.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScreenText {
    pub text: String,
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

struct Session {
    mode: SnipMode,
    /// The captured screen's top-left, physical pixels.
    left: i32,
    top: i32,
    shot: Shot,
    backdrop: Option<String>,
}

#[derive(Default)]
pub struct SnipState {
    session: Mutex<Option<Session>>,
    pins: Mutex<HashMap<u32, String>>,
    next_pin: AtomicU32,
    last_text: Mutex<ScreenText>,
}

/// Capture the screen under the cursor and let the user drag out an area on it.
pub fn start(app: &AppHandle, mode: SnipMode) -> AppResult<()> {
    // The popup that asked must be gone from the picture.
    crate::shell::panel::close(app)?;
    std::thread::sleep(std::time::Duration::from_millis(180));
    #[cfg(windows)]
    {
        let rect = super::capture::cursor_screen()
            .ok_or_else(|| AppError::Platform("no screen under the cursor".into()))?;
        let shot =
            super::capture::grab(rect).ok_or_else(|| AppError::Platform("screen capture failed".into()))?;
        let (width, height) = (shot.width, shot.height);
        *app.state::<SnipState>().session.lock().expect("snip lock") = Some(Session {
            mode,
            left: rect.left,
            top: rect.top,
            shot,
            backdrop: None,
        });
        let window = match app.get_webview_window(SNIP_LABEL) {
            Some(window) => window,
            None => WebviewWindowBuilder::new(app, SNIP_LABEL, WebviewUrl::App("index.html#snip".into()))
                .title("Choose an area")
                .decorations(false)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .build()?,
        };
        window.set_position(PhysicalPosition::new(rect.left, rect.top))?;
        window.set_size(PhysicalSize::new(width, height))?;
        app.emit_to(SNIP_LABEL, SNIP_OPEN, mode)?;
        window.show()?;
        window.set_focus()?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = mode;
        Err(AppError::Platform("not supported".into()))
    }
}

/// The capture to choose from, for the picker page.
pub fn backdrop(app: &AppHandle) -> Option<SnipBackdrop> {
    let state = app.state::<SnipState>();
    let mut session = state.session.lock().expect("snip lock");
    let session = session.as_mut()?;
    if session.backdrop.is_none() {
        session.backdrop = session.shot.jpeg_data_url();
    }
    Some(SnipBackdrop {
        mode: session.mode,
        image: session.backdrop.clone()?,
    })
}

fn hide_picker(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(SNIP_LABEL) {
        window.hide()?;
    }
    Ok(())
}

pub fn cancel(app: &AppHandle) -> AppResult<()> {
    app.state::<SnipState>().session.lock().expect("snip lock").take();
    hide_picker(app)
}

/// The area was chosen: fractions (0–1) of the picker window's size.
pub fn finish(app: &AppHandle, x: f64, y: f64, width: f64, height: f64) -> AppResult<()> {
    let session = app.state::<SnipState>().session.lock().expect("snip lock").take();
    hide_picker(app)?;
    let Some(session) = session else { return Ok(()) };
    let Some(rect) = fraction_rect(session.shot.width, session.shot.height, x, y, width, height) else {
        return Ok(());
    };
    let area = session.shot.crop(rect);
    match session.mode {
        SnipMode::Pin => open_pin(
            app,
            &area,
            Some((session.left + rect.x as i32, session.top + rect.y as i32)),
        ),
        SnipMode::Text => {
            let outcome = read_and_copy(&area);
            *app.state::<SnipState>().last_text.lock().expect("snip lock") = outcome.clone();
            app.emit(SCREEN_TEXT, &outcome)?;
            Ok(())
        }
    }
}

fn read_and_copy(area: &Shot) -> ScreenText {
    #[cfg(windows)]
    {
        let read = super::capture::read_text(area).and_then(|text| {
            if !text.trim().is_empty() {
                super::capture::copy_text(&text)?;
            }
            Ok(text)
        });
        match read {
            Ok(text) if text.trim().is_empty() => ScreenText {
                text,
                problem: Some("No text found in that area.".into()),
            },
            Ok(text) => ScreenText { text, problem: None },
            Err(error) => ScreenText {
                text: String::new(),
                problem: Some(match error {
                    AppError::InvalidInput(message) | AppError::Platform(message) => message,
                    other => other.to_string(),
                }),
            },
        }
    }
    #[cfg(not(windows))]
    {
        let _ = area;
        ScreenText::default()
    }
}

pub fn last_text(app: &AppHandle) -> ScreenText {
    app.state::<SnipState>()
        .last_text
        .lock()
        .expect("snip lock")
        .clone()
}

// ---------- Pinned pictures ----------

/// Pin the clipboard's picture. `false` when the clipboard holds no picture.
pub fn pin_clipboard(app: &AppHandle) -> AppResult<bool> {
    crate::shell::panel::close(app)?;
    #[cfg(windows)]
    {
        let Some(shot) = super::capture::clipboard_image()? else {
            return Ok(false);
        };
        open_pin(app, &shot, None)?;
        Ok(true)
    }
    #[cfg(not(windows))]
    Ok(false)
}

/// The size a pin first opens at: the picture's own pixels, shrunk to fit its share of the screen.
pub fn pin_size(width: u32, height: u32, screen: (u32, u32)) -> (u32, u32) {
    let max_w = screen.0 as f64 * PIN_MAX_SHARE;
    let max_h = screen.1 as f64 * PIN_MAX_SHARE;
    let scale = (max_w / width as f64).min(max_h / height as f64).min(1.0);
    (
        ((width as f64 * scale).round() as u32).max(24),
        ((height as f64 * scale).round() as u32).max(24),
    )
}

/// Float `shot` in a window of its own, at `at` (physical pixels) or the middle of the screen.
fn open_pin(app: &AppHandle, shot: &Shot, at: Option<(i32, i32)>) -> AppResult<()> {
    let image = shot
        .png_data_url()
        .ok_or_else(|| AppError::Platform("could not encode the picture".into()))?;
    let state = app.state::<SnipState>();
    let id = state.next_pin.fetch_add(1, Ordering::SeqCst) + 1;
    state.pins.lock().expect("snip lock").insert(id, image);
    let label = format!("{PIN_PREFIX}{id}");
    let window =
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App(format!("index.html#{label}").into()))
            .title("Pinned picture")
            .decorations(false)
            .shadow(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .visible(false)
            .build()?;
    let monitor = window.current_monitor()?.or(app.primary_monitor()?);
    let (screen_pos, screen_size) = monitor
        .map(|m| (*m.position(), *m.size()))
        .unwrap_or((PhysicalPosition::new(0, 0), PhysicalSize::new(1920, 1080)));
    let (width, height) = pin_size(shot.width, shot.height, (screen_size.width, screen_size.height));
    let (x, y) = at.unwrap_or((
        screen_pos.x + (screen_size.width as i32 - width as i32) / 2,
        screen_pos.y + (screen_size.height as i32 - height as i32) / 2,
    ));
    window.set_size(PhysicalSize::new(width, height))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    window.show()?;
    app.emit(PINS_CHANGED, pin_count(app))?;
    Ok(())
}

/// A pin's picture, for its window's page.
pub fn pin_image(app: &AppHandle, id: u32) -> Option<String> {
    app.state::<SnipState>()
        .pins
        .lock()
        .expect("snip lock")
        .get(&id)
        .cloned()
}

pub fn pin_count(app: &AppHandle) -> u32 {
    app.state::<SnipState>().pins.lock().expect("snip lock").len() as u32
}

pub fn close_pin(app: &AppHandle, id: u32) -> AppResult<()> {
    app.state::<SnipState>()
        .pins
        .lock()
        .expect("snip lock")
        .remove(&id);
    if let Some(window) = app.get_webview_window(&format!("{PIN_PREFIX}{id}")) {
        window.destroy()?;
    }
    app.emit(PINS_CHANGED, pin_count(app))?;
    Ok(())
}

pub fn close_all_pins(app: &AppHandle) -> AppResult<()> {
    let ids: Vec<u32> = app
        .state::<SnipState>()
        .pins
        .lock()
        .expect("snip lock")
        .keys()
        .copied()
        .collect();
    for id in ids {
        close_pin(app, id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::pin_size;

    #[test]
    fn pins_open_at_their_own_size_unless_too_big() {
        assert_eq!(pin_size(400, 300, (2560, 1600)), (400, 300));
        // 60% of 1000×1000 is 600; the wider side sets the scale.
        assert_eq!(pin_size(2000, 1000, (1000, 1000)), (600, 300));
        assert_eq!(pin_size(5, 5, (1000, 1000)), (24, 24));
    }
}
