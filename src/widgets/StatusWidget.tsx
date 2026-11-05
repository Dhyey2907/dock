import {
  Battery,
  BatteryCharging,
  BatteryFull,
  BatteryLow,
  BatteryMedium,
  Cable,
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

export function StatusPanel() {
  const { sound, network, stats } = useWidgets();
  const VolIcon = VOLUME[volumeKey(sound)];
  const NetIcon = NETWORK[networkKey(network)];
  const level = Math.round((sound?.level ?? 0) * 100);
  const battery = stats?.battery;
  const BatIcon = battery != null ? BATTERY[batteryKey(battery, stats?.charging ?? false)] : null;
  return (
    <div className="panel-body">
      <section className="status-row">
        <button
          className="icon-btn"
          aria-label={sound?.muted ? "Unmute" : "Mute"}
          onClick={() => ipc.volumeToggleMute()}
          disabled={!sound}
        >
          <VolIcon size={20} />
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
        <span className="status-value">{sound ? level : "—"}</span>
      </section>
      {sound?.device && <p className="panel-faint status-sub">{sound.device}</p>}

      <button className="status-line" onClick={() => ipc.openWindowsSettings("network")}>
        <NetIcon size={18} aria-hidden="true" />
        <span className="status-text">
          {network?.connected ? (network.name ?? (network.wifi ? "Wi-Fi" : "Ethernet")) : "Not connected"}
          <span className="panel-faint">
            {network?.connected ? (network.internet ? "Connected" : "No internet") : "Network settings"}
          </span>
        </span>
      </button>

      {BatIcon && battery != null && (
        <button className="status-line" onClick={() => ipc.openWindowsSettings("battery")}>
          <BatIcon size={18} aria-hidden="true" />
          <span className="status-text">
            {battery}%<span className="panel-faint">{stats?.charging ? "Charging" : "On battery"}</span>
          </span>
        </button>
      )}
      <button className="panel-link" onClick={() => ipc.openWindowsSettings("sound")}>
        Sound settings
      </button>
    </div>
  );
}
