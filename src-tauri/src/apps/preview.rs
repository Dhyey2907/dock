//! Window previews: hovering an app on the dock shows live pictures of its windows in a small
//! popup above it; clicking one brings that window forward. The pictures are DWM thumbnails
//! drawn into the popup window (`DwmRegisterThumbnail`) where its page lays out the cards. The
//! popup never takes the focus, so hovering doesn't change which app is in front.

use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use ts_rs::TS;
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Dwm::{
        DwmRegisterThumbnail, DwmUnregisterThumbnail, DwmUpdateThumbnailProperties, DWM_THUMBNAIL_PROPERTIES,
        DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_RECTSOURCE, DWM_TNP_SOURCECLIENTAREAONLY,
        DWM_TNP_VISIBLE,
    },
};

use super::{layout, DockApp};
use crate::{
    error::{AppError, AppResult},
    shell,
};

pub const PREVIEW_LABEL: &str = "preview";
/// Event with what the preview popup shows (to it), and that previews are showing (to docks).
pub const PREVIEW_OPEN: &str = "preview-open";
pub const PREVIEW_CLOSED: &str = "preview-closed";
/// Height of a card's picture, in logical pixels.
const THUMB_HEIGHT: f64 = 150.0;

/// What the preview popup shows.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewContent {
    pub dock: String,
    pub key: String,
    pub name: String,
    pub icon: String,
    pub width: f64,
    pub height: f64,
    pub cards: Vec<layout::PreviewCard>,
    /// Just a label (a button's name, like the taskbar's tooltips), instead of window pictures.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
}

struct State {
    open: Mutex<Option<PreviewContent>>,
    thumbnails: Mutex<Vec<isize>>,
    /// Bumped by every show, so a hide planned before it doesn't hide the new previews.
    generation: AtomicU64,
    /// The cursor is on the popup.
    hovered: AtomicBool,
}

static STATE: State = State {
    open: Mutex::new(None),
    thumbnails: Mutex::new(Vec::new()),
    generation: AtomicU64::new(0),
    hovered: AtomicBool::new(false),
};

fn window(app: &AppHandle) -> AppResult<tauri::WebviewWindow> {
    if let Some(window) = app.get_webview_window(PREVIEW_LABEL) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(app, PREVIEW_LABEL, WebviewUrl::App("index.html#preview".into()))
        .title("Dock window previews")
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
    shell::panel::round_corners(&window);
    // A quiet tool window from the start, so its size is measured with the style it shows with.
    super::win::show_on_top_quietly(window.hwnd()?.0 as isize);
    super::win::hide_quietly(window.hwnd()?.0 as isize);
    Ok(window)
}

fn unregister_all() {
    for id in STATE
        .thumbnails
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
    {
        // SAFETY: a thumbnail id this module registered.
        let _ = unsafe { DwmUnregisterThumbnail(id) };
    }
}

/// The part of `hwnd` its picture should show, relative to its window rectangle: the visible
/// frame, without the invisible resize borders Windows 10 and 11 put around it (and that hang
/// off the screen when it's maximized). Drawn whole, those borders leave a see-through margin
/// around the picture and squash it. `None` for a minimized window, which is drawn whole.
fn visible_part(hwnd: isize) -> Option<RECT> {
    if super::win::is_minimized(hwnd) {
        return None;
    }
    let outer = super::win::window_rect(hwnd)?;
    let frame = super::win::frame_rect(hwnd)?;
    let left = frame.x - outer.x;
    let top = frame.y - outer.y;
    (left >= 0 && top >= 0 && frame.width > 0 && frame.height > 0).then(|| RECT {
        left,
        top,
        right: left + frame.width as i32,
        bottom: top + frame.height as i32,
    })
}

/// Draw each (not minimized) window's live picture into the popup, at its card's picture area.
/// `scale` is the popup's own (its screen's), since the areas are in its page's pixels.
fn register(dest: isize, cards: &[layout::PreviewCard], scale: f64) {
    unregister_all();
    let px = |v: f64| (v * scale).round() as i32;
    let mut ids = Vec::new();
    for card in cards.iter().filter(|c| !c.minimized) {
        // SAFETY: both are live top-level windows (the source may close meanwhile, which just
        // makes the calls fail); the properties are a fully initialised local.
        unsafe {
            let Ok(id) = DwmRegisterThumbnail(HWND(dest as *mut c_void), HWND(card.hwnd as *mut c_void))
            else {
                continue;
            };
            let source = visible_part(card.hwnd);
            let mut flags =
                DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_OPACITY | DWM_TNP_SOURCECLIENTAREAONLY;
            if source.is_some() {
                flags |= DWM_TNP_RECTSOURCE;
            }
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: flags,
                rcDestination: RECT {
                    left: px(card.thumb_x),
                    top: px(card.thumb_y),
                    right: px(card.thumb_x + card.thumb_width),
                    bottom: px(card.thumb_y + card.thumb_height),
                },
                rcSource: source.unwrap_or_default(),
                opacity: 255,
                fVisible: true.into(),
                fSourceClientAreaOnly: false.into(),
                ..Default::default()
            };
            let _ = DwmUpdateThumbnailProperties(id, &props);
            ids.push(id);
        }
    }
    *STATE.thumbnails.lock().unwrap_or_else(|e| e.into_inner()) = ids;
}

/// Show `dock_app`'s windows above it on dock `label`, centred on `anchor_x` (screen pixels).
pub fn show(app: &AppHandle, label: &str, dock_app: &DockApp, anchor_x: i32) -> AppResult<()> {
    if dock_app.windows.is_empty() {
        return hide(app);
    }
    // A menu or popup that's open wins.
    if shell::panel::is_open(app) {
        return Ok(());
    }
    let geometry =
        shell::geometry_of(app, label).ok_or_else(|| AppError::InvalidInput("unknown dock".into()))?;
    let windows: Vec<_> = dock_app
        .windows
        .iter()
        .map(|w| {
            // Minimized windows get their live picture too (at the size they restore to); only
            // a window Windows can't size shows its icon instead.
            let size = super::win::picture_size(w.hwnd);
            let (sw, sh) = size.unwrap_or((0, 0));
            (w.hwnd, w.title.clone(), w.active, size.is_none(), w.hung, sw, sh)
        })
        .collect();
    let max_width = f64::from(geometry.screen_width) / geometry.scale - 48.0;
    let ((width, height), cards) = layout::preview_layout(&windows, THUMB_HEIGHT, max_width);
    let content = PreviewContent {
        dock: label.to_string(),
        key: dock_app.key.clone(),
        name: dock_app.name.clone(),
        icon: dock_app.icon.clone(),
        width,
        height,
        cards,
        tooltip: None,
    };
    present(app, label, anchor_x, content, geometry.scale)
}

/// Height of a tooltip, logical pixels.
const TOOLTIP_HEIGHT: f64 = 32.0;

/// A tooltip's width for `text`, logical pixels: roughly its text at 12 px, plus padding.
fn tooltip_width(text: &str) -> f64 {
    let chars = text.chars().count() as f64;
    (chars * 7.4 + 26.0).clamp(40.0, 420.0).round()
}

/// Show `text` as a tooltip over dock `label`, centred on `anchor_x` (screen pixels), the way the
/// taskbar labels its buttons.
pub fn show_tooltip(app: &AppHandle, label: &str, text: &str, anchor_x: i32) -> AppResult<()> {
    let text: String = text.chars().take(120).collect();
    if text.trim().is_empty() || shell::panel::is_open(app) {
        return Ok(());
    }
    let geometry =
        shell::geometry_of(app, label).ok_or_else(|| AppError::InvalidInput("unknown dock".into()))?;
    let content = PreviewContent {
        dock: label.to_string(),
        key: String::new(),
        name: String::new(),
        icon: String::new(),
        width: tooltip_width(&text),
        height: TOOLTIP_HEIGHT,
        cards: Vec::new(),
        tooltip: Some(text),
    };
    present(app, label, anchor_x, content, geometry.scale)
}

/// Put `content` in the popup above dock `label` at `anchor_x`, and its live pictures in it.
fn present(
    app: &AppHandle,
    label: &str,
    anchor_x: i32,
    content: PreviewContent,
    scale: f64,
) -> AppResult<()> {
    let (width, height) = (content.width, content.height);
    STATE.generation.fetch_add(1, Ordering::SeqCst);
    let unchanged = STATE.open.lock().unwrap_or_else(|e| e.into_inner()).as_ref() == Some(&content);
    let popup = window(app)?;
    if unchanged && popup.is_visible()? {
        return Ok(());
    }
    *STATE.open.lock().unwrap_or_else(|e| e.into_inner()) = Some(content.clone());
    app.emit(PREVIEW_OPEN, &content)?;
    // Sized for the screen it was last on; when it lands on a screen with another scale,
    // Windows rescales it, so size and place it again for the new one.
    let mut sized_for = popup.scale_factor()?;
    for _ in 0..2 {
        shell::panel::set_page_size(&popup, width, height)?;
        let outer = popup.outer_size()?;
        let Some(rect) = shell::panel::popup_rect(app, label, anchor_x, (outer.width, outer.height)) else {
            return Ok(());
        };
        popup.set_position(PhysicalPosition::new(rect.x, rect.y))?;
        let now = popup.scale_factor()?;
        if (now - sized_for).abs() < 0.001 {
            break;
        }
        sized_for = now;
    }
    let hwnd = popup.hwnd()?.0 as isize;
    super::win::show_on_top_quietly(hwnd);
    // The pictures go in the popup's page pixels, so they use its scale (the dock's may differ).
    let scale = if sized_for > 0.0 { sized_for } else { scale };
    register(hwnd, &content.cards, scale);
    Ok(())
}

/// What the popup shows (for its page when it first loads).
pub fn current() -> Option<PreviewContent> {
    STATE.open.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

pub fn hide(app: &AppHandle) -> AppResult<()> {
    unregister_all();
    STATE.hovered.store(false, Ordering::SeqCst);
    let was_open = STATE.open.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(window) = app.get_webview_window(PREVIEW_LABEL) {
        window.hide()?;
        // It was shown straight through Windows, so hide it the same way.
        if let Ok(hwnd) = window.hwnd() {
            super::win::hide_quietly(hwnd.0 as isize);
        }
    }
    if was_open.is_some() {
        app.emit(PREVIEW_CLOSED, ())?;
    }
    Ok(())
}

/// Hide after `delay` unless the cursor reaches the popup (or another app's previews open)
/// meanwhile.
pub fn hide_later(app: &AppHandle, delay: Duration) {
    let generation = STATE.generation.load(Ordering::SeqCst);
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if STATE.generation.load(Ordering::SeqCst) == generation && !STATE.hovered.load(Ordering::SeqCst) {
            if let Err(error) = hide(&app) {
                tracing::warn!(%error, "failed to hide window previews");
            }
        }
    });
}

/// The cursor arrived on (or left) the popup.
pub fn set_hovered(app: &AppHandle, hovered: bool) {
    STATE.hovered.store(hovered, Ordering::SeqCst);
    if !hovered {
        hide_later(app, Duration::from_millis(250));
    }
}

fn offered(hwnd: isize) -> bool {
    STATE
        .open
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .is_some_and(|c| c.cards.iter().any(|card| card.hwnd == hwnd))
}

/// A card was clicked: bring its window forward.
pub fn activate(app: &AppHandle, hwnd: isize) -> AppResult<()> {
    if !offered(hwnd) {
        return Err(AppError::InvalidInput("not a previewed window".into()));
    }
    hide(app)?;
    super::win::activate(hwnd);
    Ok(())
}

/// A card's close button: close its window.
pub fn close_window(app: &AppHandle, hwnd: isize) -> AppResult<()> {
    if !offered(hwnd) {
        return Err(AppError::InvalidInput("not a previewed window".into()));
    }
    hide(app)?;
    super::win::close(hwnd);
    Ok(())
}
