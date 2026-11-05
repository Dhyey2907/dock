import { useMemo, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";

import { useSettings } from "../settings/store";
import { localeFirstDay, monthGrid } from "./logic";
import { useNow } from "./useNow";

const locale = navigator.language || "en";

function useTimeFormat() {
  const clock24h = useSettings((s) => s.settings?.clock24h);
  return useMemo(
    () =>
      new Intl.DateTimeFormat(locale, {
        hour: "numeric",
        minute: "2-digit",
        ...(clock24h == null ? {} : { hourCycle: clock24h ? "h23" : "h12" }),
      }),
    [clock24h],
  );
}

export function ClockTile() {
  const now = useNow(60_000);
  const time = useTimeFormat();
  const date = new Date(now);
  return (
    <span className="tile-stack">
      <span className="tile-strong">{time.format(date)}</span>
      <span className="tile-faint">
        {date.toLocaleDateString(locale, { weekday: "short", day: "numeric", month: "short" })}
      </span>
    </span>
  );
}

export function ClockPanel() {
  const now = new Date(useNow(1000));
  const time = useTimeFormat();
  const [shown, setShown] = useState(() => ({ year: now.getFullYear(), month: now.getMonth() }));
  const firstDay = localeFirstDay(locale);
  const days = monthGrid(shown.year, shown.month, firstDay);
  const weekdays = days.slice(0, 7).map((d) => d.toLocaleDateString(locale, { weekday: "narrow" }));
  const title = new Date(shown.year, shown.month, 1).toLocaleDateString(locale, {
    month: "long",
    year: "numeric",
  });
  const move = (by: number) =>
    setShown((s) => {
      const d = new Date(s.year, s.month + by, 1);
      return { year: d.getFullYear(), month: d.getMonth() };
    });
  const isToday = (d: Date) => d.toDateString() === now.toDateString();

  return (
    <div className="panel-body">
      <div className="clock-now">
        <div className="panel-big">{time.format(now)}</div>
        <div className="panel-faint">
          {now.toLocaleDateString(locale, { weekday: "long", day: "numeric", month: "long" })}
        </div>
      </div>
      <div className="month">
        <div className="month-head">
          <span className="month-title">{title}</span>
          <button className="icon-btn" aria-label="Previous month" onClick={() => move(-1)}>
            <ChevronLeft size={16} />
          </button>
          <button className="icon-btn" aria-label="Next month" onClick={() => move(1)}>
            <ChevronRight size={16} />
          </button>
        </div>
        <div className="month-grid" role="grid">
          {weekdays.map((w, i) => (
            <span key={`w${i}`} className="month-weekday">
              {w}
            </span>
          ))}
          {days.map((d) => (
            <span
              key={d.toISOString()}
              className="month-day"
              data-other={d.getMonth() !== shown.month}
              data-today={isToday(d)}
            >
              {d.getDate()}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
