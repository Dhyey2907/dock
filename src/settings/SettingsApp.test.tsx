import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Settings } from "../core/generated/Settings";

const defaults: Settings = {
  edge: "bottom",
  style: "pill",
  theme: "glass",
  size: "medium",
  look: {
    scale: 100,
    corners: 100,
    transparency: 0,
    blur: 100,
    widgetBackground: 0,
    magnify: false,
    bounce: 40,
    startupAnimation: true,
    popupStyle: "glass",
  },
  autoHide: false,
  taskbarMode: "autoHide",
  launchAtStartup: true,
  widgets: [{ kind: "clock", enabled: true, size: "normal" }],
  temperatureUnit: "celsius",
  weatherPlace: "",
  calendars: [],
  automations: { meetingMode: true, aiLimitWarnings: true, focusMusic: false, sunsetDarkMode: false },
  quietHours: { enabled: false, start: "22:00", end: "08:00" },
  toggleShortcut: "Ctrl+Alt+D",
  screen: "",
  docksOn: "one",
  onboarded: true,
  floating: [],
  autoShrink: true,
};

const settingsSet = vi.fn(async (settings: Settings) => settings);
vi.mock("../core/ipc", () => ({
  ipc: { settingsSet: (s: Settings) => settingsSet(s), screensList: async () => [] },
  events: { onSettingsChanged: vi.fn(), onSettingsSection: async () => () => {} },
}));

const { useSettings } = await import("./store");
const { SettingsApp } = await import("./SettingsApp");

describe("SettingsApp", () => {
  beforeEach(() => {
    settingsSet.mockClear();
    useSettings.setState({ settings: defaults });
  });

  it("marks the current choice", () => {
    render(<SettingsApp />);
    expect(screen.getByRole("radio", { name: "Bottom" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("radio", { name: "Top" })).toHaveAttribute("aria-checked", "false");
  });

  it("saves the whole settings object with the change", async () => {
    render(<SettingsApp />);
    fireEvent.click(screen.getByRole("radio", { name: "Midnight" }));
    await vi.waitFor(() => expect(settingsSet).toHaveBeenCalledWith({ ...defaults, theme: "midnight" }));
    expect(useSettings.getState().settings?.theme).toBe("midnight");
  });

  it("shows one section at a time, chosen from the sidebar", () => {
    render(<SettingsApp />);
    expect(screen.getByRole("heading", { name: "Appearance" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Taskbar & startup" }));
    expect(screen.queryByRole("heading", { name: "Appearance" })).not.toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Auto-hide the dock" })).toHaveAttribute(
      "aria-checked",
      "false",
    );
    expect(screen.getByRole("button", { name: "Taskbar & startup" })).toHaveAttribute("aria-current", "page");
  });
});
