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
    // Listen first so a change made by another window while loading isn't missed.
    await events.onSettingsChanged((settings) => set({ settings }));
    set({ settings: await ipc.settingsGet() });
  },
  update: async (patch) => {
    const current = get().settings;
    if (!current) return;
    set({ settings: await ipc.settingsSet({ ...current, ...patch }) });
  },
}));
