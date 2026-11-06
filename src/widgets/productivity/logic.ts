import type { CalendarEvent } from "../../core/generated/CalendarEvent";
import type { KanbanBoard } from "../../core/generated/KanbanBoard";
import type { KanbanColumn } from "../../core/generated/KanbanColumn";
import type { Note } from "../../core/generated/Note";
import type { TimeEntry } from "../../core/generated/TimeEntry";
import type { TimeLog } from "../../core/generated/TimeLog";
import type { Todo } from "../../core/generated/Todo";

/** A new id for things made on the page (cards, projects, links…). */
export function newId(now = Date.now()): string {
  return `${now.toString(36)}${Math.floor(Math.random() * 1e6).toString(36)}`;
}

// ---------- Notes ----------

/** Meeting notes are ordinary notes with this id prefix, so the command bar finds them too. */
export const MEETING_PREFIX = "meeting-";

export function noteTitle(note: Note): string {
  const line = note.text.split("\n").find((l) => l.trim() !== "");
  return line?.trim().slice(0, 60) || "Empty note";
}

/** Newest first. */
export function sortNotes(notes: Note[]): Note[] {
  return [...notes].sort((a, b) => b.updated.localeCompare(a.updated));
}

// ---------- To-dos ----------

export function todosLeft(todos: Todo[]): number {
  return todos.filter((t) => !t.done).length;
}

/** Open ones first (oldest first), then done ones. */
export function sortTodos(todos: Todo[]): Todo[] {
  return [...todos].sort((a, b) => Number(a.done) - Number(b.done) || a.created.localeCompare(b.created));
}

// ---------- Kanban ----------

export const COLUMNS: { id: KanbanColumn; label: string }[] = [
  { id: "todo", label: "To do" },
  { id: "doing", label: "Doing" },
  { id: "done", label: "Done" },
];

/**
 * Move card `id` into `column`, before the card at `index` of that column's cards (counted
 * without the moved card); past the end puts it last.
 */
export function moveCard(board: KanbanBoard, id: string, column: KanbanColumn, index: number): KanbanBoard {
  const card = board.cards.find((c) => c.id === id);
  if (!card) return board;
  const rest = board.cards.filter((c) => c.id !== id);
  const inColumn = rest.filter((c) => c.column === column);
  const moved = { ...card, column };
  const before = inColumn[Math.max(0, index)];
  if (!before) {
    // After the column's last card (or at the end, for an empty column).
    const last = inColumn[inColumn.length - 1];
    const at = last ? rest.indexOf(last) + 1 : rest.length;
    return { cards: [...rest.slice(0, at), moved, ...rest.slice(at)] };
  }
  const at = rest.indexOf(before);
  return { cards: [...rest.slice(0, at), moved, ...rest.slice(at)] };
}

/** Which slot a pointer at `y` falls in, given the midpoints of a column's cards. */
export function slotFor(y: number, midpoints: number[]): number {
  const slot = midpoints.findIndex((mid) => y < mid);
  return slot === -1 ? midpoints.length : slot;
}

// ---------- Time tracker ----------

export function runningEntry(log: TimeLog): TimeEntry | undefined {
  return log.entries.find((e) => e.end == null);
}

/** Start timing `projectId`, stopping whatever runs. Starting the running project stops it. */
export function toggleTimer(log: TimeLog, projectId: string, now: Date): TimeLog {
  const running = runningEntry(log);
  const stamp = now.toISOString();
  const entries = log.entries.map((e) => (e.end == null ? { ...e, end: stamp } : e));
  if (running?.projectId === projectId) return { ...log, entries };
  return { ...log, entries: [...entries, { id: newId(now.getTime()), projectId, start: stamp }] };
}

/** Monday 00:00 local of the week `now` is in. */
export function weekStart(now: Date): Date {
  const start = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const sinceMonday = (start.getDay() + 6) % 7;
  start.setDate(start.getDate() - sinceMonday);
  return start;
}

/** Seconds per project this week (running timers count up to `now`). */
export function weekTotals(log: TimeLog, now: Date): Map<string, number> {
  const from = weekStart(now).getTime();
  const to = now.getTime();
  const totals = new Map<string, number>();
  for (const e of log.entries) {
    const start = Math.max(Date.parse(e.start), from);
    const end = Math.min(e.end ? Date.parse(e.end) : to, to);
    if (end > start) totals.set(e.projectId, (totals.get(e.projectId) ?? 0) + (end - start) / 1000);
  }
  return totals;
}

/** "1h 05m", "12m", "45s". */
export function formatSpan(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m`;
  return `${s}s`;
}

/** "1:05:09" or "5:09", for a running timer. */
export function formatClock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const mm = String(Math.floor((s % 3600) / 60)).padStart(h > 0 ? 2 : 1, "0");
  const ss = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
}

// ---------- Meeting notes ----------

/** The event going on now, else the next one today or later. All-day events don't count. */
export function meetingFor(events: CalendarEvent[], now: Date): CalendarEvent | undefined {
  const t = now.getTime();
  const timed = events
    .filter((e) => !e.allDay && Date.parse(e.end) > t)
    .sort((a, b) => Date.parse(a.start) - Date.parse(b.start));
  return timed.find((e) => Date.parse(e.start) <= t) ?? timed[0];
}

export function meetingNoteId(event: CalendarEvent): string {
  return `${MEETING_PREFIX}${event.id}`;
}

/** The heading a new meeting note starts with. */
export function meetingHeading(event: CalendarEvent): string {
  const when = new Date(event.start).toLocaleString(undefined, {
    weekday: "short",
    day: "numeric",
    month: "short",
    hour: "numeric",
    minute: "2-digit",
  });
  return `Meeting: ${event.title} (${when})\n\n`;
}

// ---------- Search lists ----------

/** Items whose fields contain every typed word, case-insensitively. */
export function matchAll<T>(items: T[], query: string, fields: (item: T) => string[]): T[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return items;
  return items.filter((item) => {
    const text = fields(item).join(" ").toLowerCase();
    return words.every((w) => text.includes(w));
  });
}

/** "github.com" from a web address, for showing under a link's name. */
export function hostOf(target: string): string {
  try {
    const url = new URL(target);
    if (url.protocol !== "http:" && url.protocol !== "https:") return target;
    return url.hostname.replace(/^www\./, "");
  } catch {
    return target;
  }
}

/** Is this the dock's own day? (Songs from an earlier day are yesterday's.) */
export function isToday(day: string | null, now: Date): boolean {
  if (!day) return false;
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return day === `${y}-${m}-${d}`;
}

/** "just now", "5 min ago", "3 h ago", "2 d ago". */
export function ago(iso: string, now: Date): string {
  const s = Math.max(0, (now.getTime() - Date.parse(iso)) / 1000);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  return `${Math.floor(s / 86400)} d ago`;
}

// ---------- Area picker ----------

export interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** Turn a dragged box into the fractions of the window the backend wants. */
export function boxFractions(box: Box, width: number, height: number) {
  const left = Math.min(box.x0, box.x1);
  const top = Math.min(box.y0, box.y1);
  return {
    x: left / width,
    y: top / height,
    width: Math.abs(box.x1 - box.x0) / width,
    height: Math.abs(box.y1 - box.y0) / height,
  };
}
