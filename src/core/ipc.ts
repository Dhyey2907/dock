import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import type { MenuAction } from "./generated/MenuAction";
import type { PanelContent } from "./generated/PanelContent";
import type { QuickState } from "./generated/QuickState";
import type { RadioKind } from "./generated/RadioKind";
import type { Bubble } from "./generated/Bubble";
import type { CalendarState } from "./generated/CalendarState";
import type { ClaudeUsage } from "./generated/ClaudeUsage";
import type { DockApp } from "./generated/DockApp";
import type { DockGeometry } from "./generated/DockGeometry";
import type { FocusState } from "./generated/FocusState";
import type { GlassFrame } from "./generated/GlassFrame";
import type { MediaCommand } from "./generated/MediaCommand";
import type { MediaState } from "./generated/MediaState";
import type { NetworkState } from "./generated/NetworkState";
import type { ScreenInfo } from "./generated/ScreenInfo";
import type { Settings } from "./generated/Settings";
import type { SoundState } from "./generated/SoundState";
import type { SystemStats } from "./generated/SystemStats";
import type { Weather } from "./generated/Weather";
import type { WidgetKind } from "./generated/WidgetKind";
import type { WidgetSnapshot } from "./generated/WidgetSnapshot";
import type { BluetoothDevice } from "./generated/BluetoothDevice";
import type { DevServer } from "./generated/DevServer";
import type { DisplayMode } from "./generated/DisplayMode";
import type { DisplayState } from "./generated/DisplayState";
import type { EjectResult } from "./generated/EjectResult";
import type { MonitorBrightness } from "./generated/MonitorBrightness";
import type { TopApps } from "./generated/TopApps";
import type { Trending } from "./generated/Trending";
import type { UptimeState } from "./generated/UptimeState";
import type { UsbDrive } from "./generated/UsbDrive";
import type { WallpaperState } from "./generated/WallpaperState";
import type { WifiDetails } from "./generated/WifiDetails";

/** Windows Settings pages the widgets may open. */
export type WindowsSettingsPage =
  "location" | "network" | "sound" | "battery" | "datetime" | "bluetooth" | "home" | "update" | "display";

/** Typed wrappers for every backend command, so call sites never spell command names. */
export const ipc = {
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  dockSetContentWidth: (width: number) => invoke<void>("dock_set_content_width", { width }),
  dockReveal: (revealed: boolean) => invoke<void>("dock_reveal", { revealed }),
  dockIsRevealed: () => invoke<boolean>("dock_is_revealed"),
  openSettings: (section?: string) => invoke<void>("open_settings", { section: section ?? null }),
  screensList: () => invoke<ScreenInfo[]>("screens_list"),
  shortcutSet: (shortcut: string) => invoke<Settings>("shortcut_set", { shortcut }),
  onboardingDone: (openSettings: boolean) => invoke<void>("onboarding_done", { openSettings }),
  appQuit: () => invoke<void>("app_quit"),
  /** Hide the docks and bring back the Windows taskbar until the user switches back. */
  switchToWindowsTaskbar: () => invoke<void>("use_windows_taskbar"),
  appsList: () => invoke<DockApp[]>("apps_list"),
  appIcon: (source: string) => invoke<string | null>("app_icon", { source }),
  quickGet: () => invoke<QuickState>("quick_get"),
  quickSetRadio: (kind: RadioKind, on: boolean) => invoke<QuickState>("quick_set_radio", { kind, on }),
  quickSetBrightness: (level: number) => invoke<void>("quick_set_brightness", { level }),
  /** Windows' own Quick Settings, for Night light, Airplane mode and the rest. */
  quickOpenWindows: () => invoke<void>("quick_open_windows"),
  appClick: (key: string) => invoke<void>("app_click", { key }),
  appLaunch: (key: string) => invoke<void>("app_launch", { key }),
  /** An app's right-click menu, centred on `anchorX` (screen pixels). */
  appMenu: (key: string, anchorX: number) => invoke<void>("app_menu", { key, anchorX: Math.round(anchorX) }),
  appMenuAction: (action: MenuAction) => invoke<void>("app_menu_action", { action }),
  shellStart: () => invoke<void>("shell_start"),
  shellSearch: () => invoke<void>("shell_search"),
  dockGeometry: () => invoke<DockGeometry | null>("dock_geometry"),
  glassGet: () => invoke<GlassFrame | null>("glass_get"),
  panelToggle: (kind: WidgetKind, anchorX: number) =>
    invoke<void>("panel_toggle", { kind, anchorX: Math.round(anchorX) }),
  panelClose: () => invoke<void>("panel_close"),
  panelCurrent: () => invoke<PanelContent | null>("panel_current"),
  widgetsSnapshot: () => invoke<WidgetSnapshot>("widgets_snapshot"),
  volumeSet: (level: number) => invoke<void>("volume_set", { level }),
  volumeToggleMute: () => invoke<void>("volume_toggle_mute"),
  mediaCommand: (command: MediaCommand) => invoke<void>("media_command", { command }),
  calendarGet: () => invoke<CalendarState>("calendar_get"),
  claudeUsage: () => invoke<ClaudeUsage>("claude_usage"),
  bubblesGet: () => invoke<Bubble[]>("bubbles_get"),
  bubbleDismiss: (id: string) => invoke<void>("bubble_dismiss", { id }),
  calendarAdd: (name: string, link: string) => invoke<void>("calendar_add", { name, link }),
  calendarRemove: (id: string) => invoke<void>("calendar_remove", { id }),
  focusStart: () => invoke<void>("focus_start"),
  focusPause: () => invoke<void>("focus_pause"),
  focusReset: () => invoke<void>("focus_reset"),
  openWindowsSettings: (page: WindowsSettingsPage) => invoke<void>("open_windows_settings", { page }),
  // System widgets.
  topApps: () => invoke<TopApps>("top_apps"),
  processEnd: (pids: number[], name: string) => invoke<void>("process_end", { pids, name }),
  bluetoothDevices: () => invoke<BluetoothDevice[]>("bluetooth_devices"),
  monitorsBrightness: () => invoke<MonitorBrightness[]>("monitors_brightness"),
  monitorSetBrightness: (id: string, percent: number) =>
    invoke<void>("monitor_set_brightness", { id, percent: Math.round(percent) }),
  wifiDetails: () => invoke<WifiDetails>("wifi_details"),
  publicIp: () => invoke<string | null>("public_ip"),
  uptimeGet: () => invoke<UptimeState>("uptime_get"),
  devServers: () => invoke<DevServer[]>("dev_servers"),
  devServerStop: (port: number, pid: number) => invoke<void>("dev_server_stop", { port, pid }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  desktopIconsGet: () => invoke<boolean | null>("desktop_icons_get"),
  desktopIconsSet: (show: boolean) => invoke<boolean>("desktop_icons_set", { show }),
  screenOff: () => invoke<void>("screen_off"),
  wallpaperGet: () => invoke<WallpaperState>("wallpaper_get"),
  wallpaperNext: () => invoke<WallpaperState>("wallpaper_next"),
  wallpaperPickFolder: () => invoke<string | null>("wallpaper_pick_folder"),
  displayGet: () => invoke<DisplayState>("display_get"),
  displaySet: (mode: DisplayMode) => invoke<void>("display_set", { mode }),
  usbDrives: () => invoke<UsbDrive[]>("usb_drives"),
  usbEject: (letter: string) => invoke<EjectResult>("usb_eject", { letter }),
  githubTrending: () => invoke<Trending>("github_trending"),
  bubbleOpen: (id: string) => invoke<void>("bubble_open", { id }),
};

function on<T>(name: string) {
  return (handler: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (event) => handler(event.payload));
}

/**
 * Events the backend sends to one dock (its own apps, place and glass). A plain `listen` hears
 * events sent to every window, so with a dock per screen each would pick up the others'.
 */
function onThisWindow<T>(name: string) {
  return (handler: (payload: T) => void): Promise<UnlistenFn> =>
    getCurrentWebviewWindow().listen<T>(name, (event) => handler(event.payload));
}

/** The main dock carries the pinned apps and the widgets; docks on other screens don't. */
export const MAIN_DOCK_LABEL = "dock";

export function isMainDock(): boolean {
  try {
    return getCurrentWebviewWindow().label === MAIN_DOCK_LABEL;
  } catch {
    // Outside Tauri (tests): behave as the main dock.
    return true;
  }
}

export const events = {
  onSettingsChanged: on<Settings>("settings-changed"),
  onDockRevealed: onThisWindow<boolean>("dock-revealed"),
  onAppsChanged: onThisWindow<DockApp[]>("apps-changed"),
  onDockGeometry: onThisWindow<DockGeometry>("dock-geometry"),
  onGlassChanged: onThisWindow<GlassFrame>("glass-changed"),
  onPanelOpen: on<PanelContent>("panel-open"),
  onPanelClosed: on<null>("panel-closed"),
  onWeather: on<Weather>("widget-weather"),
  onStats: on<SystemStats>("widget-stats"),
  onSound: on<SoundState>("widget-sound"),
  onNetwork: on<NetworkState>("widget-network"),
  onFocus: on<FocusState>("widget-focus"),
  onMusic: on<MediaState | null>("widget-music"),
  onCalendar: on<CalendarState>("widget-calendar"),
  onClaude: on<ClaudeUsage>("widget-claude"),
  onBubbles: on<Bubble[]>("bubbles-changed"),
  onSettingsSection: on<string>("settings-section"),
};
