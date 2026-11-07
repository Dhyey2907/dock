import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { FolderOpen, Trash2 } from "lucide-react";

import type { AppsSettings } from "../core/generated/AppsSettings";
import type { DockApp } from "../core/generated/DockApp";
import type { EmptyAction } from "../core/generated/EmptyAction";
import type { QuickActionKind } from "../core/generated/QuickActionKind";
import type { Settings } from "../core/generated/Settings";
import { ipc } from "../core/ipc";
import { EMPTY_ACTIONS, newId } from "./appsHelpers";
import { Choice, Row, Toggle } from "./controls";

type Save = (patch: Partial<Settings>) => void;

/** A list of entries, each with a remove button. */
function List<T>({
  items,
  label,
  onRemove,
}: {
  items: T[];
  label: (item: T) => ReactNode;
  onRemove: (index: number) => void;
}) {
  if (items.length === 0) return null;
  return (
    <ul className="widget-list">
      {items.map((item, i) => (
        <li key={i} className="widget-item">
          <span className="widget-toggle">{label(item)}</span>
          <button className="widget-move" aria-label="Remove" title="Remove" onClick={() => onRemove(i)}>
            <Trash2 size={15} />
          </button>
        </li>
      ))}
    </ul>
  );
}

function without<T>(items: T[], index: number): T[] {
  return items.filter((_, i) => i !== index);
}

/** Apps on the dock: pins of your own, hidden apps, icons, menus, clicks and windows. */
export function AppSettings({ settings, save }: { settings: Settings; save: Save }) {
  const apps = settings.apps;
  const set = (patch: Partial<AppsSettings>) => save({ apps: { ...apps, ...patch } });
  const [known, setKnown] = useState<DockApp[]>([]);
  useEffect(() => {
    ipc
      .appsAll()
      .then((list) => setKnown(list.filter((a) => a.kind === "app")))
      .catch((error) => console.error("failed to list apps", error));
  }, []);
  const appName = (key: string) => known.find((a) => a.key === key)?.name ?? key;

  const [siteName, setSiteName] = useState("");
  const [siteUrl, setSiteUrl] = useState("");
  const addSite = (e: FormEvent) => {
    e.preventDefault();
    if (!siteUrl.trim()) return;
    set({
      customPins: [...apps.customPins, { id: newId(), name: siteName, kind: "website", target: siteUrl }],
    });
    setSiteName("");
    setSiteUrl("");
  };
  const addStack = () =>
    ipc
      .pickPath(true)
      .then((path) => {
        if (path)
          set({ customPins: [...apps.customPins, { id: newId(), name: "", kind: "stack", target: path }] });
      })
      .catch((error) => console.error("failed to pick a folder", error));
  const pickPack = () =>
    ipc
      .pickPath(true)
      .then((path) => path && set({ iconPack: path }))
      .catch((error) => console.error("failed to pick a folder", error));

  const [hide, setHide] = useState("");
  const [preset, setPreset] = useState({ key: "", name: "", args: "" });
  const [quick, setQuick] = useState<{ name: string; kind: QuickActionKind; target: string; app: string }>({
    name: "",
    kind: "command",
    target: "",
    app: "",
  });

  const toggle = (label: string, field: keyof AppsSettings, hint?: string) => (
    <Row label={label} hint={hint}>
      <Toggle label={label} checked={apps[field] as boolean} onChange={(on) => set({ [field]: on })} />
    </Row>
  );
  const appPicker = (value: string, onChange: (key: string) => void, empty?: string) => (
    <select
      className="settings-input"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      aria-label="App"
    >
      <option value="">{empty ?? "Choose an app"}</option>
      {known.map((a) => (
        <option key={a.key} value={a.key}>
          {a.name}
        </option>
      ))}
    </select>
  );

  return (
    <section>
      <h2>Apps</h2>

      <h3>Pinned websites and folders</h3>
      <p className="settings-hint">
        Websites open in your browser and show the site's own icon (fetched once from that site). A folder
        opens as a stack of its newest files.
      </p>
      <List
        items={apps.customPins}
        label={(p) => (
          <>
            {p.name || p.target}
            <span className="settings-hint">{p.kind === "website" ? p.target : `Folder · ${p.target}`}</span>
          </>
        )}
        onRemove={(i) => set({ customPins: without(apps.customPins, i) })}
      />
      <form className="calendar-form" onSubmit={addSite}>
        <input
          className="settings-input"
          placeholder="Name (optional)"
          aria-label="Website name"
          value={siteName}
          onChange={(e) => setSiteName(e.target.value)}
        />
        <input
          className="settings-input calendar-link"
          placeholder="example.com"
          aria-label="Website address"
          value={siteUrl}
          onChange={(e) => setSiteUrl(e.target.value)}
        />
        <button className="settings-quit" type="submit">
          Pin website
        </button>
      </form>
      <div className="calendar-form">
        <button className="settings-quit" onClick={addStack}>
          <FolderOpen size={14} /> Pin a folder…
        </button>
      </div>

      <h3>Look</h3>
      <Row
        label="Icon pack"
        hint={apps.iconPack || "A folder of PNGs named after programs or app ids (chrome.png)."}
      >
        <span className="quiet-times">
          <button className="settings-quit" onClick={pickPack}>
            Choose…
          </button>
          {apps.iconPack && (
            <button className="settings-link" onClick={() => set({ iconPack: "" })}>
              Clear
            </button>
          )}
        </span>
      </Row>
      {apps.overrides.length > 0 && (
        <>
          <p className="settings-hint">
            Apps you renamed or gave another icon (right-click an app to change these).
          </p>
          <List
            items={apps.overrides}
            label={(o) => (
              <>
                {o.name || appName(o.key)}
                {o.icon && <span className="settings-hint">{o.icon}</span>}
              </>
            )}
            onRemove={(i) => set({ overrides: without(apps.overrides, i) })}
          />
        </>
      )}
      {toggle("Sort running apps by use", "sortByUse", "Apps you use most come first, after your pins.")}
      {toggle("Bounce while starting", "launchBounce")}
      {toggle("Glow when an app needs you", "attentionFlash")}
      {toggle("Shrink windows into the dock", "minimizeAnimation", "Minimized windows fly to their icon.")}

      <h3>Hidden apps</h3>
      <p className="settings-hint">
        These never show on the dock. Right-click an app → Never show on Dock, or type its program.
      </p>
      <List items={apps.hidden} label={(h) => h} onRemove={(i) => set({ hidden: without(apps.hidden, i) })} />
      <form
        className="calendar-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (hide.trim()) set({ hidden: [...apps.hidden, hide] });
          setHide("");
        }}
      >
        <input
          className="settings-input"
          placeholder="notepad.exe"
          aria-label="Program to hide"
          value={hide}
          onChange={(e) => setHide(e.target.value)}
        />
        <button className="settings-quit" type="submit">
          Hide
        </button>
      </form>

      <h3>Windows</h3>
      {toggle("Window previews", "previews", "Hover an app to see its windows.")}
      {toggle(
        "Win+1 to Win+9",
        "numberShortcuts",
        "Open the apps on the dock in order, like the Windows taskbar.",
      )}
      {toggle(
        "Dock's own Alt+Tab",
        "altTab",
        "Replaces the Windows switcher with one in the dock's style. Off unless you turn it on.",
      )}
      {toggle("Drag an icon to an edge to snap", "edgeSnap", "Halves, quarters, or the top to maximize.")}
      {toggle("Show desktop button", "showDesktopButton")}
      {toggle("Peek at the desktop", "peek", "Hover the show desktop button.")}
      {toggle("Reopen apps on their last screen", "reopenOnLastScreen")}
      {toggle(
        "Rearrange windows when a screen is unplugged",
        "rearrangeOnUnplug",
        "They come back when the screen does.",
      )}

      <h3>Clicks on the empty dock</h3>
      <Row label="Double-click">
        <select
          className="settings-input"
          aria-label="Double-click"
          value={apps.doubleClick}
          onChange={(e) => set({ doubleClick: e.target.value as EmptyAction })}
        >
          {EMPTY_ACTIONS.map(([value, text]) => (
            <option key={value} value={value}>
              {text}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Press and hold">
        <select
          className="settings-input"
          aria-label="Press and hold"
          value={apps.pressAndHold}
          onChange={(e) => set({ pressAndHold: e.target.value as EmptyAction })}
        >
          {EMPTY_ACTIONS.map(([value, text]) => (
            <option key={value} value={value}>
              {text}
            </option>
          ))}
        </select>
      </Row>

      <h3>Open with arguments</h3>
      <p className="settings-hint">
        Extra ways to start an app from its menu, such as a Chrome profile (--profile-directory="Profile 1").
      </p>
      <List
        items={apps.launchPresets}
        label={(p) => (
          <>
            {p.name}
            <span className="settings-hint">
              {appName(p.key)} · {p.args || "no arguments"}
            </span>
          </>
        )}
        onRemove={(i) => set({ launchPresets: without(apps.launchPresets, i) })}
      />
      <form
        className="calendar-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!preset.key || !preset.name.trim()) return;
          set({ launchPresets: [...apps.launchPresets, { id: newId(), ...preset }] });
          setPreset({ key: "", name: "", args: "" });
        }}
      >
        {appPicker(preset.key, (key) => setPreset({ ...preset, key }))}
        <input
          className="settings-input"
          placeholder="Name"
          aria-label="Preset name"
          value={preset.name}
          onChange={(e) => setPreset({ ...preset, name: e.target.value })}
        />
        <input
          className="settings-input"
          placeholder="Arguments"
          aria-label="Arguments"
          value={preset.args}
          onChange={(e) => setPreset({ ...preset, args: e.target.value })}
        />
        <button className="settings-quit" type="submit">
          Add
        </button>
      </form>

      <h3>Your own buttons</h3>
      <p className="settings-hint">
        Run a command or open a file, folder or address, from an app's menu or the dock's own.
      </p>
      <List
        items={apps.quickActions}
        label={(a) => (
          <>
            {a.name}
            <span className="settings-hint">
              {a.app ? appName(a.app) : "Dock menu"} · {a.kind === "command" ? "Runs" : "Opens"} {a.target}
            </span>
          </>
        )}
        onRemove={(i) => set({ quickActions: without(apps.quickActions, i) })}
      />
      <form
        className="calendar-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (!quick.name.trim() || !quick.target.trim()) return;
          set({ quickActions: [...apps.quickActions, { id: newId(), ...quick }] });
          setQuick({ name: "", kind: "command", target: "", app: "" });
        }}
      >
        {appPicker(quick.app, (app) => setQuick({ ...quick, app }), "Dock menu")}
        <input
          className="settings-input"
          placeholder="Name"
          aria-label="Button name"
          value={quick.name}
          onChange={(e) => setQuick({ ...quick, name: e.target.value })}
        />
        <Choice<QuickActionKind>
          label="Kind"
          value={quick.kind}
          options={[
            ["command", "Command"],
            ["open", "Open"],
          ]}
          onChange={(kind) => setQuick({ ...quick, kind })}
        />
        <input
          className="settings-input"
          placeholder={quick.kind === "command" ? "ipconfig /flushdns" : "C:\\Projects"}
          aria-label="Command or path"
          value={quick.target}
          onChange={(e) => setQuick({ ...quick, target: e.target.value })}
        />
        <button className="settings-quit" type="submit">
          Add
        </button>
      </form>
    </section>
  );
}
