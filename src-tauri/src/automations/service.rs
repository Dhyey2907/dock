//! Runs the automations. It listens to the widgets' own change events (calendar, Claude, weather,
//! focus timer, settings), decides with the rules in `mod.rs`, acts, and shows reminder cards. It
//! never polls: between changes it sleeps until the next thing due (a meeting's reminder time,
//! sunrise or sunset, a card going away).
//!
//! `DOCK_DRY_RUN=1` logs what it would do to music and the Windows theme instead of doing it, for
//! trying automations without touching real playback or settings.

use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use tauri::{AppHandle, Listener, Manager};

use super::{
    claude_alert, claude_bubble, claude_reset, claude_reset_bubble, focus_bubble, focus_music, is_quiet,
    meeting_bubble, meetings_due, next_meeting_reminder, next_sun_change, theme, theme_bubble, theme_for,
    Bubble, ClaudeMemory, MusicAction, ResetMemory, ThemeChoice,
};
use crate::{
    media::{MediaCommand, PlaybackStatus},
    settings::{Settings, SettingsStore},
    shell::bubble,
    widgets::focus::FocusPhase,
};

#[derive(Default)]
pub struct AutomationState {
    bubbles: Mutex<Vec<Bubble>>,
    /// Calendar events that already had their reminder.
    reminded: Mutex<HashSet<String>>,
    claude: Mutex<ClaudeMemory>,
    claude_reset: Mutex<ResetMemory>,
    /// The theme sunset dark mode last aimed for; changes only at sunrise and sunset, so a theme
    /// you pick by hand in between is left alone.
    theme_target: Mutex<Option<ThemeChoice>>,
    focus_phase: Mutex<Option<FocusPhase>>,
    settings: Mutex<Option<Settings>>,
    /// Bumped on every evaluation, so a sleeper for an old deadline gives up.
    generation: AtomicU64,
    /// One evaluation at a time.
    running: Mutex<()>,
}

fn dry_run() -> bool {
    std::env::var("DOCK_DRY_RUN").is_ok_and(|v| v == "1")
}

pub fn bubbles(app: &AppHandle) -> Vec<Bubble> {
    app.state::<AutomationState>()
        .bubbles
        .lock()
        .expect("automation lock")
        .clone()
}

pub fn dismiss(app: &AppHandle, id: &str) {
    let state = app.state::<AutomationState>();
    let list = {
        let mut bubbles = state.bubbles.lock().expect("automation lock");
        bubbles.retain(|b| b.id != id);
        bubbles.clone()
    };
    if let Err(error) = bubble::sync(app, &list) {
        tracing::warn!(%error, "failed to update reminders");
    }
}

pub fn init(app: &AppHandle) {
    for event in [
        crate::calendar::service::WIDGET_CALENDAR,
        crate::claude::service::WIDGET_CLAUDE,
        crate::widgets::WIDGET_WEATHER,
        crate::widgets::WIDGET_MUSIC,
    ] {
        let handle = app.clone();
        app.listen_any(event, move |_| evaluate_soon(&handle));
    }
    let handle = app.clone();
    app.listen_any(crate::widgets::WIDGET_FOCUS, move |_| {
        let handle = handle.clone();
        std::thread::spawn(move || focus_changed(&handle));
    });
    let handle = app.clone();
    app.listen_any(crate::commands::SETTINGS_CHANGED, move |_| {
        let handle = handle.clone();
        std::thread::spawn(move || settings_changed(&handle));
    });
    // The widgets may have read everything before these listeners existed.
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        *handle
            .state::<AutomationState>()
            .focus_phase
            .lock()
            .expect("automation lock") = Some(crate::widgets::service::snapshot(&handle).focus.phase);
        *handle
            .state::<AutomationState>()
            .settings
            .lock()
            .expect("automation lock") = Some(handle.state::<SettingsStore>().get());
        evaluate(&handle);
    });
}

/// Bursts of changes collapse into one evaluation.
fn evaluate_soon(app: &AppHandle) {
    static PENDING: AtomicBool = AtomicBool::new(false);
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        PENDING.store(false, Ordering::SeqCst);
        evaluate(&app);
    });
}

pub(crate) fn show(app: &AppHandle, settings: &Settings, card: Bubble) {
    let quiet = is_quiet(&settings.quiet_hours, Local::now().time());
    if quiet {
        tracing::debug!(id = %card.id, "reminder held for quiet hours");
        return;
    }
    let state = app.state::<AutomationState>();
    let list = {
        let mut bubbles = state.bubbles.lock().expect("automation lock");
        bubbles.retain(|b| b.id != card.id);
        bubbles.push(card);
        bubbles.clone()
    };
    if let Err(error) = bubble::sync(app, &list) {
        tracing::warn!(%error, "failed to show a reminder");
    }
}

fn music_status(app: &AppHandle) -> Option<PlaybackStatus> {
    crate::widgets::service::snapshot(app).music.map(|m| m.status)
}

/// Play or pause whatever the music widget shows — only when it is in the other state.
fn music(app: &AppHandle, action: MusicAction, why: &str) {
    let wanted = matches!(
        (action, music_status(app)),
        (MusicAction::Pause, Some(PlaybackStatus::Playing))
            | (MusicAction::Play, Some(PlaybackStatus::Paused))
    );
    if !wanted {
        return;
    }
    if dry_run() {
        tracing::info!(?action, why, "dry run: would change music");
        return;
    }
    if let Err(error) = crate::widgets::service::media_command(app, MediaCommand::PlayPause) {
        tracing::info!(%error, why, "music didn't respond");
    }
}

fn set_theme(choice: ThemeChoice) -> bool {
    if theme::current() == Some(choice) {
        return false;
    }
    if dry_run() {
        tracing::info!(?choice, "dry run: would switch the Windows theme");
        return true;
    }
    match theme::apply(choice) {
        Ok(()) => true,
        Err(error) => {
            tracing::warn!(%error, "could not switch the Windows theme");
            false
        }
    }
}

/// Today's sunrise and sunset from the weather widget ("2026-11-05T06:46", local to the place).
fn sun_times(app: &AppHandle) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let weather = crate::widgets::service::snapshot(app).weather?;
    let parse = |s: &str| {
        let naive = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").ok()?;
        Local
            .from_local_datetime(&naive)
            .earliest()
            .map(|t| t.with_timezone(&Utc))
    };
    Some((
        parse(weather.sunrise.as_deref()?)?,
        parse(weather.sunset.as_deref()?)?,
    ))
}

fn settings_changed(app: &AppHandle) {
    let state = app.state::<AutomationState>();
    let now = app.state::<SettingsStore>().get();
    let before = state
        .settings
        .lock()
        .expect("automation lock")
        .replace(now.clone());
    // Turning sunset dark mode on puts Windows in the right mode at once; after that it only
    // changes at sunrise and sunset.
    let switched_on =
        now.automations.sunset_dark_mode && !before.as_ref().is_some_and(|b| b.automations.sunset_dark_mode);
    if switched_on {
        if let Some((rise, set)) = sun_times(app) {
            let target = theme_for(Utc::now(), rise, set);
            *state.theme_target.lock().expect("automation lock") = Some(target);
            set_theme(target);
        }
    }
    evaluate(app);
}

fn focus_changed(app: &AppHandle) {
    let state = app.state::<AutomationState>();
    let now = crate::widgets::service::snapshot(app).focus.phase;
    let Some(before) = state.focus_phase.lock().expect("automation lock").replace(now) else {
        return;
    };
    if before == now {
        return;
    }
    let settings = app.state::<SettingsStore>().get();
    if settings.automations.focus_music {
        if let Some(action) = focus_music(before, now, music_status(app)) {
            music(app, action, "focus timer");
        }
    }
    if let Some(card) = focus_bubble(before, now, Utc::now()) {
        show(app, &settings, card);
    }
}

/// Decide and act on everything that is due now, then sleep until the next thing is.
fn evaluate(app: &AppHandle) {
    let state = app.state::<AutomationState>();
    let _guard = state.running.lock().unwrap_or_else(|e| e.into_inner());
    let settings = app.state::<SettingsStore>().get();
    let now = Utc::now();
    let mut wake: Vec<DateTime<Utc>> = Vec::new();

    // Meetings.
    let events = crate::calendar::service::current(app).events;
    if settings.automations.meeting_mode {
        let due: Vec<_> = {
            let reminded = state.reminded.lock().expect("automation lock");
            meetings_due(&events, now, &reminded)
                .into_iter()
                .cloned()
                .collect()
        };
        tracing::debug!(
            events = events.len(),
            due = due.len(),
            reminded = state.reminded.lock().expect("automation lock").len(),
            "automations: checking meetings"
        );
        for event in due {
            state
                .reminded
                .lock()
                .expect("automation lock")
                .insert(event.id.clone());
            show(app, &settings, meeting_bubble(&event, now, settings.clock_24h));
            music(app, MusicAction::Pause, "meeting");
        }
        let reminded = state.reminded.lock().expect("automation lock");
        wake.extend(next_meeting_reminder(&events, now, &reminded));
    }

    // Claude.
    if settings.automations.ai_limit_warnings {
        let usage = crate::claude::service::current(app);
        let alert = claude_alert(&usage, &mut state.claude.lock().expect("automation lock"), now);
        if let Some(alert) = alert {
            show(app, &settings, claude_bubble(&alert, now, settings.clock_24h));
        }
    }

    // Claude's 5-hour window resetting.
    if settings.automations.claude_reset {
        let usage = crate::claude::service::current(app);
        let mut memory = state.claude_reset.lock().expect("automation lock");
        if let Some(reset) = claude_reset(&usage, &mut memory, now) {
            show(app, &settings, claude_reset_bubble(reset, now));
        }
        wake.extend(memory.next());
    }

    // Sunset dark mode: act only when the target changes (at sunrise or sunset).
    if settings.automations.sunset_dark_mode {
        if let Some((rise, set)) = sun_times(app) {
            let target = theme_for(now, rise, set);
            let mut last = state.theme_target.lock().expect("automation lock");
            match *last {
                // First look since starting: note the target, but don't override the theme
                // you have; the next sunrise or sunset will.
                None => *last = Some(target),
                Some(previous) if previous != target => {
                    *last = Some(target);
                    drop(last);
                    if set_theme(target) {
                        show(app, &settings, theme_bubble(target, now));
                    }
                }
                Some(_) => {}
            }
            wake.push(next_sun_change(now, rise, set));
        }
    }

    // Cards that have had their time.
    let expired = {
        let mut bubbles = state.bubbles.lock().expect("automation lock");
        let before = bubbles.len();
        bubbles.retain(|b| b.expires_at > now);
        wake.extend(bubbles.iter().map(|b| b.expires_at));
        (bubbles.len() != before).then(|| bubbles.clone())
    };
    if let Some(list) = expired {
        if let Err(error) = bubble::sync(app, &list) {
            tracing::warn!(%error, "failed to update reminders");
        }
    }

    let generation = state.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let Some(next) = wake.into_iter().filter(|t| *t > now).min() else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep((next - Utc::now()).to_std().unwrap_or_default() + Duration::from_millis(200));
        if app.state::<AutomationState>().generation.load(Ordering::SeqCst) == generation {
            evaluate(&app);
        }
    });
}
