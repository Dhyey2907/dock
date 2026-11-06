//! The system widgets' background work:
//!
//! - Bluetooth batteries: checked every ten minutes while the low-battery automation is on.
//! - Downloads: Windows reports each new file; it's announced once it stops growing.
//! - Wallpaper: sleeps until the next change is due, and wakes when its settings change.

use std::{
    path::{Path, PathBuf},
    sync::{Condvar, Mutex},
    time::{Duration, Instant},
};

use chrono::Utc;
use tauri::{AppHandle, Manager};

use super::{bluetooth, downloads, wallpaper};
use crate::settings::{Settings, SettingsStore, WidgetKind};

const BATTERY_EVERY: Duration = Duration::from_secs(10 * 60);
/// The first battery look waits for the dock to settle.
const BATTERY_FIRST: Duration = Duration::from_secs(60);
/// Most "Open" targets remembered for download cards.
const MAX_OPENS: usize = 20;

#[derive(Default)]
pub struct SystemState {
    battery: Mutex<bluetooth::BatteryMemory>,
    announced: Mutex<downloads::Announced>,
    /// What a card's Open button opens, by card id.
    opens: Mutex<Vec<(String, PathBuf)>>,
    /// Bumped on every settings change; the loops below wake when it moves.
    settings_generation: (Mutex<u64>, Condvar),
    /// The download watcher, kept alive for the app's life.
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Where the trending list is cached.
    cache_dir: Mutex<Option<PathBuf>>,
}

fn widget_on(settings: &Settings, kind: WidgetKind) -> bool {
    settings.widgets.iter().any(|w| w.kind == kind && w.enabled)
}

/// How long until the next wallpaper change; `None` when the changer is off (no widget, no
/// folder, or changing only on a click).
pub fn wallpaper_wait(settings: &Settings) -> Option<Duration> {
    let on = widget_on(settings, WidgetKind::Wallpaper)
        && !settings.wallpaper_folder.trim().is_empty()
        && settings.wallpaper_minutes > 0;
    on.then(|| Duration::from_secs(u64::from(settings.wallpaper_minutes) * 60))
}

pub fn init(app: &AppHandle) {
    let state = app.state::<SystemState>();
    *state.cache_dir.lock().expect("system lock") = app.path().app_cache_dir().ok();

    let handle = app.clone();
    std::thread::Builder::new()
        .name("bluetooth-battery".into())
        .spawn(move || battery_loop(&handle))
        .ok();

    let handle = app.clone();
    std::thread::Builder::new()
        .name("wallpaper".into())
        .spawn(move || wallpaper_loop(&handle))
        .ok();

    watch_downloads(app);
}

/// Settings changed: wake the loops so they pick up the new timing.
pub fn settings_changed(app: &AppHandle) {
    let state = app.state::<SystemState>();
    let (generation, wake) = &state.settings_generation;
    *generation.lock().expect("system lock") += 1;
    wake.notify_all();
}

/// Sleep for `wait` (forever if `None`) or until settings change. `true` if the time ran out.
fn sleep_or_settings(app: &AppHandle, wait: Option<Duration>) -> bool {
    let state = app.state::<SystemState>();
    let (generation, wake) = &state.settings_generation;
    let guard = generation.lock().expect("system lock");
    let seen = *guard;
    match wait {
        Some(wait) => {
            let (guard, result) = wake
                .wait_timeout_while(guard, wait, |g| *g == seen)
                .expect("system lock");
            drop(guard);
            result.timed_out()
        }
        None => {
            drop(wake.wait_while(guard, |g| *g == seen).expect("system lock"));
            false
        }
    }
}

fn battery_loop(app: &AppHandle) {
    std::thread::sleep(BATTERY_FIRST);
    loop {
        let settings = app.state::<SettingsStore>().get();
        if settings.automations.bluetooth_battery {
            let devices = bluetooth::list();
            let low: Vec<bluetooth::BluetoothDevice> = {
                let state = app.state::<SystemState>();
                let mut memory = state.battery.lock().expect("system lock");
                bluetooth::newly_low(&devices, &mut memory)
                    .into_iter()
                    .cloned()
                    .collect()
            };
            for device in low {
                crate::automations::service::show(
                    app,
                    &settings,
                    bluetooth::low_battery_bubble(&device, Utc::now()),
                );
            }
            sleep_or_settings(app, Some(BATTERY_EVERY));
        } else {
            sleep_or_settings(app, None);
        }
    }
}

fn wallpaper_loop(app: &AppHandle) {
    loop {
        let settings = app.state::<SettingsStore>().get();
        let wait = wallpaper_wait(&settings);
        if sleep_or_settings(app, wait) {
            // Read again: the folder may have changed while sleeping (that would have woken us,
            // but be sure).
            let settings = app.state::<SettingsStore>().get();
            if wallpaper_wait(&settings).is_some() {
                if let Err(error) = wallpaper::advance(&settings.wallpaper_folder) {
                    tracing::info!(%error, "wallpaper not changed");
                } else {
                    crate::glass::service::refresh_soon(app);
                }
            }
        }
    }
}

/// Next wallpaper now (the popup's button).
pub fn next_wallpaper(app: &AppHandle) -> crate::error::AppResult<wallpaper::WallpaperState> {
    let settings = app.state::<SettingsStore>().get();
    wallpaper::advance(&settings.wallpaper_folder)?;
    crate::glass::service::refresh_soon(app);
    // The timer starts over from this change.
    settings_changed(app);
    Ok(wallpaper::state(&settings.wallpaper_folder))
}

fn watch_downloads(app: &AppHandle) {
    use notify::{EventKind, RecursiveMode, Watcher};
    let Some(folder) = downloads::folder() else {
        tracing::info!("no Downloads folder; download cards are off");
        return;
    };
    let handle = app.clone();
    let watcher = notify::recommended_watcher(move |result: notify::Result<notify::Event>| {
        let Ok(event) = result else { return };
        if !matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(notify::event::ModifyKind::Name(_))
        ) {
            return;
        }
        for path in event.paths {
            if downloads::is_finished_download(&path) && path.is_file() {
                let handle = handle.clone();
                std::thread::spawn(move || download_arrived(&handle, &path));
            }
        }
    });
    match watcher {
        Ok(mut watcher) => {
            if let Err(error) = watcher.watch(&folder, RecursiveMode::NonRecursive) {
                tracing::warn!(%error, "could not watch Downloads");
                return;
            }
            *app.state::<SystemState>().watcher.lock().expect("system lock") = Some(watcher);
        }
        Err(error) => tracing::warn!(%error, "could not watch Downloads"),
    }
}

fn download_arrived(app: &AppHandle, path: &Path) {
    if !app.state::<SettingsStore>().get().automations.download_finished {
        return;
    }
    let Some(size) = downloads::settled_size(path) else {
        return;
    };
    let state = app.state::<SystemState>();
    if !state
        .announced
        .lock()
        .expect("system lock")
        .first_time(path, Instant::now())
    {
        return;
    }
    // Settings may have changed while the file settled.
    let settings = app.state::<SettingsStore>().get();
    if !settings.automations.download_finished {
        return;
    }
    let card = downloads::download_bubble(path, size, Utc::now());
    {
        let mut opens = state.opens.lock().expect("system lock");
        opens.retain(|(id, _)| *id != card.id);
        opens.push((card.id.clone(), path.to_path_buf()));
        let extra = opens.len().saturating_sub(MAX_OPENS);
        opens.drain(..extra);
    }
    crate::automations::service::show(app, &settings, card);
}

/// A card's Open button: open its file and put the card away.
pub fn open_from_bubble(app: &AppHandle, id: &str) -> crate::error::AppResult<()> {
    let path = {
        let opens = app
            .state::<SystemState>()
            .opens
            .lock()
            .expect("system lock")
            .clone();
        opens
            .into_iter()
            .find(|(card, _)| card == id)
            .map(|(_, path)| path)
    };
    let path = path.ok_or_else(|| crate::error::AppError::InvalidInput("nothing to open".into()))?;
    if !path.exists() {
        return Err(crate::error::AppError::InvalidInput("the file has moved".into()));
    }
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(path.to_string_lossy().into_owned()));
    crate::automations::service::dismiss(app, id);
    Ok(())
}

/// Today's GitHub trending list (cached for an hour).
pub async fn trending(app: &AppHandle) -> super::trending::Trending {
    let dir = app
        .state::<SystemState>()
        .cache_dir
        .lock()
        .expect("system lock")
        .clone();
    match dir {
        Some(dir) => super::trending::get(&dir).await,
        None => super::trending::Trending::default(),
    }
}

/// Helper for the frontend's open-in-browser buttons: only plain web links.
pub fn open_url(url: &str) -> crate::error::AppResult<()> {
    let parsed = url::Url::parse(url).map_err(|_| crate::error::AppError::InvalidInput("link".into()))?;
    let allowed = match parsed.scheme() {
        "https" => true,
        "http" => matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    };
    if !allowed {
        return Err(crate::error::AppError::InvalidInput("link".into()));
    }
    #[cfg(windows)]
    crate::apps::win::launch(&crate::apps::Launch::Program(parsed.to_string()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::WidgetEntry;

    #[test]
    fn the_wallpaper_timer_runs_only_when_set_up() {
        let mut settings = Settings::default();
        assert_eq!(wallpaper_wait(&settings), None);
        settings.widgets.push(WidgetEntry {
            kind: WidgetKind::Wallpaper,
            enabled: true,
        });
        settings.wallpaper_folder = r"C:\Pictures".into();
        settings.wallpaper_minutes = 15;
        assert_eq!(wallpaper_wait(&settings), Some(Duration::from_secs(900)));
        settings.wallpaper_minutes = 0;
        assert_eq!(wallpaper_wait(&settings), None);
        settings.wallpaper_minutes = 15;
        settings.wallpaper_folder = "  ".into();
        assert_eq!(wallpaper_wait(&settings), None);
    }

    #[test]
    fn only_web_and_local_links_open() {
        // Refused before anything is launched.
        assert!(open_url("file:///C:/Windows/System32/cmd.exe").is_err());
        assert!(open_url("http://example.com").is_err());
        assert!(open_url("not a link").is_err());
    }
}
