import { useState, type FormEvent } from "react";
import { Trash2 } from "lucide-react";

import { ipc } from "../core/ipc";
import type { Settings } from "../core/generated/Settings";

function message(error: unknown): string {
  const kind = (error as { kind?: string })?.kind;
  const text = (error as { message?: string })?.message ?? "";
  // Input problems carry a sentence for people; anything else gets a calm general one.
  if (kind === "invalid_input") return text.replace(/^invalid input: /, "");
  return "That calendar couldn't be added. Check the link and your connection.";
}

/** Add calendars by their secret iCal link, and remove them. */
export function CalendarSettings({ settings }: { settings: Settings }) {
  const [name, setName] = useState("");
  const [link, setLink] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    if (!link.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await ipc.calendarAdd(name, link);
      setName("");
      setLink("");
    } catch (err) {
      setError(message(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section>
      <h2>Calendars</h2>
      <p className="settings-hint">
        Paste a calendar's secret iCal link. Google: Settings → your calendar → "Secret address in iCal
        format". Outlook: Settings → Calendar → Shared calendars → Publish → ICS. iCloud: share the calendar
        as a public calendar. Links are kept in Windows Credential Manager and read every 15 minutes.
      </p>
      {settings.calendars.length > 0 && (
        <ul className="widget-list">
          {settings.calendars.map((c) => (
            <li key={c.id} className="widget-item">
              <span className="widget-toggle">{c.name}</span>
              <button
                className="widget-move"
                aria-label={`Remove ${c.name}`}
                title="Remove"
                onClick={() => ipc.calendarRemove(c.id).catch((err) => setError(message(err)))}
              >
                <Trash2 size={15} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <form className="calendar-form" onSubmit={add}>
        <input
          className="settings-input"
          type="text"
          placeholder="Name (e.g. Work)"
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
        />
        <input
          className="settings-input calendar-link"
          type="url"
          placeholder="https://… or webcal://…"
          value={link}
          onChange={(e) => setLink(e.target.value)}
          aria-label="Calendar link"
        />
        <button className="settings-quit" type="submit" disabled={busy || !link.trim()}>
          {busy ? "Checking…" : "Add"}
        </button>
      </form>
      {error && (
        <p className="settings-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}
