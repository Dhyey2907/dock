/** "1h 38m", "45m". */
export function duration(minutes: number): string {
  const m = Math.max(0, Math.round(minutes));
  const h = Math.floor(m / 60);
  return h ? `${h}h ${m % 60}m` : `${m}m`;
}

/** Time until `at` (ms since epoch), "2h 12m" or "4d 3h". */
export function until(at: number, now = Date.now()): string {
  const minutes = Math.max(0, Math.round((at - now) / 60_000));
  if (minutes >= 48 * 60) return `${Math.floor(minutes / 1440)}d ${Math.floor((minutes % 1440) / 60)}h`;
  return duration(minutes);
}

/** "150 KB/s". */
export function rate(bytesPerSecond: number): string {
  const units = ["B/s", "KB/s", "MB/s", "GB/s"];
  let v = bytesPerSecond;
  let u = 0;
  while (v >= 1000 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  return `${v >= 100 || u === 0 ? Math.round(v) : v.toFixed(1)} ${units[u]}`;
}

/** "12.4 GB". */
export function bytes(n: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = n;
  let u = 0;
  while (v >= 1024 && u < units.length - 1) {
    v /= 1024;
    u += 1;
  }
  return `${v >= 100 || u === 0 ? Math.round(v) : v.toFixed(1)} ${units[u]}`;
}
