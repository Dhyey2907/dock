import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DockApp } from "../core/generated/DockApp";
import { dockApp, dockWindow } from "../test/fixtures";

const calls = {
  appClick: vi.fn(async (_key: string, _anchorX?: number) => {}),
  appLaunch: vi.fn(async (_key: string) => {}),
  appMenu: vi.fn(async (_key: string, _anchorX: number) => {}),
  appIcon: vi.fn(async (_source: string) => "data:image/png;base64,AAAA"),
  appPreviews: vi.fn(async (_key: string, _anchorX: number) => {}),
  previewHide: vi.fn(async (_delay: number) => {}),
  appDragOut: vi.fn(async (_key: string) => {}),
};
vi.mock("../core/ipc", () => ({ ipc: calls, events: {} }));

const { AppButton } = await import("./AppButton");
const { useLaunching } = await import("./appFeatures");

const chrome: DockApp = dockApp("pin:chrome", {
  name: "Google Chrome",
  icon: "lnk:C:\\chrome.lnk",
  windows: [dockWindow(1, { title: "Inbox" }), dockWindow(2, { title: "Docs", active: true })],
});

describe("AppButton", () => {
  beforeEach(() => {
    Object.values(calls).forEach((fn) => fn.mockClear());
    useLaunching.setState({ launches: {} });
  });

  it("left, middle and right clicks do different things", () => {
    render(<AppButton app={chrome} />);
    const button = screen.getByRole("button", { name: "Google Chrome" });
    fireEvent.click(button);
    expect(calls.appClick).toHaveBeenCalledWith("pin:chrome", expect.any(Number));
    fireEvent(button, new MouseEvent("auxclick", { bubbles: true, button: 1 }));
    expect(calls.appLaunch).toHaveBeenCalledWith("pin:chrome");
    fireEvent.contextMenu(button);
    expect(calls.appMenu).toHaveBeenCalledWith("pin:chrome", expect.any(Number));
  });

  it("marks running, active and several windows, and titles with the active window", () => {
    render(<AppButton app={chrome} />);
    const button = screen.getByRole("button", { name: "Google Chrome" });
    expect(button).toHaveAttribute("data-running", "true");
    expect(button).toHaveAttribute("data-active", "true");
    expect(button).toHaveAttribute("data-many", "true");
    expect(button).toHaveAttribute("title", "Docs");
    expect(button).toHaveAttribute("data-app-key", "pin:chrome");
  });

  it("shows the icon once loaded, and a letter until then", async () => {
    render(<AppButton app={{ ...chrome, key: "pin:other", icon: "lnk:other", windows: [] }} />);
    expect(screen.getByText("G")).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Google Chrome" });
    await vi.waitFor(() => expect(button.querySelector("img")).not.toBeNull());
    expect(button).toHaveAttribute("data-running", "false");
  });

  it("bounces while launching, glows for attention, and marks a frozen app", () => {
    useLaunching.getState().start("pin:idle", 0);
    render(
      <>
        <AppButton app={dockApp("pin:idle", { name: "Idle" })} />
        <AppButton app={{ ...chrome, attention: true, hung: true }} />
      </>,
    );
    expect(screen.getByRole("button", { name: "Idle" })).toHaveAttribute("data-launching", "true");
    const frozen = screen.getByRole("button", { name: "Google Chrome" });
    expect(frozen).toHaveAttribute("data-attention", "true");
    expect(frozen).toHaveAttribute("data-hung", "true");
    expect(frozen.getAttribute("title")).toContain("Not responding");
  });

  it("draws a divider after it when asked", () => {
    const { container } = render(<AppButton app={{ ...chrome, dividerAfter: true }} />);
    expect(container.querySelector(".dock-separator")).not.toBeNull();
  });

  it("shows window previews after hovering, and asks them to go on leaving", () => {
    vi.useFakeTimers();
    try {
      render(<AppButton app={chrome} />);
      const button = screen.getByRole("button", { name: "Google Chrome" });
      fireEvent.mouseEnter(button);
      expect(calls.appPreviews).not.toHaveBeenCalled();
      act(() => vi.advanceTimersByTime(500));
      expect(calls.appPreviews).toHaveBeenCalledWith("pin:chrome", expect.any(Number));
      fireEvent.mouseLeave(button);
      expect(calls.previewHide).toHaveBeenCalledWith(300);
    } finally {
      vi.useRealTimers();
    }
  });

  it("dragging the icon away starts a snap and doesn't count as a click", () => {
    render(<AppButton app={chrome} />);
    const button = screen.getByRole("button", { name: "Google Chrome" });
    fireEvent.pointerDown(button, { button: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerMove(button, { buttons: 1, clientX: 12, clientY: 11 });
    expect(calls.appDragOut).not.toHaveBeenCalled();
    fireEvent.pointerMove(button, { buttons: 1, clientX: 10, clientY: -40 });
    expect(calls.appDragOut).toHaveBeenCalledWith("pin:chrome");
    fireEvent.pointerUp(button);
    fireEvent.click(button);
    expect(calls.appClick).not.toHaveBeenCalled();
  });
});
