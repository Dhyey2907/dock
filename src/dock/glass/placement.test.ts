import { describe, expect, it } from "vitest";

import type { DockGeometry } from "../../core/generated/DockGeometry";
import type { GlassFrame } from "../../core/generated/GlassFrame";
import { brightnessBehind, pictureLayout } from "./placement";

// A 2560×1600 screen at 150%, the dock window near the bottom.
const geometry: DockGeometry = {
  screenX: 0,
  screenY: 0,
  screenWidth: 2560,
  screenHeight: 1600,
  x: 760,
  y: 1504,
  scale: 1.5,
};

describe("pictureLayout", () => {
  it("sizes the picture to the screen and shifts it so the slice behind the glass shows", () => {
    const layout = pictureLayout(geometry, { left: 0, top: 0, width: 696, height: 56 }, 0);
    expect(layout.width).toBeCloseTo(2560 / 1.5);
    expect(layout.height).toBeCloseTo(1600 / 1.5);
    expect(layout.x).toBeCloseTo(-760 / 1.5);
    expect(layout.y).toBeCloseTo(-1504 / 1.5);
  });

  it("accounts for the glass's place in the window and the picture's margin", () => {
    const layout = pictureLayout(geometry, { left: 10, top: 4, width: 600, height: 50 }, 16);
    expect(layout.x).toBeCloseTo(-(760 / 1.5 + 10) + 16);
    expect(layout.y).toBeCloseTo(-(1504 / 1.5 + 4) + 16);
  });

  it("works on a screen that doesn't start at the origin", () => {
    const left = { ...geometry, screenX: -1920, x: -1500, scale: 1 };
    expect(pictureLayout(left, { left: 0, top: 0, width: 10, height: 10 }, 0).x).toBe(-420);
  });
});

describe("brightnessBehind", () => {
  // 4×2 grid: top row dark, bottom row light.
  const frame: GlassFrame = {
    image: "",
    cols: 4,
    rows: 2,
    brightness: [0.1, 0.1, 0.1, 0.1, 0.9, 0.9, 0.9, 0.9],
  };

  it("averages the cells behind the glass only", () => {
    const bottom = brightnessBehind(frame, geometry, { left: 0, top: 0, width: 696, height: 56 });
    expect(bottom).toBeCloseTo(0.9);
    const top = brightnessBehind(frame, { ...geometry, y: 12 }, { left: 0, top: 0, width: 696, height: 56 });
    expect(top).toBeCloseTo(0.1);
  });
});
