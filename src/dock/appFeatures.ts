import { create } from "zustand";

import type { DockApp } from "../core/generated/DockApp";
import { events } from "../core/ipc";

/** How long an icon bounces at most while its app starts. */
export const LAUNCH_TIMEOUT_MS = 12_000;
/** Hovering an app this long shows its window previews (less when previews already show). */
export const PREVIEW_DELAY_MS = 450;
export const PREVIEW_SWITCH_DELAY_MS = 80;
/** A file dragged over an app's icon this long brings its window forward. */
export const RAISE_DELAY_MS = 600;
/** Moving a pressed icon this far (CSS pixels) starts dragging it off the dock. */
export const DRAG_THRESHOLD = 14;
/** A press on an empty part of the dock held this long is a press-and-hold. */
export const HOLD_DELAY_MS = 480;

interface Launch {
  since: number;
  /** How many windows the app had when it started. */
  windows: number;
}

/** Still starting: a new window hasn't appeared yet, and it hasn't been too long. */
export function stillLaunching(launch: Launch | undefined, windows: number, now: number): boolean {
  return !!launch && now - launch.since < LAUNCH_TIMEOUT_MS && windows <= launch.windows;
}

interface LaunchingState {
  launches: Record<string, Launch>;
  start: (key: string, windows: number, now?: number) => void;
  /** Forget launches whose window appeared (or that took too long). */
  settle: (apps: DockApp[], now?: number) => void;
}

export const useLaunching = create<LaunchingState>((set, get) => ({
  launches: {},
  start: (key, windows, now = Date.now()) => {
    set({ launches: { ...get().launches, [key]: { since: now, windows } } });
    // Stop bouncing after a while even if no window ever appears.
    setTimeout(() => {
      const { [key]: launch, ...rest } = get().launches;
      if (launch && launch.since === now) set({ launches: rest });
    }, LAUNCH_TIMEOUT_MS);
  },
  settle: (apps, now = Date.now()) => {
    const current = get().launches;
    const kept: Record<string, Launch> = {};
    for (const [key, launch] of Object.entries(current)) {
      const app = apps.find((a) => a.key === key);
      if (stillLaunching(launch, app?.windows.length ?? 0, now)) kept[key] = launch;
    }
    if (Object.keys(kept).length !== Object.keys(current).length) set({ launches: kept });
  },
}));

/** Whether window previews are showing (moving to the next app then switches at once). */
export const usePreviewOpen = create<{ open: boolean; set: (open: boolean) => void }>((set) => ({
  open: false,
  set: (open) => set({ open }),
}));

/** Listen for launches and previews (the dock page calls this once). */
export async function initAppFeatures(apps: () => DockApp[]) {
  await events.onAppLaunching((key) => {
    const app = apps().find((a) => a.key === key);
    if (app) useLaunching.getState().start(key, app.windows.length);
  });
  await events.onPreviewOpen(() => usePreviewOpen.getState().set(true));
  await events.onPreviewClosed(() => usePreviewOpen.getState().set(false));
}

/**
 * Something hovered for a while: `over(key)` (re)starts the wait for `key`, `over(null)` or
 * `clear()` stops it; `fire(key)` runs once the same key has been hovered for `delay`.
 */
export function hoverTimer(delay: number, fire: (key: string) => void) {
  let current: string | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    clearTimeout(timer);
    timer = undefined;
    current = null;
  };
  return {
    over(key: string | null) {
      if (key === current) return;
      clear();
      if (!key) return;
      current = key;
      timer = setTimeout(() => {
        timer = undefined;
        fire(key);
      }, delay);
    },
    clear,
  };
}

/** Whether a press moved far enough to be a drag. */
export function movedPast(start: { x: number; y: number }, now: { x: number; y: number }, threshold: number) {
  return Math.hypot(now.x - start.x, now.y - start.y) >= threshold;
}

/** A point in the dock window's CSS pixels made a screen x (for popups' anchors). */
export function screenX(clientX: number, geometry: { x: number; scale: number } | null, fallback: number) {
  return geometry ? geometry.x + clientX * geometry.scale : fallback;
}

/** The app whose button is under a point in the page, by its `data-app-key`. */
export function appKeyAt(x: number, y: number): string | null {
  const element = document.elementFromPoint(x, y);
  return element?.closest<HTMLElement>("[data-app-key]")?.dataset.appKey ?? null;
}

/** Every app button's place in the window, for the minimize animation. */
export function buttonRects() {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-app-key]")).map((el) => {
    const r = el.getBoundingClientRect();
    return { key: el.dataset.appKey ?? "", x: r.left, y: r.top, width: r.width, height: r.height };
  });
}

/** Whether a pointer event landed on the dock's empty background (a drag region). */
export function isEmptySpot(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.hasAttribute("data-tauri-drag-region") && !target.closest("button");
}
