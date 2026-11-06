import type { DockSize } from "../core/generated/DockSize";
import type { Settings } from "../core/generated/Settings";

/** Bar height in logical pixels for each size choice (matches `DockSize::height` in Rust). */
const BASE_HEIGHT: Record<DockSize, number> = { small: 48, medium: 56, large: 64 };

/** The bar's height in logical pixels: the size choice scaled by the size slider. */
export function dockHeight(settings: Pick<Settings, "size" | "look">): number {
  return Math.round((BASE_HEIGHT[settings.size] * settings.look.scale) / 100);
}

/**
 * How much the dock has room to show. A big dock ("roomy") adds a second line or extra
 * numbers to its widgets; a small one keeps to the essentials.
 */
export type Room = "compact" | "normal" | "roomy";

export function roomFor(settings: Pick<Settings, "size" | "look">): Room {
  const height = dockHeight(settings);
  if (height < 50) return "compact";
  if (height >= 70) return "roomy";
  return "normal";
}

/** A spring-like easing whose overshoot grows with `bounce` (0 to 100). */
export function springEasing(bounce: number): string {
  const overshoot = 1 + (Math.min(100, Math.max(0, bounce)) / 100) * 0.9;
  return `cubic-bezier(0.34, ${overshoot.toFixed(2)}, 0.64, 1)`;
}

/** CSS variables the dock's stylesheet reads for the size slider and the other look settings. */
export function lookStyle(settings: Pick<Settings, "size" | "look">, shrink = 1): Record<string, string> {
  const { look } = settings;
  const scale = (dockHeight(settings) / BASE_HEIGHT[settings.size]) * shrink;
  return {
    "--scale": scale.toFixed(3),
    "--tscale": Math.min(1.4, Math.max(0.9, scale)).toFixed(3),
    "--dock-h": `${dockHeight(settings)}px`,
    "--corners": (look.corners / 100).toFixed(2),
    "--glass-opacity": (1 - look.transparency / 100).toFixed(2),
    "--blur-scale": (look.blur / 100).toFixed(2),
    "--widget-bg": (look.widgetBackground / 100).toFixed(2),
    "--spring": springEasing(look.bounce),
  };
}

/** Narrowest the dock shrinks to when it doesn't fit, as a fraction of its size. */
const MIN_SHRINK = 0.6;

/**
 * The shrink that makes content `width` pixels wide (drawn at `current` shrink) fit a screen
 * `screenWidth` wide, with a little room to spare. Small changes are ignored so it settles.
 */
export function fitShrink(current: number, width: number, screenWidth: number): number {
  if (width <= 0 || screenWidth <= 0) return current;
  const natural = width / current;
  const room = screenWidth - 24;
  const wanted = Math.max(MIN_SHRINK, Math.min(1, room / natural));
  const stepped = Math.floor(wanted * 50) / 50;
  return Math.abs(stepped - current) < 0.02 ? current : stepped;
}
