import { describe, expect, it } from "vitest";

import type { Settings } from "../core/generated/Settings";
import { dockHeight, fitShrink, lookStyle, roomFor, springEasing } from "./look";

const look: Settings["look"] = {
  scale: 100,
  corners: 100,
  transparency: 0,
  blur: 100,
  widgetBackground: 0,
  magnify: false,
  bounce: 40,
  startupAnimation: true,
  popupStyle: "glass",
  widgetHover: true,
  centerApps: true,
  search: "box",
  taskView: true,
  nativeFlyouts: true,
};

describe("dock look", () => {
  it("scales the chosen size by the slider", () => {
    expect(dockHeight({ size: "medium", look })).toBe(56);
    expect(dockHeight({ size: "medium", look: { ...look, scale: 150 } })).toBe(84);
    expect(dockHeight({ size: "small", look: { ...look, scale: 70 } })).toBe(34);
  });

  it("shows more on bigger docks", () => {
    expect(roomFor({ size: "small", look })).toBe("compact");
    expect(roomFor({ size: "medium", look })).toBe("normal");
    expect(roomFor({ size: "large", look: { ...look, scale: 110 } })).toBe("roomy");
  });

  it("turns settings into CSS variables", () => {
    const vars = lookStyle({ size: "medium", look: { ...look, scale: 200, transparency: 40, blur: 0 } });
    expect(vars["--scale"]).toBe("2.000");
    expect(vars["--tscale"]).toBe("1.400");
    expect(vars["--glass-opacity"]).toBe("0.60");
    expect(vars["--blur-scale"]).toBe("0.00");
  });

  it("springs more with more bounce", () => {
    expect(springEasing(0)).toBe("cubic-bezier(0.34, 1.00, 0.64, 1)");
    expect(springEasing(100)).toBe("cubic-bezier(0.34, 1.90, 0.64, 1)");
  });

  it("shrinks to fit the screen, settling instead of jittering", () => {
    expect(fitShrink(1, 1000, 2560)).toBe(1);
    const fitted = fitShrink(1, 3000, 2560);
    expect(fitted).toBeLessThan(0.86);
    expect(fitted).toBeGreaterThan(0.8);
    expect(fitShrink(fitted, 3000 * fitted, 2560)).toBe(fitted);
    expect(fitShrink(1, 10000, 2560)).toBe(0.6);
  });
});
