import { Asterisk, Bot } from "lucide-react";

import { ipc } from "../../core/ipc";
import { useWidgets } from "../store";
import { useLoaded } from "../useLoaded";
import { useNow } from "../useNow";
import { Card, IconTile, Meter } from "./parts";
import { until } from "./format";

const meterColor = (percent: number) => (percent >= 90 ? "#ff453a" : percent >= 70 ? "#ff9f0a" : "#30d158");

/** Claude Usage: the 5-hour session's share and when it resets. */
export function ClaudeCard() {
  const claude = useWidgets((s) => s.claude);
  const now = useNow(60_000);
  const window = claude?.window;
  const percent = claude?.limited ? 100 : Math.round(window?.percent ?? 0);
  const resets = claude?.limited?.resetsAt ?? window?.resetsAt;
  return (
    <Card className="card-ai">
      <IconTile color="#d97757">
        <Asterisk strokeWidth={2.6} />
      </IconTile>
      <span className="card-col card-grow">
        <span className="card-row card-between">
          <span className="card-label">Session</span>
          <span className="card-faint">{resets ? until(Date.parse(resets), now) : "—"}</span>
        </span>
        <span className="card-big">{window || claude?.limited ? `${percent}%` : "Idle"}</span>
        <Meter value={percent} color={meterColor(percent)} />
      </span>
    </Card>
  );
}

/** Claude Plan Usage: session and this week, and the model you use most. */
export function ClaudePlanCard() {
  const claude = useWidgets((s) => s.claude);
  const session = Math.round(claude?.limited ? 100 : (claude?.window?.percent ?? 0));
  const model = claude?.window?.models[0]?.model.replace(/^claude-/, "") ?? "—";
  return (
    <Card className="card-ai">
      <IconTile color="#d97757">
        <Asterisk strokeWidth={2.6} />
      </IconTile>
      <span className="card-col card-grow">
        <LimitRow label="Session" percent={session} />
        <span className="card-limit">
          <span className="card-label">Week</span>
          <span className="card-faint">{tokens(claude?.weekTokens ?? 0)} tokens</span>
        </span>
        <span className="card-faint card-small">{model}</span>
      </span>
    </Card>
  );
}

/** "4.2M", "860K". */
function tokens(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)}B`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)}M`;
  if (n >= 1e3) return `${Math.round(n / 1e3)}K`;
  return String(n);
}

function LimitRow({ label, percent, note }: { label: string; percent: number; note?: string }) {
  return (
    <span className="card-limit">
      <span className="card-label">{label}</span>
      <Meter value={percent} color={meterColor(percent)} width={70} />
      {note && <span className="card-faint card-small">{note}</span>}
      <span className="card-value">{percent}%</span>
    </span>
  );
}

const readCodex = () => ipc.codexUsage();

/** ChatGPT Usage: the Codex plan's windows, read from Codex's own files. */
export function CodexCard() {
  const { data } = useLoaded(readCodex, 60_000);
  const now = useNow(60_000);
  const window = (w?: { windowMinutes: number }) =>
    !w ? "" : w.windowMinutes >= 7 * 1440 ? (w.windowMinutes >= 28 * 1440 ? "Monthly" : "Weekly") : "Session";
  const resets = (at?: number) => (at ? until(at * 1000, now) : undefined);
  return (
    <Card className="card-ai">
      <IconTile color="#ffffff">
        <Bot color="#111" strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-grow">
        {!data && <span className="card-faint">Codex not used on this PC</span>}
        {data?.primary && (
          <LimitRow
            label={window(data.primary)}
            percent={Math.round(data.primary.usedPercent)}
            note={resets(data.primary.resetsAt)}
          />
        )}
        {data?.secondary && (
          <LimitRow
            label={window(data.secondary)}
            percent={Math.round(data.secondary.usedPercent)}
            note={resets(data.secondary.resetsAt)}
          />
        )}
        {data && !data.secondary && (
          <span className="card-faint card-small">{data.plan || "ChatGPT"} plan</span>
        )}
      </span>
    </Card>
  );
}
