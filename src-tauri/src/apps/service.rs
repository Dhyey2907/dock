//! Keeps each dock's app list current and acts on it. Pins are re-read when the taskbar's
//! record changes; windows are re-listed when Windows reports a window event (or docks come and
//! go), with bursts collapsed into one pass. Each pass also applies the user's touches (their
//! own pins, names, icons, hidden apps, order), remembers usage and recently closed apps, and
//! notices screens coming and going. Only "not responding" needs a slow look every few seconds.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
    time::{Duration, Instant},
};

use base64::Engine;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::{
    click_action, custom,
    history::{self, History},
    jump::{DockMenu, DockMenuAction, MediaKey, MenuAction, MenuMedia, NamedItem, StackView},
    layout, ClickAction, DockApp, ItemKind, Launchable, Pin, APPS_CHANGED,
};
use crate::{
    error::{AppError, AppResult},
    settings::{AppOverride, EmptyAction, QuickActionKind, Settings, SettingsStore},
    shell::{self, placement::Rect, DOCK_LABEL},
};

/// Event naming an app that is starting (its icon bounces until a window appears).
pub const APP_LAUNCHING: &str = "app-launching";
/// Icons are drawn at about 28 logical pixels; 64 stays sharp up to 225% scaling.
const ICON_SIZE: i32 = 64;
/// How often the remembered usage and recently closed apps are written, at most.
const HISTORY_SAVE_EVERY: Duration = Duration::from_secs(30);
/// Pictures bigger than this aren't used as icons.
const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// One app button's place in its dock window, in CSS pixels, as the page measured it.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ButtonRect {
    pub key: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The last press on an empty part of a dock, for double-clicks and press-and-hold.
#[derive(Default)]
struct Press {
    at: Option<Instant>,
    point: (i32, i32),
    previous: Option<(Instant, (i32, i32))>,
    handled: bool,
}

#[derive(Default)]
pub struct AppsState {
    pins: Mutex<Vec<Pin>>,
    /// Each dock's apps, by window label.
    apps: Mutex<HashMap<String, Vec<DockApp>>>,
    launchables: Mutex<Vec<Launchable>>,
    /// Data URLs by icon source; `None` when the source has no icon.
    icons: Mutex<HashMap<String, Option<String>>>,
    history: Mutex<History>,
    data_dir: OnceLock<PathBuf>,
    favicon_dir: OnceLock<PathBuf>,
    saved_at: Mutex<Option<Instant>>,
    started: OnceLock<Instant>,
    /// Every dock's apps at the last pass (`None` before the first).
    previous: Mutex<Option<Vec<DockApp>>>,
    /// The screens and where windows were at the last pass.
    places: Mutex<(Vec<layout::Screen>, Vec<layout::Placement>)>,
    /// Windows moved off a screen that went away, to put back when it returns.
    displaced: Mutex<HashMap<isize, layout::Displaced>>,
    /// The icon pack folder and its PNG file names, read once per folder.
    pack: Mutex<Option<(String, Vec<String>)>>,
    /// Each dock's app buttons on screen (physical pixels), by label, then app key.
    buttons: Mutex<HashMap<String, HashMap<String, Rect>>>,
    /// Windows not responding at the last look.
    hung: Mutex<HashSet<isize>>,
    press: Mutex<Press>,
}

pub fn init(app: &AppHandle) {
    let state = app.state::<AppsState>();
    let _ = state.started.set(Instant::now());
    if let Ok(dir) = app.path().app_data_dir() {
        *state.history.lock().expect("apps lock") = history::load(&dir);
        let _ = state.data_dir.set(dir);
    }
    if let Ok(dir) = app.path().app_cache_dir() {
        let _ = state.favicon_dir.set(dir.join("favicons"));
    }
    #[cfg(windows)]
    {
        let handle = app.clone();
        std::thread::spawn(move || reload_pins(&handle));
        let handle = app.clone();
        super::win::watch_pins(move || reload_pins(&handle));
        let handle = app.clone();
        super::win::watch_windows(move || rescan_soon(&handle));
        let handle = app.clone();
        super::hooks::start_shell_hook(move || rescan_soon(&handle));
        watch_hung(app);
    }
    settings_changed(app);
}

/// Settings changed: re-read the icon pack, switch the shortcuts and Alt+Tab, re-list.
pub fn settings_changed(app: &AppHandle) {
    let settings = app.state::<SettingsStore>().get();
    *app.state::<AppsState>().pack.lock().expect("apps lock") = None;
    #[cfg(windows)]
    {
        let handle = app.clone();
        super::hooks::set_alt_tab(settings.apps.alt_tab, move |key| {
            super::switcher::handle(&handle, key)
        });
        rescan_soon(app);
    }
    if let Err(error) = shell::shortcut::set_numbers(app, settings.apps.number_shortcuts) {
        tracing::warn!(%error, "Alt+number shortcuts not set");
    }
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

/// Docks were added, removed or moved to another screen (or screens changed): lists change.
pub fn docks_changed(app: &AppHandle) {
    #[cfg(windows)]
    rescan_soon(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// A program that stops responding sends no window events; look every few seconds.
#[cfg(windows)]
fn watch_hung(app: &AppHandle) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("hung-windows".into())
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(4));
            let state = app.state::<AppsState>();
            let listed: Vec<isize> = state
                .apps
                .lock()
                .expect("apps lock")
                .values()
                .flatten()
                .flat_map(|a| a.windows.iter().map(|w| w.hwnd))
                .collect();
            let now: HashSet<isize> = listed.into_iter().filter(|&h| super::win::is_hung(h)).collect();
            if now != *state.hung.lock().expect("apps lock") {
                rescan_soon(&app);
            }
        });
}

/// The icon pack's PNG names, read once per folder.
fn pack_files(state: &AppsState, dir: &str) -> Vec<String> {
    let mut pack = state.pack.lock().expect("apps lock");
    if let Some((cached, files)) = pack.as_ref() {
        if cached == dir {
            return files.clone();
        }
    }
    let files: Vec<String> = if dir.is_empty() {
        Vec::new()
    } else {
        std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .filter(|n| n.to_lowercase().ends_with(".png"))
                    .collect()
            })
            .unwrap_or_default()
    };
    *pack = Some((dir.to_string(), files.clone()));
    files
}

/// Re-list windows, rebuild every dock's app list, and tell each dock whose list changed.
#[cfg(windows)]
fn rescan(app: &AppHandle) {
    // One pass at a time, so two passes can't publish out of order.
    static RUNNING: Mutex<()> = Mutex::new(());
    let _guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    let windows = super::win::windows();
    let foreground = super::win::foreground();
    let docks = shell::dock_screens(app);
    let settings = app.state::<SettingsStore>().get();
    let state = app.state::<AppsState>();
    let pins = state.pins.lock().expect("apps lock").clone();
    let (mut lists, mut launchables) = super::build_per_dock(&pins, &windows, foreground, &docks);
    let (custom_items, custom_launch) = custom::custom_pins(&settings.apps);
    launchables.extend(custom_launch);
    let pack = pack_files(&state, &settings.apps.icon_pack);
    let usage = state.history.lock().expect("apps lock").usage.clone();
    let touches = custom::Touches {
        settings: &settings.apps,
        usage: &usage,
        pack_dir: &settings.apps.icon_pack,
        pack_files: &pack,
    };
    for (label, apps) in lists.iter_mut() {
        let list = std::mem::take(apps);
        *apps = custom::apply(list, label == DOCK_LABEL, &custom_items, &touches);
    }

    let all: Vec<DockApp> = lists.values().flatten().cloned().collect();
    let monitors: HashMap<isize, String> = windows
        .iter()
        .filter_map(|w| Some((w.hwnd, w.monitor.clone()?)))
        .collect();
    let screens = super::win::screens();
    remember(&state, &settings, &all, &launchables, &monitors, &screens);
    rearrange(&state, &settings, &screens, &windows);
    *state.hung.lock().expect("apps lock") = windows.iter().filter(|w| w.hung).map(|w| w.hwnd).collect();
    *state.launchables.lock().expect("apps lock") = launchables;

    let mut current = state.apps.lock().expect("apps lock");
    let changed: Vec<(String, Vec<DockApp>)> = lists
        .iter()
        .filter(|(label, apps)| current.get(*label) != Some(*apps))
        .map(|(label, apps)| (label.clone(), apps.clone()))
        .collect();
    *current = lists;
    drop(current);
    update_min_rects(app);
    for (label, apps) in changed {
        if let Err(error) = app.emit_to(label.as_str(), APPS_CHANGED, &apps) {
            tracing::warn!(%error, "failed to send the app list");
        }
    }
}

/// Usage, recently closed apps and last screens; apps that just opened go back to the screen
/// they were last used on.
#[cfg(windows)]
fn remember(
    state: &AppsState,
    settings: &Settings,
    all: &[DockApp],
    launchables: &[Launchable],
    monitors: &HashMap<isize, String>,
    screens: &[layout::Screen],
) {
    let now = state.started.get().map_or(0, |s| s.elapsed().as_secs());
    let mut previous = state.previous.lock().expect("apps lock");
    let mut history = state.history.lock().expect("apps lock");
    let opened = history.observe(
        previous.as_deref(),
        &history::Seen {
            apps: all,
            launchables,
            monitors,
            now,
        },
    );
    *previous = Some(all.to_vec());
    drop(previous);
    for (key, hwnd) in opened {
        let Some(current) = monitors.get(&hwnd) else {
            continue;
        };
        let wanted = history.last_screen.get(&key).cloned();
        let target = wanted
            .filter(|w| settings.apps.reopen_on_last_screen && w != current)
            .and_then(|w| {
                Some((
                    screens.iter().find(|s| s.name == *current)?,
                    screens.iter().find(|s| s.name == w)?,
                ))
            });
        match (target, super::win::window_rect(hwnd)) {
            (Some((from, to)), Some(rect)) => {
                let maximized = super::win::is_maximized(hwnd);
                super::win::move_window(
                    hwnd,
                    layout::move_to_screen(rect, from.work, to.work),
                    false,
                    maximized,
                );
                tracing::debug!(%key, to = %to.name, "reopened on its last screen");
                history.set_screen(&key, &to.name);
            }
            _ => history.set_screen(&key, current),
        }
    }
    if history.dirty {
        let mut saved_at = state.saved_at.lock().expect("apps lock");
        if saved_at.is_none_or(|at| at.elapsed() >= HISTORY_SAVE_EVERY) {
            if let Some(dir) = state.data_dir.get() {
                if let Err(error) = history::save(dir, &history) {
                    tracing::warn!(%error, "could not save app history");
                }
            }
            history.dirty = false;
            *saved_at = Some(Instant::now());
        }
    }
}

/// When screens come or go: move windows off a screen that went away, and back when it
/// returns. Then note where every window is now.
#[cfg(windows)]
fn rearrange(state: &AppsState, settings: &Settings, screens: &[layout::Screen], windows: &[super::WinInfo]) {
    let names = |list: &[layout::Screen]| {
        let mut names: Vec<String> = list.iter().map(|s| s.name.clone()).collect();
        names.sort();
        names
    };
    let mut places = state.places.lock().expect("apps lock");
    if !places.0.is_empty() && !screens.is_empty() && names(&places.0) != names(screens) {
        let mut displaced = state.displaced.lock().expect("apps lock");
        if settings.apps.rearrange_on_unplug {
            let plan = layout::rearrange(&places.0, screens, &places.1, &displaced, super::win::is_window);
            for &(hwnd, rect, maximize) in &plan.moves {
                super::win::move_window(hwnd, rect, false, maximize);
            }
            tracing::info!(moved = plan.moves.len(), "screens changed; windows rearranged");
            for d in plan.displaced {
                displaced.insert(d.placement.hwnd, d);
            }
            for hwnd in plan.returned {
                displaced.remove(&hwnd);
            }
        } else {
            displaced.clear();
        }
    }
    let offset = screens
        .iter()
        .find(|s| s.primary)
        .map_or((0, 0), |s| (s.work.x - s.rect.x, s.work.y - s.rect.y));
    places.1 = windows
        .iter()
        .filter_map(|w| super::win::placement(w.hwnd, offset))
        .collect();
    places.0 = screens.to_vec();
}

/// Tell the shell hook where each listed window shrinks to when minimized: its app's button.
fn update_min_rects(app: &AppHandle) {
    #[cfg(windows)]
    {
        let enabled = app.state::<SettingsStore>().get().apps.minimize_animation;
        let state = app.state::<AppsState>();
        let mut rects = HashMap::new();
        if enabled {
            let buttons = state.buttons.lock().expect("apps lock");
            for (label, apps) in state.apps.lock().expect("apps lock").iter() {
                let Some(on_dock) = buttons.get(label) else {
                    continue;
                };
                for dock_app in apps {
                    if let Some(rect) = on_dock.get(&dock_app.key) {
                        for window in &dock_app.windows {
                            rects.insert(window.hwnd, *rect);
                        }
                    }
                }
            }
        }
        super::hooks::set_min_rects(rects);
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// The page measured its app buttons.
pub fn set_button_rects(app: &AppHandle, label: &str, rects: Vec<ButtonRect>) -> AppResult<()> {
    let Some(geometry) = shell::geometry_of(app, label) else {
        return Ok(());
    };
    let on_screen: HashMap<String, Rect> = rects
        .into_iter()
        .filter(|r| [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()))
        .map(|r| {
            let rect = layout::to_screen(
                r.x,
                r.y,
                r.width,
                r.height,
                (geometry.x, geometry.y),
                geometry.scale,
            );
            (r.key, rect)
        })
        .collect();
    app.state::<AppsState>()
        .buttons
        .lock()
        .expect("apps lock")
        .insert(label.to_string(), on_screen);
    update_min_rects(app);
    Ok(())
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

/// Every dock's apps, each once (for Settings' app pickers).
pub fn all(app: &AppHandle) -> Vec<DockApp> {
    let lists = app.state::<AppsState>().apps.lock().expect("apps lock").clone();
    let mut out: Vec<DockApp> = lists.get(DOCK_LABEL).cloned().unwrap_or_default();
    for apps in lists.values() {
        for a in apps {
            if !out.iter().any(|o| o.key == a.key) {
                out.push(a.clone());
            }
        }
    }
    out
}

/// An app in a dock folder's popup.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderEntry {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub running: bool,
}

/// A dock folder's popup: its name and the apps in it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderView {
    /// The dock it was opened from.
    pub dock: String,
    pub id: String,
    pub name: String,
    pub apps: Vec<FolderEntry>,
}

impl FolderView {
    /// The popup's size in logical pixels: up to four apps a row.
    pub fn size(&self) -> (f64, f64) {
        const CELL_W: f64 = 84.0;
        const CELL_H: f64 = 88.0;
        let count = self.apps.len().max(1);
        let cols = count.clamp(3, 4);
        let rows = count.div_ceil(cols);
        (cols as f64 * CELL_W + 32.0, rows as f64 * CELL_H + 70.0)
    }
}

/// Open (or close) the popup of the folder `id` on the dock `label`.
pub fn show_folder(app: &AppHandle, label: &str, id: &str, anchor_x: i32) -> AppResult<()> {
    let settings = app.state::<SettingsStore>().get();
    let folder = settings
        .apps
        .folders
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| AppError::InvalidInput("unknown folder".into()))?;
    let apps = list(app, label);
    let view = FolderView {
        dock: label.to_string(),
        id: folder.id.clone(),
        name: folder.name.clone(),
        apps: folder
            .keys
            .iter()
            .filter_map(|key| apps.iter().find(|a| &a.key == key))
            .map(|a| FolderEntry {
                key: a.key.clone(),
                name: a.name.clone(),
                icon: a.icon.clone(),
                running: !a.windows.is_empty(),
            })
            .collect(),
    };
    #[cfg(windows)]
    let _ = super::preview::hide(app);
    crate::shell::panel::toggle_folder(app, view, anchor_x)
}

/// An app in a folder's popup was clicked: close the popup and do what clicking its icon does.
pub fn folder_click(app: &AppHandle, label: &str, key: &str) -> AppResult<()> {
    crate::shell::panel::close(app)?;
    click(app, label, key, 0)
}

fn find(app: &AppHandle, label: &str, key: &str) -> AppResult<DockApp> {
    list(app, label)
        .into_iter()
        .find(|a| a.key == key)
        .ok_or_else(|| AppError::InvalidInput("unknown app".into()))
}

/// A picture file (the user's icon, or an icon pack's) as a PNG.
fn image_file(path: &str) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path).ok()?.len();
    if size > MAX_IMAGE_BYTES {
        return None;
    }
    super::favicon::to_png(&std::fs::read(path).ok()?)
}

/// The icon for `source` as a data URL, made once and kept in memory. A miss isn't kept:
/// extraction can fail for a moment (an app mid-update, a site offline), and a later ask
/// should retry.
pub fn icon(app: &AppHandle, source: &str) -> Option<String> {
    let state = app.state::<AppsState>();
    if let Some(Some(cached)) = state.icons.lock().expect("apps lock").get(source) {
        return Some(cached.clone());
    }
    let png = match source.split_once(':') {
        Some(("image", path)) => image_file(path),
        Some(("favicon", address)) => state
            .favicon_dir
            .get()
            .and_then(|dir| super::favicon::icon(dir, address)),
        #[cfg(windows)]
        _ => super::win::icon_png(source, ICON_SIZE),
        #[cfg(not(windows))]
        _ => None,
    };
    let url = png.map(|png| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        )
    });
    if url.is_some() {
        state
            .icons
            .lock()
            .expect("apps lock")
            .insert(source.to_string(), url.clone());
    }
    url
}

fn launchable(app: &AppHandle, key: &str) -> AppResult<Launchable> {
    app.state::<AppsState>()
        .launchables
        .lock()
        .expect("apps lock")
        .iter()
        .find(|l| l.key == key)
        .cloned()
        .ok_or_else(|| AppError::InvalidInput("app can't be started".into()))
}

fn launching(app: &AppHandle, key: &str) {
    if let Err(error) = app.emit(APP_LAUNCHING, key) {
        tracing::warn!(%error, "failed to announce a launch");
    }
}

fn launch_key(app: &AppHandle, key: &str) -> AppResult<()> {
    let launchable = launchable(app, key)?;
    #[cfg(windows)]
    super::win::launch(&launchable.launch);
    #[cfg(not(windows))]
    let _ = launchable;
    launching(app, key);
    Ok(())
}

/// A left click: launch, bring forward, or minimize, as the taskbar would. A pinned folder
/// fans out its files instead.
pub fn click(app: &AppHandle, label: &str, key: &str, anchor_x: i32) -> AppResult<()> {
    let dock_app = find(app, label, key)?;
    #[cfg(windows)]
    let _ = super::preview::hide(app);
    if dock_app.kind == ItemKind::Stack {
        return show_stack(app, label, &dock_app, anchor_x);
    }
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

/// A file dragged over an app's icon for a moment: bring its window forward.
pub fn raise(app: &AppHandle, label: &str, key: &str) -> AppResult<()> {
    let dock_app = find(app, label, key)?;
    #[cfg(windows)]
    if let Some(window) = dock_app.windows.first() {
        super::win::activate(window.hwnd);
    }
    #[cfg(not(windows))]
    let _ = dock_app;
    Ok(())
}

/// Alt+`n`: what a click on the `n`th pinned item of the main dock does.
pub fn open_nth(app: &AppHandle, n: usize) {
    let apps = list(app, DOCK_LABEL);
    let Some(item) = custom::nth_pinned(&apps, n) else {
        return;
    };
    let anchor = app
        .state::<AppsState>()
        .buttons
        .lock()
        .expect("apps lock")
        .get(DOCK_LABEL)
        .and_then(|b| b.get(&item.key))
        .map_or(0, |r| r.x + r.width as i32 / 2);
    if let Err(error) = click(app, DOCK_LABEL, &item.key, anchor) {
        tracing::warn!(%error, n, "Alt+number didn't open an app");
    }
}

fn show_stack(app: &AppHandle, label: &str, dock_app: &DockApp, anchor_x: i32) -> AppResult<()> {
    let settings = app.state::<SettingsStore>().get();
    let id = dock_app.key.trim_start_matches("stack:");
    let pin = settings
        .apps
        .custom_pins
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AppError::InvalidInput("unknown stack".into()))?;
    let entries = std::fs::read_dir(&pin.target)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    let meta = e.metadata().ok()?;
                    #[cfg(windows)]
                    let hidden = {
                        use std::os::windows::fs::MetadataExt;
                        meta.file_attributes() & 0x2 != 0
                    };
                    #[cfg(not(windows))]
                    let hidden = false;
                    Some(super::jump::FolderEntry {
                        name: e.file_name().to_string_lossy().into_owned(),
                        path: e.path().to_string_lossy().into_owned(),
                        modified: meta
                            .modified()
                            .ok()
                            .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                            .map_or(0, |d| d.as_secs()),
                        hidden,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let stack = StackView {
        dock: label.to_string(),
        key: dock_app.key.clone(),
        name: dock_app.name.clone(),
        path: pin.target.clone(),
        items: super::jump::stack_items(entries),
    };
    shell::panel::show_stack(app, label, stack, anchor_x)
}

/// Open a file (or the folder itself) from the open stack.
pub fn stack_open(app: &AppHandle, path: &str) -> AppResult<()> {
    let Some(shell::panel::PanelContent::Stack { stack }) = shell::panel::current(app) else {
        return Err(AppError::InvalidInput("no stack is open".into()));
    };
    if !stack.offers(path) {
        return Err(AppError::InvalidInput("not in this stack".into()));
    }
    shell::panel::close(app)?;
    #[cfg(windows)]
    super::win::open(path);
    Ok(())
}

/// What's playing, if it is `dock_app` playing it.
fn media_for(app: &AppHandle, dock_app: &DockApp) -> Option<MenuMedia> {
    let music = crate::widgets::service::snapshot(app).music?;
    let source = music
        .source_icon
        .strip_prefix("aumid:")
        .unwrap_or(&music.source_icon);
    custom::media_matches(source, dock_app).then(|| MenuMedia {
        title: music.title.clone(),
        artist: music.artist.clone(),
        playing: music.status == crate::media::PlaybackStatus::Playing,
        can_play_pause: music.capabilities.play_pause,
        can_next: music.capabilities.next,
        can_previous: music.capabilities.previous,
    })
}

fn screen_count() -> usize {
    #[cfg(windows)]
    return super::win::screens().len();
    #[cfg(not(windows))]
    1
}

/// A right click on an app on dock `label`: its jump list, what it plays, its windows, and
/// what can be done with it.
pub fn show_menu(app: &AppHandle, label: &str, key: &str, anchor_x: i32) -> AppResult<()> {
    let dock_app = find(app, label, key)?;
    #[cfg(windows)]
    let _ = super::preview::hide(app);
    let launchable = launchable(app, key).is_ok();
    let settings = app.state::<SettingsStore>().get();
    let named = |id: &str, name: &str| NamedItem {
        id: id.to_string(),
        name: name.to_string(),
    };
    let menu = super::jump::AppMenu {
        dock: label.to_string(),
        key: dock_app.key.clone(),
        sections: if dock_app.kind == ItemKind::App {
            super::jump::sections(dock_app.app_id.as_deref())
        } else {
            Vec::new()
        },
        windows: dock_app
            .windows
            .iter()
            .take(super::jump::MAX_WINDOWS)
            .cloned()
            .collect(),
        launchable,
        kind: dock_app.kind,
        pinned: dock_app.pinned,
        custom: dock_app.kind != ItemKind::App,
        divider_after: dock_app.divider_after,
        hung: dock_app.hung,
        can_end_task: can_end_task(&dock_app),
        can_move: !dock_app.windows.is_empty() && screen_count() > 1,
        can_restart: launchable && !dock_app.windows.is_empty() && dock_app.kind == ItemKind::App,
        customised: settings.apps.override_for(key).is_some(),
        media: media_for(app, &dock_app),
        presets: settings
            .apps
            .launch_presets
            .iter()
            .filter(|p| p.key == key)
            .map(|p| named(&p.id, &p.name))
            .collect(),
        quick: settings
            .apps
            .quick_actions
            .iter()
            .filter(|q| q.app == key)
            .map(|q| named(&q.id, &q.name))
            .collect(),
        name: dock_app.name,
        icon: dock_app.icon,
    };
    shell::panel::show_app_menu(app, menu, anchor_x)
}

/// Save a settings change made from a menu, and tell every window.
fn change_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) -> AppResult<()> {
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    change(&mut settings);
    store.set(settings)?;
    app.emit(crate::commands::SETTINGS_CHANGED, &store.get())?;
    settings_changed(app);
    Ok(())
}

/// Change (or with `None` remove) the user's name and icon for `key`.
fn set_override(app: &AppHandle, key: &str, change: Option<impl FnOnce(&mut AppOverride)>) -> AppResult<()> {
    // Icons are kept by source; a new pick must not show a stale picture.
    change_settings(app, |s| {
        let overrides = &mut s.apps.overrides;
        let mut current = overrides
            .iter()
            .position(|o| o.key == key)
            .map(|at| overrides.remove(at))
            .unwrap_or_else(|| AppOverride {
                key: key.to_string(),
                ..AppOverride::default()
            });
        if let Some(change) = change {
            change(&mut current);
            overrides.push(current);
        }
    })
}

fn run_quick(app: &AppHandle, id: &str) -> AppResult<()> {
    let settings = app.state::<SettingsStore>().get();
    let action = settings
        .apps
        .quick_actions
        .iter()
        .find(|a| a.id == id)
        .ok_or_else(|| AppError::InvalidInput("unknown action".into()))?;
    #[cfg(windows)]
    match action.kind {
        QuickActionKind::Command => super::win::run_command(&action.target),
        QuickActionKind::Open => super::win::open(&action.target),
    }
    #[cfg(not(windows))]
    let _ = (action, QuickActionKind::Open);
    Ok(())
}

/// "End task" is offered for an app with open windows (a website or folder pin has none of
/// its own).
pub fn can_end_task(dock_app: &DockApp) -> bool {
    dock_app.kind == ItemKind::App && !dock_app.windows.is_empty()
}

/// The programs behind some windows, each once, never the dock's own (`own`), and none
/// unknown (0).
pub fn programs_to_end(pids: impl IntoIterator<Item = u32>, own: u32) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for pid in pids {
        if pid != 0 && pid != own && !out.contains(&pid) {
            out.push(pid);
        }
    }
    out
}

/// End the programs owning `windows` at once (TerminateProcess), then re-list.
#[cfg(windows)]
fn end_programs(app: &AppHandle, windows: &[isize]) {
    let pids = programs_to_end(
        windows.iter().map(|&h| super::win::window_pid(h)),
        std::process::id(),
    );
    for pid in pids {
        if !super::win::end_process(pid) {
            tracing::warn!(pid, "could not end a program");
        }
    }
    rescan_soon(app);
}

/// Move `dock_app`'s front window to the next screen.
#[cfg(windows)]
fn move_to_next_screen(app: &AppHandle, dock_app: &DockApp) {
    let Some(window) = dock_app
        .windows
        .iter()
        .find(|w| w.active)
        .or(dock_app.windows.first())
    else {
        return;
    };
    let screens = super::win::screens();
    let offset = screens
        .iter()
        .find(|s| s.primary)
        .map_or((0, 0), |s| (s.work.x - s.rect.x, s.work.y - s.rect.y));
    let Some(place) = super::win::placement(window.hwnd, offset) else {
        return;
    };
    let Some(from) = screens.iter().find(|s| s.name == place.monitor) else {
        return;
    };
    let Some(to) = layout::next_screen(&screens, &from.name) else {
        return;
    };
    super::win::move_window(
        window.hwnd,
        layout::move_to_screen(place.rect, from.work, to.work),
        false,
        place.maximized,
    );
    app.state::<AppsState>()
        .history
        .lock()
        .expect("apps lock")
        .set_screen(&dock_app.key, &to.name);
}

/// A choice in the open app menu. Only what that menu offered can be done.
pub fn menu_action(app: &AppHandle, action: MenuAction) -> AppResult<()> {
    let Some(shell::panel::PanelContent::App { menu }) = shell::panel::current(app) else {
        return Err(AppError::InvalidInput("no app menu is open".into()));
    };
    shell::panel::close(app)?;
    let key = menu.key.as_str();
    match action {
        #[cfg(windows)]
        MenuAction::Window { hwnd } if menu.windows.iter().any(|w| w.hwnd == hwnd) => {
            super::win::activate(hwnd)
        }
        #[cfg(windows)]
        MenuAction::Open { path } if menu.offers(&path) => super::win::launch(&super::Launch::Program(path)),
        MenuAction::Launch if menu.launchable => launch_key(app, key)?,
        #[cfg(windows)]
        MenuAction::Close => {
            for window in find(app, &menu.dock, key)?.windows {
                super::win::close(window.hwnd);
            }
        }
        MenuAction::Media { key: button } if menu.media.is_some() => {
            let command = match button {
                MediaKey::PlayPause => crate::media::MediaCommand::PlayPause,
                MediaKey::Next => crate::media::MediaCommand::Next,
                MediaKey::Previous => crate::media::MediaCommand::Previous,
            };
            crate::widgets::service::media_command(app, command)?;
        }
        #[cfg(windows)]
        MenuAction::Restart if menu.can_restart => {
            let launch = launchable(app, key)?.launch;
            let windows = find(app, &menu.dock, key)?
                .windows
                .iter()
                .map(|w| w.hwnd)
                .collect();
            super::win::restart(windows, launch);
            launching(app, key);
        }
        #[cfg(windows)]
        MenuAction::ForceClose if menu.hung => {
            let windows = find(app, &menu.dock, key)?.windows;
            let hung: Vec<isize> = windows
                .iter()
                .filter(|w| super::win::is_hung(w.hwnd))
                .map(|w| w.hwnd)
                .collect();
            end_programs(app, &hung);
        }
        #[cfg(windows)]
        MenuAction::EndTask if menu.can_end_task => {
            let windows: Vec<isize> = find(app, &menu.dock, key)?
                .windows
                .iter()
                .map(|w| w.hwnd)
                .collect();
            end_programs(app, &windows);
        }
        #[cfg(windows)]
        MenuAction::MoveToScreen if menu.can_move => move_to_next_screen(app, &find(app, &menu.dock, key)?),
        #[cfg(windows)]
        MenuAction::Preset { id } if menu.presets.iter().any(|p| p.id == id) => {
            let settings = app.state::<SettingsStore>().get();
            let preset = settings
                .apps
                .launch_presets
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| AppError::InvalidInput("unknown preset".into()))?;
            let launch = launchable(app, key)?.launch;
            let exe = find(app, &menu.dock, key)?.exe;
            super::win::launch_with_args(&launch, exe.as_deref(), &preset.args);
            launching(app, key);
        }
        MenuAction::Quick { id } if menu.quick.iter().any(|q| q.id == id) => run_quick(app, &id)?,
        MenuAction::Rename { name } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                set_override(app, key, Some(|o: &mut AppOverride| o.name.clear()))?;
            } else {
                set_override(app, key, Some(|o: &mut AppOverride| o.name = name))?;
            }
        }
        #[cfg(windows)]
        MenuAction::ChangeIcon => {
            let app = app.clone();
            let key = key.to_string();
            std::thread::spawn(move || {
                if let Some(path) = super::win::pick(false) {
                    if let Err(error) = set_override(&app, &key, Some(|o: &mut AppOverride| o.icon = path)) {
                        tracing::warn!(%error, "could not save the icon");
                    }
                }
            });
        }
        MenuAction::ResetLook if menu.customised => set_override(app, key, None::<fn(&mut AppOverride)>)?,
        MenuAction::Hide if menu.kind == ItemKind::App => {
            let pattern = custom::hide_pattern(&find(app, &menu.dock, key)?);
            change_settings(app, |s| s.apps.hidden.push(pattern))?;
        }
        MenuAction::ToggleDivider if menu.pinned => change_settings(app, |s| {
            let separators = &mut s.apps.separators;
            if let Some(at) = separators.iter().position(|k| k == key) {
                separators.remove(at);
            } else {
                separators.push(key.to_string());
            }
        })?,
        MenuAction::Unpin if menu.custom => {
            let id = key.split_once(':').map_or(key, |(_, id)| id);
            change_settings(app, |s| s.apps.custom_pins.retain(|p| p.id != id))?;
        }
        _ => return Err(AppError::InvalidInput("not in this menu".into())),
    }
    Ok(())
}

// ---------- The dock's empty space ----------

fn dock_hwnd(app: &AppHandle, label: &str) -> Option<isize> {
    app.get_webview_window(label)?.hwnd().ok().map(|h| h.0 as isize)
}

/// The dock's own menu (a right-click on an empty part of it), centred on `anchor_x`.
pub fn dock_menu(app: &AppHandle, label: &str, anchor_x: i32) -> AppResult<()> {
    #[cfg(windows)]
    let _ = super::preview::hide(app);
    let settings = app.state::<SettingsStore>().get();
    #[cfg(windows)]
    let windows = super::win::windows().len();
    #[cfg(not(windows))]
    let windows = 0;
    let menu = DockMenu {
        dock: label.to_string(),
        recent: app
            .state::<AppsState>()
            .history
            .lock()
            .expect("apps lock")
            .recent(),
        quick: settings
            .apps
            .quick_actions
            .iter()
            .filter(|a| a.app.is_empty())
            .map(|a| NamedItem {
                id: a.id.clone(),
                name: a.name.clone(),
            })
            .collect(),
        windows,
    };
    shell::panel::show_dock_menu(app, menu, anchor_x)
}

#[cfg(windows)]
fn minimize_others() {
    let front = super::win::foreground();
    for window in super::win::windows() {
        if window.hwnd != front && !window.minimized {
            super::win::minimize(window.hwnd);
        }
    }
}

/// A choice in the dock's own menu. Only what that menu offered can be done.
pub fn dock_menu_action(app: &AppHandle, action: DockMenuAction) -> AppResult<()> {
    let Some(shell::panel::PanelContent::Dock { menu }) = shell::panel::current(app) else {
        return Err(AppError::InvalidInput("no dock menu is open".into()));
    };
    shell::panel::close(app)?;
    match action {
        DockMenuAction::ShowDesktop => show_desktop(app),
        #[cfg(windows)]
        DockMenuAction::MinimizeOthers => minimize_others(),
        #[cfg(windows)]
        DockMenuAction::CloseAll if menu.windows > 0 => {
            for window in super::win::windows() {
                super::win::close(window.hwnd);
            }
        }
        #[cfg(windows)]
        DockMenuAction::TaskManager => super::win::open("taskmgr.exe"),
        DockMenuAction::Settings => shell::open_settings_window(app, Some("apps"))?,
        DockMenuAction::Reopen { key } if menu.recent.iter().any(|r| r.key == key) => {
            let launch = app
                .state::<AppsState>()
                .history
                .lock()
                .expect("apps lock")
                .closed_app(&key)
                .map(|c| c.launch.clone())
                .ok_or_else(|| AppError::InvalidInput("unknown app".into()))?;
            #[cfg(windows)]
            super::win::launch(&launch);
            #[cfg(not(windows))]
            let _ = launch;
            launching(app, &key);
        }
        DockMenuAction::Quick { id } if menu.quick.iter().any(|q| q.id == id) => run_quick(app, &id)?,
        _ => return Err(AppError::InvalidInput("not in this menu".into())),
    }
    Ok(())
}

/// The show-desktop button (and the actions that do the same).
pub fn show_desktop(app: &AppHandle) {
    #[cfg(windows)]
    {
        let _ = super::preview::hide(app);
        if let Some(dock) = dock_hwnd(app, DOCK_LABEL) {
            super::win::peek(false, dock);
        }
        super::win::show_desktop();
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// Hovering the show-desktop button peeks at the desktop (when that's on).
pub fn peek(app: &AppHandle, label: &str, on: bool) {
    #[cfg(windows)]
    {
        let allowed = app.state::<SettingsStore>().get().apps.peek;
        if let Some(dock) = dock_hwnd(app, label) {
            super::win::peek(on && allowed, dock);
        }
    }
    #[cfg(not(windows))]
    let _ = (app, label, on);
}

fn empty_action(app: &AppHandle, label: &str, action: EmptyAction) -> AppResult<()> {
    match action {
        EmptyAction::Nothing => {}
        EmptyAction::ShowDesktop => show_desktop(app),
        #[cfg(windows)]
        EmptyAction::MinimizeOthers => minimize_others(),
        #[cfg(windows)]
        EmptyAction::QuickMenu => dock_menu(app, label, super::win::cursor().0)?,
        #[cfg(windows)]
        EmptyAction::TaskManager => super::win::open("taskmgr.exe"),
        EmptyAction::Settings => shell::open_settings_window(app, None)?,
        #[cfg(windows)]
        EmptyAction::Start => super::win::press(&[windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN]),
        #[cfg(windows)]
        EmptyAction::Search => super::win::press(&[
            windows::Win32::UI::Input::KeyboardAndMouse::VK_LWIN,
            windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(u16::from(b'S')),
        ]),
        #[cfg(not(windows))]
        _ => {}
    }
    let _ = label;
    Ok(())
}

/// Two presses count as a double-click when this close in time and place.
fn is_double(previous: Option<(Instant, (i32, i32))>, now: Instant, point: (i32, i32)) -> bool {
    previous.is_some_and(|(at, p)| {
        now.duration_since(at) <= Duration::from_millis(500)
            && (p.0 - point.0).abs() <= 6
            && (p.1 - point.1).abs() <= 6
    })
}

/// The mouse went down on an empty part of dock `label` (the page tells us; the press itself
/// may start moving the dock, which swallows the release). A second press soon after at the
/// same place is a double-click.
pub fn press(app: &AppHandle, label: &str) -> AppResult<()> {
    #[cfg(windows)]
    let point = super::win::cursor();
    #[cfg(not(windows))]
    let point = (0, 0);
    let now = Instant::now();
    let state = app.state::<AppsState>();
    let mut press = state.press.lock().expect("apps lock");
    let double = is_double(press.previous, now, point);
    press.previous = if double { None } else { Some((now, point)) };
    press.at = Some(now);
    press.point = point;
    press.handled = double;
    drop(press);
    if double {
        let action = app.state::<SettingsStore>().get().apps.double_click;
        if action != EmptyAction::Nothing {
            #[cfg(windows)]
            if let Some(hwnd) = dock_hwnd(app, label) {
                super::win::cancel_mode(hwnd);
            }
            empty_action(app, label, action)?;
        }
    }
    Ok(())
}

/// The page asks, a moment after a press: is the button still held, without moving? Then it
/// is a press-and-hold.
pub fn hold(app: &AppHandle, label: &str) -> AppResult<()> {
    let state = app.state::<AppsState>();
    let mut press = state.press.lock().expect("apps lock");
    #[cfg(windows)]
    let (down, point) = (super::win::primary_button_down(), super::win::cursor());
    #[cfg(not(windows))]
    let (down, point) = (false, (0, 0));
    let held = !press.handled
        && down
        && press
            .at
            .is_some_and(|at| at.elapsed() >= Duration::from_millis(400))
        && (press.point.0 - point.0).abs() <= 6
        && (press.point.1 - point.1).abs() <= 6;
    if !held {
        return Ok(());
    }
    press.handled = true;
    press.previous = None;
    drop(press);
    let action = app.state::<SettingsStore>().get().apps.press_and_hold;
    if action != EmptyAction::Nothing {
        #[cfg(windows)]
        if let Some(hwnd) = dock_hwnd(app, label) {
            super::win::cancel_mode(hwnd);
        }
        empty_action(app, label, action)?;
    }
    Ok(())
}

// ---------- Dragging an icon off the dock ----------

/// An app's icon is being dragged off dock `label`. When the button is let go away from the
/// dock, near a screen edge, the app's window snaps there.
pub fn drag_out(app: &AppHandle, label: &str, key: &str) -> AppResult<()> {
    if !app.state::<SettingsStore>().get().apps.edge_snap {
        return Ok(());
    }
    find(app, label, key)?;
    #[cfg(windows)]
    {
        let app = app.clone();
        let label = label.to_string();
        let key = key.to_string();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            while super::win::primary_button_down() {
                if Instant::now() > deadline {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            let point = super::win::cursor();
            let on_dock = dock_hwnd(&app, &label)
                .and_then(super::win::window_rect)
                .is_some_and(|r| {
                    point.0 >= r.x
                        && point.1 >= r.y
                        && point.0 < r.x + r.width as i32
                        && point.1 < r.y + r.height as i32
                });
            if on_dock {
                return;
            }
            let Ok(dock_app) = find(&app, &label, &key) else {
                return;
            };
            let Some(window) = dock_app
                .windows
                .iter()
                .find(|w| w.active)
                .or(dock_app.windows.first())
            else {
                return;
            };
            let screens = super::win::screens();
            let Some(screen) = layout::screen_at(&screens, point) else {
                return;
            };
            let Some(zone) = layout::snap_zone(point, screen.rect) else {
                return;
            };
            match layout::snap_rect(zone, screen.work) {
                Some(rect) => super::win::move_window(window.hwnd, rect, true, false),
                None => {
                    // Maximize on the screen it was dropped on.
                    let width = screen.work.width / 2;
                    let height = screen.work.height / 2;
                    let middle = Rect {
                        x: screen.work.x + (width / 2) as i32,
                        y: screen.work.y + (height / 2) as i32,
                        width,
                        height,
                    };
                    super::win::move_window(window.hwnd, middle, true, true);
                }
            }
            super::win::activate(window.hwnd);
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_task_is_for_apps_with_windows_and_never_the_dock() {
        let mut app = DockApp::new("exe:a".into(), "A".into(), false, String::new());
        assert!(!can_end_task(&app));
        app.windows.push(super::super::DockWindow {
            hwnd: 1,
            title: String::new(),
            active: false,
            minimized: false,
            hung: false,
            flashing: false,
        });
        assert!(can_end_task(&app));
        app.kind = ItemKind::Website;
        assert!(!can_end_task(&app));
        // Windows of one program end it once; the dock itself and unknown owners are kept.
        assert_eq!(programs_to_end([40, 40, 0, 7, 99], 7), vec![40, 99]);
    }

    #[test]
    fn double_clicks_are_close_in_time_and_place() {
        let start = Instant::now();
        let soon = start + Duration::from_millis(300);
        let late = start + Duration::from_millis(900);
        assert!(is_double(Some((start, (10, 10))), soon, (13, 8)));
        assert!(!is_double(Some((start, (10, 10))), late, (10, 10)));
        assert!(!is_double(Some((start, (10, 10))), soon, (40, 10)));
        assert!(!is_double(None, soon, (10, 10)));
    }
}
