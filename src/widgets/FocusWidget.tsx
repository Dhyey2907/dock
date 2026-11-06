import { Pause, Play, RotateCcw, Timer } from "lucide-react";

import { ipc } from "../core/ipc";
import { focusSecondsLeft, formatCountdown } from "./logic";
import { useWidgets } from "./store";
import { useNow } from "./useNow";

function Ring({ fraction, size, stroke }: { fraction: number; size: number; stroke: number }) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  return (
    <svg className="ring" width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden="true">
      <circle className="ring-track" cx={size / 2} cy={size / 2} r={r} strokeWidth={stroke} />
      <circle
        className="ring-fill"
        cx={size / 2}
        cy={size / 2}
        r={r}
        strokeWidth={stroke}
        strokeDasharray={c}
        strokeDashoffset={c * (1 - Math.min(1, Math.max(0, fraction)))}
      />
    </svg>
  );
}

function useFocus() {
  const focus = useWidgets((s) => s.focus);
  const running = focus.endsAt != null;
  const now = useNow(1000, running);
  const left = focusSecondsLeft(focus, now);
  const fraction = focus.phase === "idle" ? 0 : 1 - left / focus.phaseSeconds;
  return { focus, running, left, fraction };
}

export function FocusTile() {
  const { focus, left, fraction } = useFocus();
  if (focus.phase === "idle") return <Timer size={18} strokeWidth={1.8} aria-hidden="true" />;
  return (
    <span className="tile-row" data-phase={focus.phase}>
      <Ring fraction={fraction} size={18} stroke={2.5} />
      <span className="tile-stack music-text">
        <span className="tile-strong tile-tabular">{formatCountdown(left)}</span>
        <span className="tile-faint tile-more">{focus.phase === "focus" ? "Focus" : "Break"}</span>
      </span>
    </span>
  );
}

export function FocusPanel() {
  const { focus, running, left, fraction } = useFocus();
  const label = focus.phase === "focus" ? "Focus" : focus.phase === "break" ? "Break" : "Ready to focus";
  return (
    <div className="panel-body focus-panel" data-phase={focus.phase}>
      <div className="focus-dial">
        <Ring fraction={fraction} size={132} stroke={7} />
        <div className="focus-readout">
          <div className="panel-big tile-tabular">{formatCountdown(left)}</div>
          <div className="panel-faint">{label}</div>
        </div>
      </div>
      <div className="focus-actions">
        {running ? (
          <button className="panel-button" onClick={() => ipc.focusPause()}>
            <Pause size={16} /> Pause
          </button>
        ) : (
          <button className="panel-button" onClick={() => ipc.focusStart()}>
            <Play size={16} /> {focus.pausedSeconds != null ? "Resume" : "Start"}
          </button>
        )}
        <button
          className="icon-btn"
          aria-label="Reset"
          onClick={() => ipc.focusReset()}
          disabled={focus.phase === "idle"}
        >
          <RotateCcw size={16} />
        </button>
      </div>
      <p className="panel-faint">
        {focus.completed === 0
          ? "25 minutes of focus, then a 5-minute break."
          : `${focus.completed} focus ${focus.completed === 1 ? "session" : "sessions"} done.`}
      </p>
    </div>
  );
}
