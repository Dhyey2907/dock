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
  MapPinOff,
  Moon,
  Sun,
  Wind,
  type LucideIcon,
} from "lucide-react";

import type { Weather } from "../../core/generated/Weather";
import { useWidgets } from "../store";
import { useNow } from "../useNow";
import { Card } from "./parts";

/** One shared empty list, so a store read without data stays the same between renders. */
const NONE: never[] = [];

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

/** Why there's no weather to show, in a few words for a card (the popup says more). */
const NOTICE: Record<Exclude<Weather["status"], "ready">, string> = {
  noAccess: "Location is off",
  placeNotFound: "Town not found",
  unavailable: "Weather offline",
};

/** The weather when it's ready; otherwise `undefined`, and the card shows `WeatherNotice`. */
function useReadyWeather() {
  const weather = useWidgets((s) => s.weather);
  return { weather: weather?.status === "ready" ? weather : undefined, raw: weather };
}

/** A card's stand-in when there's no weather yet, or it can't be had: says why. */
function WeatherNotice({ weather }: { weather?: Weather }) {
  const reason = !weather ? "Getting the weather…" : weather.status === "ready" ? "" : NOTICE[weather.status];
  return (
    <Card>
      {weather?.status === "noAccess" ? (
        <MapPinOff className="card-faint" />
      ) : (
        <Cloud className="card-faint" />
      )}
      <span className="card-col">
        <span className="card-label">Weather</span>
        <span className="card-faint">{reason}</span>
      </span>
    </Card>
  );
}

const weekday = (iso: string) => new Date(`${iso}T12:00`).toLocaleDateString(undefined, { weekday: "short" });
const deg = (n?: number) => (n == null ? "—" : `${n}°`);

/** Weather: the temperature now, with today's high and low. */
export function WeatherCard() {
  const { weather, raw } = useReadyWeather();
  if (!weather) return <WeatherNotice weather={raw} />;
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
  const { weather, raw } = useReadyWeather();
  if (!weather) return <WeatherNotice weather={raw} />;
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
  const { weather, raw } = useReadyWeather();
  const days = weather?.days ?? NONE;
  if (days.length < 2) return <WeatherNotice weather={raw} />;
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
  const { weather, raw } = useReadyWeather();
  if (!weather) return <WeatherNotice weather={raw} />;
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
  const { weather, raw } = useReadyWeather();
  if (!weather) return <WeatherNotice weather={raw} />;
  return (
    <Card>
      <Icon name={weather?.icon} className="card-weather-big" />
      <span className="card-col">
        <span className="card-label">{weather.condition ?? "—"}</span>
        <span className="card-faint">{weather.place ?? "Your area"}</span>
      </span>
    </Card>
  );
}

/** Hourly: the next six hours. */
export function HourlyCard() {
  const { weather, raw } = useReadyWeather();
  const hours = weather?.hours ?? NONE;
  if (!weather) return <WeatherNotice weather={raw} />;
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
  const { weather, raw } = useReadyWeather();
  const now = useNow(60_000);
  if (!weather?.sunrise || !weather.sunset) return <WeatherNotice weather={raw} />;
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
