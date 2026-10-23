import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Settings } from "../core/generated/Settings";

const defaults: Settings = {
  edge: "bottom",
  style: "pill",
  theme: "glass",
  size: "medium",
  autoHide: false,
  taskbarMode: "autoHide",
  launchAtStartup: true,
};

const settingsSet = vi.fn(async (settings: Settings) => settings);
vi.mock("../core/ipc", () => ({
  ipc: { settingsSet: (s: Settings) => settingsSet(s) },
  events: { onSettingsChanged: vi.fn() },
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
});
