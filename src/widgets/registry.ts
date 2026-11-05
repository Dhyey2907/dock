import type { ComponentType } from "react";

import type { WidgetKind } from "../core/generated/WidgetKind";
import { ClockPanel, ClockTile } from "./ClockWidget";
import { FocusPanel, FocusTile } from "./FocusWidget";
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
  focus: {
    label: "Focus timer",
    description: "25 minutes of focus, then a short break.",
    Tile: FocusTile,
    Panel: FocusPanel,
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
