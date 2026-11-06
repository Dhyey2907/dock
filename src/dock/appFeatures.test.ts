import { afterEach, describe, expect, it, vi } from "vitest";

import { dockApp, dockWindow } from "../test/fixtures";

vi.mock("../core/ipc", () => ({ ipc: {}, events: {} }));

const { LAUNCH_TIMEOUT_MS, hoverTimer, isEmptySpot, movedPast, screenX, stillLaunching, useLaunching } =
  await import("./appFeatures");

describe("launch bounce", () => {
  it("lasts until a new window appears or it takes too long", () => {
    const launch = { since: 1000, windows: 1 };
    expect(stillLaunching(launch, 1, 2000)).toBe(true);
    expect(stillLaunching(launch, 2, 2000)).toBe(false);
    expect(stillLaunching(launch, 1, 1000 + LAUNCH_TIMEOUT_MS)).toBe(false);
    expect(stillLaunching(undefined, 0, 0)).toBe(false);
  });

  it("settles launches whose window appeared", () => {
    useLaunching.setState({ launches: {} });
    useLaunching.getState().start("a", 0, 0);
    useLaunching.getState().start("b", 0, 0);
    useLaunching.getState().settle([dockApp("a", { windows: [dockWindow(1)] }), dockApp("b")], 100);
    expect(Object.keys(useLaunching.getState().launches)).toEqual(["b"]);
  });
});

describe("hoverTimer", () => {
  afterEach(() => vi.useRealTimers());

  it("fires once the same key has been hovered long enough", () => {
    vi.useFakeTimers();
    const fire = vi.fn();
    const timer = hoverTimer(600, fire);
    timer.over("a");
    vi.advanceTimersByTime(400);
    timer.over("a"); // still over the same app: keeps waiting, doesn't restart
    vi.advanceTimersByTime(250);
    expect(fire).toHaveBeenCalledWith("a");
    timer.over("b");
    vi.advanceTimersByTime(300);
    timer.over(null);
    vi.advanceTimersByTime(1000);
    expect(fire).toHaveBeenCalledTimes(1);
  });
});

describe("helpers", () => {
  it("measures drags and screen positions", () => {
    expect(movedPast({ x: 0, y: 0 }, { x: 3, y: 4 }, 5)).toBe(true);
    expect(movedPast({ x: 0, y: 0 }, { x: 3, y: 3 }, 5)).toBe(false);
    expect(screenX(100, { x: 700, scale: 1.5 }, 0)).toBe(850);
    expect(screenX(100, null, 42)).toBe(42);
  });

  it("knows the dock's empty background from its buttons", () => {
    const bar = document.createElement("div");
    bar.setAttribute("data-tauri-drag-region", "");
    const button = document.createElement("button");
    bar.append(button);
    expect(isEmptySpot(bar)).toBe(true);
    expect(isEmptySpot(button)).toBe(false);
    expect(isEmptySpot(null)).toBe(false);
  });
});
