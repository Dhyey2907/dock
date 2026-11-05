import { Sparkles } from "lucide-react";

import { formatTokens, modelName } from "./logic";
import { useWidgets } from "./store";
import { useNow } from "./useNow";

const locale = navigator.language || "en";
const time = (iso: string) =>
  new Date(iso).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });

function untilText(iso: string, now: number) {
  const minutes = Math.max(0, Math.ceil((Date.parse(iso) - now) / 60_000));
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  if (h >= 24) return new Date(iso).toLocaleDateString(locale, { weekday: "short", hour: "numeric" });
  return m ? `${h} h ${m} min` : `${h} h`;
}

function Ring({ percent }: { percent: number }) {
  const size = 18;
  const r = 7;
  const c = 2 * Math.PI * r;
  const shown = Math.min(100, percent);
  return (
    <svg className="ring" width={size} height={size} viewBox="0 0 18 18" aria-hidden="true">
      <circle className="ring-track" cx="9" cy="9" r={r} strokeWidth="2.5" />
      <circle
        className="ring-fill"
        data-high={percent >= 85}
        cx="9"
        cy="9"
        r={r}
        strokeWidth="2.5"
        strokeDasharray={c}
        strokeDashoffset={c * (1 - shown / 100)}
      />
    </svg>
  );
}

export function ClaudeTile() {
  const claude = useWidgets((s) => s.claude);
  if (claude?.limited) {
    return (
      <span className="tile-row claude-limited">
        <Sparkles size={16} aria-hidden="true" />
        <span className="tile-strong">back {time(claude.limited.resetsAt)}</span>
      </span>
    );
  }
  const window = claude?.window;
  if (!window) return <Sparkles size={18} strokeWidth={1.8} aria-hidden="true" />;
  return (
    <span className="tile-row">
      {window.percent != null ? <Ring percent={window.percent} /> : <Sparkles size={16} aria-hidden="true" />}
      <span className="tile-strong tile-tabular">
        {window.percent != null ? `≈${window.percent}%` : formatTokens(window.tokens)}
      </span>
    </span>
  );
}

export function ClaudePanel() {
  const claude = useWidgets((s) => s.claude);
  const now = useNow(60_000);
  if (!claude) return <div className="panel-body panel-empty">Reading Claude Code's history…</div>;
  if (!claude.found) {
    return (
      <div className="panel-body panel-empty">
        <Sparkles size={28} strokeWidth={1.6} />
        <p>No Claude Code activity found on this PC yet.</p>
      </div>
    );
  }
  const window = claude.window;
  return (
    <div className="panel-body claude-panel">
      <h2 className="panel-title">Claude on this PC</h2>
      {claude.limited && (
        <p className="claude-alert">
          {claude.limited.kind === "weekly"
            ? "Weekly"
            : claude.limited.kind === "fiveHour"
              ? "5-hour"
              : "Usage"}{" "}
          limit reached · back at {time(claude.limited.resetsAt)} (in{" "}
          {untilText(claude.limited.resetsAt, now)})
        </p>
      )}
      {window ? (
        <section className="claude-window">
          <div className="claude-big">
            <span className="panel-big tile-tabular">{formatTokens(window.tokens)}</span>
            <span className="panel-faint">tokens · {window.messages} messages this window</span>
          </div>
          {window.percent != null && (
            <div className="meter">
              <div
                className="meter-track"
                role="meter"
                aria-label="Share of usual limit"
                aria-valuenow={window.percent}
              >
                <div
                  className="meter-fill"
                  data-high={window.percent >= 85}
                  style={{ width: `${Math.min(100, window.percent)}%` }}
                />
              </div>
              <p className="panel-faint claude-note">
                ≈{window.percent}% of where you usually hit the 5-hour limit
              </p>
            </div>
          )}
          <p className="panel-faint claude-note">
            Resets at {time(window.resetsAt)} · in {untilText(window.resetsAt, now)}
          </p>
          {window.models.length > 0 && (
            <ul className="claude-models">
              {window.models.slice(0, 3).map((m) => (
                <li key={m.model}>
                  <span>{modelName(m.model)}</span>
                  <span className="panel-faint tile-tabular">{formatTokens(m.tokens)}</span>
                </li>
              ))}
            </ul>
          )}
        </section>
      ) : (
        <p className="panel-faint">No Claude use in the last 5 hours.</p>
      )}
      <div className="claude-totals">
        <span>
          Today <strong className="tile-tabular">{formatTokens(claude.todayTokens)}</strong>
        </span>
        <span>
          7 days <strong className="tile-tabular">{formatTokens(claude.weekTokens)}</strong>
        </span>
      </div>
      <p className="panel-faint claude-footnote">
        Estimated from Claude Code on this PC; your plan's official limits aren't available to other apps.
      </p>
    </div>
  );
}
