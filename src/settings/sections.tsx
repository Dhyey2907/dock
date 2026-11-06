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
import type { DockLook } from "../core/generated/DockLook";
import type { PopupStyle } from "../core/generated/PopupStyle";
import { Choice, Row, ShortcutField, Slider, StyleCards, Toggle } from "./controls";

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
      <StyleCards value={settings.theme} onChange={(theme) => save({ theme })} />
      <Row label="Position">
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
      <Row
        label="Shape"
        hint="A floating pill, edge to edge like the taskbar, or split: apps left, widgets right."
      >
        <Choice<DockStyle>
          label="Shape"
          value={settings.style}
          options={[
            ["pill", "Pill"],
            ["full", "Full width"],
            ["split", "Split"],
          ]}
          onChange={(style) => save({ style })}
        />
      </Row>
      {settings.style !== "full" && (
        <Row label="Shrink to fit" hint="Make the dock smaller when it has more than fits across the screen.">
          <Toggle
            label="Shrink to fit"
            checked={settings.autoShrink}
            onChange={(autoShrink) => save({ autoShrink })}
          />
        </Row>
      )}
      <Row label="Size">
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
      <LookRows settings={settings} save={save} />
      {screens.length > 1 && (
        <Row label="Docks on" hint="One dock, or one on every screen showing that screen's windows.">
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

/** Fine control over the dock's size and look. */
function LookRows({ settings, save }: { settings: Settings; save: Save }) {
  const look = settings.look;
  const set = (patch: Partial<DockLook>) => save({ look: { ...look, ...patch } });
  return (
    <>
      <Row label="Dock size" hint="Make the dock any size. Bigger docks show more on each widget.">
        <Slider
          label="Dock size"
          value={look.scale}
          min={70}
          max={200}
          step={5}
          onCommit={(scale) => set({ scale })}
        />
      </Row>
      {settings.style === "pill" && (
        <Row label="Corners" hint="100% is a full pill.">
          <Slider
            label="Corners"
            value={look.corners}
            min={0}
            max={100}
            step={5}
            onCommit={(corners) => set({ corners })}
          />
        </Row>
      )}
      <Row label="Transparency" hint="Let the desktop show through the dock.">
        <Slider
          label="Transparency"
          value={look.transparency}
          min={0}
          max={70}
          step={5}
          onCommit={(transparency) => set({ transparency })}
        />
      </Row>
      <Row label="Blur" hint="How frosted the glass is.">
        <Slider
          label="Blur"
          value={look.blur}
          min={0}
          max={300}
          step={10}
          onCommit={(blur) => set({ blur })}
        />
      </Row>
      <Row label="Widget background" hint="A soft chip behind each widget.">
        <Slider
          label="Widget background"
          value={look.widgetBackground}
          min={0}
          max={100}
          step={5}
          onCommit={(widgetBackground) => set({ widgetBackground })}
        />
      </Row>
      <Row label="Bounciness" hint="How springy clicks and animations feel.">
        <Slider
          label="Bounciness"
          value={look.bounce}
          min={0}
          max={100}
          step={5}
          onCommit={(bounce) => set({ bounce })}
        />
      </Row>
      <Row label="Magnify on hover" hint="App icons grow as the mouse passes over them.">
        <Toggle label="Magnify on hover" checked={look.magnify} onChange={(magnify) => set({ magnify })} />
      </Row>
      <Row label="Startup animation" hint="The dock rises into place when it starts.">
        <Toggle
          label="Startup animation"
          checked={look.startupAnimation}
          onChange={(startupAnimation) => set({ startupAnimation })}
        />
      </Row>
      <Row label="Popup style">
        <Choice<PopupStyle>
          label="Popup style"
          value={look.popupStyle}
          options={[
            ["glass", "Glass"],
            ["solid", "Solid"],
            ["compact", "Compact"],
          ]}
          onChange={(popupStyle) => set({ popupStyle })}
        />
      </Row>
    </>
  );
}

export function TaskbarSection({ settings, save }: { settings: Settings; save: Save }) {
  return (
    <section>
      <h2>Taskbar & startup</h2>
      <Row
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
      <Row label="Auto-hide the dock" hint="Slides away; touch the screen edge to bring it back.">
        <Toggle
          label="Auto-hide the dock"
          checked={settings.autoHide}
          onChange={(autoHide) => save({ autoHide })}
        />
      </Row>
      <Row label="Show or hide the dock" hint="A keyboard shortcut that works anywhere.">
        <ShortcutField value={settings.toggleShortcut} />
      </Row>
      <Row label="Launch at sign-in">
        <Toggle
          label="Launch at sign-in"
          checked={settings.launchAtStartup}
          onChange={(launchAtStartup) => save({ launchAtStartup })}
        />
      </Row>
      <Row label="Quit Dock" hint="Closes the dock and brings back the Windows taskbar.">
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
