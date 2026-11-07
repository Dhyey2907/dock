/**
 * Windows 11's own icons (the Segoe Fluent Icons font that ships with Windows), so the dock's
 * system buttons look like the taskbar's rather than a web app's.
 */
export const GLYPH = {
  search: 0xe721,
  taskView: 0xe7c4,
  chevronUp: 0xe70e,
  wifi: 0xe701,
  wifi1: 0xe872,
  wifi2: 0xe873,
  wifi3: 0xe874,
  ethernet: 0xe839,
  offline: 0xf384,
  volume0: 0xe992,
  volume1: 0xe993,
  volume2: 0xe994,
  volume3: 0xe995,
  mute: 0xe74f,
  bell: 0xea8f,
  quiet: 0xe708,
  settings: 0xe713,
} as const;

export type GlyphName = keyof typeof GLYPH;

/** Battery glyphs: Battery0 to Battery9, then Battery10 (full); the same for charging. */
const BATTERY = [0xe850, 0xe851, 0xe852, 0xe853, 0xe854, 0xe855, 0xe856, 0xe857, 0xe858, 0xe859, 0xe83f];
const CHARGING = [0xe85a, 0xe85b, 0xe85c, 0xe85d, 0xe85e, 0xe85f, 0xe860, 0xe861, 0xe862, 0xe83e, 0xea93];

export function batteryGlyph(percent: number, charging: boolean): number {
  const step = Math.max(0, Math.min(10, Math.round(percent / 10)));
  return (charging ? CHARGING : BATTERY)[step];
}
