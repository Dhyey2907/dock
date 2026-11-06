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
    /// A widget from a newer version of the dock; dropped on load.
    #[serde(other)]
    #[ts(skip)]
    Unknown,
}

impl WidgetKind {
    /// Every widget, in the default order (left to right).
    pub const ALL: [WidgetKind; 8] = [
        WidgetKind::Music,
        WidgetKind::Focus,
        WidgetKind::Claude,
        WidgetKind::Calendar,
        WidgetKind::System,
        WidgetKind::Status,
        WidgetKind::Weather,
        WidgetKind::Clock,
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
    /// Apps and windows on the dock: custom pins, names, icons, shortcuts and behaviour.
    pub apps: AppsSettings,
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
            apps: AppsSettings::default(),
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
        self.apps = self.apps.normalized();
        self
    }
}

// ---------- Apps and windows ----------

/// Most entries kept in each of the apps lists below.
const APPS_LIST_LIMIT: usize = 64;

/// What a pin of the dock's own (not the taskbar's) opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CustomPinKind {
    /// A web page, opened in the default browser; its icon is the site's own.
    #[default]
    Website,
    /// A folder whose files fan out in a popup.
    Stack,
}

/// A website or folder pinned to the dock, after the taskbar's pins.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CustomPin {
    pub id: String,
    pub name: String,
    pub kind: CustomPinKind,
    /// The address (website) or folder path (stack).
    pub target: String,
}

/// A name or icon the user chose for an app, by its dock key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppOverride {
    pub key: String,
    /// Empty keeps the app's own name.
    pub name: String,
    /// An image or icon file; empty keeps the app's own icon.
    pub icon: String,
}

/// "Open with arguments" in an app's menu: a Chrome profile, a project folder, a flag.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct LaunchPreset {
    pub id: String,
    /// The app's dock key.
    pub key: String,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum QuickActionKind {
    /// Run a command line (through `cmd /c`, without a window).
    #[default]
    Command,
    /// Open a file, folder or web address.
    Open,
}

/// A button of the user's own, in an app's right-click menu (or the dock's, with no app).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct QuickAction {
    pub id: String,
    pub name: String,
    pub kind: QuickActionKind,
    pub target: String,
    /// The app whose menu shows it, by dock key; empty for the dock's own menu.
    pub app: String,
}

/// What a double-click or a press-and-hold on an empty part of the dock does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EmptyAction {
    #[default]
    Nothing,
    ShowDesktop,
    MinimizeOthers,
    QuickMenu,
    TaskManager,
    Settings,
    Start,
    Search,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AppsSettings {
    /// Websites and folders pinned after the taskbar's pins, in dock order.
    pub custom_pins: Vec<CustomPin>,
    pub overrides: Vec<AppOverride>,
    /// Apps that never show on the dock: a program file name ("notepad.exe"), a program path,
    /// an app id or a dock key.
    pub hidden: Vec<String>,
    /// A folder of PNGs named after programs or app ids ("chrome.png"); empty for none.
    pub icon_pack: String,
    /// Running apps that aren't pinned go most-used first.
    pub sort_by_use: bool,
    /// Pinned apps followed by a divider, by dock key.
    pub separators: Vec<String>,
    /// Alt+1 … Alt+9 open the pinned apps in order.
    pub number_shortcuts: bool,
    /// An app's icon bounces while it starts.
    pub launch_bounce: bool,
    /// An app's icon glows when it asks for attention.
    pub attention_flash: bool,
    /// Hovering an app shows its windows.
    pub previews: bool,
    /// Dock's own Alt+Tab switcher instead of Windows'. Off unless chosen.
    pub alt_tab: bool,
    /// Dragging an app's icon to a screen edge snaps its window there.
    pub edge_snap: bool,
    /// Windows shrink into their icon on the dock when minimized.
    pub minimize_animation: bool,
    pub show_desktop_button: bool,
    /// Hovering the show-desktop button peeks at the desktop.
    pub peek: bool,
    pub double_click: EmptyAction,
    pub press_and_hold: EmptyAction,
    pub launch_presets: Vec<LaunchPreset>,
    pub quick_actions: Vec<QuickAction>,
    /// An app opens on the screen it was last used on.
    pub reopen_on_last_screen: bool,
    /// Windows on an unplugged screen move to another, and go back when it returns.
    pub rearrange_on_unplug: bool,
}

impl Default for AppsSettings {
    fn default() -> Self {
        Self {
            custom_pins: Vec::new(),
            overrides: Vec::new(),
            hidden: Vec::new(),
            icon_pack: String::new(),
            sort_by_use: false,
            separators: Vec::new(),
            number_shortcuts: true,
            launch_bounce: true,
            attention_flash: true,
            previews: true,
            alt_tab: false,
            edge_snap: true,
            minimize_animation: true,
            show_desktop_button: true,
            peek: true,
            double_click: EmptyAction::ShowDesktop,
            press_and_hold: EmptyAction::Nothing,
            launch_presets: Vec::new(),
            quick_actions: Vec::new(),
            reopen_on_last_screen: true,
            rearrange_on_unplug: true,
        }
    }
}

fn clean(text: &str, limit: usize) -> String {
    text.trim().chars().take(limit).collect()
}

/// Keep the first item for each id, drop items `keep` rejects, and cap the list.
fn dedupe_by<T>(items: &mut Vec<T>, id: impl Fn(&T) -> String, keep: impl Fn(&T) -> bool) {
    let mut seen = Vec::new();
    items.retain(|item| {
        let id = id(item);
        let ok = !id.is_empty() && !seen.contains(&id) && keep(item);
        seen.push(id);
        ok
    });
    items.truncate(APPS_LIST_LIMIT);
}

/// A website address as typed ("example.com/news") made a full http(s) address; `None` if
/// it isn't one.
pub fn website_address(text: &str) -> Option<String> {
    let text = text.trim();
    let with_scheme = if text.contains("://") {
        text.to_string()
    } else {
        format!("https://{text}")
    };
    let url = url::Url::parse(&with_scheme).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|h| h.contains('.') || h == "localhost"))
    .then(|| url.to_string())
}

impl AppsSettings {
    pub fn normalized(mut self) -> Self {
        for pin in &mut self.custom_pins {
            pin.id = clean(&pin.id, 64);
            pin.name = clean(&pin.name, 60);
            pin.target = match pin.kind {
                CustomPinKind::Website => website_address(&pin.target).unwrap_or_default(),
                CustomPinKind::Stack => clean(&pin.target, 400),
            };
            if pin.name.is_empty() {
                pin.name = match pin.kind {
                    CustomPinKind::Website => url::Url::parse(&pin.target)
                        .ok()
                        .and_then(|u| u.host_str().map(|h| h.trim_start_matches("www.").to_string()))
                        .unwrap_or_default(),
                    CustomPinKind::Stack => std::path::Path::new(&pin.target)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| pin.target.clone()),
                };
            }
        }
        dedupe_by(&mut self.custom_pins, |p| p.id.clone(), |p| !p.target.is_empty());
        for o in &mut self.overrides {
            o.key = clean(&o.key, 400);
            o.name = clean(&o.name, 60);
            o.icon = clean(&o.icon, 400);
        }
        dedupe_by(
            &mut self.overrides,
            |o| o.key.clone(),
            |o| !o.name.is_empty() || !o.icon.is_empty(),
        );
        let mut hidden: Vec<String> = self.hidden.iter().map(|h| clean(h, 400).to_lowercase()).collect();
        dedupe_by(&mut hidden, Clone::clone, |_| true);
        self.hidden = hidden;
        self.icon_pack = clean(&self.icon_pack, 400);
        let mut separators: Vec<String> = self.separators.iter().map(|s| clean(s, 400)).collect();
        dedupe_by(&mut separators, Clone::clone, |_| true);
        self.separators = separators;
        for p in &mut self.launch_presets {
            p.id = clean(&p.id, 64);
            p.key = clean(&p.key, 400);
            p.name = clean(&p.name, 60);
            p.args = clean(&p.args, 1000);
        }
        dedupe_by(
            &mut self.launch_presets,
            |p| p.id.clone(),
            |p| !p.key.is_empty() && !p.name.is_empty(),
        );
        for a in &mut self.quick_actions {
            a.id = clean(&a.id, 64);
            a.name = clean(&a.name, 60);
            a.target = clean(&a.target, 1000);
            a.app = clean(&a.app, 400);
        }
        dedupe_by(
            &mut self.quick_actions,
            |a| a.id.clone(),
            |a| !a.name.is_empty() && !a.target.is_empty(),
        );
        self
    }

    /// The user's name and icon for the app with `key`, if any.
    pub fn override_for(&self, key: &str) -> Option<&AppOverride> {
        self.overrides.iter().find(|o| o.key == key)
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
    fn apps_settings_default_when_missing_and_alt_tab_starts_off() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), r#"{"edge":"top"}"#).unwrap();
        let apps = SettingsStore::open(dir.path()).get().apps;
        assert_eq!(apps, AppsSettings::default());
        assert!(!apps.alt_tab);
        assert!(apps.previews && apps.number_shortcuts);
    }

    #[test]
    fn apps_lists_are_cleaned() {
        let pin = |id: &str, kind, target: &str| CustomPin {
            id: id.into(),
            name: String::new(),
            kind,
            target: target.into(),
        };
        let apps = AppsSettings {
            custom_pins: vec![
                pin("a", CustomPinKind::Website, "www.example.com/news"),
                pin("a", CustomPinKind::Website, "https://other.com"),
                pin("b", CustomPinKind::Website, "not a site"),
                pin("c", CustomPinKind::Stack, r"C:\Users\me\Downloads"),
            ],
            hidden: vec![" Notepad.EXE ".into(), "notepad.exe".into(), "".into()],
            overrides: vec![AppOverride {
                key: "k".into(),
                name: "  ".into(),
                icon: String::new(),
            }],
            ..AppsSettings::default()
        }
        .normalized();
        assert_eq!(apps.custom_pins.len(), 2);
        assert_eq!(apps.custom_pins[0].target, "https://www.example.com/news");
        assert_eq!(apps.custom_pins[0].name, "example.com");
        assert_eq!(apps.custom_pins[1].name, "Downloads");
        assert_eq!(apps.hidden, vec!["notepad.exe".to_string()]);
        assert!(apps.overrides.is_empty());
        assert_eq!(website_address("ftp://x.com"), None);
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
