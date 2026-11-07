import { useSettings } from "../../settings/store";
import { useWidgets } from "../store";
import { useNow } from "../useNow";
import { Card } from "./parts";

/** Hours, minutes and seconds in `zone` (the PC's own when left out). */
function partsIn(now: number, zone?: string) {
  const fmt = new Intl.DateTimeFormat("en-GB", {
    timeZone: zone,
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  });
  const get = (type: string) => Number(fmt.formatToParts(now).find((p) => p.type === type)?.value ?? 0);
  return { h: get("hour"), m: get("minute"), s: get("second") };
}

/** A city's name from its time zone ("America/New_York" -> "New York"). */
const city = (zone: string) => zone.split("/").pop()?.replace(/_/g, " ") ?? zone;

/** Hours ahead of (or behind) here, "+9:30" style. */
function offset(now: number, zone: string) {
  const there = partsIn(now, zone);
  const here = partsIn(now);
  let diff = there.h * 60 + there.m - (here.h * 60 + here.m);
  if (diff > 12 * 60) diff -= 24 * 60;
  if (diff < -12 * 60) diff += 24 * 60;
  const sign = diff >= 0 ? "+" : "−";
  const abs = Math.abs(diff);
  return `${sign}${Math.floor(abs / 60)}${abs % 60 ? `:${String(abs % 60).padStart(2, "0")}` : ""}h`;
}

function use24h() {
  return useSettings((s) => s.settings?.clock24h);
}

/** "5:49" and "PM" in the user's 12/24-hour choice. */
function hm(h: number, m: number, h24: boolean | undefined) {
  const twentyFour = h24 ?? !new Intl.DateTimeFormat(undefined, { hour: "numeric" }).resolvedOptions().hour12;
  if (twentyFour) return { time: `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`, half: "" };
  return { time: `${h % 12 || 12}:${String(m).padStart(2, "0")}`, half: h < 12 ? "AM" : "PM" };
}

function Face({
  zone,
  size = 52,
  dark = false,
  ticks = "numbers",
  seconds = true,
}: {
  zone?: string;
  size?: number;
  dark?: boolean;
  ticks?: "numbers" | "lines";
  seconds?: boolean;
}) {
  const now = useNow(seconds ? 1000 : 30_000);
  const { h, m, s } = partsIn(now, zone);
  const hand = (deg: number, length: number, width: number, cls: string) => (
    <line
      x1="50"
      y1="50"
      x2={50 + Math.sin((deg * Math.PI) / 180) * length}
      y2={50 - Math.cos((deg * Math.PI) / 180) * length}
      strokeWidth={width}
      strokeLinecap="round"
      className={cls}
    />
  );
  return (
    <svg
      viewBox="0 0 100 100"
      className="card-face"
      data-dark={dark}
      style={{ width: `calc(${size} * var(--cu))`, height: `calc(${size} * var(--cu))` }}
      aria-hidden="true"
    >
      <circle cx="50" cy="50" r="48" className="face-dial" />
      {Array.from({ length: 12 }, (_, i) => {
        const a = (i * 30 * Math.PI) / 180;
        if (ticks === "numbers") {
          return (
            <text
              key={i}
              x={50 + Math.sin(a) * 36}
              y={50 - Math.cos(a) * 36 + 5}
              textAnchor="middle"
              className="face-number"
            >
              {i || 12}
            </text>
          );
        }
        return (
          <line
            key={i}
            x1={50 + Math.sin(a) * 40}
            y1={50 - Math.cos(a) * 40}
            x2={50 + Math.sin(a) * 45}
            y2={50 - Math.cos(a) * 45}
            className="face-tick"
            strokeWidth={i % 3 ? 2 : 3.5}
          />
        );
      })}
      {hand(((h % 12) + m / 60) * 30, 24, 5, "face-hand")}
      {hand((m + s / 60) * 6, 34, 3.5, "face-hand")}
      {seconds && hand(s * 6, 38, 1.5, "face-second")}
      <circle cx="50" cy="50" r="3" className="face-pin" />
    </svg>
  );
}

/** Clock: large time with its seconds and the date. */
export function BigClockCard() {
  const now = useNow(1000);
  const { h, m, s } = partsIn(now);
  const { time, half } = hm(h, m, use24h());
  return (
    <Card>
      <span className="card-col">
        <span className="card-huge card-tabular">
          {time}
          <span className="card-seconds">:{String(s).padStart(2, "0")}</span>
          {half && <span className="card-half">{half}</span>}
        </span>
        <span className="card-faint">
          {new Date(now).toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" })}
        </span>
      </span>
    </Card>
  );
}

/** Flip Clock: split-flap digits. */
export function FlipClockCard() {
  const now = useNow(1000);
  const { h, m } = partsIn(now);
  const { time, half } = hm(h, m, use24h());
  const digits = time.replace(":", "").padStart(4, "0").split("");
  return (
    <Card className="card-flip">
      <span className="flip-pair">
        <span className="flip">{digits[0]}</span>
        <span className="flip">{digits[1]}</span>
      </span>
      <span className="flip-pair">
        <span className="flip">{digits[2]}</span>
        <span className="flip">{digits[3]}</span>
      </span>
      {half && <span className="card-half card-faint">{half}</span>}
    </Card>
  );
}

function useZones() {
  return useSettings((s) => s.settings?.cards.worldClocks ?? []);
}

/** World Clocks: three small analog faces. */
export function WorldClocksCard() {
  const zones = useZones();
  return (
    <Card>
      {zones.map((zone) => (
        <span key={zone} className="card-col card-center">
          <Face zone={zone} size={40} seconds={false} ticks="lines" />
          <span className="card-faint card-small">{city(zone)}</span>
        </span>
      ))}
    </Card>
  );
}

/** Analog Classic: a white face with a sweeping second hand. */
export function AnalogClassicCard() {
  return (
    <Card className="card-square">
      <Face size={56} />
    </Card>
  );
}

/** Analog Classic Dark. */
export function AnalogDarkCard() {
  return (
    <Card className="card-square">
      <Face size={56} dark />
    </Card>
  );
}

/** Analog Minimal: hands and hour ticks, nothing else. */
export function AnalogMinimalCard() {
  return (
    <Card className="card-square">
      <Face size={56} dark ticks="lines" seconds={false} />
    </Card>
  );
}

/** 7 Days: this week as a strip, today lit, with today's temperature. */
export function SevenDaysCard() {
  const now = new Date(useNow(60_000));
  const temperature = useWidgets((s) => s.weather?.temperature);
  const start = new Date(now);
  start.setDate(now.getDate() - now.getDay());
  return (
    <Card className="card-week">
      {Array.from({ length: 7 }, (_, i) => {
        const d = new Date(start);
        d.setDate(start.getDate() + i);
        const today = d.toDateString() === now.toDateString();
        return (
          <span key={i} className="week-day" data-today={today}>
            <span className="card-faint card-small">
              {d.toLocaleDateString(undefined, { weekday: "narrow" })}
            </span>
            <span className="card-label">{d.getDate()}</span>
          </span>
        );
      })}
      {temperature != null && <span className="card-label week-temp">{temperature}°</span>}
    </Card>
  );
}

/** World Clock: three cities in a list with their hour difference. */
export function WorldClockListCard() {
  const zones = useZones();
  const now = useNow(30_000);
  const h24 = use24h();
  return (
    <Card>
      <span className="card-col card-list">
        {zones.map((zone) => {
          const { h, m } = partsIn(now, zone);
          const { time, half } = hm(h, m, h24);
          return (
            <span key={zone} className="card-row card-between card-list-row">
              <span className="card-small">{city(zone)}</span>
              <span className="card-faint card-small">{offset(now, zone)}</span>
              <span className="card-small card-tabular">
                {time} {half}
              </span>
            </span>
          );
        })}
      </span>
    </Card>
  );
}

/** World Time: one more city in big digits. */
export function WorldTimeCard() {
  const zone = useZones()[0] ?? "UTC";
  const now = useNow(30_000);
  const { h, m } = partsIn(now, zone);
  const { time, half } = hm(h, m, use24h());
  return (
    <Card>
      <span className="card-col">
        <span className="card-faint">
          {city(zone)} · {offset(now, zone)}
        </span>
        <span className="card-huge card-tabular">
          {time}
          {half && <span className="card-half">{half}</span>}
        </span>
      </span>
    </Card>
  );
}

/** Today: weekday, date, time and weather in one card. */
export function TodayCard() {
  const now = useNow(30_000);
  const { h, m } = partsIn(now);
  const { time, half } = hm(h, m, use24h());
  const weather = useWidgets((s) => s.weather);
  const date = new Date(now);
  return (
    <Card>
      <span className="card-col">
        <span className="card-label card-red">{date.toLocaleDateString(undefined, { weekday: "long" })}</span>
        <span className="card-big card-tabular">
          {time}
          {half && <span className="card-half">{half}</span>}
        </span>
      </span>
      <span className="card-col card-end">
        <span className="card-label">
          {date.toLocaleDateString(undefined, { day: "numeric", month: "short" })}
        </span>
        <span className="card-faint">{weather?.temperature != null ? `${weather.temperature}°` : ""}</span>
      </span>
    </Card>
  );
}

/** Date: the full date, set large. */
export function DateCard() {
  const date = new Date(useNow(60_000));
  return (
    <Card>
      <span className="card-col">
        <span className="card-faint">{date.toLocaleDateString(undefined, { weekday: "long" })}</span>
        <span className="card-big">
          {date.toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" })}
        </span>
      </span>
    </Card>
  );
}

/** Day Number: today as a calendar page. */
export function DayNumberCard() {
  const date = new Date(useNow(60_000));
  return (
    <Card className="card-square card-page-card">
      <span className="page-top">{date.toLocaleDateString(undefined, { month: "short" }).toUpperCase()}</span>
      <span className="page-day">{date.getDate()}</span>
    </Card>
  );
}

const SEGMENTS: Record<string, string> = {
  "0": "abcdef",
  "1": "bc",
  "2": "abdeg",
  "3": "abcdg",
  "4": "bcfg",
  "5": "acdfg",
  "6": "acdefg",
  "7": "abc",
  "8": "abcdefg",
  "9": "abcdfg",
};

function Digit({ value }: { value: string }) {
  const on = SEGMENTS[value] ?? "";
  const seg = (name: string, d: string) => <path key={name} d={d} data-on={on.includes(name)} />;
  return (
    <svg viewBox="0 0 20 34" className="segment" aria-hidden="true">
      {seg("a", "M4,2 L16,2 L14,4 L6,4 Z")}
      {seg("b", "M17,3 L17,16 L15,15 L15,5 Z")}
      {seg("c", "M17,18 L17,31 L15,29 L15,19 Z")}
      {seg("d", "M4,32 L16,32 L14,30 L6,30 Z")}
      {seg("e", "M3,18 L3,31 L5,29 L5,19 Z")}
      {seg("f", "M3,3 L3,16 L5,15 L5,5 Z")}
      {seg("g", "M5,16 L15,16 L16,17 L15,18 L5,18 L4,17 Z")}
    </svg>
  );
}

/** Segment Clock: the time as a seven-segment display, under the weekday and date. */
export function SegmentClockCard() {
  const now = useNow(1000);
  const { h, m } = partsIn(now);
  const { time } = hm(h, m, use24h());
  const digits = time.replace(":", "").padStart(4, "0").split("");
  const colon = Math.floor(now / 1000) % 2 === 0;
  return (
    <Card>
      <span className="card-col">
        <span className="card-faint card-small">
          {new Date(now).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })}
        </span>
        <span className="segments">
          <Digit value={digits[0]} />
          <Digit value={digits[1]} />
          <span className="segment-colon" data-on={colon}>
            :
          </span>
          <Digit value={digits[2]} />
          <Digit value={digits[3]} />
        </span>
      </span>
    </Card>
  );
}

/** Clock & Weather: time, date and temperature as three colour blocks. */
export function ClockWeatherCard() {
  const now = useNow(30_000);
  const { h, m } = partsIn(now);
  const { time, half } = hm(h, m, use24h());
  const temperature = useWidgets((s) => s.weather?.temperature);
  const date = new Date(now);
  return (
    <Card className="card-square card-blocks">
      <span className="block-time">
        {time}
        {half && <small>{half}</small>}
      </span>
      <span className="block-date">
        <small>{date.toLocaleDateString(undefined, { weekday: "short" })}</small>
        {date.getDate()}
      </span>
      <span className="block-temp">{temperature != null ? `${temperature}°` : "—"}</span>
    </Card>
  );
}
