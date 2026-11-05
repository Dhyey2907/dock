import type { ReactNode } from "react";

import type { DockSize } from "../core/generated/DockSize";
import type { DockStyle } from "../core/generated/DockStyle";
import type { Edge } from "../core/generated/Edge";
import type { TaskbarMode } from "../core/generated/TaskbarMode";
import type { Theme } from "../core/generated/Theme";
import { ipc } from "../core/ipc";
import { useSettings } from "./store";
import { AutomationSettings } from "./AutomationSettings";
import { CalendarSettings } from "./CalendarSettings";
import { WidgetSettings } from "./WidgetSettings";

function Choice<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
}) {
  return (
    <div className="choice" role="radiogroup">
      {options.map(([option, label]) => (
        <button
          key={option}
          role="radio"
          aria-checked={value === option}
          className={value === option ? "choice-on" : undefined}
          onClick={() => onChange(option)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="settings-row">
      <span>{label}</span>
      {children}
    </div>
  );
}

export function SettingsApp() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  if (!settings) return null;
  const save = (patch: Parameters<typeof update>[0]) =>
    update(patch).catch((error) => console.error("failed to save settings", error));

  return (
    <main className="settings">
      <h1>Dock</h1>
      <section>
        <h2>Appearance</h2>
        <Row label="Position">
          <Choice<Edge>
            value={settings.edge}
            options={[
              ["bottom", "Bottom"],
              ["top", "Top"],
            ]}
            onChange={(edge) => save({ edge })}
          />
        </Row>
        <Row label="Shape">
          <Choice<DockStyle>
            value={settings.style}
            options={[
              ["pill", "Pill"],
              ["full", "Full width"],
            ]}
            onChange={(style) => save({ style })}
          />
        </Row>
        <Row label="Size">
          <Choice<DockSize>
            value={settings.size}
            options={[
              ["small", "S"],
              ["medium", "M"],
              ["large", "L"],
            ]}
            onChange={(size) => save({ size })}
          />
        </Row>
        <Row label="Style">
          <Choice<Theme>
            value={settings.theme}
            options={[
              ["glass", "Glass"],
              ["iosGlass", "iOS Glass"],
              ["midnight", "Midnight"],
              ["white", "White"],
              ["black", "Black"],
              ["clear", "Clear"],
            ]}
            onChange={(theme) => save({ theme })}
          />
        </Row>
      </section>
      <WidgetSettings settings={settings} save={save} />
      <CalendarSettings settings={settings} />
      <AutomationSettings settings={settings} save={save} />
      <section>
        <h2>Taskbar</h2>
        <Row label="Windows taskbar">
          <Choice<TaskbarMode>
            value={settings.taskbarMode}
            options={[
              ["autoHide", "Auto-hide"],
              ["hidden", "Hidden"],
            ]}
            onChange={(taskbarMode) => save({ taskbarMode })}
          />
        </Row>
        <Row label="Auto-hide the dock">
          <input
            type="checkbox"
            checked={settings.autoHide}
            onChange={(e) => save({ autoHide: e.target.checked })}
          />
        </Row>
      </section>
      <section>
        <h2>Startup</h2>
        <Row label="Launch at sign-in">
          <input
            type="checkbox"
            checked={settings.launchAtStartup}
            onChange={(e) => save({ launchAtStartup: e.target.checked })}
          />
        </Row>
        <Row label="Quit the dock and bring back the Windows taskbar">
          <button
            className="settings-quit"
            onClick={() => ipc.appQuit().catch((error) => console.error("failed to quit", error))}
          >
            Quit Dock
          </button>
        </Row>
      </section>
    </main>
  );
}
