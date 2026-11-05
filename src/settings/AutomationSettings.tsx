import type { Automations } from "../core/generated/Automations";
import type { Settings } from "../core/generated/Settings";

type Save = (patch: Partial<Settings>) => void;

const AUTOMATIONS: { key: keyof Automations; label: string; hint: string }[] = [
  {
    key: "meetingMode",
    label: "Meeting mode",
    hint: "Five minutes before a calendar event: a reminder, and your music pauses.",
  },
  {
    key: "aiLimitWarnings",
    label: "Claude limit warnings",
    hint: "A heads-up near your usual 5-hour limit, if your pace will run out early, and when a limit is reached.",
  },
  {
    key: "focusMusic",
    label: "Focus music",
    hint: "Starting the focus timer plays your music; the end of a focus stretch pauses it.",
  },
  {
    key: "sunsetDarkMode",
    label: "Sunset dark mode",
    hint: "Windows switches to dark at sunset and back to light at sunrise (from the weather's sun times). Changes a Windows setting.",
  },
];

/** One switch per automation, and the quiet hours that hold reminders. */
export function AutomationSettings({ settings, save }: { settings: Settings; save: Save }) {
  const quiet = settings.quietHours;
  return (
    <section>
      <h2>Automations</h2>
      <ul className="widget-list">
        {AUTOMATIONS.map(({ key, label, hint }) => (
          <li key={key} className="widget-item">
            <label className="widget-toggle">
              <input
                type="checkbox"
                checked={settings.automations[key]}
                onChange={(e) => save({ automations: { ...settings.automations, [key]: e.target.checked } })}
              />
              <span>
                {label}
                <span className="settings-hint">{hint}</span>
              </span>
            </label>
          </li>
        ))}
      </ul>
      <div className="settings-row">
        <label className="widget-toggle">
          <input
            type="checkbox"
            checked={quiet.enabled}
            onChange={(e) => save({ quietHours: { ...quiet, enabled: e.target.checked } })}
          />
          <span>
            Quiet hours
            <span className="settings-hint">No reminder cards between these times.</span>
          </span>
        </label>
        <span className="quiet-times">
          <input
            className="settings-input quiet-time"
            type="time"
            aria-label="Quiet from"
            value={quiet.start}
            disabled={!quiet.enabled}
            onChange={(e) => e.target.value && save({ quietHours: { ...quiet, start: e.target.value } })}
          />
          –
          <input
            className="settings-input quiet-time"
            type="time"
            aria-label="Quiet until"
            value={quiet.end}
            disabled={!quiet.enabled}
            onChange={(e) => e.target.value && save({ quietHours: { ...quiet, end: e.target.value } })}
          />
        </span>
      </div>
    </section>
  );
}
