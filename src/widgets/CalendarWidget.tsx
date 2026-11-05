import { CalendarDays, MapPin } from "lucide-react";

import { ipc } from "../core/ipc";
import type { CalendarEvent } from "../core/generated/CalendarEvent";
import { agenda, calendarColor, isOngoing, nextEvent, startsIn } from "./logic";
import { useWidgets } from "./store";
import { useNow } from "./useNow";

const locale = navigator.language || "en";
const time = (iso: string) =>
  new Date(iso).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });

export function CalendarTile() {
  const calendar = useWidgets((s) => s.calendar);
  const now = useNow(30_000);
  const next = calendar ? nextEvent(calendar.events, now) : undefined;
  const soon = next ? startsIn(next, now) : null;
  if (!next || !soon) return <CalendarDays size={18} strokeWidth={1.8} aria-hidden="true" />;
  return (
    <span
      className="tile-row calendar-tile"
      data-now={soon === "now"}
      data-soon={Date.parse(next.start) > now && Date.parse(next.start) - now <= 5 * 60_000}
    >
      <span className="calendar-dot" style={{ background: calendarColor(next.calendarId) }} />
      <span className="tile-stack music-text">
        <span className="tile-strong">{next.title}</span>
        <span className="tile-faint">{soon === "now" ? `until ${time(next.end)}` : soon}</span>
      </span>
    </span>
  );
}

function dayLabel(day: Date, now: number) {
  const today = new Date(now);
  today.setHours(0, 0, 0, 0);
  const diff = Math.round((day.getTime() - today.getTime()) / 86_400_000);
  if (diff === 0) return "Today";
  if (diff === 1) return "Tomorrow";
  return day.toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "short" });
}

function EventRow({ event, now }: { event: CalendarEvent; now: number }) {
  return (
    <li className="agenda-event" data-now={isOngoing(event, now)}>
      <span className="calendar-dot" style={{ background: calendarColor(event.calendarId) }} />
      <span className="agenda-time panel-faint">
        {event.allDay ? "All day" : `${time(event.start)} – ${time(event.end)}`}
      </span>
      <span className="agenda-text">
        <span className="agenda-title">{event.title}</span>
        {event.location && (
          <span className="panel-faint agenda-place">
            <MapPin size={11} aria-hidden="true" /> {event.location}
          </span>
        )}
      </span>
    </li>
  );
}

export function CalendarPanel() {
  const calendar = useWidgets((s) => s.calendar);
  const now = useNow(30_000);
  if (!calendar || calendar.calendars.length === 0) {
    return (
      <div className="panel-body panel-empty">
        <CalendarDays size={28} strokeWidth={1.6} />
        <p>Add a calendar to see what's coming up.</p>
        <button className="panel-button" onClick={() => ipc.openSettings()}>
          Add a calendar
        </button>
        <p className="panel-faint">
          Google, Outlook and iCloud calendars all have a secret link you can paste.
        </p>
      </div>
    );
  }
  const groups = agenda(calendar.events, now, 7);
  const problems = calendar.calendars.filter((c) => c.problem);
  return (
    <div className="panel-body calendar-panel">
      <h2 className="panel-title">
        Coming up {calendar.sample && <span className="panel-faint">(sample calendar)</span>}
      </h2>
      {problems.map((c) => (
        <p key={c.id} className="calendar-problem">
          {c.name} {c.problem}.
        </p>
      ))}
      {groups.length === 0 ? (
        <p className="panel-faint">Nothing in the next week.</p>
      ) : (
        <div className="agenda">
          {groups.map((group) => (
            <section key={group.day.toISOString()}>
              <h3 className="agenda-day">{dayLabel(group.day, now)}</h3>
              <ul>
                {group.events.map((event) => (
                  <EventRow key={event.id} event={event} now={now} />
                ))}
              </ul>
            </section>
          ))}
        </div>
      )}
    </div>
  );
}
