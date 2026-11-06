import { CalendarClock } from "lucide-react";

import { useWidgets } from "../store";
import { useNow } from "../useNow";
import { meetingFor, meetingHeading, meetingNoteId } from "./logic";
import { AutosaveText } from "./NotesWidget";
import { useProductivity } from "./store";

function useMeeting() {
  const events = useWidgets((s) => s.calendar?.events);
  const now = useNow(60_000);
  return { meeting: events ? meetingFor(events, new Date(now)) : undefined, now };
}

export function MeetingNotesTile() {
  const { meeting } = useMeeting();
  const notes = useProductivity((s) => s.notes);
  const hasNote = meeting ? notes.some((n) => n.id === meetingNoteId(meeting)) : false;
  return (
    <span className="tile-row">
      <CalendarClock size={18} strokeWidth={1.8} aria-hidden="true" />
      {hasNote && <span className="calendar-dot" aria-label="Has notes" />}
    </span>
  );
}

export function MeetingNotesPanel() {
  const { meeting, now } = useMeeting();
  const notes = useProductivity((s) => s.notes);
  const loaded = useProductivity((s) => s.loaded);
  const calendar = useWidgets((s) => s.calendar);

  if (!meeting) {
    return (
      <div className="panel-body panel-empty">
        <CalendarClock size={28} aria-hidden="true" />
        <p>
          {calendar && calendar.calendars.length === 0
            ? "Add a calendar in Settings to take notes for your meetings."
            : "No meeting now or coming up."}
        </p>
      </div>
    );
  }
  const id = meetingNoteId(meeting);
  const note = notes.find((n) => n.id === id);
  const start = new Date(meeting.start);
  const live = start.getTime() <= now;
  return (
    <div className="panel-body notes-panel">
      <div>
        <h2 className="panel-title">{meeting.title}</h2>
        <span className="panel-faint">
          {live ? "Now" : "Next"} ·{" "}
          {start.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}
          {meeting.location ? ` · ${meeting.location}` : ""}
        </span>
      </div>
      {loaded && (
        <AutosaveText
          key={id}
          noteId={id}
          initial={note?.text ?? meetingHeading(meeting)}
          placeholder="Agenda, decisions, follow-ups…"
        />
      )}
    </div>
  );
}
