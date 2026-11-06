import { describe, expect, it } from "vitest";

import type { CalendarEvent } from "../../core/generated/CalendarEvent";
import type { KanbanBoard } from "../../core/generated/KanbanBoard";
import type { TimeLog } from "../../core/generated/TimeLog";
import {
  ago,
  formatClock,
  formatSpan,
  hostOf,
  isToday,
  matchAll,
  meetingFor,
  moveCard,
  noteTitle,
  runningEntry,
  slotFor,
  sortTodos,
  todosLeft,
  toggleTimer,
  weekStart,
  weekTotals,
} from "./logic";

const board: KanbanBoard = {
  cards: [
    { id: "a", text: "A", column: "todo" },
    { id: "b", text: "B", column: "todo" },
    { id: "c", text: "C", column: "doing" },
  ],
};
const ids = (b: KanbanBoard, column: string) => b.cards.filter((c) => c.column === column).map((c) => c.id);

describe("kanban", () => {
  it("moves a card to another column at a position", () => {
    const moved = moveCard(board, "a", "doing", 0);
    expect(ids(moved, "doing")).toEqual(["a", "c"]);
    expect(ids(moved, "todo")).toEqual(["b"]);
  });
  it("moves a card to the end, or into an empty column", () => {
    expect(ids(moveCard(board, "a", "doing", 9), "doing")).toEqual(["c", "a"]);
    expect(ids(moveCard(board, "b", "done", 0), "done")).toEqual(["b"]);
  });
  it("reorders within a column", () => {
    expect(ids(moveCard(board, "b", "todo", 0), "todo")).toEqual(["b", "a"]);
    expect(ids(moveCard(board, "a", "todo", 1), "todo")).toEqual(["b", "a"]);
  });
  it("ignores unknown cards", () => {
    expect(moveCard(board, "zz", "done", 0)).toBe(board);
  });
  it("finds the slot under the pointer", () => {
    expect(slotFor(5, [10, 30])).toBe(0);
    expect(slotFor(20, [10, 30])).toBe(1);
    expect(slotFor(40, [10, 30])).toBe(2);
  });
});

describe("time tracker", () => {
  const empty: TimeLog = {
    projects: [
      { id: "p", name: "Work" },
      { id: "q", name: "Study" },
    ],
    entries: [],
  };
  it("starts, switches and stops timers", () => {
    const t0 = new Date(2026, 10, 6, 9, 0);
    const started = toggleTimer(empty, "p", t0);
    expect(runningEntry(started)?.projectId).toBe("p");
    const switched = toggleTimer(started, "q", new Date(2026, 10, 6, 10, 0));
    expect(runningEntry(switched)?.projectId).toBe("q");
    expect(switched.entries[0].end).toBe(new Date(2026, 10, 6, 10, 0).toISOString());
    const stopped = toggleTimer(switched, "q", new Date(2026, 10, 6, 10, 30));
    expect(runningEntry(stopped)).toBeUndefined();
    const totals = weekTotals(stopped, new Date(2026, 10, 6, 12, 0));
    expect(totals.get("p")).toBe(3600);
    expect(totals.get("q")).toBe(1800);
  });
  it("counts only this week, and running timers up to now", () => {
    const log: TimeLog = {
      ...empty,
      entries: [
        // Sunday 23:00 to Monday 01:00: one hour falls in the week.
        {
          id: "1",
          projectId: "p",
          start: new Date(2026, 10, 1, 23, 0).toISOString(),
          end: new Date(2026, 10, 2, 1, 0).toISOString(),
        },
        { id: "2", projectId: "q", start: new Date(2026, 10, 6, 11, 0).toISOString() },
      ],
    };
    const totals = weekTotals(log, new Date(2026, 10, 6, 11, 15));
    expect(totals.get("p")).toBe(3600);
    expect(totals.get("q")).toBe(900);
  });
  it("weeks start on Monday", () => {
    expect(weekStart(new Date(2026, 10, 6, 15)).getDay()).toBe(1);
    expect(weekStart(new Date(2026, 10, 8, 15)).getDate()).toBe(2); // Sunday → the Monday before
  });
  it("formats spans", () => {
    expect(formatSpan(45)).toBe("45s");
    expect(formatSpan(720)).toBe("12m");
    expect(formatSpan(3900)).toBe("1h 05m");
    expect(formatClock(309)).toBe("5:09");
    expect(formatClock(3909)).toBe("1:05:09");
  });
});

describe("meeting notes", () => {
  const event = (id: string, start: number, end: number, allDay = false): CalendarEvent => ({
    id,
    calendarId: "c",
    title: id,
    start: new Date(2026, 10, 6, start).toISOString(),
    end: new Date(2026, 10, 6, end).toISOString(),
    allDay,
  });
  const events = [event("all", 0, 23, true), event("later", 15, 16), event("now", 10, 11)];
  it("picks the meeting going on now, else the next one", () => {
    expect(meetingFor(events, new Date(2026, 10, 6, 10, 30))?.id).toBe("now");
    expect(meetingFor(events, new Date(2026, 10, 6, 12))?.id).toBe("later");
    expect(meetingFor(events, new Date(2026, 10, 6, 17))).toBeUndefined();
  });
});

describe("lists", () => {
  it("counts and orders to-dos", () => {
    const todos = [
      { id: "1", text: "a", done: true, created: "2026-01-01T00:00:00Z" },
      { id: "2", text: "b", done: false, created: "2026-01-02T00:00:00Z" },
    ];
    expect(todosLeft(todos)).toBe(1);
    expect(sortTodos(todos).map((t) => t.id)).toEqual(["2", "1"]);
  });
  it("titles notes by their first line", () => {
    expect(noteTitle({ id: "1", text: "\n  Shopping \nmilk", updated: "" })).toBe("Shopping");
    expect(noteTitle({ id: "1", text: " ", updated: "" })).toBe("Empty note");
  });
  it("matches every word", () => {
    const items = ["GitHub code", "Gmail", "Google code search"];
    expect(matchAll(items, "code g", (s) => [s])).toEqual(["GitHub code", "Google code search"]);
    expect(matchAll(items, "  ", (s) => [s])).toEqual(items);
  });
  it("shows hosts and times", () => {
    expect(hostOf("https://www.github.com/x")).toBe("github.com");
    expect(hostOf("C:\\Users")).toBe("C:\\Users");
    const now = new Date(2026, 10, 6, 12);
    expect(isToday("2026-11-06", now)).toBe(true);
    expect(isToday("2026-11-05", now)).toBe(false);
    expect(isToday(null, now)).toBe(false);
    expect(ago(new Date(2026, 10, 6, 11, 55).toISOString(), now)).toBe("5 min ago");
  });
});
