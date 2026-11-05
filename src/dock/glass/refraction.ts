/**
 * The lens map behind the "liquid glass" refraction: an image whose red and green channels say
 * how far, horizontally and vertically, each pixel of the picture under the glass should be
 * pulled from (128 = not at all). Near the rim of the shape pixels pull from further inside,
 * which bends the wallpaper the way a thick glass edge does; the middle stays undistorted.
 *
 * `feDisplacementMap` reads it as: shown(x, y) = picture(x + k·(R/255 − ½), y + k·(G/255 − ½)).
 */

export interface LensShape {
  /** Map size, in CSS pixels. */
  width: number;
  height: number;
  /** Space around the glass shape inside the map (the picture is drawn larger than the glass so
   *  blur and refraction never sample past its edge). */
  inset: number;
  /** Corner radius of the glass shape. */
  radius: number;
  /** How deep the bent rim reaches into the glass. */
  bezel: number;
}

/** Signed distance from (x, y) to the rounded rectangle (negative inside) and its outward
 *  direction there. */
function roundedBox(x: number, y: number, s: LensShape): { distance: number; nx: number; ny: number } {
  const w = s.width - 2 * s.inset;
  const h = s.height - 2 * s.inset;
  const r = Math.max(0, Math.min(s.radius, w / 2, h / 2));
  const cx = s.inset + w / 2;
  const cy = s.inset + h / 2;
  const px = x - cx;
  const py = y - cy;
  const qx = Math.abs(px) - (w / 2 - r);
  const qy = Math.abs(py) - (h / 2 - r);
  const sx = px < 0 ? -1 : 1;
  const sy = py < 0 ? -1 : 1;
  if (qx > 0 && qy > 0) {
    const len = Math.hypot(qx, qy);
    return { distance: len - r, nx: (sx * qx) / len, ny: (sy * qy) / len };
  }
  const distance = Math.max(qx, qy) - r;
  return qx > qy ? { distance, nx: sx, ny: 0 } : { distance, nx: 0, ny: sy };
}

/** RGBA pixels of the lens map, row by row. */
export function lensPixels(shape: LensShape): Uint8ClampedArray {
  const w = Math.max(1, Math.round(shape.width));
  const h = Math.max(1, Math.round(shape.height));
  const pixels = new Uint8ClampedArray(w * h * 4);
  const bezel = Math.max(1, shape.bezel);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const { distance, nx, ny } = roundedBox(x + 0.5, y + 0.5, shape);
      const inside = -distance;
      // Strongest at the rim, fading smoothly to nothing `bezel` pixels in; outside the shape
      // (hidden by the clip) it keeps pulling inward so the edge never shows the margin.
      const t = inside <= 0 ? 1 : Math.max(0, 1 - inside / bezel);
      const strength = t * t;
      const i = (y * w + x) * 4;
      // Pull inward: against the outward direction.
      pixels[i] = 128 - nx * strength * 127;
      pixels[i + 1] = 128 - ny * strength * 127;
      pixels[i + 2] = 128;
      pixels[i + 3] = 255;
    }
  }
  return pixels;
}

/** The lens map as a PNG data URL (needs a real canvas, so not in unit tests). */
export function lensMapUrl(shape: LensShape): string | null {
  const w = Math.max(1, Math.round(shape.width));
  const h = Math.max(1, Math.round(shape.height));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const context = canvas.getContext("2d");
  if (!context) return null;
  const image = context.createImageData(w, h);
  image.data.set(lensPixels(shape));
  context.putImageData(image, 0, 0);
  return canvas.toDataURL("image/png");
}
