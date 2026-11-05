import type { CalendarEvent } from "../core/generated/CalendarEvent";
import type { FocusState } from "../core/generated/FocusState";
import type { MediaState } from "../core/generated/MediaState";
import type { TemperatureUnit } from "../core/generated/TemperatureUnit";
import type { WidgetEntry } from "../core/generated/WidgetEntry";

/**
 * Move the `from`-th visible widget to visible position `to`. Hidden widgets keep their slots in
 * the saved list, so turning one back on puts it where it was.
 */
export function reorderVisible(
  widgets: WidgetEntry[],
  visible: (w: WidgetEntry) => boolean,
  from: number,
  to: number,
) {
  const slots = widgets.flatMap((w, i) => (visible(w) ? [i] : []));
  if (from === to || from < 0 || to < 0 || from >= slots.length || to >= slots.length) return widgets;
  const shown = slots.map((i) => widgets[i]);
  const [moved] = shown.splice(from, 1);
  shown.splice(to, 0, moved);
  const next = [...widgets];
  slots.forEach((slot, i) => (next[slot] = shown[i]));
  return next;
}

/** Which visible slot a pointer at `x` is over, given the slots' midpoints left to right. */
export function slotAt(x: number, midpoints: number[]): number {
  const index = midpoints.findIndex((mid) => x < mid);
  return index === -1 ? midpoints.length - 1 : index;
}

/** Seconds left in the focus timer's current phase. */
export function focusSecondsLeft(focus: FocusState, now: number): number {
  if (focus.pausedSeconds != null) return focus.pausedSeconds;
  if (focus.endsAt) return Math.max(0, Math.ceil((Date.parse(focus.endsAt) - now) / 1000));
  return focus.phaseSeconds;
}

/** Where playback is now: the last reported position, counted on while playing. */
export function playbackPosition(music: MediaState, now: number): number | undefined {
  if (music.positionMs == null) return undefined;
  const elapsed = music.status === "playing" ? (now - Date.parse(music.positionUpdatedAt)) * music.rate : 0;
  const position = music.positionMs + Math.max(0, elapsed);
  return music.durationMs != null ? Math.min(position, music.durationMs) : position;
}

/** The event happening now, or else the next one to start (all-day events don't count). */
export function nextEvent(events: CalendarEvent[], now: number): CalendarEvent | undefined {
  return events.find((e) => !e.allDay && Date.parse(e.end) > now);
}

/** Whether an event is under way at `now`. */
export function isOngoing(event: CalendarEvent, now: number): boolean {
  return Date.parse(event.start) <= now && now < Date.parse(event.end);
}

/** "now", "in 6 min", "in 2 h 5 min", or `null` when it's further off than a day. */
export function startsIn(event: CalendarEvent, now: number): string | null {
  if (isOngoing(event, now)) return "now";
  const minutes = Math.ceil((Date.parse(event.start) - now) / 60_000);
  if (minutes > 24 * 60) return null;
  if (minutes < 60) return `in ${minutes} min`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `in ${h} h ${m} min` : `in ${h} h`;
}

/** Events from today through `days` days ahead, grouped by local day (days with none left out). */
export function agenda(events: CalendarEvent[], now: number, days: number) {
  const startOfToday = new Date(now);
  startOfToday.setHours(0, 0, 0, 0);
  const groups: { day: Date; events: CalendarEvent[] }[] = [];
  for (let i = 0; i < days; i++) {
    const day = new Date(startOfToday.getFullYear(), startOfToday.getMonth(), startOfToday.getDate() + i);
    const next = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
    const inDay = events.filter((e) => {
      const start = Date.parse(e.start);
      const end = Date.parse(e.end);
      // On this day, and (today) not already over.
      return start < next.getTime() && end > day.getTime() && (i > 0 || e.allDay || end > now);
    });
    if (inDay.length) groups.push({ day, events: inDay });
  }
  return groups;
}

/** "999", "12K", "1.2M". */
export function formatTokens(tokens: number): string {
  if (tokens < 1000) return String(tokens);
  if (tokens < 1_000_000) return `${Math.round(tokens / 1000)}K`;
  const millions = tokens / 1_000_000;
  return `${millions < 10 ? millions.toFixed(1) : Math.round(millions)}M`;
}

/** "Opus 5.5" from "claude-opus-5-5"; other names as they are. */
export function modelName(model: string): string {
  const m = /^claude-([a-z]+)-(\d+)-(\d+)/.exec(model);
  if (!m) return model;
  return `${m[1][0].toUpperCase()}${m[1].slice(1)} ${m[2]}.${m[3]}`;
}

/** A steady colour per calendar, from its id. */
export function calendarColor(id: string): string {
  let hash = 0;
  for (const c of id) hash = (hash * 31 + c.charCodeAt(0)) >>> 0;
  return `hsl(${hash % 360} 70% 62%)`;
}

/** "3:07" or "1:02:45". */
export function formatDuration(ms: number): string {
  const total = Math.floor(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

export function formatCountdown(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function formatTemperature(value: number | undefined, unit: TemperatureUnit): string {
  if (value == null) return "—";
  return `${value}°${unit === "fahrenheit" ? "F" : ""}`;
}

/** The days to show in a month view: whole weeks, starting on `firstDay` (0 = Sunday). */
export function monthGrid(year: number, month: number, firstDay: number): Date[] {
  const first = new Date(year, month, 1);
  const offset = (first.getDay() - firstDay + 7) % 7;
  const start = new Date(year, month, 1 - offset);
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const weeks = Math.ceil((offset + daysInMonth) / 7);
  return Array.from(
    { length: weeks * 7 },
    (_, i) => new Date(start.getFullYear(), start.getMonth(), start.getDate() + i),
  );
}

/** The first day of the week for the user's locale (0 = Sunday), Monday if unknown. */
export function localeFirstDay(locale: string): number {
  try {
    const info =
      (
        new Intl.Locale(locale) as Intl.Locale & {
          getWeekInfo?: () => { firstDay: number };
          weekInfo?: { firstDay: number };
        }
      ).getWeekInfo?.() ?? (new Intl.Locale(locale) as { weekInfo?: { firstDay: number } }).weekInfo;
    // Intl numbers days 1–7 from Monday.
    return info ? info.firstDay % 7 : 1;
  } catch {
    return 1;
  }
}
