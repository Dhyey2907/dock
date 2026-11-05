//! Calendar links: added and removed from Settings, kept in Windows Credential Manager (a secret
//! iCal link lets anyone read the calendar, so it never goes in the settings file or the log),
//! read every 15 minutes, and expanded into the events of the coming weeks.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{Duration as Span, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::{expand, parse_events, CalendarEvent};
use crate::{
    error::{AppError, AppResult},
    settings::{CalendarSource, SettingsStore},
};

pub const WIDGET_CALENDAR: &str = "widget-calendar";
const KEYRING_SERVICE: &str = "com.dhyey.dock";
const REFRESH_EVERY: Duration = Duration::from_secs(15 * 60);
/// Biggest calendar file read; real ones are far smaller.
const MAX_BYTES: usize = 10 * 1024 * 1024;
/// Events are listed from yesterday to this many days ahead.
const DAYS_AHEAD: i64 = 45;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarStatus {
    pub id: String,
    pub name: String,
    /// Why it couldn't be read last time; `None` when it was.
    #[ts(optional)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarState {
    /// Every calendar's events, sorted by start.
    pub events: Vec<CalendarEvent>,
    pub calendars: Vec<CalendarStatus>,
    /// Built from the sample calendar (`DOCK_MOCK=calendar`), not real links.
    pub sample: bool,
}

#[derive(Default)]
pub struct CalendarService {
    state: Mutex<CalendarState>,
    refresh_now: Arc<tokio::sync::Notify>,
}

pub fn current(app: &AppHandle) -> CalendarState {
    app.state::<CalendarService>()
        .state
        .lock()
        .expect("calendar lock")
        .clone()
}

fn sample_mode() -> bool {
    std::env::var("DOCK_MOCK").is_ok_and(|v| v.split(',').any(|p| p.trim() == "calendar"))
}

fn secret(id: &str) -> AppResult<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("calendar/{id}"))
        .map_err(|e| AppError::Platform(e.to_string()))
}

/// `webcal://` is just `https://` for calendar apps.
pub fn normalize_link(link: &str) -> AppResult<String> {
    let link = link.trim();
    let rest = link
        .strip_prefix("webcal://")
        .or_else(|| link.strip_prefix("webcals://"))
        .map(|rest| format!("https://{rest}"));
    let link = rest.unwrap_or_else(|| link.to_string());
    let parsed =
        url::Url::parse(&link).map_err(|_| AppError::InvalidInput("That isn't a web link.".into()))?;
    if !matches!(parsed.scheme(), "https" | "http") || parsed.host_str().is_none() {
        return Err(AppError::InvalidInput(
            "Calendar links start with https:// or webcal://".into(),
        ));
    }
    Ok(link)
}

async fn fetch(http: &reqwest::Client, link: &str) -> Result<String, String> {
    let response = http
        .get(link)
        .send()
        .await
        .map_err(|_| "couldn't be reached".to_string())?;
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 | 404 => "no longer works".into(),
            code => format!("got error {code} from its server"),
        });
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| "couldn't be downloaded".to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("is too large".into());
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if !text.contains("BEGIN:VCALENDAR") {
        return Err("isn't a calendar".into());
    }
    Ok(text)
}

fn http() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("Dock (Windows desktop)")
        .build()
        .map_err(|e| AppError::Platform(e.to_string()))
}

/// Check a link really is a calendar, then save it.
pub async fn add(app: &AppHandle, name: &str, link: &str) -> AppResult<()> {
    let link = normalize_link(link)?;
    let name = name.trim().chars().take(40).collect::<String>();
    fetch(&http()?, &link)
        .await
        .map_err(|why| AppError::InvalidInput(format!("That link {why}.")))?;
    let id = format!("{:x}", Utc::now().timestamp_nanos_opt().unwrap_or_default());
    secret(&id)?
        .set_password(&link)
        .map_err(|e| AppError::Platform(e.to_string()))?;
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    settings.calendars.push(CalendarSource {
        id,
        name: if name.is_empty() { "Calendar".into() } else { name },
    });
    store.set(settings.clone())?;
    app.emit(crate::commands::SETTINGS_CHANGED, &settings)?;
    app.state::<CalendarService>().refresh_now.notify_one();
    Ok(())
}

pub fn remove(app: &AppHandle, id: &str) -> AppResult<()> {
    if let Ok(entry) = secret(id) {
        // Already gone is fine.
        let _ = entry.delete_credential();
    }
    let store = app.state::<SettingsStore>();
    let mut settings = store.get();
    settings.calendars.retain(|c| c.id != id);
    store.set(settings.clone())?;
    app.emit(crate::commands::SETTINGS_CHANGED, &settings)?;
    app.state::<CalendarService>().refresh_now.notify_one();
    Ok(())
}

pub fn init(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let http = match http() {
            Ok(http) => http,
            Err(error) => {
                tracing::warn!(%error, "calendars unavailable");
                return;
            }
        };
        let wake = app.state::<CalendarService>().refresh_now.clone();
        loop {
            refresh(&app, &http).await;
            tokio::select! {
                _ = tokio::time::sleep(REFRESH_EVERY) => {}
                _ = wake.notified() => {}
            }
        }
    });
}

async fn refresh(app: &AppHandle, http: &reqwest::Client) {
    let now = Utc::now();
    let (from, to) = (now - Span::days(1), now + Span::days(DAYS_AHEAD));
    let mut state = CalendarState::default();
    if sample_mode() {
        state.sample = true;
        state.events = expand(&parse_events(&sample_calendar(now)), "sample", from, to);
        state.calendars.push(CalendarStatus {
            id: "sample".into(),
            name: "Sample calendar".into(),
            problem: None,
        });
    }
    let sources = app.state::<SettingsStore>().get().calendars;
    for source in sources {
        let link =
            secret(&source.id).and_then(|e| e.get_password().map_err(|e| AppError::Platform(e.to_string())));
        let result = match link {
            Ok(link) => fetch(http, &link).await,
            Err(_) => Err("has no saved link; add it again".into()),
        };
        let problem = match result {
            Ok(text) => {
                state
                    .events
                    .extend(expand(&parse_events(&text), &source.id, from, to));
                None
            }
            Err(why) => {
                tracing::info!(calendar = %source.name, why, "calendar not read");
                // Keep showing what we had for it rather than blanking the list.
                let previous = current(app);
                state
                    .events
                    .extend(previous.events.into_iter().filter(|e| e.calendar_id == source.id));
                Some(why)
            }
        };
        state.calendars.push(CalendarStatus {
            id: source.id,
            name: source.name,
            problem,
        });
    }
    state.events.sort_by_key(|e| e.start);
    let service = app.state::<CalendarService>();
    let mut stored = service.state.lock().expect("calendar lock");
    if *stored != state {
        *stored = state.clone();
        drop(stored);
        if let Err(error) = app.emit(WIDGET_CALENDAR, &state) {
            tracing::warn!(%error, "failed to send calendar events");
        }
    }
}

/// A made-up calendar for trying the widget (and meeting reminders) without real links: a
/// meeting starting a few minutes from now, one later today, a repeat, and an all-day event.
fn sample_calendar(now: chrono::DateTime<Utc>) -> String {
    let at = |minutes: i64| {
        (now + Span::minutes(minutes))
            .format("%Y%m%dT%H%M00Z")
            .to_string()
    };
    let today = chrono::Local::now().date_naive();
    format!(
        "BEGIN:VCALENDAR\r\nVERSION:2.0\r\n\
BEGIN:VEVENT\r\nUID:sample-1\r\nSUMMARY:Sample: team standup\r\nDTSTART:{}\r\nDURATION:PT15M\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:sample-2\r\nSUMMARY:Sample: design review\r\nLOCATION:Room 4\r\nDTSTART:{}\r\nDURATION:PT1H\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:sample-3\r\nSUMMARY:Sample: gym\r\nDTSTART:{}\r\nDURATION:PT1H\r\nRRULE:FREQ=DAILY;COUNT=5\r\nEND:VEVENT\r\n\
BEGIN:VEVENT\r\nUID:sample-4\r\nSUMMARY:Sample: holiday\r\nDTSTART;VALUE=DATE:{}\r\nEND:VEVENT\r\n\
END:VCALENDAR\r\n",
        at(6),
        at(150),
        at(26 * 60),
        (today + Span::days(3)).format("%Y%m%d"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_are_checked_and_webcal_becomes_https() {
        assert_eq!(
            normalize_link(" webcal://p01-caldav.icloud.com/published/2/abc ").unwrap(),
            "https://p01-caldav.icloud.com/published/2/abc"
        );
        assert!(normalize_link("https://calendar.google.com/calendar/ical/x/private-y/basic.ics").is_ok());
        assert!(normalize_link("ftp://example.com/a.ics").is_err());
        assert!(normalize_link("not a link").is_err());
    }

    #[test]
    fn the_sample_calendar_expands() {
        let now = Utc::now();
        let events = expand(
            &parse_events(&sample_calendar(now)),
            "sample",
            now - Span::days(1),
            now + Span::days(10),
        );
        assert!(events.len() >= 8, "{} events", events.len());
        assert!(events.iter().any(|e| e.all_day));
        let next = events.iter().find(|e| e.start > now).unwrap();
        assert_eq!(next.title, "Sample: team standup");
    }
}
