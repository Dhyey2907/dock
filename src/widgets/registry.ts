import type { ComponentType } from "react";

import type { WidgetKind } from "../core/generated/WidgetKind";
import {
  AskClaudePanel,
  AskClaudeTile,
  GithubActionsPanel,
  GithubActionsTile,
  GoogleTasksPanel,
  GoogleTasksTile,
  NotionPanel,
  NotionTile,
  SpotifyPanel,
  SpotifyTile,
} from "./accounts/AccountWidgets";
import { CalendarPanel, CalendarTile } from "./CalendarWidget";
import { ClaudePanel, ClaudeTile } from "./ClaudeWidget";
import { ClockPanel, ClockTile } from "./ClockWidget";
import { FocusPanel, FocusTile } from "./FocusWidget";
import { MusicPanel, MusicTile } from "./MusicWidget";
import { StatusPanel, StatusTile } from "./StatusWidget";
import { SystemPanel, SystemTile } from "./SystemWidget";
import { WeatherPanel, WeatherTile } from "./WeatherWidget";
import {
  ClaudeSessionsPanel,
  ClaudeSessionsTile,
  SongsTodayPanel,
  SongsTodayTile,
} from "./productivity/ActivityWidgets";
import { AppFoldersPanel, AppFoldersTile } from "./productivity/AppFoldersWidget";
import { BookmarksPanel, BookmarksTile } from "./productivity/BookmarksWidget";
import { PinImagePanel, PinImageTile, ScreenTextPanel, ScreenTextTile } from "./productivity/CaptureWidgets";
import { FileSearchPanel, FileSearchTile } from "./productivity/FileSearchWidget";
import { KanbanPanel, KanbanTile } from "./productivity/KanbanWidget";
import { LinksPanel, LinksTile } from "./productivity/LinksWidget";
import { MeetingNotesPanel, MeetingNotesTile } from "./productivity/MeetingNotesWidget";
import { NotesPanel, NotesTile } from "./productivity/NotesWidget";
import { TimeTrackerPanel, TimeTrackerTile } from "./productivity/TimeTrackerWidget";
import { TodosPanel, TodosTile } from "./productivity/TodosWidget";

/** Where a widget sits in the gallery. */
export type WidgetCategory = "productivity" | "system" | "info" | "media" | "accounts";

/** Gallery sections, top to bottom. */
export const CATEGORY_ORDER: WidgetCategory[] = ["productivity", "system", "info", "media", "accounts"];

export const CATEGORY_LABELS: Record<WidgetCategory, string> = {
  productivity: "Productivity",
  system: "System",
  info: "Information",
  media: "Media",
  accounts: "Accounts",
};
import {
  BluetoothPanel,
  BluetoothTile,
  BrightnessPanel,
  BrightnessTile,
  DesktopIconsPanel,
  DesktopIconsTile,
  DisplayModePanel,
  DisplayModeTile,
  ScreenOffPanel,
  ScreenOffTile,
  WallpaperPanel,
  WallpaperTile,
} from "./DeviceWidgets";
import { GithubTrendingPanel, GithubTrendingTile, WifiPanel, WifiTile } from "./NetWidgets";
import {
  DevServersPanel,
  DevServersTile,
  TopAppsPanel,
  TopAppsTile,
  UptimePanel,
  UptimeTile,
  UsbDrivesPanel,
  UsbDrivesTile,
} from "./ToolWidgets";

export interface WidgetDefinition {
  label: string;
  /** One line for the widget picker. */
  description: string;
  /** The gallery section; "info" when left out. */
  category?: WidgetCategory;
  Tile: ComponentType;
  Panel: ComponentType;
}

/** The widgets this version can show. Kinds not listed here are skipped on the dock. */
export const WIDGETS: Partial<Record<WidgetKind, WidgetDefinition>> = {
  music: {
    label: "Music",
    category: "media",
    description: "What's playing in Spotify, your browser or any media app, with controls.",
    Tile: MusicTile,
    Panel: MusicPanel,
  },
  focus: {
    label: "Focus timer",
    category: "productivity",
    description: "25 minutes of focus, then a short break.",
    Tile: FocusTile,
    Panel: FocusPanel,
  },
  claude: {
    label: "Claude",
    category: "info",
    description: "Your Claude Code use this 5-hour window and week, estimated on this PC.",
    Tile: ClaudeTile,
    Panel: ClaudePanel,
  },
  calendar: {
    label: "Calendar",
    category: "productivity",
    description: "Your next event, and the week ahead, from Google, Outlook or iCloud links.",
    Tile: CalendarTile,
    Panel: CalendarPanel,
  },
  system: {
    label: "System",
    category: "system",
    description: "Processor and memory use.",
    Tile: SystemTile,
    Panel: SystemPanel,
  },
  status: {
    label: "Status",
    category: "system",
    description: "Network, volume and battery, like the taskbar's corner.",
    Tile: StatusTile,
    Panel: StatusPanel,
  },
  weather: {
    label: "Weather",
    category: "info",
    description: "Now, today and the days ahead.",
    Tile: WeatherTile,
    Panel: WeatherPanel,
  },
  clock: {
    label: "Clock",
    category: "info",
    description: "Time, date and a month calendar.",
    Tile: ClockTile,
    Panel: ClockPanel,
  },
  notes: {
    label: "Quick notes",
    description: "A sticky note that saves as you type. Drop text on the dock to make one.",
    Tile: NotesTile,
    Panel: NotesPanel,
    category: "productivity",
  },
  todos: {
    label: "To-do",
    description: "A checklist; the dock shows how many are left. Ctrl+Alt+N adds one from anywhere.",
    Tile: TodosTile,
    Panel: TodosPanel,
    category: "productivity",
  },
  kanban: {
    label: "Kanban board",
    description: "To do, Doing and Done, with cards you drag between them.",
    Tile: KanbanTile,
    Panel: KanbanPanel,
    category: "productivity",
  },
  meetingNotes: {
    label: "Meeting notes",
    description: "Notes for your current or next calendar event.",
    Tile: MeetingNotesTile,
    Panel: MeetingNotesPanel,
    category: "productivity",
  },
  timeTracker: {
    label: "Time tracker",
    description: "Start and stop a timer per project, with this week's totals.",
    Tile: TimeTrackerTile,
    Panel: TimeTrackerPanel,
    category: "productivity",
  },
  quickLinks: {
    label: "Quick links",
    description: "Favourite websites and folders, one click each.",
    Tile: LinksTile,
    Panel: LinksPanel,
    category: "productivity",
  },
  appFolders: {
    label: "App folders",
    description: "Group apps into one button that opens a small grid.",
    Tile: AppFoldersTile,
    Panel: AppFoldersPanel,
    category: "productivity",
  },
  bookmarks: {
    label: "Bookmarks",
    description: "Search and open your Chrome and Edge bookmarks.",
    Tile: BookmarksTile,
    Panel: BookmarksPanel,
    category: "productivity",
  },
  fileSearch: {
    label: "File search",
    description: "Instant search through the Windows Search index.",
    Tile: FileSearchTile,
    Panel: FileSearchPanel,
    category: "productivity",
  },
  pinImage: {
    label: "Pin image",
    description: "Float a screen area or the clipboard's picture above everything.",
    Tile: PinImageTile,
    Panel: PinImagePanel,
    category: "productivity",
  },
  screenText: {
    label: "Copy text from screen",
    description: "Read the text in an area of the screen, on this PC, and copy it.",
    Tile: ScreenTextTile,
    Panel: ScreenTextPanel,
    category: "productivity",
  },
  songsToday: {
    label: "Songs played today",
    description: "Everything you've played today, from the music widget.",
    Tile: SongsTodayTile,
    Panel: SongsTodayPanel,
    category: "productivity",
  },
  claudeSessions: {
    label: "Claude Code activity",
    description: "Running and recent Claude Code sessions on this PC.",
    Tile: ClaudeSessionsTile,
    Panel: ClaudeSessionsPanel,
    category: "productivity",
  },
  topApps: {
    label: "Top apps",
    description: "The programs using the most processor and memory, with End task.",
    Tile: TopAppsTile,
    Panel: TopAppsPanel,
    category: "system",
  },
  bluetooth: {
    label: "Bluetooth devices",
    description: "Your paired devices and their battery levels.",
    Tile: BluetoothTile,
    Panel: BluetoothPanel,
    category: "system",
  },
  brightness: {
    label: "Screen brightness",
    description: "A brightness slider for each external screen.",
    Tile: BrightnessTile,
    Panel: BrightnessPanel,
    category: "system",
  },
  wifi: {
    label: "Wi-Fi details",
    description: "Network name, signal, and your local and public addresses.",
    Tile: WifiTile,
    Panel: WifiPanel,
    category: "system",
  },
  uptime: {
    label: "Uptime",
    description: "Time since the PC started, and whether an update needs a restart.",
    Tile: UptimeTile,
    Panel: UptimePanel,
    category: "system",
  },
  devServers: {
    label: "Local servers",
    description: "Dev servers running on this PC: open one in the browser or stop it.",
    Tile: DevServersTile,
    Panel: DevServersPanel,
    category: "system",
  },
  desktopIcons: {
    label: "Desktop icons",
    description: "Show or hide the icons on your desktop.",
    Tile: DesktopIconsTile,
    Panel: DesktopIconsPanel,
    category: "system",
  },
  screenOff: {
    label: "Screen off",
    description: "Turn the screens off; the mouse or a key wakes them.",
    Tile: ScreenOffTile,
    Panel: ScreenOffPanel,
    category: "system",
  },
  wallpaper: {
    label: "Wallpaper changer",
    description: "Pictures from a folder as your wallpaper, in turn.",
    Tile: WallpaperTile,
    Panel: WallpaperPanel,
    category: "system",
  },
  displayMode: {
    label: "Display mode",
    description: "Extend, duplicate or use one screen only.",
    Tile: DisplayModeTile,
    Panel: DisplayModePanel,
    category: "system",
  },
  usbDrives: {
    label: "USB drives",
    description: "Removable drives, with Safely Remove.",
    Tile: UsbDrivesTile,
    Panel: UsbDrivesPanel,
    category: "system",
  },
  githubTrending: {
    label: "GitHub trending",
    description: "Today's trending repositories on GitHub.",
    Tile: GithubTrendingTile,
    Panel: GithubTrendingPanel,
    category: "info",
  },
  askClaude: {
    label: "Ask Claude",
    description: "Ask Claude a question and read the answer as it's written. Uses your Anthropic API key.",
    Tile: AskClaudeTile,
    Panel: AskClaudePanel,
    category: "accounts",
  },
  spotify: {
    label: "Spotify playlists",
    description: "Your Spotify playlists; play one on the device you're using.",
    Tile: SpotifyTile,
    Panel: SpotifyPanel,
    category: "accounts",
  },
  notion: {
    label: "Notion tasks",
    description: "Open tasks from a Notion database; tick them off from the dock.",
    Tile: NotionTile,
    Panel: NotionPanel,
    category: "accounts",
  },
  googleTasks: {
    label: "Google Tasks",
    description: "Your open Google Tasks; tick them off from the dock.",
    Tile: GoogleTasksTile,
    Panel: GoogleTasksPanel,
    category: "accounts",
  },
  githubActions: {
    label: "GitHub Actions",
    description: "The latest workflow run of each repo you watch.",
    Tile: GithubActionsTile,
    Panel: GithubActionsPanel,
    category: "accounts",
  },
};
