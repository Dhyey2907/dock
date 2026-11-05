import { create } from "zustand";

import { events, ipc } from "../core/ipc";
import type { Settings } from "../core/generated/Settings";

interface SettingsState {
  settings: Settings | null;
  load: () => Promise<void>;
  update: (patch: Partial<Settings>) => Promise<void>;
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: null,
  load: async () => {
    // Listen first so a change made by another window while loading isn't missed; a change
    // that arrives first is newer than the copy being read.
    let changed = false;
    await events.onSettingsChanged((settings) => {
      changed = true;
      set({ settings });
    });
    const settings = await ipc.settingsGet();
    if (!changed) set({ settings });
  },
  update: async (patch) => {
    const current = get().settings;
    if (!current) return;
    set({ settings: await ipc.settingsSet({ ...current, ...patch }) });
  },
}));
