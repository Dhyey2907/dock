import { Hourglass, Play, Square, X } from "lucide-react";
import { useState } from "react";

import { ipc } from "../../core/ipc";
import type { TimeLog } from "../../core/generated/TimeLog";
import { useNow } from "../useNow";
import { formatClock, formatSpan, newId, runningEntry, toggleTimer, weekTotals } from "./logic";
import { report, useProductivity } from "./store";

const save = (log: TimeLog) => ipc.timelogSet(log).catch(report("save the time log"));

export function TimeTrackerTile() {
  const log = useProductivity((s) => s.timelog);
  const running = runningEntry(log);
  const now = useNow(1000, running != null);
  if (!running) return <Hourglass size={18} strokeWidth={1.8} aria-hidden="true" />;
  const project = log.projects.find((p) => p.id === running.projectId);
  return (
    <span className="tile-row" title={project?.name}>
      <span className="prod-live-dot" aria-hidden="true" />
      <span className="tile-strong tile-tabular">
        {formatClock((now - Date.parse(running.start)) / 1000)}
      </span>
    </span>
  );
}

export function TimeTrackerPanel() {
  const log = useProductivity((s) => s.timelog);
  const running = runningEntry(log);
  const now = useNow(1000, running != null);
  const totals = weekTotals(log, new Date(now));
  const week = [...totals.values()].reduce((a, b) => a + b, 0);
  const [draft, setDraft] = useState("");

  return (
    <div className="panel-body">
      <div className="prod-head">
        <h2 className="panel-title">Time this week</h2>
        <span className="panel-faint tile-tabular">{formatSpan(week)}</span>
      </div>
      <ul className="prod-list">
        {log.projects.map((p) => {
          const active = running?.projectId === p.id;
          return (
            <li key={p.id} className="time-row" data-active={active}>
              <button
                className="icon-btn time-toggle"
                aria-label={active ? `Stop ${p.name}` : `Start ${p.name}`}
                onClick={() => save(toggleTimer(log, p.id, new Date()))}
              >
                {active ? <Square size={14} /> : <Play size={14} />}
              </button>
              <span className="time-name">{p.name}</span>
              <span className="tile-tabular time-total">
                {active
                  ? formatClock((now - Date.parse(running.start)) / 1000)
                  : formatSpan(totals.get(p.id) ?? 0)}
              </span>
              <button
                className="icon-btn prod-hover"
                aria-label={`Remove ${p.name}`}
                title="Remove project and its time"
                onClick={() => save({ ...log, projects: log.projects.filter((x) => x.id !== p.id) })}
              >
                <X size={14} />
              </button>
            </li>
          );
        })}
      </ul>
      {log.projects.length === 0 && <p className="panel-faint">Add a project, then press play to time it.</p>}
      <form
        className="prod-add"
        onSubmit={(e) => {
          e.preventDefault();
          const name = draft.trim();
          if (name) save({ ...log, projects: [...log.projects, { id: newId(), name }] });
          setDraft("");
        }}
      >
        <input
          className="prod-input"
          value={draft}
          placeholder="New project"
          onChange={(e) => setDraft(e.target.value)}
        />
      </form>
    </div>
  );
}
