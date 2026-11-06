//! Typed user preferences, stored as one JSON file in the app config folder. Unknown or missing
//! fields fall back to defaults, so adding a field never breaks an older file. Secrets never
//! live here — they go to Windows Credential Manager.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AppResult;

const FILE_NAME: &str = "settings.json";

/// How many docks there are: one, or one on every screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DocksOn {
    /// One dock, on `Settings::screen`.
    #[default]
    One,
    /// A dock on every screen; `Settings::screen` picks the main one (pins and widgets).
    Every,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Edge {
    #[default]
    Bottom,
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DockStyle {
    /// Centered and floating, as wide as its content.
    #[default]
    Pill,
    /// Edge to edge, like the Windows taskbar.
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    #[default]
    Glass,
    IosGlass,
    Midnight,
    White,
    Black,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum DockSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl DockSize {
    /// Dock bar height in logical pixels.
    pub fn height(self) -> f64 {
        match self {
            DockSize::Small => 48.0,
            DockSize::Medium => 56.0,
            DockSize::Large => 64.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TaskbarMode {
    /// The Windows taskbar auto-hides while the dock runs, peeking out at the screen edge.
    AutoHide,
    /// "Dock only": the Windows taskbar is hidden completely while the dock runs.
    #[default]
    Hidden,
}

/// A widget on the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WidgetKind {
    Clock,
    Weather,
    /// Volume, network and battery at a glance (the taskbar's tray area).
    Status,
    /// CPU and memory.
    System,
    Focus,
    Music,
    Claude,
    Calendar,
    /// A sticky note that saves as you type.
    Notes,
    /// A checklist; the dock shows how many are left.
    Todos,
    /// To do, Doing and Done, with cards you drag between them.
    Kanban,
    /// Notes for your current or next calendar event.
    MeetingNotes,
    /// A timer per project, with this week's totals.
    TimeTracker,
    /// Favourite websites and folders, one click each.
    QuickLinks,
    /// Apps grouped into one button that opens a small grid.
    AppFolders,
    /// Chrome and Edge bookmarks.
    Bookmarks,
    /// Instant search through the Windows Search index.
    FileSearch,
    /// Float a screen area or the clipboard's picture on top.
    PinImage,
    /// Copy the text in an area of the screen.
    ScreenText,
    /// The songs you've played today.
    SongsToday,
    /// Running and recent Claude Code sessions.
    ClaudeSessions,
    /// A widget from a newer version of the dock; dropped on load.
    #[serde(other)]
    #[ts(skip)]
    Unknown,
}

impl WidgetKind {
    /// Every widget, in the default order (left to right).
    pub const ALL: [WidgetKind; 21] = [
        WidgetKind::Music,
        WidgetKind::Focus,
        WidgetKind::Claude,
        WidgetKind::Calendar,
        WidgetKind::System,
        WidgetKind::Status,
        WidgetKind::Weather,
        WidgetKind::Clock,
        WidgetKind::Notes,
        WidgetKind::Todos,
        WidgetKind::Kanban,
        WidgetKind::MeetingNotes,
        WidgetKind::TimeTracker,
        WidgetKind::QuickLinks,
        WidgetKind::AppFolders,
        WidgetKind::Bookmarks,
        WidgetKind::FileSearch,
        WidgetKind::PinImage,
        WidgetKind::ScreenText,
        WidgetKind::SongsToday,
        WidgetKind::ClaudeSessions,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WidgetEntry {
    pub kind: WidgetKind,
    pub enabled: bool,
}

fn default_widgets() -> Vec<WidgetEntry> {
    WidgetKind::ALL
        .iter()
        .map(|&kind| WidgetEntry {
            kind,
            // On by default: the ones that work with no setup.
            enabled: matches!(
                kind,
                WidgetKind::Music | WidgetKind::Status | WidgetKind::Weather | WidgetKind::Clock
            ),
        })
        .collect()
}

/// A calendar added by link. The link itself is a secret kept in Windows Credential Manager.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarSource {
    pub id: String,
    pub name: String,
}

/// The dock's automations, one switch each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Automations {
    /// Five minutes before a meeting: a reminder, and music pauses.
    pub meeting_mode: bool,
    /// Warn as Claude use nears your usual limit, and when a limit is reached.
    pub ai_limit_warnings: bool,
    /// Starting the focus timer plays your music; the end of a focus stretch pauses it.
    pub focus_music: bool,
    /// Windows switches to dark at sunset and back to light at sunrise. Changes a Windows
    /// setting, so it starts off.
    pub sunset_dark_mode: bool,
}

impl Default for Automations {
    fn default() -> Self {
        Self {
            meeting_mode: true,
            ai_limit_warnings: true,
            focus_music: false,
            sunset_dark_mode: false,
        }
    }
}

/// Hours when reminders stay quiet. Times are "HH:MM", local; the range may cross midnight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct QuietHours {
    pub enabled: bool,
    pub start: String,
    pub end: String,
}

impl Default for QuietHours {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".into(),
            end: "08:00".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum TemperatureUnit {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub edge: Edge,
    pub style: DockStyle,
    pub theme: Theme,
    pub size: DockSize,
    /// The dock slides away and comes back when the cursor touches the edge.
    pub auto_hide: bool,
    pub taskbar_mode: TaskbarMode,
    pub launch_at_startup: bool,
    /// Widgets in dock order, each on or off. Every kind appears exactly once.
    pub widgets: Vec<WidgetEntry>,
    pub temperature_unit: TemperatureUnit,
    /// A town to show weather for; empty means "where this PC is" (Windows location).
    pub weather_place: String,
    /// 24-hour clock; `None` follows the system's format.
    #[ts(optional)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_24h: Option<bool>,
    /// Calendars added by link, in the order they were added.
    pub calendars: Vec<CalendarSource>,
    pub automations: Automations,
    pub quiet_hours: QuietHours,
    /// Keyboard shortcut that shows or hides the dock ("Ctrl+Alt+D"); empty for none.
    pub toggle_shortcut: String,
    /// The screen the dock lives on, by Windows' device name ("\\.\DISPLAY2"); empty for the
    /// main screen. A screen that's been unplugged falls back to the main one.
    pub screen: String,
    /// One dock, or one on every screen.
    pub docks_on: DocksOn,
    /// The welcome has been seen.
    pub onboarded: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: Edge::default(),
            style: DockStyle::default(),
            theme: Theme::default(),
            size: DockSize::default(),
            auto_hide: false,
            taskbar_mode: TaskbarMode::default(),
            launch_at_startup: true,
            widgets: default_widgets(),
            temperature_unit: TemperatureUnit::default(),
            weather_place: String::new(),
            clock_24h: None,
            calendars: Vec::new(),
            automations: Automations::default(),
            quiet_hours: QuietHours::default(),
            toggle_shortcut: "Ctrl+Alt+D".into(),
            screen: String::new(),
            docks_on: DocksOn::default(),
            onboarded: false,
        }
    }
}

impl Settings {
    /// Make the widget list sane: unknown kinds and repeats dropped, missing kinds added (off)
    /// at the end, so older and newer files both load.
    pub fn normalized(mut self) -> Self {
        let mut seen = Vec::new();
        self.widgets.retain(|w| {
            let keep = w.kind != WidgetKind::Unknown && !seen.contains(&w.kind);
            seen.push(w.kind);
            keep
        });
        for kind in WidgetKind::ALL {
            if !self.widgets.iter().any(|w| w.kind == kind) {
                self.widgets.push(WidgetEntry { kind, enabled: false });
            }
        }
        self.weather_place = self.weather_place.trim().chars().take(80).collect();
        let valid = |t: &str| chrono::NaiveTime::parse_from_str(t.trim(), "%H:%M").is_ok();
        if !valid(&self.quiet_hours.start) || !valid(&self.quiet_hours.end) {
            self.quiet_hours = QuietHours {
                enabled: self.quiet_hours.enabled,
                ..QuietHours::default()
            };
        }
        let mut ids = Vec::new();
        self.calendars.retain(|c| {
            let keep = !c.id.is_empty() && !ids.contains(&c.id);
            ids.push(c.id.clone());
            keep
        });
        self
    }
}

/// The settings file plus an in-memory copy.
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// Read the file in `dir`; a missing or unreadable file gives defaults (an unreadable one
    /// is kept as `settings.json.bad` so nothing is silently lost).
    pub fn open(dir: &Path) -> Self {
        let path = dir.join(FILE_NAME);
        let current = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str::<Settings>(&text)
                .map(Settings::normalized)
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "settings file unreadable; using defaults");
                    let _ = std::fs::rename(&path, path.with_extension("json.bad"));
                    Settings::default()
                }),
            Err(_) => Settings::default(),
        };
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().expect("settings lock").clone()
    }

    /// Save to disk (write a temp file, then rename, so a crash never leaves half a file).
    pub fn set(&self, settings: Settings) -> AppResult<()> {
        let settings = settings.normalized();
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&settings)?)?;
        std::fs::rename(&tmp, &self.path)?;
        *self.current.lock().expect("settings lock") = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());
    }

    #[test]
    fn saved_settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path());
        let changed = Settings {
            edge: Edge::Top,
            theme: Theme::Midnight,
            ..Settings::default()
        };
        store.set(changed.clone()).unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), changed);
    }

    #[test]
    fn unknown_and_missing_fields_fall_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), r#"{"edge":"top","fromTheFuture":1}"#).unwrap();
        let settings = SettingsStore::open(dir.path()).get();
        assert_eq!(settings.edge, Edge::Top);
        assert_eq!(settings.size, DockSize::Medium);
    }

    #[test]
    fn broken_file_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "{not json").unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());
        assert!(dir.path().join("settings.json.bad").exists());
    }

    #[test]
    fn widget_lists_are_normalized_on_load() {
        let dir = tempfile::tempdir().unwrap();
        // A repeat, a widget from a newer version, and most kinds missing.
        let file = r#"{"widgets":[
            {"kind":"clock","enabled":true},
            {"kind":"hologram","enabled":true},
            {"kind":"clock","enabled":false},
            {"kind":"music","enabled":false}]}"#;
        std::fs::write(dir.path().join(FILE_NAME), file).unwrap();
        let widgets = SettingsStore::open(dir.path()).get().widgets;
        assert_eq!(widgets.len(), WidgetKind::ALL.len());
        assert_eq!(
            widgets[0],
            WidgetEntry {
                kind: WidgetKind::Clock,
                enabled: true
            }
        );
        assert_eq!(
            widgets[1],
            WidgetEntry {
                kind: WidgetKind::Music,
                enabled: false
            }
        );
        // Missing kinds were added, switched off.
        assert!(widgets[2..].iter().all(|w| !w.enabled));
        for kind in WidgetKind::ALL {
            assert_eq!(widgets.iter().filter(|w| w.kind == kind).count(), 1);
        }
    }

    #[test]
    fn saved_widget_order_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path());
        let mut settings = Settings::default();
        settings.widgets.reverse();
        store.set(settings.clone()).unwrap();
        assert_eq!(SettingsStore::open(dir.path()).get().widgets, settings.widgets);
    }
}
