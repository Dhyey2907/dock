import { describe, expect, it } from "vitest";

import type { CalendarEvent } from "../core/generated/CalendarEvent";
import type { FocusState } from "../core/generated/FocusState";
import type { MediaState } from "../core/generated/MediaState";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import {
  agenda,
  calendarColor,
  focusSecondsLeft,
  formatDuration,
  formatCountdown,
  formatTemperature,
  formatTokens,
  modelName,
  monthGrid,
  nextEvent,
  playbackPosition,
  reorderVisible,
  slotAt,
  startsIn,
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

describe("playbackPosition", () => {
  const music: MediaState = {
    sourceApp: "Spotify",
    title: "Song",
    artist: "Artist",
    album: "",
    status: "playing",
    positionMs: 60_000,
    durationMs: 200_000,
    positionUpdatedAt: "2026-11-05T10:00:00Z",
    rate: 1,
    capabilities: { playPause: true, next: true, previous: true, seek: true },
  };
  const at = (iso: string) => Date.parse(iso);

  it("counts on from the last report while playing", () => {
    expect(playbackPosition(music, at("2026-11-05T10:00:10Z"))).toBe(70_000);
  });

  it("holds still while paused, and never passes the end", () => {
    expect(playbackPosition({ ...music, status: "paused" }, at("2026-11-05T10:05:00Z"))).toBe(60_000);
    expect(playbackPosition(music, at("2026-11-05T11:00:00Z"))).toBe(200_000);
  });

  it("has no position for sources without a timeline", () => {
    expect(playbackPosition({ ...music, positionMs: undefined }, at("2026-11-05T10:00:10Z"))).toBeUndefined();
  });

  it("formats durations", () => {
    expect(formatDuration(187_000)).toBe("3:07");
    expect(formatDuration(3_765_000)).toBe("1:02:45");
  });
});

describe("calendar helpers", () => {
  const now = new Date(2026, 10, 5, 9, 54).getTime(); // Thu 5 Nov, 9:54 local
  const local = (d: number, h: number, m = 0) => new Date(2026, 10, d, h, m).toISOString();
  const event = (title: string, start: string, end: string, allDay = false): CalendarEvent => ({
    id: title,
    calendarId: "work",
    title,
    start,
    end,
    allDay,
  });
  const events = [
    event("Holiday", local(5, 0), local(6, 0), true),
    event("Early", local(5, 8), local(5, 9)),
    event("Standup", local(5, 10), local(5, 10, 15)),
    event("Review", local(5, 14), local(5, 15)),
    event("Gym", local(7, 7), local(7, 8)),
  ];

  it("finds the next timed event, skipping finished and all-day ones", () => {
    expect(nextEvent(events, now)?.title).toBe("Standup");
    expect(nextEvent(events, new Date(2026, 10, 5, 10, 5).getTime())?.title).toBe("Standup");
    expect(nextEvent(events, new Date(2026, 10, 9).getTime())).toBeUndefined();
  });

  it("says how soon an event starts", () => {
    expect(startsIn(events[2], now)).toBe("in 6 min");
    expect(startsIn(events[3], now)).toBe("in 4 h 6 min");
    expect(startsIn(events[2], new Date(2026, 10, 5, 10, 5).getTime())).toBe("now");
    expect(startsIn(events[4], new Date(2026, 10, 5, 6).getTime())).toBeNull();
  });

  it("groups the coming days, dropping what's over today", () => {
    const groups = agenda(events, now, 7);
    expect(groups.map((g) => g.day.getDate())).toEqual([5, 7]);
    expect(groups[0].events.map((e) => e.title)).toEqual(["Holiday", "Standup", "Review"]);
    expect(groups[1].events.map((e) => e.title)).toEqual(["Gym"]);
  });

  it("gives each calendar a steady colour", () => {
    expect(calendarColor("work")).toBe(calendarColor("work"));
    expect(calendarColor("work")).not.toBe(calendarColor("home"));
  });
});

describe("Claude formatting", () => {
  it("shortens token counts", () => {
    expect(formatTokens(999)).toBe("999");
    expect(formatTokens(12_400)).toBe("12K");
    expect(formatTokens(1_240_000)).toBe("1.2M");
    expect(formatTokens(23_600_000)).toBe("24M");
  });

  it("names models", () => {
    expect(modelName("claude-opus-5-5")).toBe("Opus 5.5");
    expect(modelName("claude-sonnet-5-5-20260601")).toBe("Sonnet 5.5");
    expect(modelName("gpt-something")).toBe("gpt-something");
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
