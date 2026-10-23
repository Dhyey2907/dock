import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { Settings } from "./generated/Settings";

/** Typed wrappers for every backend command, so call sites never spell command names. */
export const ipc = {
  settingsGet: () => invoke<Settings>("settings_get"),
  settingsSet: (settings: Settings) => invoke<Settings>("settings_set", { settings }),
  dockSetContentWidth: (width: number) => invoke<void>("dock_set_content_width", { width }),
  openSettings: () => invoke<void>("open_settings"),
};

export const events = {
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
    listen<Settings>("settings-changed", (event) => handler(event.payload)),
};
