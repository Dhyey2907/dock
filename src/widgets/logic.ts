import type { FocusState } from "../core/generated/FocusState";
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
