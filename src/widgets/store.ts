import { create } from "zustand";

import { events, ipc } from "../core/ipc";
import type { WidgetSnapshot } from "../core/generated/WidgetSnapshot";

interface WidgetsState extends WidgetSnapshot {
  loaded: boolean;
  load: () => Promise<void>;
}

/** Live widget data, shared by the dock's tiles and the popup. */
export const useWidgets = create<WidgetsState>((set) => ({
  focus: { phase: "idle", phaseSeconds: 25 * 60, completed: 0 },
  loaded: false,
  load: async () => {
    // Listen first so a change while loading isn't missed; events win over the snapshot.
    const seen = new Set<string>();
    const mark = <K extends keyof WidgetSnapshot>(key: K, value: WidgetSnapshot[K]) => {
      seen.add(key);
      set({ [key]: value } as Pick<WidgetSnapshot, K>);
    };
    await Promise.all([
      events.onWeather((weather) => mark("weather", weather)),
      events.onStats((stats) => mark("stats", stats)),
      events.onSound((sound) => mark("sound", sound)),
      events.onNetwork((network) => mark("network", network)),
      events.onFocus((focus) => mark("focus", focus)),
    ]);
    const snapshot = await ipc.widgetsSnapshot();
    const fresh = Object.fromEntries(Object.entries(snapshot).filter(([key]) => !seen.has(key)));
    set({ ...fresh, loaded: true });
  },
}));
