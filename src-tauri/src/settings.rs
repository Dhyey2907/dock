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
/// The longest wallpaper interval: a day.
const MAX_WALLPAPER_MINUTES: u32 = 24 * 60;

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
    /// Two floating pills: apps on the left, widgets on the right.
    Split,
}

/// A widget shown outside the dock, in its own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FloatingWidget {
    pub kind: WidgetKind,
    /// Window position in physical screen pixels.
    pub x: i32,
    pub y: i32,
    /// Stays above other windows; otherwise it sits on the desktop, behind them.
    pub on_top: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Theme {
    /// Like Windows 11's own taskbar: acrylic, its icons, its accent colour.
    #[default]
    Windows,
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

/// How a widget's tile is drawn on the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum WidgetSize {
    /// Just the essentials.
    Small,
    #[default]
    Normal,
    /// Extra detail (a second line, more numbers).
    Large,
}

/// How widget popups look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum PopupStyle {
    /// Acrylic glass, like the dock.
    #[default]
    Glass,
    /// An opaque card.
    Solid,
    /// Glass with tighter spacing.
    Compact,
}

/// How widgets are drawn on the dock: compact text and icons, or Lumabar-style cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum WidgetStyle {
    #[default]
    Simple,
    /// Rounded cards with rings, bars and charts, as tall as the dock allows.
    Cards,
}

/// Choices for the card widgets that show things of your choosing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CardSettings {
    /// Time zones for the world clocks ("America/New_York"), up to three.
    pub world_clocks: Vec<String>,
    /// Stock tickers ("AAPL"), up to three; the first is the single-stock cards'.
    pub stocks: Vec<String>,
    /// The Metric card's coin, by CoinGecko id ("bitcoin").
    pub metric: String,
    pub currency_from: String,
    pub currency_to: String,
    pub github_user: String,
    pub leetcode_user: String,
    /// The Timer card's countdown, in minutes.
    pub timer_minutes: u32,
}

impl Default for CardSettings {
    fn default() -> Self {
        Self {
            world_clocks: vec![
                "America/New_York".into(),
                "Europe/London".into(),
                "Asia/Tokyo".into(),
            ],
            stocks: vec!["AAPL".into(), "NVDA".into(), "MSFT".into()],
            metric: "bitcoin".into(),
            currency_from: "USD".into(),
            currency_to: "INR".into(),
            github_user: String::new(),
            leetcode_user: String::new(),
            timer_minutes: 10,
        }
    }
}

impl CardSettings {
    fn tidied(mut self) -> Self {
        let clean = |list: Vec<String>, upper: bool| -> Vec<String> {
            list.into_iter()
                .map(|t| t.trim().chars().take(40).collect::<String>())
                .map(|t| if upper { t.to_uppercase() } else { t })
                .filter(|t| !t.is_empty())
                .take(3)
                .collect()
        };
        self.world_clocks = clean(self.world_clocks, false);
        self.stocks = clean(self.stocks, true);
        let short = |t: &str, n: usize| t.trim().chars().take(n).collect::<String>();
        self.metric = short(&self.metric, 40).to_lowercase();
        self.currency_from = short(&self.currency_from, 3).to_uppercase();
        self.currency_to = short(&self.currency_to, 3).to_uppercase();
        self.github_user = short(&self.github_user, 39);
        self.leetcode_user = short(&self.leetcode_user, 40);
        self.timer_minutes = self.timer_minutes.clamp(1, 600);
        self
    }
}

/// The search button: an icon, a box like Windows 11's "Search" box, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum SearchStyle {
    Icon,
    #[default]
    Box,
    Hidden,
}

/// Fine control over the dock's size and look, on top of the size and theme choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DockLook {
    /// Dock size as a percentage of the chosen size (70 to 200). Bigger docks show more.
    pub scale: u16,
    /// Corner roundness in percent: 100 is a full pill, 0 square.
    pub corners: u8,
    /// How see-through the glass is, in percent (0 to 70).
    pub transparency: u8,
    /// Blur strength in percent of the theme's own blur (0 to 300).
    pub blur: u16,
    /// Background behind each widget tile, in percent (0 is none).
    pub widget_background: u8,
    /// App icons grow as the mouse passes over them.
    pub magnify: bool,
    /// How springy animations are, in percent (0 is none).
    pub bounce: u8,
    /// The dock rises into place when it starts.
    pub startup_animation: bool,
    pub popup_style: PopupStyle,
    /// Hovering a widget shows its popup as a preview (like Windows 11's Widgets button);
    /// a click keeps it open.
    pub widget_hover: bool,
    /// Start, search and the apps in the middle of a full-width dock (like Windows 11), rather
    /// than at its left.
    pub center_apps: bool,
    pub search: SearchStyle,
    /// The Task View button, next to search.
    pub task_view: bool,
    /// The clock, Status and tray buttons open Windows' own flyouts (calendar, Quick Settings)
    /// rather than the dock's popups.
    pub native_flyouts: bool,
    pub widget_style: WidgetStyle,
}

impl Default for DockLook {
    fn default() -> Self {
        Self {
            scale: 100,
            corners: 100,
            transparency: 0,
            blur: 100,
            widget_background: 0,
            magnify: false,
            bounce: 40,
            startup_animation: true,
            popup_style: PopupStyle::default(),
            widget_hover: true,
            center_apps: true,
            search: SearchStyle::default(),
            task_view: true,
            native_flyouts: true,
            widget_style: WidgetStyle::default(),
        }
    }
}

impl DockLook {
    fn clamped(self) -> Self {
        Self {
            scale: self.scale.clamp(70, 200),
            corners: self.corners.min(100),
            transparency: self.transparency.min(70),
            blur: self.blur.min(300),
            widget_background: self.widget_background.min(100),
            bounce: self.bounce.min(100),
            ..self
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
    /// The programs using the most processor and memory, with End task.
    TopApps,
    /// Paired Bluetooth devices and their batteries.
    Bluetooth,
    /// Brightness for external screens (DDC/CI).
    Brightness,
    /// Network name, signal, local and public address.
    Wifi,
    /// Time since the PC started, and a pending Windows Update restart.
    Uptime,
    /// Local servers listening on this PC.
    DevServers,
    /// Show or hide the desktop icons.
    DesktopIcons,
    /// Turn the screens off.
    ScreenOff,
    /// Pictures from a folder as the wallpaper, in turn.
    Wallpaper,
    /// Extend, duplicate or one screen only.
    DisplayMode,
    /// Removable drives, with Safely Remove.
    UsbDrives,
    /// Today's trending GitHub repositories.
    GithubTrending,
    /// Ask Claude a question with your own Anthropic API key.
    AskClaude,
    /// Your Spotify playlists, played on the active device.
    Spotify,
    /// Undone tasks from a Notion database.
    Notion,
    GoogleTasks,
    /// The latest GitHub Actions run of each repo you list.
    GithubActions,
    /// The taskbar's ^: tray icons that don't fit, in Windows' own flyout.
    HiddenIcons,
    /// The input language (ENG IN); click to switch keyboards.
    Language,
    /// The notification bell: Windows' notification centre.
    Notifications,
    /// Claude's 5-hour and weekly use, and your most-used model.
    ClaudePlan,
    /// ChatGPT/Codex plan limits, read from Codex's own files on this PC.
    CodexUsage,
    /// Your day on one line: time on the PC by hour, focus rounds, meetings, sunrise and sunset.
    DayTimeline,
    /// Today's time at the PC, hour by hour, and per app.
    ScreenTime,
    /// A countdown that starts in one click.
    Timer,
    /// Start, pause and lap from the dock.
    Stopwatch,
    /// Convert between two currencies at today's rate.
    Currency,
    /// Processor load as a ring.
    Cpu,
    /// Memory in use as a ring.
    Ram,
    /// Processor temperature as a gauge, with processor and memory load.
    Temperature,
    /// Download and upload speed with a live line.
    NetSpeed,
    /// Charge as a filling gauge.
    Battery,
    /// Wi-Fi, Bluetooth and airplane mode.
    QuickToggles,
    /// Switch Windows between light and dark.
    ThemeToggle,
    /// How much of today is behind you.
    DayProgress,
    /// How much of the month is behind you.
    MonthProgress,
    /// The year as a grid of dots, filling up.
    YearProgress,
    /// Volume for your speakers or headphones.
    AudioOutput,
    /// Title, artist and play/pause in a smaller card.
    NowPlayingCompact,
    /// One round play button with the track's progress.
    NowPlayingMini,
    /// The track with a progress bar and skip.
    NowPlayingBar,
    /// A live price, such as BTC/USD.
    Metric,
    /// Your GitHub contribution grid and streak.
    GithubActivity,
    /// LeetCode problems solved.
    Leetcode,
    /// The files you opened last.
    RecentFiles,
    /// What the Recycle Bin holds, and a click to empty it.
    RecycleBin,
    /// Search the web, or open a folder by its path.
    WebSearch,
    /// Free space on your drive.
    Storage,
    /// Your newest screenshots.
    Screenshots,
    /// The newest file in Downloads.
    Downloads,
    /// Capture the screen, a window or an area.
    ScreenCapture,
    /// Wind, rain and the day's range.
    WeatherDetails,
    /// The next four days.
    WeatherForecast,
    /// Today next to the next three days.
    TodayForecast,
    /// The conditions in words, with your town.
    WeatherStatus,
    /// The next six hours.
    Hourly,
    /// Where the sun is between sunrise and sunset.
    SunriseSunset,
    /// One ticker's price and daily change.
    Stock,
    /// One ticker with today's line.
    StockChart,
    /// Three tickers at a glance.
    StockWatchlist,
    /// Large time with seconds and the date.
    BigClock,
    /// Split-flap digits that turn every minute.
    FlipClock,
    /// Three small analog faces for other cities.
    WorldClocks,
    /// A white analog face with a sweeping second hand.
    AnalogClassic,
    /// The classic face, in black.
    AnalogDark,
    /// This week as a strip of days, with today's temperature.
    SevenDays,
    /// Three cities in a list, with their hour difference.
    WorldClockList,
    /// One more city in big digits.
    WorldTime,
    /// Weekday, date, time and weather in one card.
    Today,
    /// The full date, set large.
    Date,
    /// Today's date as a calendar page.
    DayNumber,
    /// Hands and hour ticks, nothing else.
    AnalogMinimal,
    /// The time as a seven-segment display.
    SegmentClock,
    /// Time, date and temperature as three colour blocks.
    ClockWeather,
    /// A widget from a newer version of the dock; dropped on load.
    #[serde(other)]
    #[ts(skip)]
    Unknown,
}

impl WidgetKind {
    /// Every widget, in the default order (left to right).
    pub const ALL: [WidgetKind; 95] = [
        WidgetKind::Music,
        WidgetKind::Focus,
        WidgetKind::Claude,
        WidgetKind::Calendar,
        WidgetKind::System,
        WidgetKind::HiddenIcons,
        WidgetKind::Language,
        WidgetKind::Status,
        WidgetKind::Weather,
        WidgetKind::Clock,
        WidgetKind::Notifications,
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
        WidgetKind::TopApps,
        WidgetKind::Bluetooth,
        WidgetKind::Brightness,
        WidgetKind::Wifi,
        WidgetKind::Uptime,
        WidgetKind::DevServers,
        WidgetKind::DesktopIcons,
        WidgetKind::ScreenOff,
        WidgetKind::Wallpaper,
        WidgetKind::DisplayMode,
        WidgetKind::UsbDrives,
        WidgetKind::GithubTrending,
        WidgetKind::AskClaude,
        WidgetKind::Spotify,
        WidgetKind::Notion,
        WidgetKind::GoogleTasks,
        WidgetKind::GithubActions,
        WidgetKind::ClaudePlan,
        WidgetKind::CodexUsage,
        WidgetKind::DayTimeline,
        WidgetKind::ScreenTime,
        WidgetKind::Timer,
        WidgetKind::Stopwatch,
        WidgetKind::Currency,
        WidgetKind::Cpu,
        WidgetKind::Ram,
        WidgetKind::Temperature,
        WidgetKind::NetSpeed,
        WidgetKind::Battery,
        WidgetKind::QuickToggles,
        WidgetKind::ThemeToggle,
        WidgetKind::DayProgress,
        WidgetKind::MonthProgress,
        WidgetKind::YearProgress,
        WidgetKind::AudioOutput,
        WidgetKind::NowPlayingCompact,
        WidgetKind::NowPlayingMini,
        WidgetKind::NowPlayingBar,
        WidgetKind::Metric,
        WidgetKind::GithubActivity,
        WidgetKind::Leetcode,
        WidgetKind::RecentFiles,
        WidgetKind::RecycleBin,
        WidgetKind::WebSearch,
        WidgetKind::Storage,
        WidgetKind::Screenshots,
        WidgetKind::Downloads,
        WidgetKind::ScreenCapture,
        WidgetKind::WeatherDetails,
        WidgetKind::WeatherForecast,
        WidgetKind::TodayForecast,
        WidgetKind::WeatherStatus,
        WidgetKind::Hourly,
        WidgetKind::SunriseSunset,
        WidgetKind::Stock,
        WidgetKind::StockChart,
        WidgetKind::StockWatchlist,
        WidgetKind::BigClock,
        WidgetKind::FlipClock,
        WidgetKind::WorldClocks,
        WidgetKind::AnalogClassic,
        WidgetKind::AnalogDark,
        WidgetKind::SevenDays,
        WidgetKind::WorldClockList,
        WidgetKind::WorldTime,
        WidgetKind::Today,
        WidgetKind::Date,
        WidgetKind::DayNumber,
        WidgetKind::AnalogMinimal,
        WidgetKind::SegmentClock,
        WidgetKind::ClockWeather,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WidgetEntry {
    pub kind: WidgetKind,
    pub enabled: bool,
    #[serde(default)]
    pub size: WidgetSize,
}

fn default_widgets() -> Vec<WidgetEntry> {
    WidgetKind::ALL
        .iter()
        .map(|&kind| WidgetEntry {
            size: WidgetSize::Normal,
            kind,
            // On by default: the ones that work with no setup.
            enabled: matches!(
                kind,
                WidgetKind::Music
                    | WidgetKind::HiddenIcons
                    | WidgetKind::Language
                    | WidgetKind::Status
                    | WidgetKind::Weather
                    | WidgetKind::Clock
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
    /// A card when a connected Bluetooth device's battery runs low.
    pub bluetooth_battery: bool,
    /// A card, with Open, when a download finishes in your Downloads folder.
    pub download_finished: bool,
    /// A card when Claude's 5-hour window resets.
    pub claude_reset: bool,
}

impl Default for Automations {
    fn default() -> Self {
        Self {
            meeting_mode: true,
            ai_limit_warnings: true,
            focus_music: false,
            sunset_dark_mode: false,
            bluetooth_battery: true,
            download_finished: true,
            claude_reset: true,
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

/// Global keyboard shortcuts besides show/hide (which is `Settings::toggle_shortcut`). Each is
/// shortcut text ("Ctrl+Space"); empty for none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Hotkeys {
    /// Opens the command bar.
    pub command_bar: String,
    /// Opens quick capture (a note from anywhere).
    pub quick_capture: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            command_bar: "Ctrl+Space".into(),
            quick_capture: "Ctrl+Alt+Q".into(),
        }
    }
}

/// The non-secret half of the account widgets' setup. Keys and tokens are in Windows Credential
/// Manager; these are ids the user types in, which are not secrets.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AccountSettings {
    /// The client id of the user's own Spotify app.
    pub spotify_client_id: String,
    /// The client id of the user's own Google OAuth "Desktop app" client.
    pub google_client_id: String,
    /// The Notion database the tasks come from.
    pub notion_database_id: String,
    /// GitHub repos to watch, as "owner/name".
    pub github_repos: Vec<String>,
}

/// A GitHub "owner/name", or `None` when `text` isn't one.
pub fn github_repo(text: &str) -> Option<String> {
    let text = text
        .trim()
        .trim_start_matches("https://github.com/")
        .trim_end_matches('/');
    let (owner, name) = text.split_once('/')?;
    let ok = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (ok(owner) && ok(name)).then(|| format!("{owner}/{name}"))
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
    /// Size slider, corners, transparency, blur and other fine touches.
    pub look: DockLook,
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
    /// The wallpaper changer's folder; empty for none.
    pub wallpaper_folder: String,
    /// Minutes between wallpaper changes; 0 changes only on a click.
    pub wallpaper_minutes: u32,
    /// The welcome has been seen.
    pub onboarded: bool,
    /// Widgets popped out of the dock (on top) or put on the desktop.
    pub floating: Vec<FloatingWidget>,
    /// Shrink the dock to fit when it has more than the screen is wide.
    pub auto_shrink: bool,
    /// Apps and windows on the dock: custom pins, names, icons, shortcuts and behaviour.
    pub apps: AppsSettings,
    /// The command bar and quick capture shortcuts.
    pub hotkeys: Hotkeys,
    /// Setup for the account widgets (no secrets).
    pub accounts: AccountSettings,
    /// A new screenshot shows on the dock for two minutes, ready to drag.
    pub latest_screenshot: bool,
    /// Choices for the card widgets (cities, tickers, usernames).
    pub cards: CardSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            edge: Edge::default(),
            style: DockStyle::default(),
            theme: Theme::default(),
            size: DockSize::default(),
            look: DockLook::default(),
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
            wallpaper_folder: String::new(),
            wallpaper_minutes: 30,
            onboarded: false,
            floating: Vec::new(),
            auto_shrink: true,
            apps: AppsSettings::default(),
            hotkeys: Hotkeys::default(),
            accounts: AccountSettings::default(),
            latest_screenshot: true,
            cards: CardSettings::default(),
        }
    }
}

impl Settings {
    /// Dock bar height in logical pixels: the size choice scaled by the size slider.
    pub fn dock_height(&self) -> f64 {
        (self.size.height() * f64::from(self.look.scale) / 100.0).round()
    }

    /// Make the widget list sane: unknown kinds and repeats dropped, missing kinds added (off)
    /// at the end, so older and newer files both load.
    pub fn normalized(mut self) -> Self {
        let mut seen = Vec::new();
        self.widgets.retain(|w| {
            let keep = w.kind != WidgetKind::Unknown && !seen.contains(&w.kind);
            seen.push(w.kind);
            keep
        });
        // The taskbar corner's buttons arrived after most docks were set up: they go in on, just
        // before Status (where the taskbar has them), the first time a dock without them loads.
        for kind in [WidgetKind::HiddenIcons, WidgetKind::Language] {
            if !self.widgets.iter().any(|w| w.kind == kind) {
                let at = self
                    .widgets
                    .iter()
                    .position(|w| w.kind == WidgetKind::Status)
                    .unwrap_or(self.widgets.len());
                self.widgets.insert(
                    at,
                    WidgetEntry {
                        kind,
                        enabled: true,
                        size: WidgetSize::Normal,
                    },
                );
            }
        }
        for kind in WidgetKind::ALL {
            if !self.widgets.iter().any(|w| w.kind == kind) {
                self.widgets.push(WidgetEntry {
                    kind,
                    enabled: false,
                    size: WidgetSize::Normal,
                });
            }
        }
        self.look = self.look.clamped();
        self.cards = self.cards.tidied();
        self.weather_place = self.weather_place.trim().chars().take(80).collect();
        self.wallpaper_folder = self.wallpaper_folder.trim().to_string();
        self.wallpaper_minutes = self.wallpaper_minutes.min(MAX_WALLPAPER_MINUTES);
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
        let mut repos: Vec<String> = Vec::new();
        for repo in self.accounts.github_repos.iter().filter_map(|r| github_repo(r)) {
            if repos.len() < 20 && !repos.iter().any(|r| r.eq_ignore_ascii_case(&repo)) {
                repos.push(repo);
            }
        }
        self.accounts.github_repos = repos;
        let clean = |t: &str| t.trim().chars().take(200).collect::<String>();
        self.accounts.spotify_client_id = clean(&self.accounts.spotify_client_id);
        self.accounts.google_client_id = clean(&self.accounts.google_client_id);
        self.accounts.notion_database_id = clean(&self.accounts.notion_database_id);
        self
    }
}

/// How many layout changes "Undo layout" can step back through.
const UNDO_DEPTH: usize = 30;

/// The parts of the settings that "Undo layout" restores: widgets and the look.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Layout {
    widgets: Vec<WidgetEntry>,
    look: DockLook,
    size: DockSize,
    style: DockStyle,
}

impl Layout {
    fn of(settings: &Settings) -> Self {
        Self {
            widgets: settings.widgets.clone(),
            look: settings.look,
            size: settings.size,
            style: settings.style,
        }
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
    /// Apps grouped into folders on the dock, like a phone's home screen.
    pub folders: Vec<DockFolder>,
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
    /// Win+1 … Win+9 open the apps on the dock in order.
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
            folders: Vec::new(),
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

/// Most apps in one dock folder, and most folders.
const FOLDER_LIMIT: usize = 24;

/// Apps grouped under one button on the dock.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct DockFolder {
    pub id: String,
    pub name: String,
    /// The apps in it, by dock key, in order.
    pub keys: Vec<String>,
}

impl AppsSettings {
    /// Keep each app in one folder at most, drop empty folders and give each a name.
    fn tidy_folders(&mut self) {
        let mut seen: Vec<String> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        for folder in &mut self.folders {
            folder.id = clean(&folder.id, 64);
            folder.name = clean(&folder.name, 30);
            if folder.name.is_empty() {
                folder.name = "Folder".into();
            }
            folder.keys.retain(|k| !k.is_empty() && !seen.contains(k));
            folder.keys.truncate(FOLDER_LIMIT);
            seen.extend(folder.keys.iter().cloned());
        }
        self.folders.retain(|f| {
            let keep = !f.id.is_empty() && !f.keys.is_empty() && !ids.contains(&f.id);
            ids.push(f.id.clone());
            keep
        });
        self.folders.truncate(FOLDER_LIMIT);
    }

    pub fn normalized(mut self) -> Self {
        self.tidy_folders();
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
    /// Earlier layouts, newest last.
    undo: Mutex<Vec<Layout>>,
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
            undo: Mutex::new(Vec::new()),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().expect("settings lock").clone()
    }

    /// Save to disk (write a temp file, then rename, so a crash never leaves half a file).
    pub fn set(&self, settings: Settings) -> AppResult<()> {
        let settings = settings.normalized();
        let before = Layout::of(&self.get());
        if before != Layout::of(&settings) {
            let mut undo = self.undo.lock().expect("undo lock");
            undo.push(before);
            if undo.len() > UNDO_DEPTH {
                undo.remove(0);
            }
        }
        self.write(settings)
    }

    /// Whether there is a layout change to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.lock().expect("undo lock").is_empty()
    }

    /// Put back the layout before the last change; `None` when there's nothing to undo.
    pub fn undo_layout(&self) -> AppResult<Option<Settings>> {
        let Some(layout) = self.undo.lock().expect("undo lock").pop() else {
            return Ok(None);
        };
        let settings = Settings {
            widgets: layout.widgets,
            look: layout.look,
            size: layout.size,
            style: layout.style,
            ..self.get()
        };
        self.write(settings.clone())?;
        Ok(Some(self.get()))
    }

    fn write(&self, settings: Settings) -> AppResult<()> {
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
    fn layout_changes_can_be_undone() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::open(dir.path());
        assert!(!store.can_undo());
        let mut s = store.get();
        s.look.scale = 150;
        store.set(s.clone()).unwrap();
        s.widgets.reverse();
        store.set(s.clone()).unwrap();
        // Not a layout change: nothing new to undo.
        s.weather_place = "Pune".into();
        store.set(s).unwrap();
        let back = store.undo_layout().unwrap().unwrap();
        assert_eq!(back.widgets, Settings::default().widgets);
        assert_eq!(back.look.scale, 150);
        assert_eq!(back.weather_place, "Pune");
        let back = store.undo_layout().unwrap().unwrap();
        assert_eq!(back.look.scale, 100);
        assert!(store.undo_layout().unwrap().is_none());
    }

    #[test]
    fn look_is_clamped_and_scales_height() {
        let mut s = Settings::default();
        s.look.scale = 999;
        s.look.transparency = 200;
        let s = s.normalized();
        assert_eq!(s.look.scale, 200);
        assert_eq!(s.look.transparency, 70);
        assert_eq!(s.dock_height(), 112.0);
    }

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
                enabled: true,
                size: WidgetSize::Normal,
            }
        );
        assert_eq!(
            widgets[1],
            WidgetEntry {
                kind: WidgetKind::Music,
                enabled: false,
                size: WidgetSize::Normal,
            }
        );
        // Missing kinds were added, switched off, except the taskbar corner's buttons.
        let corner = [WidgetKind::HiddenIcons, WidgetKind::Language];
        assert!(widgets[2..].iter().all(|w| w.enabled == corner.contains(&w.kind)));
        for kind in WidgetKind::ALL {
            assert_eq!(widgets.iter().filter(|w| w.kind == kind).count(), 1);
        }
    }

    #[test]
    fn the_taskbar_corner_goes_in_before_status() {
        let dir = tempfile::tempdir().unwrap();
        let file = r#"{"widgets":[
            {"kind":"music","enabled":true},
            {"kind":"status","enabled":true},
            {"kind":"clock","enabled":true}]}"#;
        std::fs::write(dir.path().join(FILE_NAME), file).unwrap();
        let widgets = SettingsStore::open(dir.path()).get().widgets;
        let kinds: Vec<_> = widgets.iter().take(5).map(|w| (w.kind, w.enabled)).collect();
        assert_eq!(
            kinds,
            [
                (WidgetKind::Music, true),
                (WidgetKind::HiddenIcons, true),
                (WidgetKind::Language, true),
                (WidgetKind::Status, true),
                (WidgetKind::Clock, true),
            ]
        );
    }

    #[test]
    fn wallpaper_settings_are_tidied() {
        let settings = Settings {
            wallpaper_folder: r"  C:\Pictures  ".into(),
            wallpaper_minutes: 100_000,
            ..Settings::default()
        }
        .normalized();
        assert_eq!(settings.wallpaper_folder, r"C:\Pictures");
        assert_eq!(settings.wallpaper_minutes, 24 * 60);
    }

    #[test]
    fn new_automations_default_on() {
        let a = Automations::default();
        assert!(a.bluetooth_battery && a.download_finished && a.claude_reset);
        // An older file without them gets the defaults.
        let old: Automations = serde_json::from_str(r#"{"meetingMode":false}"#).unwrap();
        assert!(!old.meeting_mode && old.download_finished);
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
    fn folders_keep_each_app_once_and_drop_empty_ones() {
        let folder = |id: &str, name: &str, keys: &[&str]| DockFolder {
            id: id.into(),
            name: name.into(),
            keys: keys.iter().map(|k| k.to_string()).collect(),
        };
        let apps = AppsSettings {
            folders: vec![
                folder("a", "  Coding  ", &["pin:code", "pin:term"]),
                folder("b", "", &["pin:code", "pin:chrome"]),
                folder("c", "Empty", &["pin:term"]),
                folder("a", "Again", &["pin:x"]),
            ],
            ..AppsSettings::default()
        }
        .normalized();
        assert_eq!(apps.folders.len(), 2);
        assert_eq!(apps.folders[0].name, "Coding");
        assert_eq!(apps.folders[1].name, "Folder");
        assert_eq!(apps.folders[1].keys, vec!["pin:chrome".to_string()]);
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
    fn github_repos_are_cleaned() {
        let mut settings = Settings::default();
        settings.accounts.github_repos = vec![
            " tauri-apps/tauri ".into(),
            "https://github.com/rust-lang/rust/".into(),
            "Tauri-Apps/Tauri".into(),
            "not a repo".into(),
            "a/b/c".into(),
        ];
        assert_eq!(
            settings.normalized().accounts.github_repos,
            vec!["tauri-apps/tauri".to_string(), "rust-lang/rust".to_string()]
        );
    }

    #[test]
    fn older_files_get_default_hotkeys() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), r#"{"toggleShortcut":"Ctrl+Alt+K"}"#).unwrap();
        let settings = SettingsStore::open(dir.path()).get();
        assert_eq!(settings.toggle_shortcut, "Ctrl+Alt+K");
        assert_eq!(settings.hotkeys.command_bar, "Ctrl+Space");
        assert!(settings.latest_screenshot);
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
