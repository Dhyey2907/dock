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

export interface WidgetDefinition {
  label: string;
  /** One line for the widget picker. */
  description: string;
  Tile: ComponentType;
  Panel: ComponentType;
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
};
