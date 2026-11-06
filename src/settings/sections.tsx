import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";

import type { DockSize } from "../core/generated/DockSize";
import type { DocksOn } from "../core/generated/DocksOn";
import type { DockStyle } from "../core/generated/DockStyle";
import type { Edge } from "../core/generated/Edge";
import type { ScreenInfo } from "../core/generated/ScreenInfo";
import type { Settings } from "../core/generated/Settings";
import type { TaskbarMode } from "../core/generated/TaskbarMode";
import { ipc } from "../core/ipc";
import { Choice, Row, ShortcutField, StyleCards, Toggle } from "./controls";

type Save = (patch: Partial<Settings>) => void;

export function AppearanceSection({ settings, save }: { settings: Settings; save: Save }) {
  const [screens, setScreens] = useState<ScreenInfo[]>([]);
  useEffect(() => {
    ipc
      .screensList()
      .then(setScreens)
      .catch((error) => console.error("failed to list screens", error));
  }, []);
  return (
    <section>
      <h2>Appearance</h2>
      <div data-setting="theme">
        <StyleCards value={settings.theme} onChange={(theme) => save({ theme })} />
      </div>
      <Row label="Position" id="edge">
        <Choice<Edge>
          label="Position"
          value={settings.edge}
          options={[
            ["bottom", "Bottom"],
            ["top", "Top"],
          ]}
          onChange={(edge) => save({ edge })}
        />
      </Row>
      <Row label="Shape" id="style" hint="A floating pill, or edge to edge like the taskbar.">
        <Choice<DockStyle>
          label="Shape"
          value={settings.style}
          options={[
            ["pill", "Pill"],
            ["full", "Full width"],
          ]}
          onChange={(style) => save({ style })}
        />
      </Row>
      <Row label="Size" id="size">
        <Choice<DockSize>
          label="Size"
          value={settings.size}
          options={[
            ["small", "Small"],
            ["medium", "Medium"],
            ["large", "Large"],
          ]}
          onChange={(size) => save({ size })}
        />
      </Row>
      {screens.length > 1 && (
        <Row
          label="Docks on"
          id="docksOn"
          hint="One dock, or one on every screen showing that screen's windows."
        >
          <Choice<DocksOn>
            label="Docks on"
            value={settings.docksOn}
            options={[
              ["one", "This screen"],
              ["every", "Every screen"],
            ]}
            onChange={(docksOn) => save({ docksOn })}
          />
        </Row>
      )}
      {screens.length > 1 && (
        <Row
          id="screen"
          label={settings.docksOn === "every" ? "Main dock" : "Screen"}
          hint={
            settings.docksOn === "every"
              ? "The screen whose dock has your pinned apps and widgets."
              : "Which screen the dock lives on."
          }
        >
          <select
            value={settings.screen}
            onChange={(e) => save({ screen: e.target.value })}
            aria-label={settings.docksOn === "every" ? "Main dock" : "Screen"}
          >
            {screens.map((s) => (
              <option key={s.name} value={s.primary ? "" : s.name}>
                {s.label}
                {s.primary ? " (main)" : ""}
              </option>
            ))}
          </select>
        </Row>
      )}
    </section>
  );
}

export function TaskbarSection({ settings, save }: { settings: Settings; save: Save }) {
  return (
    <section>
      <h2>Taskbar & startup</h2>
      <Row
        id="taskbarMode"
        label="Windows taskbar"
        hint={
          settings.taskbarMode === "hidden"
            ? "Only Dock shows. The Windows taskbar never peeks out at the screen edge."
            : "Auto-hides, and peeks out when the mouse reaches the screen edge."
        }
      >
        <Choice<TaskbarMode>
          label="Windows taskbar"
          value={settings.taskbarMode}
          options={[
            ["hidden", "Dock only"],
            ["autoHide", "Peek at edge"],
          ]}
          onChange={(taskbarMode) => save({ taskbarMode })}
        />
      </Row>
      <Row
        label="Auto-hide the dock"
        id="autoHide"
        hint="Slides away; touch the screen edge to bring it back."
      >
        <Toggle
          label="Auto-hide the dock"
          checked={settings.autoHide}
          onChange={(autoHide) => save({ autoHide })}
        />
      </Row>
      <Row label="Show or hide the dock" hint="A keyboard shortcut that works anywhere.">
        <ShortcutField value={settings.toggleShortcut} />
      </Row>
      <Row label="Launch at sign-in" id="launchAtStartup">
        <Toggle
          label="Launch at sign-in"
          checked={settings.launchAtStartup}
          onChange={(launchAtStartup) => save({ launchAtStartup })}
        />
      </Row>
      <Row label="Quit Dock" id="quit" hint="Closes the dock and brings back the Windows taskbar.">
        <button
          className="settings-quit"
          onClick={() => ipc.appQuit().catch((error) => console.error("failed to quit", error))}
        >
          Quit Dock
        </button>
      </Row>
    </section>
  );
}

export function AboutSection() {
  const [version, setVersion] = useState("");
  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);
  return (
    <section className="about">
      <h2>About</h2>
      <div className="about-head">
        <span className="about-logo" />
        <span>
          <strong>Dock</strong> {version && <span className="settings-hint">Version {version}</span>}
        </span>
      </div>
      <p>
        A smart dock for Windows: your apps, live widgets and a few helpful automations, in place of the
        taskbar.
      </p>
      <h3>Your data stays on this PC</h3>
      <p className="settings-hint">
        There's no account. Settings live in your user folder; calendar links are kept in Windows Credential
        Manager; Claude usage is read from Claude Code's files on this PC. The only things that go online are
        the weather (Open-Meteo, with your position rounded to about a kilometre) and the calendars you add.
      </p>
      <h3>If the taskbar ever stays hidden</h3>
      <p className="settings-hint">
        Dock puts the Windows taskbar back whenever it quits, and again the next time it starts if it ever
        crashed. You can also turn auto-hide off yourself in Windows Settings → Personalization → Taskbar.
      </p>
    </section>
  );
}
