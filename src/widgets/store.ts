import { create } from "zustand";

import { events, ipc } from "../core/ipc";
import type { CalendarState } from "../core/generated/CalendarState";
import type { ClaudeUsage } from "../core/generated/ClaudeUsage";
import type { WidgetSnapshot } from "../core/generated/WidgetSnapshot";

interface WidgetsState extends WidgetSnapshot {
  calendar?: CalendarState;
  claude?: ClaudeUsage;
  loaded: boolean;
  load: () => Promise<void>;
}

type Live = WidgetSnapshot & { calendar?: CalendarState; claude?: ClaudeUsage };

/** Live widget data, shared by the dock's tiles and the popup. */
export const useWidgets = create<WidgetsState>((set) => ({
  focus: { phase: "idle", phaseSeconds: 25 * 60, completed: 0 },
  loaded: false,
  load: async () => {
    // Listen first so a change while loading isn't missed; events win over the snapshot.
    const seen = new Set<string>();
    const mark = <K extends keyof Live>(key: K, value: Live[K]) => {
      seen.add(key);
      set({ [key]: value } as Pick<Live, K>);
    };
    await Promise.all([
      events.onWeather((weather) => mark("weather", weather)),
      events.onStats((stats) => mark("stats", stats)),
      events.onSound((sound) => mark("sound", sound)),
      events.onNetwork((network) => mark("network", network)),
      events.onFocus((focus) => mark("focus", focus)),
      events.onMusic((music) => mark("music", music ?? undefined)),
      events.onCalendar((calendar) => mark("calendar", calendar)),
      events.onClaude((claude) => mark("claude", claude)),
    ]);
    const [snapshot, calendar, claude] = await Promise.all([
      ipc.widgetsSnapshot(),
      ipc.calendarGet(),
      ipc.claudeUsage(),
    ]);
    const fresh = Object.fromEntries(
      Object.entries({ ...snapshot, calendar, claude }).filter(([key]) => !seen.has(key)),
    );
    set({ ...fresh, loaded: true });
  },
}));
