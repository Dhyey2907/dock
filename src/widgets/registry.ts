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
};
