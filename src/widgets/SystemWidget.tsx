import { Cpu, MemoryStick } from "lucide-react";

import { useWidgets } from "./store";

function Meter({ label, value }: { label: string; value?: number }) {
  const v = value ?? 0;
  return (
    <div className="meter">
      <div className="meter-head">
        <span>{label}</span>
        <span className="meter-value">{value == null ? "—" : `${v}%`}</span>
      </div>
      <div
        className="meter-track"
        role="meter"
        aria-label={label}
        aria-valuenow={v}
        aria-valuemin={0}
        aria-valuemax={100}
      >
        <div className="meter-fill" data-high={v >= 85} style={{ width: `${v}%` }} />
      </div>
    </div>
  );
}

export function SystemTile() {
  const stats = useWidgets((s) => s.stats);
  return (
    <span className="tile-stack tile-system">
      <span className="tile-row">
        <Cpu size={12} aria-hidden="true" />
        <span className="tile-mini tile-more">CPU</span>
        <span className="tile-mini">{stats?.cpu ?? "—"}%</span>
      </span>
      <span className="tile-row">
        <MemoryStick size={12} aria-hidden="true" />
        <span className="tile-mini tile-more">RAM</span>
        <span className="tile-mini">{stats?.ram ?? "—"}%</span>
      </span>
    </span>
  );
}

export function SystemPanel() {
  const stats = useWidgets((s) => s.stats);
  return (
    <div className="panel-body">
      <h2 className="panel-title">This PC</h2>
      <Meter label="Processor" value={stats?.cpu} />
      <Meter label="Memory" value={stats?.ram} />
      {stats?.battery != null && (
        <Meter label={stats.charging ? "Battery (charging)" : "Battery"} value={stats.battery} />
      )}
    </div>
  );
}
