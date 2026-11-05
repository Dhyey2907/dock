import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { DockApp } from "./generated/DockApp";
import type { Settings } from "./generated/Settings";

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
};

export const events = {
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>("settings-changed", (event) => handler(event.payload)),
  onDockRevealed: (handler: (revealed: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("dock-revealed", (event) => handler(event.payload)),
  onAppsChanged: (handler: (apps: DockApp[]) => void): Promise<UnlistenFn> =>
    listen<DockApp[]>("apps-changed", (event) => handler(event.payload)),
};
