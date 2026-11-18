/**
 * A stand-in for `src/core/ipc` so the widgets draw in a plain browser. `?state=` picks what the
 * "backend" does: `full` (sample data, the default), `empty` (nothing to show yet) or `fail`
 * (every read fails), so every card can be checked in all three.
 */
import { sample } from "./sampleData";

export type WindowsSettingsPage = string;

const state = new URLSearchParams(window.location.search).get("state") ?? "full";

function answer(name: string): Promise<unknown> {
  if (state === "fail" && !name.startsWith("settings")) {
    return Promise.reject(new Error(`${name} failed (gallery)`));
  }
  if (state === "empty" && !name.startsWith("settings")) {
    const value = sample[name];
    if (Array.isArray(value)) return Promise.resolve([]);
    return Promise.resolve(null);
  }
  return Promise.resolve(name in sample ? structuredClone(sample[name]) : null);
}

export const ipc = new Proxy({} as Record<string, (...args: unknown[]) => Promise<unknown>>, {
  get: (_t, name: string) => () => answer(name),
}) as never;

const noop = () => Promise.resolve(() => {});
export const events = new Proxy({}, { get: () => noop }) as never;

export const MAIN_DOCK_LABEL = "dock";
export const isMainDock = () => true;

export const announceServicesChanged = () => Promise.resolve();
