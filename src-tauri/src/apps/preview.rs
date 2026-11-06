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
        DWM_TNP_OPACITY, DWM_TNP_RECTDESTINATION, DWM_TNP_SOURCECLIENTAREAONLY, DWM_TNP_VISIBLE,
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
const THUMB_HEIGHT: f64 = 132.0;

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

/// Draw each (not minimized) window's live picture into the popup, at its card's picture area.
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
            let props = DWM_THUMBNAIL_PROPERTIES {
                dwFlags: DWM_TNP_RECTDESTINATION
                    | DWM_TNP_VISIBLE
                    | DWM_TNP_OPACITY
                    | DWM_TNP_SOURCECLIENTAREAONLY,
                rcDestination: RECT {
                    left: px(card.thumb_x),
                    top: px(card.thumb_y),
                    right: px(card.thumb_x + card.thumb_width),
                    bottom: px(card.thumb_y + card.thumb_height),
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
            let size = if w.minimized {
                None
            } else {
                super::win::frame_rect(w.hwnd)
            };
            let (sw, sh) = size.map_or((0, 0), |r| (r.width, r.height));
            (w.hwnd, w.title.clone(), w.active, w.minimized, w.hung, sw, sh)
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
    };
    STATE.generation.fetch_add(1, Ordering::SeqCst);
    let unchanged = STATE.open.lock().unwrap_or_else(|e| e.into_inner()).as_ref() == Some(&content);
    let popup = window(app)?;
    if unchanged && popup.is_visible()? {
        return Ok(());
    }
    *STATE.open.lock().unwrap_or_else(|e| e.into_inner()) = Some(content.clone());
    app.emit(PREVIEW_OPEN, &content)?;
    popup.set_size(tauri::LogicalSize::new(width, height))?;
    let outer = popup.outer_size()?;
    let Some(rect) = shell::panel::popup_rect(app, label, anchor_x, (outer.width, outer.height)) else {
        return Ok(());
    };
    popup.set_position(PhysicalPosition::new(rect.x, rect.y))?;
    let hwnd = popup.hwnd()?.0 as isize;
    super::win::show_on_top_quietly(hwnd);
    register(hwnd, &content.cards, geometry.scale);
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
