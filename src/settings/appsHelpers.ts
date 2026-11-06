import type { EmptyAction } from "../core/generated/EmptyAction";

export const EMPTY_ACTIONS: [EmptyAction, string][] = [
  ["nothing", "Nothing"],
  ["showDesktop", "Show desktop"],
  ["minimizeOthers", "Minimize all but the current window"],
  ["quickMenu", "Open the dock menu"],
  ["taskManager", "Task Manager"],
  ["settings", "Dock settings"],
  ["start", "Start menu"],
  ["search", "Search"],
];

/** A short random id for a new list entry. */
export function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);
}
