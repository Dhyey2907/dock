//! Screen time: the minutes you actively use the PC, per hour and per app, kept for 30 days in
//! a small JSON file. A minute counts when the keyboard or mouse was used in the last five
//! minutes, and goes to the app in front.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use chrono::{Local, NaiveDate, Timelike};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// No input for this long means you've stepped away.
const IDLE_AFTER_MS: u64 = 5 * 60 * 1000;
const KEEP_DAYS: i64 = 30;
const FILE_NAME: &str = "screen-time.json";

/// One day's use: minutes in each hour, and per app.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Day {
    pub hours: Vec<u32>,
    pub apps: BTreeMap<String, u32>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Log {
    /// "2026-11-07" -> that day.
    days: BTreeMap<String, Day>,
}

/// An app and its minutes.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppMinutes {
    pub name: String,
    pub minutes: u32,
}

/// Today's screen time for the cards.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScreenTime {
    pub minutes: u32,
    /// Minutes in each hour of today, 24 of them.
    pub hours: Vec<u32>,
    /// Today's apps, most used first (up to eight).
    pub apps: Vec<AppMinutes>,
    /// Minutes on each of the last seven days, oldest first (today last).
    pub week: Vec<u32>,
}

struct Tracker {
    file: PathBuf,
    log: Mutex<Log>,
}

static TRACKER: OnceLock<Tracker> = OnceLock::new();

/// Add a minute at `hour` to `app` on `day` (an hour holds at most 60).
pub fn add_minute(day: &mut Day, hour: u32, app: &str) {
    if day.hours.len() != 24 {
        day.hours.resize(24, 0);
    }
    let slot = &mut day.hours[hour as usize % 24];
    *slot = (*slot + 1).min(60);
    if !app.is_empty() {
        *day.apps.entry(app.to_string()).or_default() += 1;
    }
}

/// Today's summary from the log, given today's date.
fn summary(log: &Log, today: NaiveDate) -> ScreenTime {
    let key = |d: NaiveDate| d.format("%Y-%m-%d").to_string();
    let day = log.days.get(&key(today)).cloned().unwrap_or_default();
    let mut hours = day.hours.clone();
    hours.resize(24, 0);
    let mut apps: Vec<AppMinutes> = day
        .apps
        .iter()
        .map(|(name, &minutes)| AppMinutes {
            name: name.clone(),
            minutes,
        })
        .collect();
    apps.sort_by(|a, b| b.minutes.cmp(&a.minutes).then(a.name.cmp(&b.name)));
    apps.truncate(8);
    let week = (0..7)
        .rev()
        .map(|back| {
            let date = today - chrono::Duration::days(back);
            log.days.get(&key(date)).map_or(0, |d| d.hours.iter().sum())
        })
        .collect();
    ScreenTime {
        minutes: hours.iter().sum(),
        hours,
        apps,
        week,
    }
}

fn load(path: &Path) -> Log {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(path: &Path, log: &Log) {
    if let Ok(text) = serde_json::to_string(log) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, path);
        }
    }
}

/// Today's screen time (empty before tracking starts).
pub fn today() -> ScreenTime {
    let today = Local::now().date_naive();
    match TRACKER.get() {
        Some(tracker) => summary(&tracker.log.lock().unwrap_or_else(|e| e.into_inner()), today),
        None => summary(&Log::default(), today),
    }
}

/// Start counting: once a minute, the app in front gets a minute if you used the PC lately.
pub fn start(data_dir: &Path) {
    let file = data_dir.join(FILE_NAME);
    let log = load(&file);
    if TRACKER
        .set(Tracker {
            file,
            log: Mutex::new(log),
        })
        .is_err()
    {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("screen-time".into())
        .spawn(|| loop {
            std::thread::sleep(std::time::Duration::from_secs(60));
            #[cfg(windows)]
            {
                if win::idle_ms() >= IDLE_AFTER_MS {
                    continue;
                }
                let app = win::front_app();
                let now = Local::now();
                let Some(tracker) = TRACKER.get() else { return };
                let mut log = tracker.log.lock().unwrap_or_else(|e| e.into_inner());
                let key = now.format("%Y-%m-%d").to_string();
                add_minute(log.days.entry(key).or_default(), now.hour(), &app);
                let oldest = (now - chrono::Duration::days(KEEP_DAYS))
                    .format("%Y-%m-%d")
                    .to_string();
                log.days.retain(|day, _| *day >= oldest);
                save(&tracker.file, &log);
            }
        });
}

#[cfg(windows)]
mod win {
    use windows::Win32::{
        System::SystemInformation::GetTickCount,
        UI::{
            Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
            WindowsAndMessaging::GetForegroundWindow,
        },
    };

    /// Milliseconds since the last key press or mouse move.
    pub fn idle_ms() -> u64 {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // SAFETY: plain queries into a correctly sized local.
        unsafe {
            if !GetLastInputInfo(&mut info).as_bool() {
                return 0;
            }
            u64::from(GetTickCount().wrapping_sub(info.dwTime))
        }
    }

    /// The name of the app in front ("Google Chrome"), or empty.
    pub fn front_app() -> String {
        // SAFETY: a plain query.
        let hwnd = unsafe { GetForegroundWindow() }.0 as isize;
        if hwnd == 0 {
            return String::new();
        }
        crate::apps::win::app_name_of_window(hwnd).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minutes_add_up_per_hour_and_app() {
        let mut day = Day::default();
        for _ in 0..70 {
            add_minute(&mut day, 9, "Chrome");
        }
        add_minute(&mut day, 10, "Code");
        assert_eq!(day.hours[9], 60, "an hour holds at most an hour");
        assert_eq!(day.hours[10], 1);
        assert_eq!(day.apps["Chrome"], 70);
    }

    #[test]
    fn summary_has_today_the_week_and_top_apps() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let mut log = Log::default();
        let mut day = Day::default();
        add_minute(&mut day, 9, "Chrome");
        add_minute(&mut day, 9, "Code");
        add_minute(&mut day, 9, "Code");
        log.days.insert("2026-11-07".into(), day);
        let mut yesterday = Day::default();
        add_minute(&mut yesterday, 20, "Chrome");
        log.days.insert("2026-11-06".into(), yesterday);
        let s = summary(&log, today);
        assert_eq!(s.minutes, 3);
        assert_eq!(s.hours.len(), 24);
        assert_eq!(s.apps[0].name, "Code");
        assert_eq!(s.week, vec![0, 0, 0, 0, 0, 1, 3]);
    }
}
