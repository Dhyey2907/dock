import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DockApp } from "../core/generated/DockApp";

const calls = {
  appClick: vi.fn(async (_key: string) => {}),
  appLaunch: vi.fn(async (_key: string) => {}),
  appMenu: vi.fn(async (_key: string, _anchorX: number) => {}),
  appIcon: vi.fn(async (_source: string) => "data:image/png;base64,AAAA"),
};
vi.mock("../core/ipc", () => ({ ipc: calls }));

const { AppButton } = await import("./AppButton");

const chrome: DockApp = {
  key: "pin:chrome",
  name: "Google Chrome",
  pinned: true,
  icon: "lnk:C:\\chrome.lnk",
  windows: [
    { hwnd: 1, title: "Inbox", active: false, minimized: false },
    { hwnd: 2, title: "Docs", active: true, minimized: false },
  ],
};

describe("AppButton", () => {
  beforeEach(() => Object.values(calls).forEach((fn) => fn.mockClear()));

  it("left, middle and right clicks do different things", () => {
    render(<AppButton app={chrome} />);
    const button = screen.getByRole("button", { name: "Google Chrome" });
    fireEvent.click(button);
    expect(calls.appClick).toHaveBeenCalledWith("pin:chrome");
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
  });

  it("shows the icon once loaded, and a letter until then", async () => {
    render(<AppButton app={{ ...chrome, key: "pin:other", icon: "lnk:other", windows: [] }} />);
    expect(screen.getByText("G")).toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Google Chrome" });
    await vi.waitFor(() => expect(button.querySelector("img")).not.toBeNull());
    expect(button).toHaveAttribute("data-running", "false");
  });
});
