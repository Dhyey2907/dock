import { useEffect, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Bluetooth, ChevronUp, Headphones, Moon, Plane, Sun, Wifi } from "lucide-react";

import { ipc } from "../../core/ipc";
import type { QuickState } from "../../core/generated/QuickState";
import type { ThemeChoice } from "../../core/generated/ThemeChoice";
import { useInputLanguage } from "../../core/system";
import { useWidgets } from "../store";
import { useLoaded } from "../useLoaded";
import { Card, CardButton, Meter, Ring, Spark } from "./parts";
import { rate } from "./format";

const load = (percent: number) => (percent >= 85 ? "#ff453a" : percent >= 60 ? "#ff9f0a" : "#0a84ff");

/** System Stats: processor and memory as two live rings. */
/** A load ring; "—" until the first reading arrives (never a made-up 0). */
function LoadRing({ label, value }: { label: string; value?: number }) {
  const shown = value == null ? undefined : Math.round(value);
  return (
    <Ring value={shown ?? 0} size={50} color={load(shown ?? 0)}>
      <span className="ring-number">{shown ?? "—"}</span>
      <span className="ring-unit">{label}</span>
    </Ring>
  );
}

export function SystemStatsCard() {
  const stats = useWidgets((s) => s.stats);
  return (
    <Card>
      <LoadRing label="CPU" value={stats?.cpu} />
      <LoadRing label="RAM" value={stats?.ram} />
    </Card>
  );
}

/** CPU: processor load as a ring. */
export function CpuCard() {
  const cpu = useWidgets((s) => s.stats?.cpu);
  return (
    <Card>
      <LoadRing label="CPU" value={cpu} />
    </Card>
  );
}

/** RAM: memory in use as a ring. */
export function RamCard() {
  const ram = useWidgets((s) => s.stats?.ram);
  return (
    <Card>
      <LoadRing label="RAM" value={ram} />
    </Card>
  );
}

const readTemperature = () => ipc.cpuTemperature();

/** Temperature: the PC's heat as a gauge, with processor and memory load beside it. */
export function TemperatureCard() {
  const { data } = useLoaded(readTemperature, 5_000);
  const stats = useWidgets((s) => s.stats);
  const temp = data ?? null;
  const color = temp == null ? "#8e8e93" : temp >= 85 ? "#ff453a" : temp >= 70 ? "#ff9f0a" : "#30d158";
  const cpu = stats?.cpu == null ? undefined : Math.round(stats.cpu);
  const ram = stats?.ram == null ? undefined : Math.round(stats.ram);
  return (
    <Card>
      <Ring value={temp == null ? 0 : Math.min(100, temp)} size={50} color={color} gap={90}>
        <span className="ring-number">{temp == null ? "—" : `${Math.round(temp)}°`}</span>
        <span className="ring-unit">CPU</span>
      </Ring>
      <span className="card-col card-limits">
        <span className="card-limit">
          <span className="card-label card-dim">CPU</span>
          <Meter value={cpu ?? 0} color="#0a84ff" width={44} />
          <span className="card-value">{cpu == null ? "—" : `${cpu}%`}</span>
        </span>
        <span className="card-limit">
          <span className="card-label card-dim">RAM</span>
          <Meter value={ram ?? 0} color="#0a84ff" width={44} />
          <span className="card-value">{ram == null ? "—" : `${ram}%`}</span>
        </span>
      </span>
    </Card>
  );
}

/** Network: download and upload speed with a live line. */
export function NetSpeedCard() {
  const [history, setHistory] = useState<{ down: number[]; up: number[] }>({ down: [], up: [] });
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    const read = () =>
      ipc
        .netSpeed()
        .then((s) => {
          if (!alive.current) return;
          setHistory((h) => ({ down: [...h.down, s.down].slice(-30), up: [...h.up, s.up].slice(-30) }));
        })
        .catch(() => {});
    read();
    const timer = window.setInterval(read, 1000);
    return () => {
      alive.current = false;
      window.clearInterval(timer);
    };
  }, []);
  const down = history.down.at(-1) ?? 0;
  const up = history.up.at(-1) ?? 0;
  return (
    <Card className="card-net">
      <span className="card-col card-grow">
        <span className="card-row card-between">
          <span className="card-row">
            <ArrowDown className="card-sky" />
            <span className="card-label card-tabular">{rate(down)}</span>
          </span>
          <span className="card-row">
            <span className="card-label card-tabular">{rate(up)}</span>
            <ArrowUp className="card-violet" />
          </span>
        </span>
        <span className="card-overlay">
          <Spark points={history.down} width={150} height={18} color="#64d2ff" />
          <Spark points={history.up} width={150} height={18} color="#bf5af2" />
        </span>
      </span>
    </Card>
  );
}

/** Battery: charge as a filling gauge. */
export function BatteryCard() {
  const stats = useWidgets((s) => s.stats);
  const level = stats?.battery;
  const color = level == null ? "#8e8e93" : level <= 15 ? "#ff453a" : level <= 30 ? "#ff9f0a" : "#30d158";
  return (
    <Card>
      <span className="card-battery" data-charging={stats?.charging}>
        <span style={{ width: `${level ?? 0}%`, background: color }} />
      </span>
      <span className="card-col">
        <span className="card-big card-tabular">{level == null ? "—" : `${level}%`}</span>
        <span className="card-faint">
          {level == null ? "No battery" : stats?.charging ? "Charging" : "On battery"}
        </span>
      </span>
    </Card>
  );
}

const readQuick = () => ipc.quickGet();

/** Quick Toggles: Wi-Fi, Bluetooth, and airplane mode (both off). */
export function QuickTogglesCard() {
  const { data, setData } = useLoaded(readQuick, 15_000);
  const set = (change: Partial<QuickState>) => setData((q) => (q ? { ...q, ...change } : q));
  const flip = (kind: "wifi" | "bluetooth") => {
    const on = !(data?.[kind] ?? false);
    set({ [kind]: on });
    ipc.quickSetRadio(kind, on).then(setData).catch(console.error);
  };
  const airplane = data != null && data.wifi === false && data.bluetooth === false;
  const toggleAirplane = () => {
    const on = !airplane;
    set({ wifi: !on, bluetooth: !on });
    Promise.all([ipc.quickSetRadio("wifi", !on), ipc.quickSetRadio("bluetooth", !on)])
      .then(([, last]) => setData(last))
      .catch(console.error);
  };
  return (
    <Card>
      <CardButton label="Wi-Fi" className="card-toggle" active={data?.wifi} onPress={() => flip("wifi")}>
        <Wifi />
      </CardButton>
      <CardButton
        label="Bluetooth"
        className="card-toggle"
        active={data?.bluetooth}
        onPress={() => flip("bluetooth")}
      >
        <Bluetooth />
      </CardButton>
      <CardButton label="Airplane mode" className="card-toggle" active={airplane} onPress={toggleAirplane}>
        <Plane />
      </CardButton>
    </Card>
  );
}

const readTheme = () => ipc.windowsTheme();

/** Theme Toggle: Windows light or dark. */
export function ThemeToggleCard() {
  const { data, setData } = useLoaded(readTheme, 30_000);
  const dark = data !== "light";
  const flip = () => {
    if (data == null) return;
    const next: ThemeChoice = dark ? "light" : "dark";
    setData(next);
    // Put the switch back if Windows didn't take it.
    ipc.windowsThemeSet(next).catch((error) => {
      console.error(error);
      setData(data);
    });
  };
  return (
    <Card>
      <CardButton
        label={dark ? "Switch to light" : "Switch to dark"}
        className="card-switch"
        active={dark}
        onPress={flip}
      >
        <span className="card-switch-knob">{dark ? <Moon /> : <Sun />}</span>
      </CardButton>
      <span className="card-label">{data == null ? "Theme" : dark ? "Dark" : "Light"}</span>
    </Card>
  );
}

/** Audio Output: volume for the speakers or headphones in use. */
export function AudioOutputCard() {
  const sound = useWidgets((s) => s.sound);
  const level = Math.round((sound?.muted ? 0 : (sound?.level ?? 0)) * 100);
  return (
    <Card>
      <CardButton
        label={sound?.muted ? "Unmute" : "Mute"}
        className="card-round card-quiet"
        onPress={() => ipc.volumeToggleMute().catch(console.error)}
      >
        <Headphones />
      </CardButton>
      <span className="card-col card-grow">
        <span className="card-label card-line">{sound?.device ?? "Speakers"}</span>
        <span className="card-row">
          <Meter value={level} width={80} />
          <span className="card-value">{level}</span>
        </span>
      </span>
    </Card>
  );
}

/** Language: the keyboard layout (click to switch, through Windows). */
export function LanguageCard() {
  const language = useInputLanguage();
  return (
    <Card className="card-square">
      <span className="card-big">{language?.language || "ENG"}</span>
      <span className="card-faint">{language?.region ?? ""}</span>
    </Card>
  );
}

/** Tray: the taskbar's hidden icons, in Windows' own flyout. */
export function TrayCard() {
  return (
    <Card className="card-square">
      <ChevronUp className="card-glyph" />
    </Card>
  );
}

/** Brightness: the built-in screen's level. */
export function BrightnessCard() {
  const { data } = useLoaded(readQuick, 15_000);
  const level = data?.brightness;
  return (
    <Card>
      <Sun className="card-yellow" />
      <span className="card-col card-grow">
        <span className="card-label">Brightness</span>
        <span className="card-row">
          <Meter value={level ?? 0} width={80} color="#ffd60a" />
          <span className="card-value">{level ?? "—"}</span>
        </span>
      </span>
    </Card>
  );
}
