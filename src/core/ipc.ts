import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import type { DockFolder } from "./generated/DockFolder";
import type { InputLanguage } from "./generated/InputLanguage";
import type { SystemClock } from "./generated/SystemClock";
import type { SystemLook } from "./generated/SystemLook";
import type { TrayButton } from "./generated/TrayButton";
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
import type { AppFolders } from "./generated/AppFolders";
import type { Bookmark } from "./generated/Bookmark";
import type { CaptureKind } from "./generated/CaptureKind";
import type { ClaudeActivity } from "./generated/ClaudeActivity";
import type { FileHit } from "./generated/FileHit";
import type { FolderApp } from "./generated/FolderApp";
import type { KanbanBoard } from "./generated/KanbanBoard";
import type { Note } from "./generated/Note";
import type { QuickLinks } from "./generated/QuickLinks";
import type { ScreenText } from "./generated/ScreenText";
import type { SnipBackdrop } from "./generated/SnipBackdrop";
import type { SnipMode } from "./generated/SnipMode";
import type { SongsToday } from "./generated/SongsToday";
import type { TimeLog } from "./generated/TimeLog";
import type { Todo } from "./generated/Todo";
import type { TodoChange } from "./generated/TodoChange";
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
  /** Put back the widget layout and look from before the last change. */
  settingsUndoLayout: () => invoke<Settings>("settings_undo_layout"),
  settingsCanUndo: () => invoke<boolean>("settings_can_undo"),
  /** Turn the open widget popup into its own window that stays on top. */
  panelDetach: () => invoke<void>("panel_detach"),
  /** Show a widget in its own window: on top of other windows, or on the desktop. */
  floatingShow: (kind: WidgetKind, onTop: boolean) => invoke<void>("floating_show", { kind, onTop }),
  floatingMoved: (kind: WidgetKind, x: number, y: number) => invoke<void>("floating_moved", { kind, x, y }),
  floatingClose: (kind: WidgetKind) => invoke<void>("floating_close", { kind }),
  dashboardOpen: () => invoke<void>("dashboard_open"),
  /** Open or close a dock folder's popup, centred on `anchorX` (screen pixels). */
  folderOpen: (id: string, anchorX: number) =>
    invoke<void>("folder_open", { id, anchorX: Math.round(anchorX) }),
  /** Save the dock's folders without touching any other setting. */
  dockFoldersSet: (folders: DockFolder[]) => invoke<Settings>("dock_folders_set", { folders }),
  folderAppClick: (dock: string, key: string) => invoke<void>("folder_app_click", { dock, key }),
  folderAppMenu: (dock: string, key: string, anchorX: number) =>
    invoke<void>("folder_app_menu", { dock, key, anchorX: Math.round(anchorX) }),
  /** Open or close the widget gallery, centred on `anchorX` (screen pixels). */
  galleryToggle: (anchorX: number) => invoke<void>("gallery_toggle", { anchorX: Math.round(anchorX) }),
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
  taskView: () => invoke<void>("shell_task_view"),
  dockGeometry: () => invoke<DockGeometry | null>("dock_geometry"),
  glassGet: () => invoke<GlassFrame | null>("glass_get"),
  panelToggle: (kind: WidgetKind, anchorX: number) =>
    invoke<void>("panel_toggle", { kind, anchorX: Math.round(anchorX) }),
  panelClose: () => invoke<void>("panel_close"),
  widgetPeek: (kind: WidgetKind, anchorX: number) =>
    invoke<void>("widget_peek", { kind, anchorX: Math.round(anchorX) }),
  widgetUnpeek: (delayMs: number) => invoke<void>("widget_unpeek", { delayMs: Math.round(delayMs) }),
  panelHover: (hovered: boolean) => invoke<void>("panel_hover", { hovered }),
  panelPin: () => invoke<void>("panel_pin"),
  panelFit: (height: number) => invoke<void>("panel_fit", { height: Math.ceil(height) }),
  trayPress: (button: TrayButton) => invoke<void>("tray_press", { button }),
  inputLanguage: () => invoke<InputLanguage>("input_language"),
  systemLook: () => invoke<SystemLook>("system_look"),
  systemClock: () => invoke<SystemClock>("system_clock"),
  tooltipShow: (text: string, anchorX: number) =>
    invoke<void>("tooltip_show", { text, anchorX: Math.round(anchorX) }),
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
  // Productivity widgets.
  notesGet: () => invoke<Note[]>("notes_get"),
  /** Save a note's text (a new note when `id` is null); resolves to its id. */
  noteSave: (id: string | null, text: string) => invoke<string>("note_save", { id, text }),
  noteDelete: (id: string) => invoke<void>("note_delete", { id }),
  todosGet: () => invoke<Todo[]>("todos_get"),
  todoChange: (change: TodoChange) => invoke<Todo[]>("todo_change", { change }),
  captureSave: (kind: CaptureKind, text: string) => invoke<void>("capture_save", { kind, text }),
  kanbanGet: () => invoke<KanbanBoard>("kanban_get"),
  kanbanSet: (board: KanbanBoard) => invoke<KanbanBoard>("kanban_set", { board }),
  timelogGet: () => invoke<TimeLog>("timelog_get"),
  timelogSet: (log: TimeLog) => invoke<TimeLog>("timelog_set", { log }),
  linksGet: () => invoke<QuickLinks>("links_get"),
  linksSet: (links: QuickLinks) => invoke<QuickLinks>("links_set", { links }),
  foldersGet: () => invoke<AppFolders>("folders_get"),
  foldersSet: (folders: AppFolders) => invoke<AppFolders>("folders_set", { folders }),
  /** Open a web address or a folder/file on this PC. */
  linkOpen: (target: string) => invoke<void>("link_open", { target }),
  folderAppLaunch: (path: string) => invoke<void>("folder_app_launch", { path }),
  startMenuApps: () => invoke<FolderApp[]>("start_menu_apps"),
  bookmarksGet: () => invoke<Bookmark[]>("bookmarks_get"),
  fileSearch: (query: string) => invoke<FileHit[]>("file_search", { query }),
  fileOpen: (path: string, reveal: boolean) => invoke<void>("file_open", { path, reveal }),
  songsGet: () => invoke<SongsToday>("songs_get"),
  claudeSessionsGet: () => invoke<ClaudeActivity>("claude_sessions_get"),
  snipStart: (mode: SnipMode) => invoke<void>("snip_start", { mode }),
  snipBackdrop: () => invoke<SnipBackdrop | null>("snip_backdrop"),
  /** The chosen area, as fractions (0 to 1) of the picker window. */
  snipFinish: (x: number, y: number, width: number, height: number) =>
    invoke<void>("snip_finish", { x, y, width, height }),
  snipCancel: () => invoke<void>("snip_cancel"),
  /** Pin the clipboard's picture; false when it holds none. */
  pinClipboard: () => invoke<boolean>("pin_clipboard"),
  pinImage: (id: number) => invoke<string | null>("pin_image", { id }),
  pinClose: (id: number) => invoke<void>("pin_close", { id }),
  pinsCloseAll: () => invoke<void>("pins_close_all"),
  pinsCount: () => invoke<number>("pins_count"),
  screenTextLast: () => invoke<ScreenText>("screen_text_last"),
  textCopy: (text: string) => invoke<void>("text_copy", { text }),
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
  onPanelPinned: on<null>("panel-pinned"),
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
  onNotes: on<Note[]>("notes-changed"),
  onTodos: on<Todo[]>("todos-changed"),
  onKanban: on<KanbanBoard>("kanban-changed"),
  onTimelog: on<TimeLog>("timelog-changed"),
  onLinks: on<QuickLinks>("links-changed"),
  onFolders: on<AppFolders>("folders-changed"),
  onSongs: on<SongsToday>("songs-changed"),
  onClaudeActivity: on<ClaudeActivity>("claude-activity"),
  onPins: on<number>("pins-changed"),
  onScreenText: on<ScreenText>("screen-text"),
  onSnipOpen: on<SnipMode>("snip-open"),
  onAppLaunching: on<string>("app-launching"),
  onPreviewOpen: on<PreviewContent>("preview-open"),
  onPreviewClosed: on<null>("preview-closed"),
  onSwitcherState: on<SwitcherState>("switcher-state"),
};
