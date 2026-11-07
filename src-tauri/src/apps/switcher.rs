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
/// Height of each window's live picture, in logical pixels (smaller when they don't fit).
const THUMB_H: f64 = 190.0;
/// Space around the cards, between them, and for each card's icon and title.
const PADDING: f64 = 44.0;
const GAP: f64 = 22.0;
const TITLE: f64 = 46.0;
const INSET: f64 = 8.0;

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
    /// No live picture (Windows couldn't size the window), so the page shows its icon instead.
    pub minimized: bool,
    /// The card, and the live picture inside it, in logical pixels within the switcher.
    pub card: Area,
    pub thumb: Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, TS, Default)]
#[ts(export)]
pub struct Area {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Eq for Area {}

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

/// Lay the cards out like Windows' Alt+Tab: a card per window, as wide as the window's shape
/// at a fixed picture height, in rows that wrap at `max_width` and are each centred.
/// `shapes` is each window's (width, height), zero when minimized. Returns the switcher's
/// size and each card's (card, picture) areas, in logical pixels.
pub fn layout(shapes: &[(u32, u32)], max_width: f64, max_height: f64) -> ((f64, f64), Vec<(Area, Area)>) {
    let room = (max_width - 2.0 * PADDING).max(200.0);
    let fit = |thumb_h: f64| -> Vec<Vec<(usize, f64)>> {
        let mut rows: Vec<Vec<(usize, f64)>> = vec![Vec::new()];
        let mut used = 0.0;
        for (i, &(w, h)) in shapes.iter().enumerate() {
            let aspect = if w > 0 && h > 0 {
                f64::from(w) / f64::from(h)
            } else {
                1.6
            };
            let width = (thumb_h * aspect).clamp(thumb_h * 0.75, thumb_h * 2.2) + 2.0 * INSET;
            let row = rows.last_mut().expect("a row");
            if !row.is_empty() && used + GAP + width > room {
                rows.push(vec![(i, width)]);
                used = width;
            } else {
                used += if row.is_empty() { width } else { GAP + width };
                row.push((i, width));
            }
        }
        rows
    };
    // Shrink the pictures until every row fits on the screen.
    let mut thumb_h = THUMB_H;
    let mut rows = fit(thumb_h);
    let card_h = |t: f64| TITLE + t + INSET;
    while thumb_h > 70.0 && rows.len() as f64 * (card_h(thumb_h) + GAP) - GAP + 2.0 * PADDING > max_height {
        thumb_h -= 10.0;
        rows = fit(thumb_h);
    }
    let row_width =
        |row: &Vec<(usize, f64)>| row.iter().map(|r| r.1).sum::<f64>() + GAP * (row.len().max(1) - 1) as f64;
    let width = rows.iter().map(row_width).fold(0.0, f64::max) + 2.0 * PADDING;
    let height = rows.len() as f64 * (card_h(thumb_h) + GAP) - GAP + 2.0 * PADDING;
    let mut areas = vec![(Area::default(), Area::default()); shapes.len()];
    for (r, row) in rows.iter().enumerate() {
        let mut x = (width - row_width(row)) / 2.0;
        let y = PADDING + r as f64 * (card_h(thumb_h) + GAP);
        for &(i, w) in row {
            let card = Area {
                x,
                y,
                width: w,
                height: card_h(thumb_h),
            };
            let (sw, sh) = shapes[i];
            let inner_w = w - 2.0 * INSET;
            let (tw, th) = if sw > 0 && sh > 0 {
                let scale = (inner_w / f64::from(sw)).min(thumb_h / f64::from(sh));
                (f64::from(sw) * scale, f64::from(sh) * scale)
            } else {
                (inner_w, thumb_h)
            };
            let thumb = Area {
                x: (x + INSET + (inner_w - tw) / 2.0).round(),
                y: (y + TITLE + (thumb_h - th) / 2.0).round(),
                width: tw.round(),
                height: th.round(),
            };
            areas[i] = (card, thumb);
            x += w + GAP;
        }
    }
    ((width.round(), height.round()), areas)
}

/// The windows to switch between, laid out for a screen `max_width` × `max_height` logical
/// pixels; also returns the switcher's size.
fn items(max_width: f64, max_height: f64) -> ((f64, f64), Vec<SwitchItem>) {
    let windows: Vec<_> = super::win::windows().into_iter().take(MAX_ITEMS).collect();
    // Minimized windows get their live picture too, at the size they restore to; only a window
    // Windows can't size shows its icon instead.
    let sizes: Vec<Option<(u32, u32)>> = windows.iter().map(|w| super::win::picture_size(w.hwnd)).collect();
    let minimized: Vec<bool> = sizes.iter().map(Option::is_none).collect();
    let shapes: Vec<(u32, u32)> = sizes.iter().map(|s| s.unwrap_or((0, 0))).collect();
    let (size, areas) = layout(&shapes, max_width, max_height);
    let items = windows
        .into_iter()
        .zip(minimized)
        .zip(areas)
        .map(|((w, minimized), (card, thumb))| SwitchItem {
            icon: match (&w.app_id, &w.exe) {
                (Some(id), _) => format!("aumid:{id}"),
                (None, Some(exe)) => format!("exe:{exe}"),
                (None, None) => String::new(),
            },
            hwnd: w.hwnd,
            title: w.title,
            app_name: w.app_name,
            minimized,
            card,
            thumb,
        })
        .collect();
    (size, items)
}

/// Live pictures drawn into the switcher, to remove when it closes.
static THUMBNAILS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn unregister_thumbnails() {
    use windows::Win32::Graphics::Dwm::DwmUnregisterThumbnail;
    for id in THUMBNAILS.lock().unwrap_or_else(|e| e.into_inner()).drain(..) {
        // SAFETY: a thumbnail id this module registered.
        let _ = unsafe { DwmUnregisterThumbnail(id) };
    }
}

/// Draw each window's live picture into the switcher window `dest`, at its card's picture area.
fn register_thumbnails(dest: isize, items: &[SwitchItem], scale: f64) {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        Graphics::Dwm::{
            DwmRegisterThumbnail, DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES, DWM_TNP_OPACITY,
            DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
        },
    };
    unregister_thumbnails();
    let px = |v: f64| (v * scale).round() as i32;
    let mut ids = Vec::new();
    for item in items.iter().filter(|i| !i.minimized) {
        // SAFETY: both are live top-level windows (the source may close meanwhile, which just
        // makes the calls fail); the properties are a fully initialised local.
        unsafe {
            let Ok(id) = DwmRegisterThumbnail(HWND(dest as *mut _), HWND(item.hwnd as *mut _)) else {
                continue;
            };
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION
                    | DWM_TNP_VISIBLE
                    | DWM_TNP_OPACITY
                    | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination: RECT {
                    left: px(item.thumb.x),
                    top: px(item.thumb.y),
                    right: px(item.thumb.x + item.thumb.width),
                    bottom: px(item.thumb.y + item.thumb.height),
                },
                opacity: 255,
                fVisible: true.into(),
                fSourceClientAreaOnly: false.into(),
                ..Default::default()
            };
            let _ = DwmUpdateThumbnailProperties(id, &props);
            ids.push(id);
        }
    }
    *THUMBNAILS.lock().unwrap_or_else(|e| e.into_inner()) = ids;
}

/// The screen the switcher opens on (the front window's, else the cursor's), its scale, and
/// its work area in logical pixels.
fn target_screen(app: &AppHandle) -> Option<(layout::Screen, f64)> {
    let screens = super::win::screens();
    let front = super::win::window_rect(super::win::foreground());
    let point = front.map_or_else(super::win::cursor, |r| {
        (r.x + r.width as i32 / 2, r.y + r.height as i32 / 2)
    });
    let screen = layout::screen_at(&screens, point)?.clone();
    let scale = app
        .monitor_from_point(f64::from(point.0), f64::from(point.1))
        .ok()
        .flatten()
        .map_or(1.0, |m| m.scale_factor());
    Some((screen, scale))
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
fn show(app: &AppHandle, state: &SwitcherState, size: (f64, f64), screen: &layout::Screen) -> AppResult<()> {
    let popup = window(app)?;
    app.emit_to(SWITCHER_LABEL, SWITCHER_STATE, state)?;
    let (width, height) = size;
    popup.set_size(tauri::LogicalSize::new(width, height))?;
    let outer = popup.outer_size()?;
    let work = screen.work;
    popup.set_position(PhysicalPosition::new(
        work.x + (work.width as i32 - outer.width as i32) / 2,
        work.y + (work.height as i32 - outer.height as i32) / 2,
    ))?;
    let hwnd = popup.hwnd()?.0 as isize;
    super::win::show_on_top_quietly(hwnd);
    register_thumbnails(hwnd, &state.items, popup.scale_factor()?);
    Ok(())
}

fn hide(app: &AppHandle) {
    unregister_thumbnails();
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
            let mut placed = None;
            if opening {
                let Some((screen, scale)) = target_screen(app) else {
                    return;
                };
                let max_w = f64::from(screen.work.width) / scale * 0.92;
                let max_h = f64::from(screen.work.height) / scale * 0.85;
                let (size, items) = items(max_w, max_h);
                if items.is_empty() {
                    return;
                }
                let selected = step(None, items.len(), forward);
                *state = Some(SwitcherState { items, selected });
                placed = Some((size, screen));
            } else if let Some(s) = state.as_mut() {
                s.selected = step(Some(s.selected), s.items.len(), forward);
            }
            let Some(snapshot) = state.clone() else { return };
            drop(state);
            let result = if let Some((size, screen)) = placed {
                show(app, &snapshot, size, &screen)
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
    fn cards_wrap_into_centred_rows_that_fit() {
        let wide = (1920, 1080);
        let ((w, h), areas) = layout(&[wide; 6], 1600.0, 1000.0);
        assert!(w <= 1600.0);
        assert!(h <= 1000.0);
        // More than one row, each card's picture inside its card.
        assert!(areas.iter().any(|(c, _)| c.y > areas[0].0.y));
        for (card, thumb) in &areas {
            assert!(thumb.x >= card.x && thumb.x + thumb.width <= card.x + card.width + 0.5);
            assert!(thumb.y >= card.y && thumb.y + thumb.height <= card.y + card.height + 0.5);
        }
        // A minimized window still gets a card.
        let (_, one) = layout(&[(0, 0)], 1600.0, 1000.0);
        assert!(one[0].0.width > 0.0);
    }
}
