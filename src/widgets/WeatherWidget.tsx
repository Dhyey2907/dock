import {
  Cloud,
  CloudDrizzle,
  CloudFog,
  CloudLightning,
  CloudMoon,
  CloudRain,
  CloudSnow,
  CloudSun,
  MapPinOff,
  Moon,
  Sun,
  Sunrise,
  Sunset,
  type LucideIcon,
} from "lucide-react";

import { ipc } from "../core/ipc";
import { formatTemperature } from "./logic";
import { useWidgets } from "./store";

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

function WeatherIcon({ name, size }: { name?: string; size: number }) {
  const Icon = (name && ICONS[name]) || Cloud;
  return <Icon size={size} strokeWidth={1.8} aria-hidden="true" />;
}

const locale = navigator.language || "en";

function time(iso?: string) {
  if (!iso) return "—";
  return new Date(iso).toLocaleTimeString(locale, { hour: "numeric", minute: "2-digit" });
}

export function WeatherTile() {
  const weather = useWidgets((s) => s.weather);
  if (!weather || weather.status !== "ready") {
    return <WeatherIcon name={weather ? "cloud" : undefined} size={18} />;
  }
  return (
    <span className="tile-row">
      <WeatherIcon name={weather.icon} size={18} />
      <span className="tile-strong">{formatTemperature(weather.temperature, weather.unit)}</span>
    </span>
  );
}

export function WeatherPanel() {
  const weather = useWidgets((s) => s.weather);
  if (!weather) return <div className="panel-body panel-empty">Getting the weather…</div>;
  if (weather.status === "noAccess") {
    return (
      <div className="panel-body panel-empty">
        <MapPinOff size={28} strokeWidth={1.6} />
        <p>Windows location is off, so the weather doesn't know where you are.</p>
        <button className="panel-button" onClick={() => ipc.openWindowsSettings("location")}>
          Turn on location
        </button>
        <p className="panel-faint">Or type a town under Widgets in Dock settings.</p>
      </div>
    );
  }
  if (weather.status === "placeNotFound") {
    return (
      <div className="panel-body panel-empty">
        <p>That town wasn't found. Check its spelling under Widgets in Dock settings.</p>
      </div>
    );
  }
  if (weather.status === "unavailable") {
    return (
      <div className="panel-body panel-empty">
        <p>The weather can't be reached right now. It will try again shortly.</p>
      </div>
    );
  }
  const today = weather.days[0];
  return (
    <div className="panel-body">
      <div className="weather-now">
        <WeatherIcon name={weather.icon} size={44} />
        <div>
          <div className="panel-big">{formatTemperature(weather.temperature, weather.unit)}</div>
          <div className="panel-faint">
            {weather.condition}
            {weather.place ? ` · ${weather.place}` : ""}
          </div>
        </div>
      </div>
      <div className="weather-facts">
        {today && (
          <span>
            H {formatTemperature(today.high, weather.unit)} · L {formatTemperature(today.low, weather.unit)}
          </span>
        )}
        <span>
          <Sunrise size={14} aria-label="Sunrise" /> {time(weather.sunrise)}
        </span>
        <span>
          <Sunset size={14} aria-label="Sunset" /> {time(weather.sunset)}
        </span>
      </div>
      <ul className="weather-days">
        {weather.days.slice(1, 6).map((day) => (
          <li key={day.date}>
            <span>{new Date(`${day.date}T12:00`).toLocaleDateString(locale, { weekday: "short" })}</span>
            <WeatherIcon name={day.icon} size={18} />
            <span className="weather-range">
              {formatTemperature(day.high, weather.unit)}
              <span className="panel-faint"> {formatTemperature(day.low, weather.unit)}</span>
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
