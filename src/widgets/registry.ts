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
};
