import {
  Cloud,
  CloudDrizzle,
  CloudFog,
  CloudLightning,
  CloudMoon,
  CloudRain,
  CloudSnow,
  CloudSun,
  Droplets,
  Moon,
  Sun,
  Wind,
  type LucideIcon,
} from "lucide-react";

import { useWidgets } from "../store";
import { useNow } from "../useNow";
import { Card } from "./parts";

const ICONS: Record<string, LucideIcon> = {
  sun: Sun,
  moon: Moon,
  "cloud-sun": CloudSun,
  "cloud-moon": CloudMoon,
  cloud: Cloud,
  fog: CloudFog,
  drizzle: CloudDrizzle,
  rain: CloudRain,
  snow: CloudSnow,
  storm: CloudLightning,
};

/** Colour for each condition, the way weather apps tint them. */
const TINT: Record<string, string> = {
  sun: "#ffd60a",
  moon: "#d0d6ff",
  "cloud-sun": "#ffd60a",
  "cloud-moon": "#d0d6ff",
  cloud: "#c7c7cc",
  fog: "#aeaeb2",
  drizzle: "#64d2ff",
  rain: "#0a84ff",
  snow: "#ffffff",
  storm: "#bf5af2",
};

function Icon({ name, className = "" }: { name?: string; className?: string }) {
  const Glyph = (name && ICONS[name]) || Cloud;
  return (
    <Glyph
      className={`card-weather-icon ${className}`}
      style={{ color: TINT[name ?? "cloud"] }}
      aria-hidden="true"
    />
  );
}

const weekday = (iso: string) => new Date(`${iso}T12:00`).toLocaleDateString(undefined, { weekday: "short" });
const deg = (n?: number) => (n == null ? "—" : `${n}°`);

/** Weather: the temperature now, with today's high and low. */
export function WeatherCard() {
  const weather = useWidgets((s) => s.weather);
  const today = weather?.days[0];
  return (
    <Card>
      <Icon name={weather?.icon} className="card-weather-big" />
      <span className="card-col">
        <span className="card-big">{deg(weather?.temperature)}</span>
        <span className="card-faint">
          H {deg(today?.high)} · L {deg(today?.low)}
        </span>
      </span>
    </Card>
  );
}

/** Weather Details: wind, rain and the day's range. */
export function WeatherDetailsCard() {
  const weather = useWidgets((s) => s.weather);
  const today = weather?.days[0];
  return (
    <Card>
      <span className="card-col card-facts">
        <span className="card-row">
          <Wind className="card-sky" />
          <span className="card-label">{weather?.windKmh != null ? `${weather.windKmh} km/h` : "—"}</span>
        </span>
        <span className="card-row">
          <Droplets className="card-blue" />
          <span className="card-label">{weather?.rainChance != null ? `${weather.rainChance}%` : "—"}</span>
        </span>
      </span>
      <span className="card-col">
        <span className="card-faint">Range</span>
        <span className="card-label">
          {deg(today?.low)} – {deg(today?.high)}
        </span>
      </span>
    </Card>
  );
}

function Day({ date, icon, high, low }: { date: string; icon: string; high: number; low: number }) {
  return (
    <span className="card-day">
      <span className="card-faint">{weekday(date)}</span>
      <Icon name={icon} />
      <span className="card-small">
        {high}° <span className="card-faint">{low}°</span>
      </span>
    </span>
  );
}

/** Weather Forecast: the next four days. */
export function WeatherForecastCard() {
  const days = useWidgets((s) => s.weather?.days ?? []);
  return (
    <Card className="card-days">
      {days.slice(1, 5).map((d) => (
        <Day key={d.date} {...d} />
      ))}
    </Card>
  );
}

/** Today & Forecast: today next to the next three days. */
export function TodayForecastCard() {
  const weather = useWidgets((s) => s.weather);
  return (
    <Card className="card-days">
      <span className="card-col card-today">
        <span className="card-big">{deg(weather?.temperature)}</span>
        <Icon name={weather?.icon} />
      </span>
      <span className="card-divider" />
      {(weather?.days ?? []).slice(1, 4).map((d) => (
        <Day key={d.date} {...d} />
      ))}
    </Card>
  );
}

/** Weather Status: the conditions in words, with your town. */
export function WeatherStatusCard() {
  const weather = useWidgets((s) => s.weather);
  return (
    <Card>
      <Icon name={weather?.icon} className="card-weather-big" />
      <span className="card-col">
        <span className="card-label">{weather?.condition ?? "—"}</span>
        <span className="card-faint">{weather?.place ?? "Here"}</span>
      </span>
    </Card>
  );
}

/** Hourly: the next six hours. */
export function HourlyCard() {
  const hours = useWidgets((s) => s.weather?.hours ?? []);
  return (
    <Card className="card-days">
      {hours.map((h) => (
        <span key={h.time} className="card-day">
          <span className="card-faint">
            {new Date(h.time).toLocaleTimeString(undefined, { hour: "numeric" })}
          </span>
          <Icon name={h.icon} />
          <span className="card-small">{h.temperature}°</span>
        </span>
      ))}
      {hours.length === 0 && <span className="card-faint">No hourly forecast yet</span>}
    </Card>
  );
}

/** Sunrise & Sunset: the sun on its arc between the two. */
export function SunriseSunsetCard() {
  const weather = useWidgets((s) => s.weather);
  const now = useNow(60_000);
  const rise = weather?.sunrise ? Date.parse(weather.sunrise) : NaN;
  const set = weather?.sunset ? Date.parse(weather.sunset) : NaN;
  const t =
    Number.isFinite(rise) && Number.isFinite(set) ? Math.min(1, Math.max(0, (now - rise) / (set - rise))) : 0;
  const up = now >= rise && now <= set;
  // The sun on a half-ellipse from (8,34) to (92,34).
  const x = 8 + t * 84;
  const y = 34 - Math.sin(t * Math.PI) * 28;
  const at = (ms: number) =>
    Number.isFinite(ms)
      ? new Date(ms).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
      : "—";
  return (
    <Card className="card-sun">
      <svg viewBox="0 0 100 40" className="card-sun-arc" aria-hidden="true">
        <path
          d="M8,34 A42,28 0 0 1 92,34"
          fill="none"
          stroke="currentColor"
          strokeOpacity="0.25"
          strokeDasharray="2 3"
        />
        <line x1="2" y1="34" x2="98" y2="34" stroke="currentColor" strokeOpacity="0.2" />
        <circle cx={x} cy={y} r="4.5" fill={up ? "#ffd60a" : "#8e8e93"} />
      </svg>
      <span className="card-row card-between card-sun-times">
        <span className="card-faint">{at(rise)}</span>
        <span className="card-faint">{at(set)}</span>
      </span>
    </Card>
  );
}
