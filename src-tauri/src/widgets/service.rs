//! Runs the widget data sources and sends each change to the pages.
//!
//! - Volume and network: Windows reports every change.
//! - Weather: fetched every half hour while the widget is on, and at once when its settings change.
//! - CPU, memory, battery: sampled every few seconds, only while a widget shows them.
//! - Focus timer: sleeps until the running phase ends.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};

use chrono::Utc;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    focus::FocusState,
    network, stats,
    weather::{self, Located, Weather, WeatherStatus},
    WidgetSnapshot, WIDGET_FOCUS, WIDGET_NETWORK, WIDGET_SOUND, WIDGET_STATS, WIDGET_WEATHER,
};
use crate::settings::{Settings, SettingsStore, WidgetKind};

const WEATHER_EVERY: Duration = Duration::from_secs(30 * 60);
/// After a failed weather fetch, try again sooner.
const WEATHER_RETRY: Duration = Duration::from_secs(5 * 60);
const STATS_EVERY: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct WidgetsState {
    snapshot: Mutex<WidgetSnapshot>,
    /// Wakes the weather loop (settings changed).
    weather_now: Arc<tokio::sync::Notify>,
    /// Whether some widget shows CPU, memory or battery; the sampler waits while it's false.
    stats_wanted: (Mutex<bool>, Condvar),
    /// Bumped on every focus change, so a sleeper for an old phase knows to give up.
    focus_generation: AtomicU64,
    #[cfg(windows)]
    audio: std::sync::OnceLock<super::audio::Audio>,
}

fn enabled(settings: &Settings, kind: WidgetKind) -> bool {
    settings.widgets.iter().any(|w| w.kind == kind && w.enabled)
}

fn wants_stats(settings: &Settings) -> bool {
    enabled(settings, WidgetKind::System) || enabled(settings, WidgetKind::Status)
}

pub fn snapshot(app: &AppHandle) -> WidgetSnapshot {
    app.state::<WidgetsState>()
        .snapshot
        .lock()
        .expect("widgets lock")
        .clone()
}

/// Store a changed value and send it; unchanged values are not re-sent.
fn publish<T: Serialize + Clone + PartialEq>(
    app: &AppHandle,
    event: &str,
    value: T,
    slot: impl FnOnce(&mut WidgetSnapshot) -> &mut T,
) {
    let state = app.state::<WidgetsState>();
    let mut snapshot = state.snapshot.lock().expect("widgets lock");
    let place = slot(&mut snapshot);
    if *place == value {
        return;
    }
    *place = value.clone();
    drop(snapshot);
    if let Err(error) = app.emit(event, value) {
        tracing::warn!(%error, event, "failed to send widget data");
    }
}

pub fn init(app: &AppHandle) {
    let settings = app.state::<SettingsStore>().get();
    *app.state::<WidgetsState>()
        .stats_wanted
        .0
        .lock()
        .expect("widgets lock") = wants_stats(&settings);

    #[cfg(windows)]
    {
        let handle = app.clone();
        let audio = super::audio::start(move |sound, _cause| {
            publish(&handle, WIDGET_SOUND, Some(sound), |s| &mut s.sound);
        });
        let _ = app.state::<WidgetsState>().audio.set(audio);
    }

    let handle = app.clone();
    publish(app, WIDGET_NETWORK, Some(network::read()), |s| &mut s.network);
    network::watch(move || {
        // Windows reports from its own thread; reading is quick.
        publish(&handle, WIDGET_NETWORK, Some(network::read()), |s| &mut s.network);
    });

    let handle = app.clone();
    std::thread::Builder::new()
        .name("stats".into())
        .spawn(move || stats_loop(&handle))
        .ok();

    let handle = app.clone();
    tauri::async_runtime::spawn(async move { weather_loop(handle).await });
}

/// Settings changed: wake whatever depends on them.
pub fn settings_changed(app: &AppHandle, before: &Settings, after: &Settings) {
    let state = app.state::<WidgetsState>();
    if enabled(before, WidgetKind::Weather) != enabled(after, WidgetKind::Weather)
        || before.weather_place != after.weather_place
        || before.temperature_unit != after.temperature_unit
    {
        state.weather_now.notify_one();
    }
    let (wanted, wake) = &state.stats_wanted;
    *wanted.lock().expect("widgets lock") = wants_stats(after);
    wake.notify_all();
}

fn stats_loop(app: &AppHandle) {
    loop {
        {
            let state = app.state::<WidgetsState>();
            let (wanted, wake) = &state.stats_wanted;
            let guard = wanted.lock().expect("widgets lock");
            // Sleeps here, without waking, while no widget needs the numbers.
            drop(wake.wait_while(guard, |wanted| !*wanted).expect("widgets lock"));
        }
        publish(app, WIDGET_STATS, Some(stats::read()), |s| &mut s.stats);
        std::thread::sleep(STATS_EVERY);
    }
}

async fn weather_loop(app: AppHandle) {
    let http = match reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Dock (Windows desktop)")
        .build()
    {
        Ok(http) => http,
        Err(error) => {
            tracing::warn!(%error, "weather unavailable");
            return;
        }
    };
    let wake = app.state::<WidgetsState>().weather_now.clone();
    loop {
        let settings = app.state::<SettingsStore>().get();
        let wait = if enabled(&settings, WidgetKind::Weather) {
            let unit = settings.temperature_unit;
            let weather = fetch_weather(&http, &settings).await;
            let wait = if weather.status == WeatherStatus::Ready {
                WEATHER_EVERY
            } else {
                WEATHER_RETRY
            };
            // A failed refresh keeps the last good forecast on screen.
            let keep_old = weather.status == WeatherStatus::Unavailable
                && snapshot(&app)
                    .weather
                    .is_some_and(|w| w.status == WeatherStatus::Ready && w.unit == unit);
            if !keep_old {
                publish(&app, WIDGET_WEATHER, Some(weather), |s| &mut s.weather);
            }
            Some(wait)
        } else {
            None
        };
        match wait {
            Some(wait) => {
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = wake.notified() => {}
                }
            }
            None => wake.notified().await,
        }
    }
}

async fn fetch_weather(http: &reqwest::Client, settings: &Settings) -> Weather {
    let unit = settings.temperature_unit;
    let place = settings.weather_place.trim();
    let (lat, lon, name) = if place.is_empty() {
        let located = tauri::async_runtime::spawn_blocking(weather::locate)
            .await
            .unwrap_or(Located::Failed);
        match located {
            Located::At(lat, lon) => (lat, lon, None),
            Located::NoAccess => return Weather::with_status(WeatherStatus::NoAccess, unit),
            Located::Failed => return Weather::with_status(WeatherStatus::Unavailable, unit),
        }
    } else {
        match weather::geocode(http, place).await {
            Some((name, lat, lon)) => (lat, lon, Some(name)),
            None => return Weather::with_status(WeatherStatus::PlaceNotFound, unit),
        }
    };
    let Some(mut weather) = weather::forecast(http, lat, lon, unit).await else {
        return Weather::with_status(WeatherStatus::Unavailable, unit);
    };
    weather.place = match name {
        Some(name) => Some(name),
        None => weather::place_name(http, lat, lon).await,
    };
    weather
}

// ---------- Volume ----------

pub fn set_volume(app: &AppHandle, level: f32) {
    #[cfg(windows)]
    if let Some(audio) = app.state::<WidgetsState>().audio.get() {
        audio.set_level(level);
    }
    #[cfg(not(windows))]
    let _ = (app, level);
}

pub fn toggle_mute(app: &AppHandle) {
    #[cfg(windows)]
    if let Some(audio) = app.state::<WidgetsState>().audio.get() {
        audio.toggle_mute();
    }
    #[cfg(not(windows))]
    let _ = app;
}

// ---------- Focus ----------

/// Apply a change to the focus timer, send it, and sleep until it next needs to move on.
pub fn focus_update(app: &AppHandle, change: impl FnOnce(&FocusState) -> FocusState) {
    focus_set(app, change(&snapshot(app).focus));
}

fn focus_set(app: &AppHandle, next: FocusState) {
    let state = app.state::<WidgetsState>();
    let generation = state.focus_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let ends_at = next.ends_at;
    publish(app, WIDGET_FOCUS, next, |s| &mut s.focus);
    let Some(ends_at) = ends_at else { return };
    let app = app.clone();
    std::thread::spawn(move || {
        let wait = (ends_at - Utc::now()).to_std().unwrap_or_default();
        std::thread::sleep(wait);
        // A newer change (pause, reset, restart) owns the timer now.
        if app
            .state::<WidgetsState>()
            .focus_generation
            .load(Ordering::SeqCst)
            == generation
        {
            focus_set(&app, snapshot(&app).focus.advance(Utc::now()));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::WidgetEntry;

    #[test]
    fn stats_run_only_for_widgets_that_show_them() {
        let mut settings = Settings::default();
        for w in &mut settings.widgets {
            w.enabled = false;
        }
        assert!(!wants_stats(&settings));
        settings.widgets[0] = WidgetEntry {
            kind: WidgetKind::System,
            enabled: true,
        };
        assert!(wants_stats(&settings));
    }
}
