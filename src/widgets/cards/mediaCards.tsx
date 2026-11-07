import { Disc3, Pause, Play, SkipBack, SkipForward } from "lucide-react";

import { ipc } from "../../core/ipc";
import type { MediaCommand } from "../../core/generated/MediaCommand";
import { useIcon } from "../../core/useIcon";
import { playbackPosition } from "../logic";
import { useWidgets } from "../store";
import { useNow } from "../useNow";
import { Card, CardButton, Meter, Ring } from "./parts";

function send(command: MediaCommand) {
  ipc.mediaCommand(command).catch((error) => console.error("media command failed", error));
}

/** What's playing, how far in (0–100), and its picture. */
function useTrack() {
  const music = useWidgets((s) => s.music);
  const playing = music?.status === "playing";
  const now = useNow(1000, playing);
  const appIcon = useIcon(music?.sourceIcon, !!music && !music.artwork);
  const position = music ? playbackPosition(music, now) : undefined;
  const progress = music?.durationMs && position != null ? (position / music.durationMs) * 100 : 0;
  return { music, playing, progress, art: music?.artwork ?? appIcon };
}

function Art({ src, size = "" }: { src: string | null | undefined; size?: string }) {
  return src ? (
    <img className={`card-art ${size}`} src={src} alt="" draggable={false} />
  ) : (
    <span className={`card-art card-art-empty ${size}`}>
      <Disc3 />
    </span>
  );
}

function PlayButton({ playing, big = false }: { playing: boolean; big?: boolean }) {
  return (
    <CardButton
      label={playing ? "Pause" : "Play"}
      className={big ? "card-play card-play-big" : "card-play"}
      onPress={() => send({ kind: "playPause" })}
    >
      {playing ? <Pause /> : <Play />}
    </CardButton>
  );
}

/** Now Playing: cover art, title and full controls. */
export function NowPlayingCard() {
  const { music, playing, progress, art } = useTrack();
  return (
    <Card className="card-wide card-media">
      <Art src={art} />
      <span className="card-col card-text">
        <span className="card-line card-title">{music?.title || "Nothing playing"}</span>
        <span className="card-line card-faint">{music?.artist || music?.sourceApp || ""}</span>
      </span>
      <span className="card-row card-controls">
        <CardButton
          label="Previous"
          className="card-round card-quiet"
          onPress={() => send({ kind: "previous" })}
        >
          <SkipBack />
        </CardButton>
        <PlayButton playing={playing} big />
        <CardButton label="Next" className="card-round card-quiet" onPress={() => send({ kind: "next" })}>
          <SkipForward />
        </CardButton>
      </span>
      <span className="card-progress" style={{ width: `${progress}%` }} />
    </Card>
  );
}

/** Now Playing (compact): title, artist and play/pause. */
export function NowPlayingCompactCard() {
  const { music, playing, art } = useTrack();
  return (
    <Card className="card-media">
      <Art src={art} size="card-art-small" />
      <span className="card-col card-text">
        <span className="card-line card-title">{music?.title || "Nothing playing"}</span>
        <span className="card-line card-faint">{music?.artist ?? ""}</span>
      </span>
      <PlayButton playing={playing} />
    </Card>
  );
}

/** Now Playing Mini: one round play button with the track's progress around it. */
export function NowPlayingMiniCard() {
  const { playing, progress } = useTrack();
  return (
    <Card className="card-square">
      <Ring value={progress} size={52} color="#0a84ff">
        <PlayButton playing={playing} />
      </Ring>
    </Card>
  );
}

/** Now Playing Bar: the track with a progress bar and skip. */
export function NowPlayingBarCard() {
  const { music, playing, progress } = useTrack();
  return (
    <Card className="card-media">
      <PlayButton playing={playing} />
      <span className="card-col card-grow card-text">
        <span className="card-line card-title">{music?.title || "Nothing playing"}</span>
        <Meter value={progress} width={110} color="#0a84ff" />
      </span>
      <CardButton label="Next" className="card-round card-quiet" onPress={() => send({ kind: "next" })}>
        <SkipForward />
      </CardButton>
    </Card>
  );
}
