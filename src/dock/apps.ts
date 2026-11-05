import { create } from "zustand";

import { events, ipc } from "../core/ipc";
import type { DockApp } from "../core/generated/DockApp";

interface AppsState {
  apps: DockApp[];
  load: () => Promise<void>;
}

export const useApps = create<AppsState>((set) => ({
  apps: [],
  load: async () => {
    // Listen first so a change while loading isn't missed.
    await events.onAppsChanged((apps) => set({ apps }));
    set({ apps: await ipc.appsList() });
  },
}));

/** Icons by source, fetched once per session; a source with no icon resolves to null. */
const icons = new Map<string, Promise<string | null>>();

export function loadIcon(source: string): Promise<string | null> {
  if (!source) return Promise.resolve(null);
  let icon = icons.get(source);
  if (!icon) {
    icon = ipc.appIcon(source).catch((error) => {
      console.error("failed to load app icon", error);
      icons.delete(source); // try again next time
      return null;
    });
    icons.set(source, icon);
  }
  return icon;
}
