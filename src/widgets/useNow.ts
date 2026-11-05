import { useEffect, useState } from "react";

/**
 * The current time, re-read every `period` ms while `active`, lined up with the period's
 * boundary (a minute clock ticks on the minute, not 40 s into it).
 */
export function useNow(period: number, active = true): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    let timer: number | undefined;
    const tick = () => {
      const t = Date.now();
      setNow(t);
      timer = window.setTimeout(tick, period - (t % period) + 5);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, [period, active]);
  return now;
}
