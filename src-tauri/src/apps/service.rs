//! Keeps the dock's app list current and acts on it. Pins are re-read when the taskbar's record
//! changes; windows are re-listed when Windows reports a window event, with bursts collapsed
//! into one pass. Nothing is polled.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use base64::Engine;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    AppHandle, Emitter, Manager,
};

use super::{click_action, ClickAction, DockApp, Launchable, Pin, APPS_CHANGED};
use crate::error::{AppError, AppResult};

/// Icons are drawn at about 28 logical pixels; 64 stays sharp up to 225% scaling.
const ICON_SIZE: i32 = 64;
/// Menu item ids for app menus start with this.
const MENU_PREFIX: &str = "app:";
/// Longest window title shown in an app's menu.
const MENU_TITLE_CHARS: usize = 60;

#[derive(Default)]
pub struct AppsState {
    pins: Mutex<Vec<Pin>>,
    apps: Mutex<Vec<DockApp>>,
    launchables: Mutex<Vec<Launchable>>,
    /// Data URLs by icon source; `None` when the source has no icon.
    icons: Mutex<HashMap<String, Option<String>>>,
}

pub fn init(app: &AppHandle) {
    #[cfg(windows)]
    {
        let handle = app.clone();
        std::thread::spawn(move || reload_pins(&handle));
        let handle = app.clone();
        super::win::watch_pins(move || reload_pins(&handle));
        let handle = app.clone();
        super::win::watch_windows(move || rescan_soon(&handle));
    }
    #[cfg(not(windows))]
    let _ = app;
}

#[cfg(windows)]
fn reload_pins(app: &AppHandle) {
    let pins = super::win::read_pins();
    tracing::debug!(count = pins.len(), "read taskbar pins");
    for pin in &pins {
        tracing::trace!(name = %pin.name, app_id = ?pin.app_id, exe = ?pin.exe, "pin");
    }
    *app.state::<AppsState>().pins.lock().expect("apps lock") = pins;
    rescan(app);
}

/// One re-list for a burst of window events.
#[cfg(windows)]
fn rescan_soon(app: &AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        PENDING.store(false, Ordering::SeqCst);
        rescan(&app);
    });
}

/// Re-list windows, rebuild the app list, and tell the page if anything changed.
#[cfg(windows)]
fn rescan(app: &AppHandle) {
    // One pass at a time, so two passes can't publish out of order.
    static RUNNING: Mutex<()> = Mutex::new(());
    let _guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let windows = super::win::windows();
    let foreground = super::win::foreground();
    let state = app.state::<AppsState>();
    let pins = state.pins.lock().expect("apps lock").clone();
    let (apps, launchables) = super::build(&pins, &windows, foreground);
    *state.launchables.lock().expect("apps lock") = launchables;
    let mut current = state.apps.lock().expect("apps lock");
    if *current != apps {
        *current = apps.clone();
        drop(current);
        if let Err(error) = app.emit(APPS_CHANGED, &apps) {
            tracing::warn!(%error, "failed to send the app list");
        }
    }
}

pub fn list(app: &AppHandle) -> Vec<DockApp> {
    app.state::<AppsState>().apps.lock().expect("apps lock").clone()
}

fn find(app: &AppHandle, key: &str) -> AppResult<DockApp> {
    list(app)
        .into_iter()
        .find(|a| a.key == key)
        .ok_or_else(|| AppError::InvalidInput("unknown app".into()))
}

/// The icon for `source` as a data URL, extracted once and kept in memory.
pub fn icon(app: &AppHandle, source: &str) -> Option<String> {
    let state = app.state::<AppsState>();
    if let Some(cached) = state.icons.lock().expect("apps lock").get(source) {
        return cached.clone();
    }
    #[cfg(windows)]
    let url = super::win::icon_png(source, ICON_SIZE).map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    #[cfg(not(windows))]
    let url = None;
    state
        .icons
        .lock()
        .expect("apps lock")
        .insert(source.to_string(), url.clone());
    url
}

fn launch_key(app: &AppHandle, key: &str) -> AppResult<()> {
    let launchable = app
        .state::<AppsState>()
        .launchables
        .lock()
        .expect("apps lock")
        .iter()
        .find(|l| l.key == key)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("app can't be started".into()))?;
    #[cfg(windows)]
    super::win::launch(&launchable.launch);
    #[cfg(not(windows))]
    let _ = launchable;
    Ok(())
}

/// A left click: launch, bring forward, or minimize, as the taskbar would.
pub fn click(app: &AppHandle, key: &str) -> AppResult<()> {
    let dock_app = find(app, key)?;
    #[cfg(windows)]
    match click_action(&dock_app) {
        ClickAction::Launch => launch_key(app, key)?,
        ClickAction::Activate(hwnd) => super::win::activate(hwnd),
        ClickAction::Minimize(hwnd) => super::win::minimize(hwnd),
    }
    #[cfg(not(windows))]
    let _ = click_action(&dock_app);
    Ok(())
}

/// A middle click: start another instance.
pub fn launch(app: &AppHandle, key: &str) -> AppResult<()> {
    launch_key(app, key)
}

/// `&` marks a keyboard shortcut in menu text; double it to show it literally.
fn menu_text(text: &str) -> String {
    let mut short: String = text.chars().take(MENU_TITLE_CHARS).collect();
    if text.chars().count() > MENU_TITLE_CHARS {
        short.push('…');
    }
    short.replace('&', "&&")
}

/// A right click: the app's windows, a new window, and closing.
pub fn show_menu(app: &AppHandle, key: &str) -> AppResult<()> {
    let dock_app = find(app, key)?;
    let menu = Menu::new(app)?;
    for window in dock_app.windows.iter().take(12) {
        let title = if window.title.is_empty() {
            &dock_app.name
        } else {
            &window.title
        };
        let id = format!("{MENU_PREFIX}win:{}", window.hwnd);
        menu.append(&MenuItem::with_id(app, id, menu_text(title), true, None::<&str>)?)?;
    }
    if !dock_app.windows.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    let launchable = app
        .state::<AppsState>()
        .launchables
        .lock()
        .expect("apps lock")
        .iter()
        .any(|l| l.key == key);
    if launchable {
        let id = format!("{MENU_PREFIX}new:{key}");
        menu.append(&MenuItem::with_id(
            app,
            id,
            menu_text(&dock_app.name),
            true,
            None::<&str>,
        )?)?;
    }
    let close = match dock_app.windows.len() {
        0 => None,
        1 => Some("Close window"),
        _ => Some("Close all windows"),
    };
    if let Some(label) = close {
        menu.append(&MenuItem::with_id(
            app,
            format!("{MENU_PREFIX}close:{key}"),
            label,
            true,
            None::<&str>,
        )?)?;
    }
    let window = crate::shell::dock_window(app).ok_or_else(|| AppError::Platform("no dock window".into()))?;
    window.popup_menu(&menu)?;
    Ok(())
}

/// A choice from an app menu.
pub fn on_menu(app: &AppHandle, id: &str) {
    let Some(action) = id.strip_prefix(MENU_PREFIX) else {
        return;
    };
    let result = (|| -> AppResult<()> {
        match action.split_once(':') {
            #[cfg(windows)]
            Some(("win", hwnd)) => {
                let hwnd = hwnd
                    .parse()
                    .map_err(|_| AppError::InvalidInput("window".into()))?;
                super::win::activate(hwnd);
            }
            Some(("new", key)) => launch_key(app, key)?,
            #[cfg(windows)]
            Some(("close", key)) => {
                for window in find(app, key)?.windows {
                    super::win::close(window.hwnd);
                }
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(error) = result {
        tracing::warn!(%error, "app menu action failed");
    }
}

#[cfg(test)]
mod tests {
    use super::menu_text;

    #[test]
    fn menu_text_escapes_ampersands_and_shortens() {
        assert_eq!(menu_text("Tom & Jerry"), "Tom && Jerry");
        let long = "x".repeat(80);
        assert_eq!(menu_text(&long).chars().count(), 61);
        assert!(menu_text(&long).ends_with('…'));
    }
}
