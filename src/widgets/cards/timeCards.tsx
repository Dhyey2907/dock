import { ArrowRightLeft, Hourglass, NotebookPen, Pause, Play, RotateCcw, Timer, Watch } from "lucide-react";

import { ipc } from "../../core/ipc";
import { useSettings } from "../../settings/store";
import { useProductivity } from "../productivity/store";
import { useWidgets } from "../store";
import { useLoaded } from "../useLoaded";
import { useNow } from "../useNow";
import { Bars, Card, CardButton, DotGrid, IconTile, Ring } from "./parts";
import { duration, until } from "./format";
import { useStopwatch, useTimer } from "./clockStore";

/** One shared empty list, so a store read without data stays the same between renders. */
const NONE: never[] = [];

const readScreenTime = () => ipc.screenTime();

/** Screen Time: today's time at the PC, with the hours as bars. */
export function ScreenTimeCard() {
  const { data } = useLoaded(readScreenTime, 60_000);
  const hour = new Date().getHours();
  return (
    <Card className="card-wide">
      <IconTile color="#0a84ff">
        <Watch strokeWidth={2.2} />
      </IconTile>
      <span className="card-col">
        <span className="card-label card-dim">Screen Time</span>
        <span className="card-big">{duration(data?.minutes ?? 0)}</span>
      </span>
      <Bars values={data?.hours ?? []} mark={hour} width={100} />
    </Card>
  );
}

/** Day Timeline: the whole day on one line, with time on the PC, focus rounds and meetings. */
export function DayTimelineCard() {
  const { data } = useLoaded(readScreenTime, 60_000);
  const focus = useWidgets((s) => s.focus);
  const weather = useWidgets((s) => s.weather);
  const events = useWidgets((s) => s.calendar?.events) ?? NONE;
  const now = new Date(useNow(60_000));
  const today = now.toDateString();
  const meetings = events.filter((e) => !e.allDay && new Date(e.start).toDateString() === today);
  const hourOf = (iso?: string) =>
    iso ? new Date(iso).getHours() + new Date(iso).getMinutes() / 60 : undefined;
  const marks = [
    { at: hourOf(weather?.sunrise), kind: "sun" },
    { at: hourOf(weather?.sunset), kind: "sun" },
    ...meetings.map((m) => ({ at: hourOf(m.start), kind: "meet" })),
  ].filter((m): m is { at: number; kind: string } => m.at !== undefined);
  const nowAt = now.getHours() + now.getMinutes() / 60;
  return (
    <Card className="card-wide card-timeline">
      <span className="card-row card-between">
        <span className="card-label">
          {now.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })}
        </span>
        <span className="card-faint">
          {duration(data?.minutes ?? 0)} on the PC
          {focus.completed > 0 && <span className="card-red"> · {focus.completed} rounds</span>}
        </span>
      </span>
      <span className="timeline">
        <Bars values={data?.hours ?? []} width={240} height={22} />
        {marks.map((m, i) => (
          <span
            key={i}
            className="timeline-mark"
            data-kind={m.kind}
            style={{ left: `${(m.at / 24) * 100}%` }}
          />
        ))}
        <span className="timeline-now" style={{ left: `${(nowAt / 24) * 100}%` }} />
      </span>
    </Card>
  );
}

function secondsLeft(endsAt?: string, paused?: number) {
  if (paused != null) return paused;
  return endsAt ? Math.max(0, Math.round((Date.parse(endsAt) - Date.now()) / 1000)) : 0;
}

const clock = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

/** Pomodoro: focus rounds and breaks, started from the dock. */
export function PomodoroCard() {
  const focus = useWidgets((s) => s.focus);
  useNow(1000, focus.phase !== "idle");
  const left = focus.phase === "idle" ? focus.phaseSeconds : secondsLeft(focus.endsAt, focus.pausedSeconds);
  const running = focus.phase !== "idle" && focus.pausedSeconds == null;
  const done = focus.phaseSeconds ? 100 - (left / focus.phaseSeconds) * 100 : 0;
  return (
    <Card>
      <Ring
        value={focus.phase === "idle" ? 0 : done}
        size={46}
        color={focus.phase === "break" ? "#30d158" : "#ff453a"}
      >
        <Timer />
      </Ring>
      <span className="card-col">
        <span className="card-label card-dim">{focus.phase === "break" ? "Break" : "Focus"}</span>
        <span className="card-big card-tabular">{clock(left)}</span>
      </span>
      <CardButton
        label={running ? "Pause" : "Start"}
        className="card-round"
        onPress={() => (running ? ipc.focusPause() : ipc.focusStart()).catch(console.error)}
      >
        {running ? <Pause /> : <Play />}
      </CardButton>
    </Card>
  );
}

/** Timer: a countdown that starts in one click. */
export function TimerCard() {
  const minutes = useSettings((s) => s.settings?.cards.timerMinutes ?? 10);
  const timer = useTimer();
  useNow(1000, timer.running);
  const left = timer.left(minutes);
  return (
    <Card>
      <Ring value={(left / (minutes * 60)) * 100} size={46} color="#ff9f0a">
        <Hourglass />
      </Ring>
      <span className="card-big card-tabular">{clock(left)}</span>
      <CardButton
        label={timer.running ? "Pause" : "Start"}
        className="card-round"
        onPress={() => timer.toggle(minutes)}
      >
        {timer.running ? <Pause /> : <Play />}
      </CardButton>
      <CardButton label="Reset" className="card-round card-quiet" onPress={timer.reset}>
        <RotateCcw />
      </CardButton>
    </Card>
  );
}

/** Stopwatch: start, pause and lap from the dock. */
export function StopwatchCard() {
  const watch = useStopwatch();
  useNow(100, watch.running);
  const ms = watch.elapsed();
  const text = `${clock(Math.floor(ms / 1000))}.${Math.floor((ms % 1000) / 100)}`;
  return (
    <Card>
      <span className="card-col">
        <span className="card-big card-tabular">{text}</span>
        <span className="card-faint card-small">
          {watch.laps.length ? `${watch.laps.length} laps` : "Stopwatch"}
        </span>
      </span>
      <CardButton
        label={watch.running ? "Lap" : "Reset"}
        className="card-round card-quiet"
        onPress={watch.running ? watch.lap : watch.reset}
      >
        {watch.running ? <span className="card-small">Lap</span> : <RotateCcw />}
      </CardButton>
      <CardButton label={watch.running ? "Pause" : "Start"} className="card-round" onPress={watch.toggle}>
        {watch.running ? <Pause /> : <Play />}
      </CardButton>
    </Card>
  );
}

/** Todos: the next open task with a progress ring; tick it off from the dock. */
export function TodosCard() {
  const todos = useProductivity((s) => s.todos);
  const done = todos.filter((t) => t.done).length;
  const next = todos.find((t) => !t.done);
  return (
    <Card>
      <Ring value={todos.length ? (done / todos.length) * 100 : 0} size={44} color="#30d158">
        <span className="card-small">
          {done}/{todos.length}
        </span>
      </Ring>
      <span className="card-col card-text">
        <span className="card-label card-dim">Next</span>
        <span className="card-line">{next?.text ?? "All done"}</span>
      </span>
      {next && (
        <CardButton
          label="Done"
          className="card-check"
          onPress={() => {
            ipc
              .todoChange({ kind: "setDone", id: next.id, done: true })
              .then((list) => useProductivity.setState({ todos: list }))
              .catch(console.error);
          }}
        >
          ✓
        </CardButton>
      )}
    </Card>
  );
}

/** Notes: the latest note, one click from editing. */
export function NotesCard() {
  const notes = useProductivity((s) => s.notes);
  const latest = [...notes].sort((a, b) => b.updated.localeCompare(a.updated))[0];
  return (
    <Card>
      <IconTile color="#ffd60a">
        <NotebookPen color="#3a2f00" strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">Notes</span>
        <span className="card-line">{latest?.text.split("\n")[0] || "A new note"}</span>
      </span>
    </Card>
  );
}

/** Calendar: the next event and how soon it starts. */
export function CalendarCard() {
  const events = useWidgets((s) => s.calendar?.events) ?? NONE;
  const now = useNow(60_000);
  const next = events.find((e) => Date.parse(e.end) > now);
  const started = next && Date.parse(next.start) <= now;
  const date = new Date(now);
  return (
    <Card>
      <span className="card-page">
        <span>{date.toLocaleDateString(undefined, { weekday: "short" })}</span>
        <strong>{date.getDate()}</strong>
      </span>
      <span className="card-col card-text">
        <span className="card-line card-label">{next?.title ?? "No more events"}</span>
        <span className="card-faint">
          {next ? (started ? "now" : `in ${until(Date.parse(next.start), now)}`) : "Enjoy the day"}
        </span>
      </span>
    </Card>
  );
}

const readRate = () => ipc.currencyRate();

/** Currency: what one unit is worth in the other. */
export function CurrencyCard() {
  const { data, failed } = useLoaded(readRate, 3_600_000);
  return (
    <Card>
      <IconTile color="#30d158">
        <ArrowRightLeft strokeWidth={2.4} />
      </IconTile>
      <span className="card-col">
        <span className="card-label card-dim">{data ? `${data.from} → ${data.to}` : "…"}</span>
        <span className="card-big card-tabular">
          {failed ? "—" : data ? data.rate.toFixed(data.rate >= 100 ? 1 : 3) : "…"}
        </span>
      </span>
    </Card>
  );
}

function Progress({ label, value, sub }: { label: string; value: number; sub: string }) {
  return (
    <Card>
      <Ring value={value} size={46} color="var(--card-accent)">
        <span className="card-small">{Math.floor(value)}%</span>
      </Ring>
      <span className="card-col">
        <span className="card-label">{label}</span>
        <span className="card-faint">{sub}</span>
      </span>
    </Card>
  );
}

/** Day Progress. */
export function DayProgressCard() {
  const now = new Date(useNow(60_000));
  const minutes = now.getHours() * 60 + now.getMinutes();
  return <Progress label="Today" value={(minutes / 1440) * 100} sub={`${duration(1440 - minutes)} left`} />;
}

/** Month Progress. */
export function MonthProgressCard() {
  const now = new Date(useNow(3_600_000));
  const days = new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate();
  const value = ((now.getDate() - 1 + now.getHours() / 24) / days) * 100;
  return (
    <Progress
      label={now.toLocaleDateString(undefined, { month: "long" })}
      value={value}
      sub={`${days - now.getDate()} days left`}
    />
  );
}

/** Year Progress: the year as dots, filling up. */
export function YearProgressCard() {
  const now = new Date(useNow(3_600_000));
  const start = new Date(now.getFullYear(), 0, 1).getTime();
  const end = new Date(now.getFullYear() + 1, 0, 1).getTime();
  const value = ((now.getTime() - start) / (end - start)) * 100;
  // 52 weeks as 4 rows of 13.
  const filled = Math.round((value / 100) * 52);
  return (
    <Card className="card-year">
      <span className="card-col">
        <DotGrid total={52} filled={filled} columns={13} cell={6} />
        <span className="card-row card-between">
          <span className="card-faint">{now.getFullYear()}</span>
          <span className="card-label">{Math.floor(value)}%</span>
        </span>
      </span>
    </Card>
  );
}
