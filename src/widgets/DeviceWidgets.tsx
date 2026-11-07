import { useMemo, useState } from "react";
import { Fluent } from "../core/Fluent";
import { useWindowsLook } from "../core/system";
import {
  Bluetooth,
  BatteryLow,
  BatteryMedium,
  BatteryFull,
  Copy,
  Eye,
  EyeOff,
  Folder,
  Image,
  Laptop,
  Monitor,
  MonitorOff,
  MonitorSmartphone,
  Settings,
  SkipForward,
  SunMedium,
  type LucideIcon,
} from "lucide-react";

import { ipc } from "../core/ipc";
import type { DisplayMode } from "../core/generated/DisplayMode";
import { useSettings } from "../settings/store";
import { intervalLabel, latestOnly, WALLPAPER_INTERVALS } from "./systemLogic";
import { Note } from "./systemShared";
import { useLoaded } from "./useLoaded";

// ---------- Bluetooth devices ----------

function batteryIcon(level: number): LucideIcon {
  if (level <= 20) return BatteryLow;
  if (level <= 70) return BatteryMedium;
  return BatteryFull;
}

export function BluetoothTile() {
  const windowsLook = useWindowsLook();
  return windowsLook ? (
    <Fluent glyph={0xe702} />
  ) : (
    <Bluetooth size={18} strokeWidth={1.9} aria-hidden="true" />
  );
}

export function BluetoothPanel() {
  const { data, loading } = useLoaded(ipc.bluetoothDevices);
  return (
    <div className="panel-body">
      <h2 className="panel-title">Bluetooth devices</h2>
      {!data && loading && <Note>Looking for your devices…</Note>}
      {data && data.length === 0 && <Note>No paired Bluetooth devices.</Note>}
      <ul className="tool-list">
        {data?.map((device) => {
          const Battery = device.battery != null ? batteryIcon(device.battery) : null;
          return (
            <li key={device.id} className="tool-row" data-off={!device.connected}>
              <span className="tool-text">
                <span className="tool-name">{device.name}</span>
                <span className="panel-faint">{device.connected ? "Connected" : "Not connected"}</span>
              </span>
              {Battery && device.battery != null && (
                <span className="tool-battery" data-low={device.battery <= 20}>
                  <Battery size={16} aria-hidden="true" />
                  {device.battery}%
                </span>
              )}
            </li>
          );
        })}
      </ul>
      <button className="panel-link" onClick={() => ipc.openWindowsSettings("bluetooth")}>
        Bluetooth settings
      </button>
    </div>
  );
}

// ---------- Second-screen brightness ----------

export function BrightnessTile() {
  return <SunMedium size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function BrightnessPanel() {
  const { data, setData, loading } = useLoaded(ipc.monitorsBrightness);
  const send = useMemo(
    () =>
      latestOnly<{ id: string; percent: number }>(({ id, percent }) => ipc.monitorSetBrightness(id, percent)),
    [],
  );
  return (
    <div className="panel-body">
      <h2 className="panel-title">Screen brightness</h2>
      {!data && loading && <Note>Asking your screens…</Note>}
      {data && data.length === 0 && <Note>No screens found.</Note>}
      {data?.map((monitor) => (
        <div key={monitor.id} className="tool-slider">
          <span className="tool-slider-head">
            <span className="tool-name">
              {monitor.name}
              {monitor.primary ? <span className="panel-faint"> · main</span> : null}
            </span>
            {monitor.percent != null && <span className="meter-value">{monitor.percent}%</span>}
          </span>
          {monitor.percent != null ? (
            <input
              className="slider"
              type="range"
              min={0}
              max={100}
              value={monitor.percent}
              aria-label={`${monitor.name} brightness`}
              onChange={(e) => {
                const percent = Number(e.target.value);
                setData(data.map((m) => (m.id === monitor.id ? { ...m, percent } : m)));
                send({ id: monitor.id, percent });
              }}
            />
          ) : (
            <span className="panel-faint">
              Doesn't support brightness control over DDC/CI. Built-in screens are in the Status popup.
            </span>
          )}
        </div>
      ))}
    </div>
  );
}

// ---------- Display mode ----------

const MODES: { mode: DisplayMode; label: string; icon: LucideIcon }[] = [
  { mode: "extend", label: "Extend", icon: MonitorSmartphone },
  { mode: "duplicate", label: "Duplicate", icon: Copy },
  { mode: "secondOnly", label: "Second screen only", icon: Monitor },
  { mode: "pcOnly", label: "PC screen only", icon: Laptop },
];

export function DisplayModeTile() {
  return <MonitorSmartphone size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function DisplayModePanel() {
  const { data, reload } = useLoaded(ipc.displayGet);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const choose = (mode: DisplayMode) => {
    setBusy(true);
    setFailed(false);
    ipc
      .displaySet(mode)
      .catch(() => setFailed(true))
      .finally(() => {
        setBusy(false);
        // Windows takes a moment to settle the new layout.
        window.setTimeout(reload, 1500);
      });
  };
  return (
    <div className="panel-body">
      <h2 className="panel-title">Display mode</h2>
      <div className="tool-choices" role="radiogroup" aria-label="Display mode">
        {MODES.map(({ mode, label, icon: Icon }) => (
          <button
            key={mode}
            role="radio"
            aria-checked={data?.mode === mode}
            className="tool-choice"
            disabled={busy || (data != null && data.screens < 2 && mode !== "pcOnly")}
            onClick={() => choose(mode)}
          >
            <Icon size={18} aria-hidden="true" />
            {label}
          </button>
        ))}
      </div>
      {data && data.screens < 2 && <Note>Only one screen is connected.</Note>}
      {failed && <Note>Windows didn't switch. Try Win+P.</Note>}
      <button className="panel-link" onClick={() => ipc.openWindowsSettings("display")}>
        Display settings
      </button>
    </div>
  );
}

// ---------- Desktop icons ----------

export function DesktopIconsTile() {
  return <Eye size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function DesktopIconsPanel() {
  const { data: shown, setData } = useLoaded(ipc.desktopIconsGet);
  const toggle = () => {
    if (shown == null) return;
    ipc
      .desktopIconsSet(!shown)
      .then(setData)
      .catch((e) => console.error(e));
  };
  const Icon = shown ? EyeOff : Eye;
  return (
    <div className="panel-body panel-empty">
      <h2 className="panel-title">Desktop icons</h2>
      {shown === null ? (
        <Note>The desktop isn't available right now.</Note>
      ) : (
        <button className="panel-button" disabled={shown === undefined} onClick={toggle}>
          <Icon size={16} aria-hidden="true" />
          {shown ? "Hide desktop icons" : "Show desktop icons"}
        </button>
      )}
    </div>
  );
}

// ---------- Screen off ----------

export function ScreenOffTile() {
  return <MonitorOff size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function ScreenOffPanel() {
  return (
    <div className="panel-body panel-empty">
      <h2 className="panel-title">Screen off</h2>
      <button className="panel-button" onClick={() => ipc.screenOff().catch((e) => console.error(e))}>
        <MonitorOff size={16} aria-hidden="true" />
        Turn screens off
      </button>
      <span className="panel-faint">Move the mouse or press a key to wake them.</span>
    </div>
  );
}

// ---------- Wallpaper changer ----------

export function WallpaperTile() {
  return <Image size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function WallpaperPanel() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const folder = settings?.wallpaperFolder ?? "";
  const { data, setData, reload } = useLoaded(ipc.wallpaperGet);
  const [problem, setProblem] = useState<string | null>(null);

  const pick = () => {
    ipc
      .wallpaperPickFolder()
      .then((chosen) => (chosen ? update({ wallpaperFolder: chosen }).then(reload) : undefined))
      .catch((e) => console.error(e));
  };
  const next = () => {
    setProblem(null);
    ipc
      .wallpaperNext()
      .then(setData)
      .catch(() => setProblem("No pictures to change to in that folder."));
  };

  return (
    <div className="panel-body">
      <h2 className="panel-title">Wallpaper</h2>
      <button className="tool-folder" onClick={pick} title={folder || undefined}>
        <Folder size={16} aria-hidden="true" />
        <span className="tool-text">
          <span className="tool-name">{folder ? folder.split(/[\\/]/).pop() : "Choose a folder…"}</span>
          {folder && (
            <span className="panel-faint">
              {data ? `${data.count} picture${data.count === 1 ? "" : "s"}` : ""}
              {data?.current ? ` · now ${data.current}` : ""}
            </span>
          )}
        </span>
      </button>
      <label className="tool-select">
        <span>Change</span>
        <select
          className="tool-input"
          value={settings?.wallpaperMinutes ?? 30}
          onChange={(e) =>
            update({ wallpaperMinutes: Number(e.target.value) }).catch((err) => console.error(err))
          }
        >
          {WALLPAPER_INTERVALS.map((m) => (
            <option key={m} value={m}>
              {intervalLabel(m)}
            </option>
          ))}
        </select>
      </label>
      <button className="panel-button tool-start" disabled={!folder || data?.count === 0} onClick={next}>
        <SkipForward size={16} aria-hidden="true" />
        Next wallpaper
      </button>
      {problem && <Note>{problem}</Note>}
      {!folder && (
        <Note>
          <Settings size={12} aria-hidden="true" /> Pick a folder of pictures to rotate through.
        </Note>
      )}
    </div>
  );
}
