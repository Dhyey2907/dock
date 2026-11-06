import { ListMusic, SquareTerminal } from "lucide-react";
import { useEffect } from "react";

import { ipc } from "../../core/ipc";
import { useNow } from "../useNow";
import { ago, isToday } from "./logic";
import { report, useProductivity } from "./store";

// ---------- Songs played today ----------

function useTodaysSongs() {
  const songs = useProductivity((s) => s.songs);
  const now = useNow(60_000);
  return isToday(songs.day, new Date(now)) ? songs.songs : [];
}

export function SongsTodayTile() {
  const count = useTodaysSongs().length;
  return (
    <span className="tile-row">
      <ListMusic size={18} strokeWidth={1.8} aria-hidden="true" />
      {count > 0 && <span className="tile-strong tile-tabular">{count}</span>}
    </span>
  );
}

export function SongsTodayPanel() {
  const songs = [...useTodaysSongs()].reverse();
  return (
    <div className="panel-body">
      <div className="prod-head">
        <h2 className="panel-title">Played today</h2>
        <span className="panel-faint">{songs.length === 1 ? "1 song" : `${songs.length} songs`}</span>
      </div>
      <ul className="prod-list">
        {songs.map((song, i) => (
          <li key={`${song.at}-${i}`} className="song-row">
            <span className="tile-tabular panel-faint song-time">
              {new Date(song.at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}
            </span>
            <span className="prod-row-two">
              <span className="prod-row-main">{song.title}</span>
              <span className="panel-faint prod-row-sub">
                {[song.artist, song.sourceApp].filter(Boolean).join(" · ")}
              </span>
            </span>
          </li>
        ))}
      </ul>
      {songs.length === 0 && (
        <p className="panel-faint">Songs you play today show up here. The list starts over each day.</p>
      )}
    </div>
  );
}

// ---------- Claude Code sessions ----------

export function ClaudeSessionsTile() {
  const running = useProductivity((s) => s.claude.running);
  return (
    <span className="tile-row">
      <SquareTerminal size={18} strokeWidth={1.8} aria-hidden="true" />
      {running > 0 && (
        <>
          <span className="prod-live-dot" aria-hidden="true" />
          <span className="tile-strong tile-tabular">{running}</span>
        </>
      )}
    </span>
  );
}

export function ClaudeSessionsPanel() {
  const activity = useProductivity((s) => s.claude);
  const now = useNow(30_000);
  useEffect(() => {
    // Look now rather than waiting for the next check; the result arrives as an event.
    ipc.claudeSessionsGet().catch(report("read Claude Code sessions"));
  }, []);
  return (
    <div className="panel-body">
      <div className="prod-head">
        <h2 className="panel-title">Claude Code</h2>
        <span className="panel-faint">
          {activity.running > 0 ? `${activity.running} running` : "Nothing running"}
        </span>
      </div>
      <ul className="prod-list">
        {activity.sessions.map((s) => (
          <li key={s.id} className="session-row" data-running={s.running}>
            <span className="session-dot" aria-label={s.running ? "Running" : "Idle"} />
            <span className="prod-row-two">
              <span className="prod-row-main" title={s.title}>
                {s.title}
              </span>
              <span className="panel-faint prod-row-sub">
                {s.project} · {s.running ? "working now" : ago(s.modified, new Date(now))}
              </span>
            </span>
          </li>
        ))}
      </ul>
      {activity.sessions.length === 0 && <p className="panel-faint">No Claude Code sessions this week.</p>}
    </div>
  );
}
