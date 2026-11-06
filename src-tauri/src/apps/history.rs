//! What the dock remembers about apps between runs: how long each has been the front app (for
//! "sort by use"), the apps you closed recently (to reopen them), and the screen each app was
//! last used on (to reopen it there). Kept in one small JSON file; the tracking here is pure,
//! fed by the service after each re-list.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{DockApp, ItemKind, Launch, Launchable};

const FILE_NAME: &str = "apps-history.json";
/// Recently closed apps kept.
pub const RECENT_LIMIT: usize = 8;
/// Most seconds one stretch in front counts for: a PC left alone (or asleep) with one app in
/// front shouldn't make it the most used.
const MAX_STRETCH_SECS: u64 = 30 * 60;

/// An app you closed, to start again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedApp {
    pub key: String,
    pub name: String,
    pub icon: String,
    pub launch: Launch,
}

/// A recently closed app as the dock's menu shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecentApp {
    pub key: String,
    pub name: String,
    pub icon: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct History {
    /// Seconds each app (by key) has been the front app.
    pub usage: HashMap<String, u64>,
    /// The screen (device name) each app was last used on.
    pub last_screen: HashMap<String, String>,
    /// Newest first.
    pub closed: Vec<ClosedApp>,
    /// The front app and since when (seconds); not saved.
    #[serde(skip)]
    front: Option<(String, u64)>,
    /// Changed since the last save; not saved.
    #[serde(skip)]
    pub dirty: bool,
}

/// What one re-list saw, for `History::observe`.
pub struct Seen<'a> {
    /// Every dock's apps, merged.
    pub apps: &'a [DockApp],
    pub launchables: &'a [Launchable],
    /// Each listed window's screen.
    pub monitors: &'a HashMap<isize, String>,
    /// Seconds since some fixed point.
    pub now: u64,
}

impl History {
    pub fn recent(&self) -> Vec<RecentApp> {
        self.closed
            .iter()
            .map(|c| RecentApp {
                key: c.key.clone(),
                name: c.name.clone(),
                icon: c.icon.clone(),
            })
            .collect()
    }

    pub fn closed_app(&self, key: &str) -> Option<&ClosedApp> {
        self.closed.iter().find(|c| c.key == key)
    }

    /// Update from a re-list. `before` is the previous re-list's apps (`None` on the first,
    /// when nothing counts as opened or closed). Returns the apps that just opened their first
    /// window, with that window, for "reopen on the last screen".
    pub fn observe(&mut self, before: Option<&[DockApp]>, seen: &Seen) -> Vec<(String, isize)> {
        // Time in front goes to the app that was in front until now.
        let front_now = seen
            .apps
            .iter()
            .find(|a| a.windows.iter().any(|w| w.active))
            .map(|a| a.key.clone());
        if self.front.as_ref().map(|(k, _)| k) != front_now.as_ref() {
            if let Some((key, since)) = self.front.take() {
                let secs = seen.now.saturating_sub(since).min(MAX_STRETCH_SECS);
                if secs > 0 {
                    *self.usage.entry(key).or_default() += secs;
                    self.dirty = true;
                }
            }
            self.front = front_now.map(|k| (k, seen.now));
        }

        let with_windows: HashSet<&str> = seen
            .apps
            .iter()
            .filter(|a| !a.windows.is_empty())
            .map(|a| a.key.as_str())
            .collect();
        let mut opened = Vec::new();
        if let Some(before) = before {
            let had: HashSet<&str> = before
                .iter()
                .filter(|a| !a.windows.is_empty())
                .map(|a| a.key.as_str())
                .collect();
            // Apps whose last window closed.
            for app in before.iter().filter(|a| had.contains(a.key.as_str())) {
                if with_windows.contains(app.key.as_str()) || app.kind != ItemKind::App {
                    continue;
                }
                let Some(launchable) = seen.launchables.iter().find(|l| l.key == app.key) else {
                    continue;
                };
                self.closed.retain(|c| c.key != app.key);
                self.closed.insert(
                    0,
                    ClosedApp {
                        key: app.key.clone(),
                        name: app.name.clone(),
                        icon: app.icon.clone(),
                        launch: launchable.launch.clone(),
                    },
                );
                self.closed.truncate(RECENT_LIMIT);
                self.dirty = true;
            }
            // Apps that just opened.
            for app in seen.apps {
                if !had.contains(app.key.as_str()) && app.windows.len() == 1 {
                    opened.push((app.key.clone(), app.windows[0].hwnd));
                }
            }
        }
        // Reopened apps leave the recently closed list.
        let before_len = self.closed.len();
        self.closed.retain(|c| !with_windows.contains(c.key.as_str()));
        self.dirty |= self.closed.len() != before_len;

        // Each app's screen, by its front-most window. Apps that just opened are left for the
        // caller, which may move them back to their last screen first.
        for app in seen.apps {
            if opened.iter().any(|(k, _)| *k == app.key) {
                continue;
            }
            if let Some(monitor) = app.windows.first().and_then(|w| seen.monitors.get(&w.hwnd)) {
                if self.last_screen.get(&app.key) != Some(monitor) {
                    self.last_screen.insert(app.key.clone(), monitor.clone());
                    self.dirty = true;
                }
            }
        }
        opened
    }

    /// Remember `monitor` as the screen `key` is on.
    pub fn set_screen(&mut self, key: &str, monitor: &str) {
        if self.last_screen.get(key).map(String::as_str) != Some(monitor) {
            self.last_screen.insert(key.to_string(), monitor.to_string());
            self.dirty = true;
        }
    }
}

fn file(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// Read the history in `dir`; a missing or broken file starts afresh.
pub fn load(dir: &Path) -> History {
    std::fs::read_to_string(file(dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write it (temp file, then rename).
pub fn save(dir: &Path, history: &History) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(history)?)?;
    std::fs::rename(tmp, file(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::DockWindow;

    fn app(key: &str, windows: &[(isize, bool)]) -> DockApp {
        let mut app = DockApp::new(key.into(), format!("Name {key}"), false, format!("icon:{key}"));
        app.windows = windows
            .iter()
            .map(|&(hwnd, active)| DockWindow {
                hwnd,
                title: String::new(),
                active,
                minimized: false,
                hung: false,
                flashing: false,
            })
            .collect();
        app
    }

    fn launchables(keys: &[&str]) -> Vec<Launchable> {
        keys.iter()
            .map(|k| Launchable {
                key: k.to_string(),
                launch: Launch::Program(format!("{k}.exe")),
            })
            .collect()
    }

    fn seen<'a>(
        apps: &'a [DockApp],
        launchables: &'a [Launchable],
        monitors: &'a HashMap<isize, String>,
        now: u64,
    ) -> Seen<'a> {
        Seen {
            apps,
            launchables,
            monitors,
            now,
        }
    }

    #[test]
    fn time_in_front_is_counted_and_capped() {
        let mut h = History::default();
        let l = launchables(&["a", "b"]);
        let m = HashMap::new();
        let a_front = [app("a", &[(1, true)]), app("b", &[(2, false)])];
        let b_front = [app("a", &[(1, false)]), app("b", &[(2, true)])];
        h.observe(None, &seen(&a_front, &l, &m, 100));
        h.observe(Some(&a_front), &seen(&b_front, &l, &m, 160));
        assert_eq!(h.usage.get("a"), Some(&60));
        // Hours in front with nothing happening count as half an hour.
        h.observe(Some(&b_front), &seen(&a_front, &l, &m, 160 + 5 * 3600));
        assert_eq!(h.usage.get("b"), Some(&MAX_STRETCH_SECS));
        assert!(h.dirty);
    }

    #[test]
    fn closed_apps_are_remembered_newest_first_and_leave_when_reopened() {
        let mut h = History::default();
        let l = launchables(&["a", "b"]);
        let m = HashMap::new();
        let both = [app("a", &[(1, false)]), app("b", &[(2, false)])];
        let only_b = [app("b", &[(2, false)])];
        let none: [DockApp; 0] = [];
        // The first look never counts as closing or opening anything.
        assert!(h.observe(None, &seen(&both, &l, &m, 0)).is_empty());
        h.observe(Some(&both), &seen(&only_b, &l, &m, 1));
        h.observe(Some(&only_b), &seen(&none, &l, &m, 2));
        assert_eq!(
            h.recent().iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(h.closed_app("a").unwrap().launch, Launch::Program("a.exe".into()));
        let opened = h.observe(Some(&none), &seen(&only_b, &l, &m, 3));
        assert_eq!(opened, vec![("b".to_string(), 2)]);
        assert_eq!(
            h.recent().iter().map(|r| r.key.as_str()).collect::<Vec<_>>(),
            ["a"]
        );
    }

    #[test]
    fn apps_that_cant_be_started_again_are_not_listed() {
        let mut h = History::default();
        let m = HashMap::new();
        let before = [app("win:5", &[(5, false)])];
        h.observe(Some(&before), &seen(&[], &[], &m, 0));
        assert!(h.closed.is_empty());
    }

    #[test]
    fn last_screen_follows_the_front_window_except_for_just_opened_apps() {
        let mut h = History::default();
        let l = launchables(&["a"]);
        let m = HashMap::from([(1, "\\\\.\\DISPLAY2".to_string())]);
        let a = [app("a", &[(1, false)])];
        let opened = h.observe(Some(&[]), &seen(&a, &l, &m, 0));
        assert_eq!(opened.len(), 1);
        assert!(h.last_screen.is_empty());
        h.observe(Some(&a), &seen(&a, &l, &m, 1));
        assert_eq!(h.last_screen["a"], "\\\\.\\DISPLAY2");
    }

    #[test]
    fn history_round_trips_and_a_broken_file_starts_afresh() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = History::default();
        h.usage.insert("a".into(), 5);
        h.set_screen("a", "D1");
        save(dir.path(), &h).unwrap();
        let back = load(dir.path());
        assert_eq!(back.usage, h.usage);
        assert_eq!(back.last_screen, h.last_screen);
        std::fs::write(dir.path().join(FILE_NAME), "{oops").unwrap();
        assert_eq!(load(dir.path()), History::default());
    }
}
