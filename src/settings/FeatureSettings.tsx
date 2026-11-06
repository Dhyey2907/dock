import { useEffect, useState, type FormEvent } from "react";
import { Play, Trash2 } from "lucide-react";

import type { HotkeyAction } from "../core/generated/HotkeyAction";
import type { Settings } from "../core/generated/Settings";
import type { Workspace } from "../core/generated/Workspace";
import { errorText, featureEvents, featuresIpc, subscribe } from "../core/ipcFeatures";
import { Row, ShortcutField, Toggle } from "./controls";

type Save = (patch: Partial<Settings>) => void;

const HOTKEYS: { action: HotkeyAction; label: string; hint: string; value: (s: Settings) => string }[] = [
  {
    action: "toggle",
    label: "Show or hide the dock",
    hint: "Switches between Dock and the Windows taskbar.",
    value: (s) => s.toggleShortcut,
  },
  {
    action: "commandBar",
    label: "Command bar",
    hint: "Find apps, files, settings, notes and workspaces, or do a quick sum.",
    value: (s) => s.hotkeys.commandBar,
  },
  {
    action: "quickCapture",
    label: "Quick capture",
    hint: "Jot a note from anywhere.",
    value: (s) => s.hotkeys.quickCapture,
  },
];

/** Every global shortcut, each rebindable. */
export function ShortcutsSection({ settings }: { settings: Settings }) {
  return (
    <section>
      <h2>Shortcuts</h2>
      <p className="settings-hint">
        Click a shortcut, then press the new keys. Use Ctrl, Alt, Shift or Win with a letter, number, F-key or
        Space. A shortcut another app already uses can't be set.
      </p>
      {HOTKEYS.map(({ action, label, hint, value }) => (
        <Row key={action} id={action} label={label} hint={hint}>
          <ShortcutField action={action} value={value(settings)} />
        </Row>
      ))}
    </section>
  );
}

const savedAt = (iso: string) =>
  new Date(iso).toLocaleString(navigator.language || "en", { dateStyle: "medium", timeStyle: "short" });

/** Save the open apps under a name, put them back later; and the latest-screenshot switch. */
export function WorkspacesSection({ settings, save }: { settings: Settings; save: Save }) {
  const [list, setList] = useState<Workspace[]>([]);
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    featuresIpc
      .workspacesList()
      .then(setList)
      .catch((e) => console.error("failed to list workspaces", e));
    return subscribe(() => featureEvents.onWorkspacesChanged(setList));
  }, []);

  const add = async (e: FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setBusy(true);
    setError(null);
    try {
      await featuresIpc.workspaceSave(name);
      setName("");
    } catch (err) {
      setError(errorText(err, "The workspace couldn't be saved."));
    } finally {
      setBusy(false);
    }
  };
  const act = (work: Promise<void>) => work.catch((err) => setError(errorText(err)));

  return (
    <section>
      <h2>Workspaces & screenshots</h2>
      <p className="settings-hint">
        A workspace remembers the apps you have open and where their windows are. Restoring one opens what
        isn't running and moves every window back. You can also restore one from the command bar.
      </p>
      <form className="calendar-form" onSubmit={add} data-setting="save">
        <input
          className="settings-input"
          placeholder="Name (e.g. Coding)"
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
          aria-label="Workspace name"
        />
        <button className="settings-quit" type="submit" disabled={busy || !name.trim()}>
          {busy ? "Saving…" : "Save open apps"}
        </button>
      </form>
      {error && (
        <p className="settings-error" role="alert">
          {error}
        </p>
      )}
      {list.length > 0 && (
        <ul className="widget-list">
          {list.map((w) => (
            <li key={w.name} className="widget-item">
              <span className="widget-toggle">
                <span>
                  {w.name}
                  <span className="settings-hint">
                    {w.apps.length} {w.apps.length === 1 ? "window" : "windows"} · saved {savedAt(w.saved)}
                  </span>
                </span>
              </span>
              <button
                className="widget-move"
                aria-label={`Restore ${w.name}`}
                title="Restore"
                onClick={() => act(featuresIpc.workspaceRestore(w.name))}
              >
                <Play size={15} />
              </button>
              <button
                className="widget-move"
                aria-label={`Delete ${w.name}`}
                title="Delete"
                onClick={() => act(featuresIpc.workspaceRemove(w.name))}
              >
                <Trash2 size={15} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <h3>Latest screenshot</h3>
      <Row
        id="screenshot"
        label="Show new screenshots on the dock"
        hint="For two minutes after Win+PrtScn or the Snipping Tool saves one. Drag it into any app as a file, or click to open it."
      >
        <Toggle
          label="Show new screenshots on the dock"
          checked={settings.latestScreenshot}
          onChange={(latestScreenshot) => save({ latestScreenshot })}
        />
      </Row>
    </section>
  );
}
