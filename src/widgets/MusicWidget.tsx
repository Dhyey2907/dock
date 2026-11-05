import type { MouseEvent } from "react";
import { Music, Pause, Play, SkipBack, SkipForward } from "lucide-react";

import { ipc } from "../core/ipc";
import type { MediaCommand } from "../core/generated/MediaCommand";
import { formatDuration, playbackPosition } from "./logic";
import { useWidgets } from "./store";
import { useNow } from "./useNow";

function send(command: MediaCommand) {
  ipc.mediaCommand(command).catch((error) => console.error("media command failed", error));
}

export function MusicTile() {
  const music = useWidgets((s) => s.music);
  if (!music || (!music.title && !music.artist)) {
    return <Music size={18} strokeWidth={1.8} aria-hidden="true" />;
  }
  return (
    <span className="tile-row music-tile" data-playing={music.status === "playing"}>
      {music.artwork ? (
        <img className="music-art-small" src={music.artwork} alt="" draggable={false} />
      ) : (
        <Music size={16} aria-hidden="true" />
      )}
      <span className="tile-stack music-text">
        <span className="tile-strong">{music.title || music.sourceApp}</span>
        {music.artist && <span className="tile-faint">{music.artist}</span>}
      </span>
    </span>
  );
}

export function MusicPanel() {
  const music = useWidgets((s) => s.music);
  const playing = music?.status === "playing";
  const now = useNow(1000, playing);
  if (!music) {
    return (
      <div className="panel-body panel-empty">
        <Music size={28} strokeWidth={1.6} />
        <p>Nothing is playing.</p>
        <p className="panel-faint">Music and videos from Spotify, browsers and other apps show up here.</p>
      </div>
    );
  }
  const position = playbackPosition(music, now);
  const duration = music.durationMs;
  const fraction = position != null && duration ? position / duration : 0;
  const seek = (e: MouseEvent<HTMLDivElement>) => {
    if (!music.capabilities.seek || !duration) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const at = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    send({ kind: "seek", positionMs: Math.round(at * duration) });
  };
  return (
    <div className="panel-body music-panel">
      <div className="music-head">
        {music.artwork ? (
          <img className="music-art" src={music.artwork} alt="" draggable={false} />
        ) : (
          <span className="music-art music-art-empty">
            <Music size={30} />
          </span>
        )}
        <div className="music-info">
          <div className="music-title">{music.title || "Unknown track"}</div>
          <div className="panel-faint music-line">
            {[music.artist, music.album].filter(Boolean).join(" · ")}
          </div>
          <div className="panel-faint music-line music-source">{music.sourceApp}</div>
        </div>
      </div>
      {duration != null && position != null && (
        <div className="music-progress">
          <div
            className="music-bar"
            data-seekable={music.capabilities.seek}
            role="progressbar"
            aria-label="Playback position"
            aria-valuenow={Math.round(fraction * 100)}
            onClick={seek}
          >
            <div className="music-bar-fill" style={{ width: `${fraction * 100}%` }} />
          </div>
          <div className="music-times panel-faint">
            <span>{formatDuration(position)}</span>
            <span>{formatDuration(duration)}</span>
          </div>
        </div>
      )}
      <div className="music-controls">
        <button
          className="icon-btn"
          aria-label="Previous"
          disabled={!music.capabilities.previous}
          onClick={() => send({ kind: "previous" })}
        >
          <SkipBack size={18} />
        </button>
        <button
          className="music-play"
          aria-label={playing ? "Pause" : "Play"}
          disabled={!music.capabilities.playPause}
          onClick={() => send({ kind: "playPause" })}
        >
          {playing ? <Pause size={20} /> : <Play size={20} />}
        </button>
        <button
          className="icon-btn"
          aria-label="Next"
          disabled={!music.capabilities.next}
          onClick={() => send({ kind: "next" })}
        >
          <SkipForward size={18} />
        </button>
      </div>
    </div>
  );
}
