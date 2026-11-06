//! Automations: the widgets share what they know, so the dock can step in at the right moment —
//! a reminder and paused music before a meeting, a warning before Claude runs out, music with
//! the focus timer, dark mode at sunset. Each is one switch in Settings; quiet hours hold the
//! reminders.
//!
//! This file is the pure part (when to act, what to say); `service.rs` watches the widgets and
//! acts.

pub mod service;
pub mod theme;

use std::collections::HashSet;

use chrono::{DateTime, Duration, Local, NaiveTime, Utc};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    calendar::CalendarEvent,
    claude::{ClaudeUsage, LimitKind},
    media::PlaybackStatus,
    settings::QuietHours,
    widgets::focus::FocusPhase,
};

/// How long before a meeting the reminder comes.
pub const MEETING_LEAD: Duration = Duration::minutes(5);
/// Claude warnings at these shares of your usual limit.
pub const CLAUDE_THRESHOLDS: [u32; 2] = [80, 90];

// ---------- Reminder cards ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum BubbleKind {
    Meeting,
    Claude,
    Focus,
    Theme,
    /// A Bluetooth device's battery is low.
    Bluetooth,
    /// A download finished (the card has an Open button).
    Download,
    /// Claude's 5-hour window reset.
    ClaudeReset,
}

/// A reminder card above the dock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Bubble {
    pub id: String,
    pub kind: BubbleKind,
    pub title: String,
    pub body: String,
    /// When it goes away by itself.
    #[ts(type = "string")]
    pub expires_at: DateTime<Utc>,
}

/// A clock time in the user's style ("3:05 PM" or "15:05").
pub fn clock(at: DateTime<Utc>, clock_24h: Option<bool>) -> String {
    let local = at.with_timezone(&Local);
    if clock_24h == Some(true) {
        local.format("%H:%M").to_string()
    } else {
        local.format("%-I:%M %p").to_string()
    }
}

/// "3:05 PM", or "tomorrow at 3:05 PM", or "Sat 3:05 PM", seen from `now`.
pub fn when(at: DateTime<Utc>, now: DateTime<Utc>, clock_24h: Option<bool>) -> String {
    let days = (at.with_timezone(&Local).date_naive() - now.with_timezone(&Local).date_naive()).num_days();
    let time = clock(at, clock_24h);
    match days {
        0 => time,
        1 => format!("tomorrow at {time}"),
        _ => format!("{} {time}", at.with_timezone(&Local).format("%a")),
    }
}

// ---------- Quiet hours ----------

fn hhmm(text: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(text.trim(), "%H:%M").ok()
}

/// Whether `now` (local time of day) falls in the quiet hours; a range may cross midnight.
pub fn is_quiet(quiet: &QuietHours, now: NaiveTime) -> bool {
    if !quiet.enabled {
        return false;
    }
    let (Some(start), Some(end)) = (hhmm(&quiet.start), hhmm(&quiet.end)) else {
        return false;
    };
    if start <= end {
        start <= now && now < end
    } else {
        now >= start || now < end
    }
}

// ---------- Meetings ----------

/// Timed events starting within the lead time that haven't had their reminder yet.
pub fn meetings_due<'a>(
    events: &'a [CalendarEvent],
    now: DateTime<Utc>,
    reminded: &HashSet<String>,
) -> Vec<&'a CalendarEvent> {
    events
        .iter()
        .filter(|e| !e.all_day && e.start > now && e.start - now <= MEETING_LEAD && !reminded.contains(&e.id))
        .collect()
}

/// When the next reminder is due (an event's start minus the lead time), for sleeping until then.
pub fn next_meeting_reminder(
    events: &[CalendarEvent],
    now: DateTime<Utc>,
    reminded: &HashSet<String>,
) -> Option<DateTime<Utc>> {
    events
        .iter()
        .filter(|e| !e.all_day && e.start > now && !reminded.contains(&e.id))
        .map(|e| e.start - MEETING_LEAD)
        .filter(|t| *t > now)
        .min()
}

pub fn meeting_bubble(event: &CalendarEvent, now: DateTime<Utc>, clock_24h: Option<bool>) -> Bubble {
    let minutes = ((event.start - now).num_seconds() as f64 / 60.0).ceil().max(1.0) as i64;
    let mut body = format!("At {}", clock(event.start, clock_24h));
    if let Some(place) = &event.location {
        body.push_str(&format!(" · {place}"));
    }
    Bubble {
        id: format!("meeting:{}", event.id),
        kind: BubbleKind::Meeting,
        title: format!("{} in {minutes} min", event.title),
        body,
        // Stays until the meeting starts.
        expires_at: event.start.max(now + Duration::seconds(30)),
    }
}

// ---------- Claude ----------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeAlert {
    /// Use crossed this share of your usual limit.
    Nearing { percent: u32, resets_at: DateTime<Utc> },
    /// At the current pace, the limit comes before the window resets.
    Pace {
        runs_out_at: DateTime<Utc>,
        resets_at: DateTime<Utc>,
    },
    Reached {
        kind: LimitKind,
        resets_at: DateTime<Utc>,
    },
}

/// What has been said already, so each warning comes once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeMemory {
    window: Option<DateTime<Utc>>,
    thresholds: Vec<u32>,
    pace: bool,
    reached: Option<DateTime<Utc>>,
}

/// New Claude warnings, given what was already said. At most one per call, the most urgent.
pub fn claude_alert(
    usage: &ClaudeUsage,
    memory: &mut ClaudeMemory,
    now: DateTime<Utc>,
) -> Option<ClaudeAlert> {
    if let Some(limit) = usage.limited {
        if memory.reached != Some(limit.resets_at) {
            memory.reached = Some(limit.resets_at);
            return Some(ClaudeAlert::Reached {
                kind: limit.kind,
                resets_at: limit.resets_at,
            });
        }
        return None;
    }
    let window = usage.window.as_ref()?;
    if memory.window != Some(window.started_at) {
        // A new window: everything may be said again.
        *memory = ClaudeMemory {
            window: Some(window.started_at),
            reached: memory.reached,
            ..ClaudeMemory::default()
        };
    }
    let percent = window.percent?;
    if let Some(&threshold) = CLAUDE_THRESHOLDS.iter().rev().find(|&&t| percent >= t) {
        if !memory.thresholds.contains(&threshold) {
            // Crossing 90 also covers 80.
            memory
                .thresholds
                .extend(CLAUDE_THRESHOLDS.iter().filter(|&&t| t <= threshold));
            memory.pace = true;
            return Some(ClaudeAlert::Nearing {
                percent: threshold,
                resets_at: window.resets_at,
            });
        }
    }
    if !memory.pace && (50..100).contains(&percent) {
        let elapsed = now - window.started_at;
        if elapsed > Duration::minutes(15) {
            let to_full =
                Duration::milliseconds((elapsed.num_milliseconds() as f64 * 100.0 / percent as f64) as i64);
            let runs_out_at = window.started_at + to_full;
            if runs_out_at + Duration::minutes(10) < window.resets_at {
                memory.pace = true;
                return Some(ClaudeAlert::Pace {
                    runs_out_at,
                    resets_at: window.resets_at,
                });
            }
        }
    }
    None
}

pub fn claude_bubble(alert: &ClaudeAlert, now: DateTime<Utc>, clock_24h: Option<bool>) -> Bubble {
    let (title, body, id, lasts) = match alert {
        ClaudeAlert::Nearing { percent, resets_at } => (
            format!("Claude is at ≈{percent}% of your usual limit"),
            format!("This 5-hour window resets at {}", clock(*resets_at, clock_24h)),
            format!("claude:near:{percent}:{}", resets_at.timestamp()),
            Duration::seconds(20),
        ),
        ClaudeAlert::Pace {
            runs_out_at,
            resets_at,
        } => (
            format!("Claude may run out at {}", clock(*runs_out_at, clock_24h)),
            format!("At this pace · resets at {}", clock(*resets_at, clock_24h)),
            format!("claude:pace:{}", resets_at.timestamp()),
            Duration::seconds(20),
        ),
        ClaudeAlert::Reached { kind, resets_at } => (
            match kind {
                LimitKind::FiveHour => "Claude's 5-hour limit is reached".to_string(),
                LimitKind::Weekly => "Claude's weekly limit is reached".to_string(),
                LimitKind::Other => "Claude's usage limit is reached".to_string(),
            },
            format!("Back {}", when(*resets_at, now, clock_24h)),
            format!("claude:reached:{}", resets_at.timestamp()),
            Duration::seconds(45),
        ),
    };
    Bubble {
        id,
        kind: BubbleKind::Claude,
        title,
        body,
        expires_at: now + lasts,
    }
}

// ---------- Claude window reset ----------

/// The 5-hour window being waited on, and the last one announced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResetMemory {
    pending: Option<DateTime<Utc>>,
    announced: Option<DateTime<Utc>>,
}

impl ResetMemory {
    /// When the window being waited on resets, for sleeping until then.
    pub fn next(&self) -> Option<DateTime<Utc>> {
        self.pending
    }
}

/// Notes the window in use; returns its reset time once that has passed (once per window).
/// A window you were stopped in counts too, by the limit's reset time.
pub fn claude_reset(
    usage: &ClaudeUsage,
    memory: &mut ResetMemory,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let five_hour_limit = usage
        .limited
        .filter(|l| l.kind == LimitKind::FiveHour)
        .map(|l| l.resets_at);
    let window_reset = usage
        .window
        .as_ref()
        .filter(|w| w.messages > 0)
        .map(|w| w.resets_at);
    // A reset that has come is taken first, so a new window noted below can't replace it.
    let due = memory.pending.filter(|p| *p <= now);
    if due.is_some() {
        memory.pending = None;
    }
    for reset in [window_reset, five_hour_limit].into_iter().flatten() {
        if reset > now && memory.announced != Some(reset) && memory.pending.is_none_or(|p| p < reset) {
            memory.pending = Some(reset);
        }
    }
    let due = due?;
    if memory.announced == Some(due) {
        return None;
    }
    memory.announced = Some(due);
    Some(due)
}

pub fn claude_reset_bubble(resets_at: DateTime<Utc>, now: DateTime<Utc>) -> Bubble {
    Bubble {
        id: format!("claude:reset:{}", resets_at.timestamp()),
        kind: BubbleKind::ClaudeReset,
        title: "Claude's 5-hour window has reset".into(),
        body: "A fresh window starts with your next message.".into(),
        expires_at: now + Duration::seconds(20),
    }
}

// ---------- Sunset dark mode ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeChoice {
    Light,
    Dark,
}

/// Dark between sunset and sunrise.
pub fn theme_for(now: DateTime<Utc>, sunrise: DateTime<Utc>, sunset: DateTime<Utc>) -> ThemeChoice {
    // Today's times; outside them it is night.
    if now >= sunrise && now < sunset {
        ThemeChoice::Light
    } else {
        ThemeChoice::Dark
    }
}

/// The next sunrise or sunset after `now`. After today's sunset, tomorrow's sunrise is taken as
/// today's plus a day (close enough until the forecast refreshes).
pub fn next_sun_change(now: DateTime<Utc>, sunrise: DateTime<Utc>, sunset: DateTime<Utc>) -> DateTime<Utc> {
    [sunrise, sunset, sunrise + Duration::days(1)]
        .into_iter()
        .filter(|t| *t > now)
        .min()
        .unwrap_or(sunrise + Duration::days(1))
}

pub fn theme_bubble(choice: ThemeChoice, now: DateTime<Utc>) -> Bubble {
    let (title, id) = match choice {
        ThemeChoice::Dark => ("Sunset · Windows switched to dark", "theme:dark"),
        ThemeChoice::Light => ("Sunrise · Windows switched to light", "theme:light"),
    };
    Bubble {
        id: id.into(),
        kind: BubbleKind::Theme,
        title: title.into(),
        body: "Sunset dark mode is on in Dock settings.".into(),
        expires_at: now + Duration::seconds(10),
    }
}

// ---------- Focus ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicAction {
    Play,
    Pause,
}

/// Focus music: starting to focus plays paused music; finishing (or stopping) pauses it.
pub fn focus_music(
    before: FocusPhase,
    after: FocusPhase,
    music: Option<PlaybackStatus>,
) -> Option<MusicAction> {
    match (before, after, music?) {
        (FocusPhase::Idle | FocusPhase::Break, FocusPhase::Focus, PlaybackStatus::Paused) => {
            Some(MusicAction::Play)
        }
        (FocusPhase::Focus, FocusPhase::Break | FocusPhase::Idle, PlaybackStatus::Playing) => {
            Some(MusicAction::Pause)
        }
        _ => None,
    }
}

/// The focus timer's own reminder when a phase ends on its own.
pub fn focus_bubble(before: FocusPhase, after: FocusPhase, now: DateTime<Utc>) -> Option<Bubble> {
    let (title, body) = match (before, after) {
        (FocusPhase::Focus, FocusPhase::Break) => ("Focus done · take a 5-minute break", "Nice work."),
        (FocusPhase::Break, FocusPhase::Idle) => {
            ("Break's over", "Start another focus session when you're ready.")
        }
        _ => return None,
    };
    Some(Bubble {
        id: format!("focus:{}", now.timestamp()),
        kind: BubbleKind::Focus,
        title: title.into(),
        body: body.into(),
        expires_at: now + Duration::seconds(15),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::{ActiveLimit, UsageWindow};

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn event(id: &str, start: &str, all_day: bool) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            calendar_id: "work".into(),
            title: id.into(),
            location: None,
            start: utc(start),
            end: utc(start) + Duration::minutes(30),
            all_day,
        }
    }

    #[test]
    fn quiet_hours_may_cross_midnight() {
        let night = QuietHours {
            enabled: true,
            start: "22:00".into(),
            end: "08:00".into(),
        };
        let t = |s: &str| NaiveTime::parse_from_str(s, "%H:%M").unwrap();
        assert!(is_quiet(&night, t("23:30")));
        assert!(is_quiet(&night, t("07:59")));
        assert!(!is_quiet(&night, t("08:00")));
        assert!(!is_quiet(&night, t("12:00")));
        let lunch = QuietHours {
            start: "12:00".into(),
            end: "13:00".into(),
            ..night.clone()
        };
        assert!(is_quiet(&lunch, t("12:30")) && !is_quiet(&lunch, t("13:30")));
        assert!(!is_quiet(
            &QuietHours {
                enabled: false,
                ..night
            },
            t("23:30")
        ));
    }

    #[test]
    fn meetings_are_due_five_minutes_ahead_once() {
        let events = [
            event("standup", "2026-11-05T10:00:00Z", false),
            event("later", "2026-11-05T11:00:00Z", false),
            event("holiday", "2026-11-05T10:01:00Z", true),
        ];
        let mut reminded = HashSet::new();
        let now = utc("2026-11-05T09:56:00Z");
        let due: Vec<&str> = meetings_due(&events, now, &reminded)
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(due, vec!["standup"]);
        reminded.insert("standup".to_string());
        assert!(meetings_due(&events, now, &reminded).is_empty());
        assert_eq!(
            next_meeting_reminder(&events, now, &reminded),
            Some(utc("2026-11-05T10:55:00Z"))
        );
        // Too early for the reminder.
        assert!(meetings_due(&events, utc("2026-11-05T09:50:00Z"), &HashSet::new()).is_empty());
    }

    #[test]
    fn meeting_cards_say_when_and_where() {
        let mut e = event("Standup", "2026-11-05T10:00:00Z", false);
        e.location = Some("Room 4".into());
        let bubble = meeting_bubble(&e, utc("2026-11-05T09:55:30Z"), Some(true));
        assert_eq!(bubble.title, "Standup in 5 min");
        assert!(bubble.body.ends_with(" · Room 4"));
        assert_eq!(bubble.expires_at, e.start);
    }

    fn usage(started: &str, percent: Option<u32>) -> ClaudeUsage {
        ClaudeUsage {
            window: Some(UsageWindow {
                started_at: utc(started),
                resets_at: utc(started) + Duration::hours(5),
                tokens: 0,
                messages: 0,
                percent,
                models: Vec::new(),
            }),
            found: true,
            ..ClaudeUsage::default()
        }
    }

    #[test]
    fn claude_warns_once_per_threshold_per_window() {
        let mut memory = ClaudeMemory::default();
        let now = utc("2026-11-05T12:00:00Z");
        // Crossing 80.
        assert!(matches!(
            claude_alert(&usage("2026-11-05T09:00:00Z", Some(82)), &mut memory, now),
            Some(ClaudeAlert::Nearing { percent: 80, .. })
        ));
        assert_eq!(
            claude_alert(&usage("2026-11-05T09:00:00Z", Some(85)), &mut memory, now),
            None
        );
        // Then 90.
        assert!(matches!(
            claude_alert(&usage("2026-11-05T09:00:00Z", Some(91)), &mut memory, now),
            Some(ClaudeAlert::Nearing { percent: 90, .. })
        ));
        // A new window starts fresh.
        assert!(matches!(
            claude_alert(
                &usage("2026-11-05T14:00:00Z", Some(81)),
                &mut memory,
                utc("2026-11-05T17:00:00Z")
            ),
            Some(ClaudeAlert::Nearing { percent: 80, .. })
        ));
    }

    #[test]
    fn claude_projects_running_out_before_the_reset() {
        let mut memory = ClaudeMemory::default();
        // 60% used one hour into a five-hour window: full at about 1h40m in.
        let alert = claude_alert(
            &usage("2026-11-05T09:00:00Z", Some(60)),
            &mut memory,
            utc("2026-11-05T10:00:00Z"),
        );
        match alert {
            Some(ClaudeAlert::Pace { runs_out_at, .. }) => {
                assert_eq!(runs_out_at, utc("2026-11-05T10:40:00Z"));
            }
            other => panic!("expected a pace warning, got {other:?}"),
        }
        // Said once.
        assert_eq!(
            claude_alert(
                &usage("2026-11-05T09:00:00Z", Some(65)),
                &mut memory,
                utc("2026-11-05T10:05:00Z")
            ),
            None
        );
        // A slow pace doesn't warn: 55% at 4h in is fine.
        let mut calm = ClaudeMemory::default();
        assert_eq!(
            claude_alert(
                &usage("2026-11-05T09:00:00Z", Some(55)),
                &mut calm,
                utc("2026-11-05T13:00:00Z")
            ),
            None
        );
    }

    #[test]
    fn claude_says_when_a_limit_is_reached_once() {
        let mut memory = ClaudeMemory::default();
        let limited = ClaudeUsage {
            limited: Some(ActiveLimit {
                kind: LimitKind::FiveHour,
                resets_at: utc("2026-11-05T15:00:00Z"),
            }),
            ..ClaudeUsage::default()
        };
        let now = utc("2026-11-05T12:00:00Z");
        let alert = claude_alert(&limited, &mut memory, now).unwrap();
        assert!(matches!(alert, ClaudeAlert::Reached { .. }));
        assert_eq!(
            claude_bubble(&alert, now, Some(true)).title,
            "Claude's 5-hour limit is reached"
        );
        assert_eq!(claude_alert(&limited, &mut memory, now), None);
    }

    #[test]
    fn claude_reset_is_announced_once_when_the_window_ends() {
        let mut memory = ResetMemory::default();
        let mut use_now = usage("2026-11-05T09:00:00Z", Some(40));
        use_now.window.as_mut().unwrap().messages = 12;
        let reset = utc("2026-11-05T14:00:00Z");
        assert_eq!(
            claude_reset(&use_now, &mut memory, utc("2026-11-05T10:00:00Z")),
            None
        );
        assert_eq!(memory.next(), Some(reset));
        // Still the old summary after the reset time (nothing has been used since): announced.
        assert_eq!(
            claude_reset(&use_now, &mut memory, utc("2026-11-05T14:00:01Z")),
            Some(reset)
        );
        assert_eq!(memory.next(), None);
        assert_eq!(
            claude_reset(&use_now, &mut memory, utc("2026-11-05T14:05:00Z")),
            None
        );
        let card = claude_reset_bubble(reset, utc("2026-11-05T14:00:01Z"));
        assert_eq!(card.kind, BubbleKind::ClaudeReset);
    }

    #[test]
    fn claude_reset_follows_a_five_hour_limit() {
        let mut memory = ResetMemory::default();
        let limited = ClaudeUsage {
            limited: Some(ActiveLimit {
                kind: LimitKind::FiveHour,
                resets_at: utc("2026-11-05T15:00:00Z"),
            }),
            ..ClaudeUsage::default()
        };
        assert_eq!(
            claude_reset(&limited, &mut memory, utc("2026-11-05T12:00:00Z")),
            None
        );
        assert_eq!(
            claude_reset(&ClaudeUsage::default(), &mut memory, utc("2026-11-05T15:00:00Z")),
            Some(utc("2026-11-05T15:00:00Z"))
        );
        // A weekly limit isn't a 5-hour reset; an unused window isn't either.
        let mut fresh = ResetMemory::default();
        let weekly = ClaudeUsage {
            limited: Some(ActiveLimit {
                kind: LimitKind::Weekly,
                resets_at: utc("2026-11-07T15:00:00Z"),
            }),
            ..usage("2026-11-05T09:00:00Z", None)
        };
        assert_eq!(
            claude_reset(&weekly, &mut fresh, utc("2026-11-05T10:00:00Z")),
            None
        );
        assert_eq!(fresh.next(), None);
    }

    #[test]
    fn sunset_dark_mode_follows_the_sun() {
        let (rise, set) = (utc("2026-11-05T01:15:00Z"), utc("2026-11-05T12:30:00Z"));
        assert_eq!(
            theme_for(utc("2026-11-05T06:00:00Z"), rise, set),
            ThemeChoice::Light
        );
        assert_eq!(
            theme_for(utc("2026-11-05T13:00:00Z"), rise, set),
            ThemeChoice::Dark
        );
        assert_eq!(
            theme_for(utc("2026-11-05T00:30:00Z"), rise, set),
            ThemeChoice::Dark
        );
        assert_eq!(next_sun_change(utc("2026-11-05T06:00:00Z"), rise, set), set);
        assert_eq!(
            next_sun_change(utc("2026-11-05T13:00:00Z"), rise, set),
            rise + Duration::days(1)
        );
    }

    #[test]
    fn focus_music_plays_and_pauses_with_the_timer() {
        use FocusPhase::*;
        use PlaybackStatus::*;
        assert_eq!(focus_music(Idle, Focus, Some(Paused)), Some(MusicAction::Play));
        assert_eq!(focus_music(Idle, Focus, Some(Playing)), None);
        assert_eq!(focus_music(Focus, Break, Some(Playing)), Some(MusicAction::Pause));
        assert_eq!(focus_music(Focus, Idle, Some(Playing)), Some(MusicAction::Pause));
        assert_eq!(focus_music(Focus, Break, None), None);
        assert!(focus_bubble(Focus, Break, Utc::now()).is_some());
        assert!(focus_bubble(Idle, Focus, Utc::now()).is_none());
    }
}
