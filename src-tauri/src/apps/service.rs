//! Keeps each dock's app list current and acts on it. Pins are re-read when the taskbar's
//! record changes; windows are re-listed when Windows reports a window event (or docks come and
//! go), with bursts collapsed into one pass. Nothing is polled.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};

use base64::Engine;
use tauri::{AppHandle, Emitter, Manager};

use super::{click_action, ClickAction, DockApp, Launchable, Pin, APPS_CHANGED};
use crate::error::{AppError, AppResult};

/// Icons are drawn at about 28 logical pixels; 64 stays sharp up to 225% scaling.
const ICON_SIZE: i32 = 64;

#[derive(Default)]
pub struct AppsState {
    pins: Mutex<Vec<Pin>>,
    /// Each dock's apps, by window label.
    apps: Mutex<HashMap<String, Vec<DockApp>>>,
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

/// Docks were added, removed or moved to another screen: their lists change.
pub fn docks_changed(app: &AppHandle) {
    #[cfg(windows)]
    rescan_soon(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// Re-list windows, rebuild every dock's app list, and tell each dock whose list changed.
#[cfg(windows)]
fn rescan(app: &AppHandle) {
    // One pass at a time, so two passes can't publish out of order.
    static RUNNING: Mutex<()> = Mutex::new(());
    let _guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let windows = super::win::windows();
    let foreground = super::win::foreground();
    let docks = crate::shell::dock_screens(app);
    let state = app.state::<AppsState>();
    let pins = state.pins.lock().expect("apps lock").clone();
    let (lists, launchables) = super::build_per_dock(&pins, &windows, foreground, &docks);
    *state.launchables.lock().expect("apps lock") = launchables;
    let mut current = state.apps.lock().expect("apps lock");
    let changed: Vec<(String, Vec<DockApp>)> = lists
        .iter()
        .filter(|(label, apps)| current.get(*label) != Some(*apps))
        .map(|(label, apps)| (label.clone(), apps.clone()))
        .collect();
    *current = lists;
    drop(current);
    for (label, apps) in changed {
        if let Err(error) = app.emit_to(label.as_str(), APPS_CHANGED, &apps) {
            tracing::warn!(%error, "failed to send the app list");
        }
    }
}

/// The apps on the dock labelled `label`.
pub fn list(app: &AppHandle, label: &str) -> Vec<DockApp> {
    app.state::<AppsState>()
        .apps
        .lock()
        .expect("apps lock")
        .get(label)
        .cloned()
        .unwrap_or_default()
}

fn find(app: &AppHandle, label: &str, key: &str) -> AppResult<DockApp> {
    list(app, label)
        .into_iter()
        .find(|a| a.key == key)
        .ok_or_else(|| AppError::InvalidInput("unknown app".into()))
}

/// The icon for `source` as a data URL, extracted once and kept in memory. A miss isn't kept:
/// extraction can fail for a moment (an app mid-update, say), and a later ask should retry.
pub fn icon(app: &AppHandle, source: &str) -> Option<String> {
    let state = app.state::<AppsState>();
    if let Some(Some(cached)) = state.icons.lock().expect("apps lock").get(source) {
        return Some(cached.clone());
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
    if url.is_some() {
        state
            .icons
            .lock()
            .expect("apps lock")
            .insert(source.to_string(), url.clone());
    }
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
pub fn click(app: &AppHandle, label: &str, key: &str) -> AppResult<()> {
    let dock_app = find(app, label, key)?;
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

/// A right click on an app on dock `label`: its jump list, its windows, a new window, closing.
pub fn show_menu(app: &AppHandle, label: &str, key: &str, anchor_x: i32) -> AppResult<()> {
    let dock_app = find(app, label, key)?;
    let launchable = app
        .state::<AppsState>()
        .launchables
        .lock()
        .expect("apps lock")
        .iter()
        .any(|l| l.key == key);
    let menu = super::jump::AppMenu {
        dock: label.to_string(),
        key: dock_app.key.clone(),
        sections: super::jump::sections(dock_app.app_id.as_deref()),
        windows: dock_app.windows.iter().take(super::jump::MAX_WINDOWS).cloned().collect(),
        name: dock_app.name,
        icon: dock_app.icon,
        launchable,
    };
    crate::shell::panel::show_app_menu(app, menu, anchor_x)
}

/// A choice in the open app menu. Only what that menu offered can be done.
pub fn menu_action(app: &AppHandle, action: super::jump::MenuAction) -> AppResult<()> {
    use super::jump::MenuAction;
    let Some(crate::shell::panel::PanelContent::App { menu }) = crate::shell::panel::current(app) else {
        return Err(AppError::InvalidInput("no app menu is open".into()));
    };
    crate::shell::panel::close(app)?;
    match action {
        #[cfg(windows)]
        MenuAction::Window { hwnd } if menu.windows.iter().any(|w| w.hwnd == hwnd) => super::win::activate(hwnd),
        #[cfg(windows)]
        MenuAction::Open { path } if menu.offers(&path) => super::win::launch(&super::Launch::Program(path)),
        MenuAction::Launch if menu.launchable => launch_key(app, &menu.key)?,
        #[cfg(windows)]
        MenuAction::Close => {
            for window in find(app, &menu.dock, &menu.key)?.windows {
                super::win::close(window.hwnd);
            }
        }
        _ => return Err(AppError::InvalidInput("not in this menu".into())),
    }
    Ok(())
}
