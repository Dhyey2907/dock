import { describe, expect, it } from "vitest";

import type { FocusState } from "../core/generated/FocusState";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import {
  focusSecondsLeft,
  formatCountdown,
  formatTemperature,
  monthGrid,
  reorderVisible,
  slotAt,
} from "./logic";

const list: WidgetEntry[] = [
  { kind: "music", enabled: true },
  { kind: "focus", enabled: false },
  { kind: "weather", enabled: true },
  { kind: "clock", enabled: true },
];
const shown = (w: WidgetEntry) => w.enabled;

describe("reorderVisible", () => {
  it("moves a visible widget and leaves hidden ones in their slots", () => {
    const next = reorderVisible(list, shown, 2, 0);
    expect(next.map((w) => w.kind)).toEqual(["clock", "focus", "music", "weather"]);
    expect(next[1]).toEqual({ kind: "focus", enabled: false });
  });

  it("ignores out-of-range or no-op moves", () => {
    expect(reorderVisible(list, shown, 1, 1)).toBe(list);
    expect(reorderVisible(list, shown, 0, 9)).toBe(list);
  });
});

describe("slotAt", () => {
  it("finds the slot under the pointer", () => {
    const mids = [20, 60, 100];
    expect(slotAt(5, mids)).toBe(0);
    expect(slotAt(61, mids)).toBe(2);
    expect(slotAt(500, mids)).toBe(2);
  });
});

describe("focus countdown", () => {
  const base: FocusState = { phase: "focus", phaseSeconds: 1500, completed: 0 };
  it("counts down to the end time, and holds while paused or idle", () => {
    const now = Date.parse("2026-11-05T10:00:00Z");
    expect(focusSecondsLeft({ ...base, endsAt: "2026-11-05T10:10:00Z" }, now)).toBe(600);
    expect(focusSecondsLeft({ ...base, endsAt: "2026-11-05T09:00:00Z" }, now)).toBe(0);
    expect(focusSecondsLeft({ ...base, pausedSeconds: 42 }, now)).toBe(42);
    expect(focusSecondsLeft({ ...base, phase: "idle" }, now)).toBe(1500);
  });

  it("formats minutes and seconds", () => {
    expect(formatCountdown(1500)).toBe("25:00");
    expect(formatCountdown(61)).toBe("1:01");
    expect(formatCountdown(-5)).toBe("0:00");
  });
});

describe("formatTemperature", () => {
  it("adds F only for Fahrenheit", () => {
    expect(formatTemperature(18, "celsius")).toBe("18°");
    expect(formatTemperature(64, "fahrenheit")).toBe("64°F");
    expect(formatTemperature(undefined, "celsius")).toBe("—");
  });
});

describe("monthGrid", () => {
  it("fills whole weeks from the chosen first day", () => {
    // November 2026 starts on a Sunday.
    const monday = monthGrid(2026, 10, 1);
    expect(monday.length % 7).toBe(0);
    expect(monday[0].getDay()).toBe(1);
    expect(monday[6].getDate()).toBe(1); // Sunday the 1st ends the first week
    const sunday = monthGrid(2026, 10, 0);
    expect(sunday[0].getDate()).toBe(1);
    expect(sunday.length).toBe(35);
  });
});
