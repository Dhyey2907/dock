import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { DockApp } from "./generated/DockApp";
import type { DockGeometry } from "./generated/DockGeometry";
import type { FocusState } from "./generated/FocusState";
import type { GlassFrame } from "./generated/GlassFrame";
import type { NetworkState } from "./generated/NetworkState";
import type { Settings } from "./generated/Settings";
import type { SoundState } from "./generated/SoundState";
import type { SystemStats } from "./generated/SystemStats";
import type { Weather } from "./generated/Weather";
import type { WidgetKind } from "./generated/WidgetKind";
import type { WidgetSnapshot } from "./generated/WidgetSnapshot";

/** Windows Settings pages the widgets may open. */
export type WindowsSettingsPage = "location" | "network" | "sound" | "battery" | "datetime";

/** Typed wrappers for every backend command, so call sites never spell command names. */
export const ipc = {
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  dockSetContentWidth: (width: number) => invoke<void>("dock_set_content_width", { width }),
  dockReveal: (revealed: boolean) => invoke<void>("dock_reveal", { revealed }),
  dockIsRevealed: () => invoke<boolean>("dock_is_revealed"),
  openSettings: () => invoke<void>("open_settings"),
  appQuit: () => invoke<void>("app_quit"),
  appsList: () => invoke<DockApp[]>("apps_list"),
  appIcon: (source: string) => invoke<string | null>("app_icon", { source }),
  appClick: (key: string) => invoke<void>("app_click", { key }),
  appLaunch: (key: string) => invoke<void>("app_launch", { key }),
  appMenu: (key: string) => invoke<void>("app_menu", { key }),
  shellStart: () => invoke<void>("shell_start"),
  shellSearch: () => invoke<void>("shell_search"),
  dockGeometry: () => invoke<DockGeometry | null>("dock_geometry"),
  glassGet: () => invoke<GlassFrame | null>("glass_get"),
  panelToggle: (kind: WidgetKind, anchorX: number) =>
    invoke<void>("panel_toggle", { kind, anchorX: Math.round(anchorX) }),
  panelClose: () => invoke<void>("panel_close"),
  panelCurrent: () => invoke<WidgetKind | null>("panel_current"),
  widgetsSnapshot: () => invoke<WidgetSnapshot>("widgets_snapshot"),
  volumeSet: (level: number) => invoke<void>("volume_set", { level }),
  volumeToggleMute: () => invoke<void>("volume_toggle_mute"),
  focusStart: () => invoke<void>("focus_start"),
  focusPause: () => invoke<void>("focus_pause"),
  focusReset: () => invoke<void>("focus_reset"),
  openWindowsSettings: (page: WindowsSettingsPage) => invoke<void>("open_windows_settings", { page }),
};

function on<T>(name: string) {
  return (handler: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (event) => handler(event.payload));
}

export const events = {
  onSettingsChanged: on<Settings>("settings-changed"),
  onDockRevealed: on<boolean>("dock-revealed"),
  onAppsChanged: on<DockApp[]>("apps-changed"),
  onDockGeometry: on<DockGeometry>("dock-geometry"),
  onGlassChanged: on<GlassFrame>("glass-changed"),
  onPanelOpen: on<{ kind: WidgetKind }>("panel-open"),
  onPanelClosed: on<null>("panel-closed"),
  onWeather: on<Weather>("widget-weather"),
  onStats: on<SystemStats>("widget-stats"),
  onSound: on<SoundState>("widget-sound"),
  onNetwork: on<NetworkState>("widget-network"),
  onFocus: on<FocusState>("widget-focus"),
};
