import { useEffect, useState } from "react";
import {
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  BatteryMedium,
  Bluetooth,
  BluetoothOff,
  Cable,
  ChevronRight,
  LayoutGrid,
  Settings,
  SlidersHorizontal,
  Sun,
  Volume,
  Volume1,
  Volume2,
  VolumeX,
  Wifi,
  WifiHigh,
  WifiLow,
  WifiOff,
  WifiZero,
  type LucideIcon,
} from "lucide-react";

import { ipc } from "../core/ipc";
import type { NetworkState } from "../core/generated/NetworkState";
import type { QuickState } from "../core/generated/QuickState";
import type { RadioKind } from "../core/generated/RadioKind";
import type { SoundState } from "../core/generated/SoundState";
import { useWidgets } from "./store";

const VOLUME: Record<string, LucideIcon> = { off: VolumeX, low: Volume, mid: Volume1, high: Volume2 };
const NETWORK: Record<string, LucideIcon> = {
  off: WifiOff,
  wired: Cable,
  wifi0: WifiZero,
  wifi1: WifiLow,
  wifi2: WifiHigh,
  wifi3: Wifi,
};
const BATTERY: Record<string, LucideIcon> = {
  charging: BatteryCharging,
  full: BatteryFull,
  mid: BatteryMedium,
  low: BatteryLow,
  empty: Battery,
};

function volumeKey(sound?: SoundState) {
  if (!sound || sound.muted || sound.level === 0) return "off";
  if (sound.level < 0.34) return "low";
  return sound.level < 0.67 ? "mid" : "high";
}

function networkKey(network?: NetworkState) {
  if (!network?.connected) return "off";
  if (!network.wifi) return "wired";
  const bars = network.bars ?? 5;
  if (bars <= 1) return "wifi0";
  if (bars <= 2) return "wifi1";
  return bars <= 3 ? "wifi2" : "wifi3";
}

function batteryKey(percent: number, charging: boolean) {
  if (charging) return "charging";
  if (percent >= 85) return "full";
  if (percent >= 45) return "mid";
  return percent >= 15 ? "low" : "empty";
}

/** The taskbar's tray area at a glance: network, volume, battery. */
export function StatusTile() {
  const { sound, network, stats } = useWidgets();
  const NetIcon = NETWORK[networkKey(network)];
  const VolIcon = VOLUME[volumeKey(sound)];
  const battery = stats?.battery;
  const BatIcon = battery != null ? BATTERY[batteryKey(battery, stats?.charging ?? false)] : null;
  return (
    <span className="tile-row">
      <NetIcon size={16} strokeWidth={1.9} aria-hidden="true" />
      <VolIcon size={16} strokeWidth={1.9} aria-hidden="true" />
      {BatIcon && <BatIcon size={18} strokeWidth={1.9} aria-hidden="true" />}
    </span>
  );
}

/** Send the newest value, one call at a time, so dragging a slider never queues up calls. */
function latestOnly(send: (value: number) => Promise<unknown>) {
  let busy = false;
  let next: number | null = null;
  const run = (value: number) => {
    busy = true;
    send(value)
      .catch((error) => console.error(error))
      .finally(() => {
        busy = false;
        if (next != null) {
          const value = next;
          next = null;
          run(value);
        }
      });
  };
  return (value: number) => {
    if (busy) next = value;
    else run(value);
  };
}

const setBrightness = latestOnly((level) => ipc.quickSetBrightness(level));

/** A Windows-style toggle tile: the main part switches, the arrow opens its settings page. */
function QuickTile({
  icon: Icon,
  label,
  on,
  onToggle,
  onMore,
  disabled,
}: {
  icon: LucideIcon;
  label: string;
  on: boolean;
  onToggle?: () => void;
  onMore?: () => void;
  disabled?: boolean;
}) {
  return (
    <div className="quick-tile" data-on={on}>
      <div className="quick-tile-face">
        <button
          className="quick-tile-main"
          aria-pressed={onToggle ? on : undefined}
          aria-label={label}
          disabled={disabled}
          onClick={onToggle ?? onMore}
        >
          <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
        </button>
        {onToggle && onMore && (
          <button className="quick-tile-more" aria-label={`${label} settings`} onClick={onMore}>
            <ChevronRight size={16} aria-hidden="true" />
          </button>
        )}
      </div>
      <span className="quick-tile-label" title={label}>
        {label}
      </span>
    </div>
  );
}

export function StatusPanel() {
  const { sound, network, stats } = useWidgets();
  const [quick, setQuick] = useState<QuickState | null>(null);
  const [brightness, setLocalBrightness] = useState<number | null>(null);
  useEffect(() => {
    let live = true;
    ipc
      .quickGet()
      .then((state) => {
        if (!live) return;
        setQuick(state);
        setLocalBrightness(state.brightness ?? null);
      })
      .catch((error) => console.error(error));
    return () => {
      live = false;
    };
  }, []);
  const toggle = (kind: RadioKind, on: boolean) => {
    // Show the change at once; Windows confirms (or not) a moment later.
    setQuick((q) => (q ? { ...q, [kind]: on } : q));
    ipc
      .quickSetRadio(kind, on)
      .then(setQuick)
      .catch((error) => {
        console.error(error);
        ipc
          .quickGet()
          .then(setQuick)
          .catch(() => {});
      });
  };

  const VolIcon = VOLUME[volumeKey(sound)];
  const level = Math.round((sound?.level ?? 0) * 100);
  const battery = stats?.battery;
  const BatIcon = battery != null ? BATTERY[batteryKey(battery, stats?.charging ?? false)] : null;
  const wifiOn = quick?.wifi ?? Boolean(network?.wifi);
  const wifiLabel = wifiOn && network?.connected && network.wifi ? (network.name ?? "Wi-Fi") : "Wi-Fi";
  return (
    <div className="panel-body quick-panel">
      <section className="quick-grid">
        <QuickTile
          icon={wifiOn ? NETWORK[networkKey(network)] : WifiOff}
          label={network?.connected && !network.wifi ? "Ethernet" : wifiLabel}
          on={wifiOn || Boolean(network?.connected && !network.wifi)}
          disabled={quick != null && quick.wifi == null}
          onToggle={quick?.wifi != null ? () => toggle("wifi", !wifiOn) : undefined}
          onMore={() => ipc.openWindowsSettings("network")}
        />
        {quick?.bluetooth != null && (
          <QuickTile
            icon={quick.bluetooth ? Bluetooth : BluetoothOff}
            label="Bluetooth"
            on={quick.bluetooth}
            onToggle={() => toggle("bluetooth", !quick.bluetooth)}
            onMore={() => ipc.openWindowsSettings("bluetooth")}
          />
        )}
        <QuickTile
          icon={LayoutGrid}
          label="More"
          on={false}
          onMore={() => ipc.quickOpenWindows().catch((error) => console.error(error))}
        />
      </section>

      <section className="quick-sliders">
        {brightness != null && (
          <div className="status-row">
            <span className="icon-btn" aria-hidden="true">
              <Sun size={19} />
            </span>
            <input
              className="slider"
              type="range"
              min={0}
              max={100}
              value={brightness}
              aria-label="Brightness"
              onChange={(e) => {
                const value = Number(e.target.value);
                setLocalBrightness(value);
                setBrightness(value);
              }}
              style={{ "--fill": `${brightness}%` } as React.CSSProperties}
            />
          </div>
        )}
        <div className="status-row">
          <button
            className="icon-btn"
            aria-label={sound?.muted ? "Unmute" : "Mute"}
            onClick={() => ipc.volumeToggleMute()}
            disabled={!sound}
          >
            <VolIcon size={19} />
          </button>
          <input
            className="slider"
            type="range"
            min={0}
            max={100}
            value={level}
            aria-label="Volume"
            disabled={!sound}
            onChange={(e) => ipc.volumeSet(Number(e.target.value) / 100)}
            style={{ "--fill": `${level}%` } as React.CSSProperties}
          />
          <button
            className="icon-btn"
            aria-label="Sound settings"
            title={sound?.device ?? "Sound settings"}
            onClick={() => ipc.openWindowsSettings("sound")}
          >
            <SlidersHorizontal size={17} />
          </button>
        </div>
      </section>

      <footer className="quick-footer">
        {BatIcon && battery != null ? (
          <button className="quick-battery" onClick={() => ipc.openWindowsSettings("battery")}>
            <BatIcon size={18} aria-hidden="true" />
            <span>{battery}%</span>
            {stats?.charging && <span className="panel-faint">Charging</span>}
          </button>
        ) : (
          <span />
        )}
        <button
          className="icon-btn"
          aria-label="All settings"
          title="All settings"
          onClick={() => ipc.openWindowsSettings("home")}
        >
          <Settings size={17} />
        </button>
      </footer>
    </div>
  );
}
