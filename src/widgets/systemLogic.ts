/** "3 d 4 h", "5 h 12 min", "8 min": how long the PC has been on. */
export function formatUptime(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60);
  const days = Math.floor(minutes / (24 * 60));
  const hours = Math.floor((minutes % (24 * 60)) / 60);
  const mins = minutes % 60;
  if (days > 0) return hours ? `${days} d ${hours} h` : `${days} d`;
  if (hours > 0) return mins ? `${hours} h ${mins} min` : `${hours} h`;
  return `${mins} min`;
}

/** The short form for a dock tile: "3d", "5h", "8m". */
export function shortUptime(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60);
  if (minutes >= 24 * 60) return `${Math.floor(minutes / (24 * 60))}d`;
  if (minutes >= 60) return `${Math.floor(minutes / 60)}h`;
  return `${minutes}m`;
}

/** "512 bytes", "1.5 KB", "4.2 GB". */
export function formatBytes(bytes: number): string {
  const units = ["KB", "MB", "GB", "TB"];
  if (bytes < 1024) return `${Math.max(0, Math.round(bytes))} bytes`;
  let value = bytes;
  let unit = -1;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  return `${value >= 100 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

/** "12.3k" for big star counts. */
export function compactCount(n: number): string {
  if (n < 1000) return String(n);
  if (n < 100_000) return `${(n / 1000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${Math.round(n / 1000)}k`;
}

/** "Excellent", "Good", "Fair" or "Weak" for a 0 to 100 Wi-Fi signal. */
export function signalWord(percent: number): string {
  if (percent >= 80) return "Excellent";
  if (percent >= 60) return "Good";
  if (percent >= 40) return "Fair";
  return "Weak";
}

/** How full a drive is, 0 to 100. */
export function usedPercent(total: number, free: number): number {
  if (total <= 0) return 0;
  return Math.min(100, Math.max(0, Math.round(((total - free) / total) * 100)));
}

/** The common wallpaper intervals, in minutes (0 = only when you click). */
export const WALLPAPER_INTERVALS = [0, 5, 15, 30, 60, 180, 1440];

export function intervalLabel(minutes: number): string {
  if (minutes === 0) return "Only when I click";
  if (minutes < 60) return `Every ${minutes} minutes`;
  if (minutes === 60) return "Every hour";
  if (minutes === 1440) return "Every day";
  return `Every ${minutes / 60} hours`;
}

/** Send the newest value, one call at a time, so dragging a slider never queues up calls. */
export function latestOnly<T>(send: (value: T) => Promise<unknown>) {
  let busy = false;
  let next: { value: T } | null = null;
  const run = (value: T) => {
    busy = true;
    send(value)
      .catch((error) => console.error(error))
      .finally(() => {
        busy = false;
        if (next) {
          const { value } = next;
          next = null;
          run(value);
        }
      });
  };
  return (value: T) => {
    if (busy) next = { value };
    else run(value);
  };
}
