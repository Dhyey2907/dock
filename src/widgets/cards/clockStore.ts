import { create } from "zustand";

interface Timer {
  running: boolean;
  /** When it ends (ms since epoch) while running. */
  endsAt: number | null;
  /** Seconds left while paused; null before it starts. */
  pausedLeft: number | null;
  left: (minutes: number) => number;
  toggle: (minutes: number) => void;
  reset: () => void;
}

/** The Timer card's countdown (kept by the dock page, so it runs with the popup closed). */
export const useTimer = create<Timer>((set, get) => ({
  running: false,
  endsAt: null,
  pausedLeft: null,
  left: (minutes) => {
    const { endsAt, pausedLeft } = get();
    if (endsAt) return Math.max(0, Math.ceil((endsAt - Date.now()) / 1000));
    return pausedLeft ?? minutes * 60;
  },
  toggle: (minutes) => {
    const { running, left } = get();
    if (running) {
      set({ running: false, pausedLeft: left(minutes), endsAt: null });
    } else {
      const seconds = left(minutes) || minutes * 60;
      set({ running: true, endsAt: Date.now() + seconds * 1000, pausedLeft: null });
    }
  },
  reset: () => set({ running: false, endsAt: null, pausedLeft: null }),
}));

interface Stopwatch {
  running: boolean;
  /** When the current run started (ms since epoch). */
  startedAt: number | null;
  /** Time from earlier runs, ms. */
  banked: number;
  laps: number[];
  elapsed: () => number;
  toggle: () => void;
  lap: () => void;
  reset: () => void;
}

export const useStopwatch = create<Stopwatch>((set, get) => ({
  running: false,
  startedAt: null,
  banked: 0,
  laps: [],
  elapsed: () => {
    const { startedAt, banked } = get();
    return banked + (startedAt ? Date.now() - startedAt : 0);
  },
  toggle: () => {
    const { running, elapsed } = get();
    if (running) set({ running: false, banked: elapsed(), startedAt: null });
    else set({ running: true, startedAt: Date.now() });
  },
  lap: () => set((s) => ({ laps: [...s.laps, get().elapsed()] })),
  reset: () => set({ running: false, startedAt: null, banked: 0, laps: [] }),
}));
