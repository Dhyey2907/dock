import { describe, expect, it } from "vitest";

import { lensPixels, type LensShape } from "./refraction";

const shape: LensShape = { width: 220, height: 80, inset: 10, radius: 30, bezel: 16 };

function at(pixels: Uint8ClampedArray, x: number, y: number) {
  const i = (y * shape.width + x) * 4;
  return { r: pixels[i], g: pixels[i + 1] };
}

describe("lensPixels", () => {
  const pixels = lensPixels(shape);

  it("leaves the middle of the glass undistorted", () => {
    expect(at(pixels, 110, 40)).toEqual({ r: 128, g: 128 });
  });

  it("pulls inward at each edge, strongest at the rim", () => {
    // Middle of the top edge, just inside: pull down (into the glass).
    const top = at(pixels, 110, 11);
    expect(top.g).toBeGreaterThan(200);
    expect(top.r).toBe(128);
    // Bottom edge: pull up.
    expect(at(pixels, 110, 68).g).toBeLessThan(56);
    // Left and right ends of the pill: pull right and left.
    expect(at(pixels, 11, 40).r).toBeGreaterThan(200);
    expect(at(pixels, 208, 40).r).toBeLessThan(56);
  });

  it("fades out across the bezel", () => {
    const rim = at(pixels, 110, 11).g;
    const halfway = at(pixels, 110, 18).g;
    const past = at(pixels, 110, 30).g;
    expect(rim).toBeGreaterThan(halfway);
    expect(halfway).toBeGreaterThan(128);
    expect(past).toBe(128);
  });

  it("bends round corners diagonally", () => {
    const corner = at(pixels, 16, 16);
    expect(corner.r).toBeGreaterThan(128);
    expect(corner.g).toBeGreaterThan(128);
  });

  it("is fully opaque, so the map is never blended away", () => {
    for (let i = 3; i < pixels.length; i += 4) {
      if (pixels[i] !== 255) throw new Error(`pixel ${(i - 3) / 4} not opaque`);
    }
  });
});
