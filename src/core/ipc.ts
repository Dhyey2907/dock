import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import type { ButtonRect } from "./generated/ButtonRect";
import type { DockMenuAction } from "./generated/DockMenuAction";
import type { MenuAction } from "./generated/MenuAction";
import type { PreviewContent } from "./generated/PreviewContent";
import type { SwitcherState } from "./generated/SwitcherState";
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

/** Windows Settings pages the widgets may open. */
export type WindowsSettingsPage =
  "location" | "network" | "sound" | "battery" | "datetime" | "bluetooth" | "home";

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
  /** A left click; `anchorX` (screen pixels) is where a pinned folder's popup opens. */
  appClick: (key: string, anchorX = 0) => invoke<void>("app_click", { key, anchorX: Math.round(anchorX) }),
  /** A file dragged over the app's icon for a moment: bring its window forward. */
  appRaise: (key: string) => invoke<void>("app_raise", { key }),
  /** The icon is being dragged off the dock; its window snaps where the button is let go. */
  appDragOut: (key: string) => invoke<void>("app_drag_out", { key }),
  appRects: (rects: ButtonRect[]) => invoke<void>("app_rects", { rects }),
  appsAll: () => invoke<DockApp[]>("apps_all"),
  appPreviews: (key: string, anchorX: number) =>
    invoke<void>("app_previews", { key, anchorX: Math.round(anchorX) }),
  previewHide: (delayMs: number) => invoke<void>("preview_hide", { delayMs: Math.round(delayMs) }),
  previewHover: (hovered: boolean) => invoke<void>("preview_hover", { hovered }),
  previewCurrent: () => invoke<PreviewContent | null>("preview_current"),
  previewActivate: (hwnd: number) => invoke<void>("preview_activate", { hwnd }),
  previewCloseWindow: (hwnd: number) => invoke<void>("preview_close_window", { hwnd }),
  switcherCurrent: () => invoke<SwitcherState | null>("switcher_current"),
  switcherPick: (hwnd: number) => invoke<void>("switcher_pick", { hwnd }),
  stackOpen: (path: string) => invoke<void>("stack_open", { path }),
  dockMenu: (anchorX: number) => invoke<void>("dock_menu", { anchorX: Math.round(anchorX) }),
  dockMenuAction: (action: DockMenuAction) => invoke<void>("dock_menu_action", { action }),
  dockPress: () => invoke<void>("dock_press"),
  dockHold: () => invoke<void>("dock_hold"),
  showDesktop: () => invoke<void>("show_desktop"),
  desktopPeek: (on: boolean) => invoke<void>("desktop_peek", { on }),
  /** Ask for an image (or with `folder`, a folder); null when cancelled. */
  pickPath: (folder: boolean) => invoke<string | null>("pick_path", { folder }),
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
  onAppLaunching: on<string>("app-launching"),
  onPreviewOpen: on<PreviewContent>("preview-open"),
  onPreviewClosed: on<null>("preview-closed"),
  onSwitcherState: on<SwitcherState>("switcher-state"),
};
