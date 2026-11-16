import type { ComponentType } from "react";
import { CloudSun, Disc3, Watch } from "lucide-react";

import { ipc } from "../../core/ipc";
import { useIcon } from "../../core/useIcon";
import { useProductivity } from "../productivity/store";
import { useWidgets } from "../store";
import { useLoaded } from "../useLoaded";
import { useNow } from "../useNow";
import { Card, Ring } from "./parts";

/**
 * Small square versions of the cards, for when a row is full: the one thing each card is
 * about (the cover, the percentage, the date…), so many fit where a few full cards would.
 */
function Square({ children, color }: { children: React.ReactNode; color?: string }) {
  return (
    <Card className="card-square card-compact" tone={color}>
      {children}
    </Card>
  );
}

export function MusicCompact() {
  const music = useWidgets((s) => s.music);
  const appIcon = useIcon(music?.sourceIcon, !!music && !music.artwork);
  const art = music?.artwork ?? appIcon;
  return (
    <Square>
      {art ? (
        <img className="compact-art" src={art} alt="" draggable={false} />
      ) : (
        <Disc3 className="compact-glyph" />
      )}
    </Square>
  );
}

function PercentRing({
  value,
  color,
  children,
}: {
  value: number;
  color: string;
  children?: React.ReactNode;
}) {
  return (
    <Square>
      <Ring value={value} size={50} stroke={6} color={color}>
        {children ?? <span className="compact-number">{Math.round(value)}</span>}
      </Ring>
    </Square>
  );
}

export function ClaudeCompact() {
  const claude = useWidgets((s) => s.claude);
  const percent = claude?.limited ? 100 : (claude?.window?.percent ?? 0);
  return <PercentRing value={percent} color="#d97757" />;
}

export function SystemCompact() {
  const cpu = useWidgets((s) => s.stats?.cpu ?? 0);
  return <PercentRing value={cpu} color="#0a84ff" />;
}

export function RamCompact() {
  const ram = useWidgets((s) => s.stats?.ram ?? 0);
  return <PercentRing value={ram} color="#ff9f0a" />;
}

export function TodosCompact() {
  const todos = useProductivity((s) => s.todos);
  const left = todos.filter((t) => !t.done).length;
  const done = todos.length ? ((todos.length - left) / todos.length) * 100 : 0;
  return (
    <PercentRing value={done} color="#30d158">
      <span className="compact-number">{left}</span>
    </PercentRing>
  );
}

export function CalendarCompact() {
  const date = new Date(useNow(60_000));
  return (
    <Square>
      <span className="compact-page">
        <span>{date.toLocaleDateString(undefined, { month: "short" })}</span>
        <strong>{date.getDate()}</strong>
      </span>
    </Square>
  );
}

export function WeatherCompact() {
  const temperature = useWidgets((s) => s.weather?.temperature);
  return (
    <Square>
      <span className="compact-stack">
        <CloudSun className="compact-small-glyph" color="#ffd60a" />
        <span className="compact-number">{temperature != null ? `${temperature}°` : "—"}</span>
      </span>
    </Square>
  );
}

export function YearCompact() {
  const now = new Date(useNow(3_600_000));
  const start = new Date(now.getFullYear(), 0, 1).getTime();
  const end = new Date(now.getFullYear() + 1, 0, 1).getTime();
  return <PercentRing value={((now.getTime() - start) / (end - start)) * 100} color="#ffffff" />;
}

const readCodex = () => ipc.codexUsage();

export function CodexCompact() {
  const { data } = useLoaded(readCodex, 60_000);
  return <PercentRing value={data?.primary?.usedPercent ?? 0} color="#30d158" />;
}

const readScreenTime = () => ipc.screenTime();

export function ScreenTimeCompact() {
  const { data } = useLoaded(readScreenTime, 60_000);
  const minutes = data?.minutes ?? 0;
  return (
    <Square>
      <span className="compact-stack">
        <Watch className="compact-small-glyph" color="#0a84ff" />
        <span className="compact-number">
          {minutes >= 60 ? `${Math.floor(minutes / 60)}h` : `${minutes}m`}
        </span>
      </span>
    </Square>
  );
}

export function Glyph({
  icon: Icon,
  color,
}: {
  icon: ComponentType<{ className?: string; color?: string }>;
  color: string;
}) {
  return (
    <Square>
      <Icon className="compact-glyph" color={color} />
    </Square>
  );
}
