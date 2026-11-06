import type { ComponentType } from "react";

import type { WidgetKind } from "../core/generated/WidgetKind";
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

export interface WidgetDefinition {
  label: string;
  /** One line for the widget picker. */
  description: string;
  Tile: ComponentType;
  Panel: ComponentType;
  /** Where the widget gallery files it. */
  category?: "productivity" | "system" | "info" | "media" | "accounts";
}

/** The widgets this version can show. Kinds not listed here are skipped on the dock. */
export const WIDGETS: Partial<Record<WidgetKind, WidgetDefinition>> = {
  music: {
    label: "Music",
    description: "What's playing in Spotify, your browser or any media app, with controls.",
    Tile: MusicTile,
    Panel: MusicPanel,
  },
  focus: {
    label: "Focus timer",
    description: "25 minutes of focus, then a short break.",
    Tile: FocusTile,
    Panel: FocusPanel,
  },
  claude: {
    label: "Claude",
    description: "Your Claude Code use this 5-hour window and week, estimated on this PC.",
    Tile: ClaudeTile,
    Panel: ClaudePanel,
  },
  calendar: {
    label: "Calendar",
    description: "Your next event, and the week ahead, from Google, Outlook or iCloud links.",
    Tile: CalendarTile,
    Panel: CalendarPanel,
  },
  system: {
    label: "System",
    description: "Processor and memory use.",
    Tile: SystemTile,
    Panel: SystemPanel,
  },
  status: {
    label: "Status",
    description: "Network, volume and battery, like the taskbar's corner.",
    Tile: StatusTile,
    Panel: StatusPanel,
  },
  weather: {
    label: "Weather",
    description: "Now, today and the days ahead.",
    Tile: WeatherTile,
    Panel: WeatherPanel,
  },
  clock: {
    label: "Clock",
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
};
