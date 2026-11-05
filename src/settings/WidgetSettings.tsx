import { useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";

import type { Settings } from "../core/generated/Settings";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import { WIDGETS } from "../widgets/registry";

type Save = (patch: Partial<Settings>) => void;

/** Turn widgets on and off, put them in order, and set them up. */
export function WidgetSettings({ settings, save }: { settings: Settings; save: Save }) {
  // Only widgets this version can show are listed; others keep their place in the saved list.
  const listed = settings.widgets.filter((w) => WIDGETS[w.kind]);

  const setWidgets = (next: WidgetEntry[]) => save({ widgets: next });
  const toggle = (entry: WidgetEntry) =>
    setWidgets(settings.widgets.map((w) => (w.kind === entry.kind ? { ...w, enabled: !w.enabled } : w)));
  const move = (entry: WidgetEntry, by: -1 | 1) => {
    const at = listed.findIndex((w) => w.kind === entry.kind);
    const other = listed[at + by];
    if (!other) return;
    const next = [...settings.widgets];
    const a = next.findIndex((w) => w.kind === entry.kind);
    const b = next.findIndex((w) => w.kind === other.kind);
    [next[a], next[b]] = [next[b], next[a]];
    setWidgets(next);
  };

  return (
    <section>
      <h2>Widgets</h2>
      <p className="settings-hint">Shown left to right on the dock. You can also drag them along the dock.</p>
      <ul className="widget-list">
        {listed.map((entry, i) => {
          const def = WIDGETS[entry.kind]!;
          return (
            <li key={entry.kind} className="widget-item">
              <label className="widget-toggle">
                <input type="checkbox" checked={entry.enabled} onChange={() => toggle(entry)} />
                <span>
                  {def.label}
                  <span className="settings-hint">{def.description}</span>
                </span>
              </label>
              <button
                className="widget-move"
                aria-label={`Move ${def.label} left`}
                disabled={i === 0}
                onClick={() => move(entry, -1)}
              >
                <ChevronUp size={16} />
              </button>
              <button
                className="widget-move"
                aria-label={`Move ${def.label} right`}
                disabled={i === listed.length - 1}
                onClick={() => move(entry, 1)}
              >
                <ChevronDown size={16} />
              </button>
            </li>
          );
        })}
      </ul>
      <WeatherPlace value={settings.weatherPlace} onCommit={(weatherPlace) => save({ weatherPlace })} />
      <div className="settings-row">
        <span>Temperature</span>
        <select
          value={settings.temperatureUnit}
          onChange={(e) => save({ temperatureUnit: e.target.value as Settings["temperatureUnit"] })}
        >
          <option value="celsius">°C</option>
          <option value="fahrenheit">°F</option>
        </select>
      </div>
      <div className="settings-row">
        <span>Clock</span>
        <select
          value={settings.clock24h == null ? "system" : settings.clock24h ? "24" : "12"}
          onChange={(e) =>
            save({ clock24h: e.target.value === "system" ? undefined : e.target.value === "24" })
          }
        >
          <option value="system">Like Windows</option>
          <option value="12">12-hour</option>
          <option value="24">24-hour</option>
        </select>
      </div>
    </section>
  );
}

/** A town for the weather; saved when you press Enter or leave the box, not on every key. */
function WeatherPlace({ value, onCommit }: { value: string; onCommit: (value: string) => void }) {
  const [draft, setDraft] = useState(value);
  const commit = () => {
    if (draft.trim() !== value) onCommit(draft.trim());
  };
  return (
    <div className="settings-row">
      <span>
        Weather for
        <span className="settings-hint">Leave empty to use this PC's location.</span>
      </span>
      <input
        className="settings-input"
        type="text"
        placeholder="This PC's location"
        value={draft}
        maxLength={80}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === "Enter" && commit()}
      />
    </div>
  );
}
