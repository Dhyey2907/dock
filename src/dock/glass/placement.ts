import type { DockGeometry } from "../../core/generated/DockGeometry";
import type { GlassFrame } from "../../core/generated/GlassFrame";

/** Where the glass sits inside the dock window, in CSS pixels. */
export interface GlassBox {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Wallpaper brighter than this behind the glass gets dark text. */
export const LIGHT_BACKGROUND = 0.62;

/** The desktop picture's size and offset, in CSS pixels, so that the part of the screen behind
 *  the glass shows through it. `margin` is how far the picture extends past the glass. */
export function pictureLayout(geometry: DockGeometry, box: GlassBox, margin: number) {
  const s = geometry.scale || 1;
  return {
    width: geometry.screenWidth / s,
    height: geometry.screenHeight / s,
    x: -((geometry.x - geometry.screenX) / s + box.left) + margin,
    y: -((geometry.y - geometry.screenY) / s + box.top) + margin,
  };
}

/** Average brightness (0–1) of the screen behind the glass, from the picture's coarse grid. */
export function brightnessBehind(frame: GlassFrame, geometry: DockGeometry, box: GlassBox): number {
  const s = geometry.scale || 1;
  const left = (geometry.x - geometry.screenX + box.left * s) / geometry.screenWidth;
  const top = (geometry.y - geometry.screenY + box.top * s) / geometry.screenHeight;
  const right = left + (box.width * s) / geometry.screenWidth;
  const bottom = top + (box.height * s) / geometry.screenHeight;
  const clamp = (v: number, max: number) => Math.min(max - 1, Math.max(0, v));
  const c0 = clamp(Math.floor(left * frame.cols), frame.cols);
  const c1 = clamp(Math.ceil(right * frame.cols) - 1, frame.cols);
  const r0 = clamp(Math.floor(top * frame.rows), frame.rows);
  const r1 = clamp(Math.ceil(bottom * frame.rows) - 1, frame.rows);
  let sum = 0;
  let count = 0;
  for (let r = r0; r <= r1; r++) {
    for (let c = c0; c <= c1; c++) {
      sum += frame.brightness[r * frame.cols + c] ?? 0;
      count++;
    }
  }
  return count ? sum / count : 0;
}
