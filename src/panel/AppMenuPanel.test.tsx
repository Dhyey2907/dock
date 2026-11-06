import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AppMenu } from "../core/generated/AppMenu";
import { dockWindow } from "../test/fixtures";

// The menu only ever asks the backend; nothing here can end a real program.
const appMenuAction = vi.fn(async (_action: unknown) => {});
vi.mock("../core/ipc", () => ({
  ipc: { appMenuAction: (a: unknown) => appMenuAction(a), appIcon: async () => null },
}));

const { AppMenuPanel } = await import("./AppMenuPanel");

const base: AppMenu = {
  dock: "dock",
  key: "pin:code",
  name: "Code",
  icon: "",
  launchable: true,
  windows: [dockWindow(1)],
  sections: [],
  kind: "app",
  pinned: true,
  custom: false,
  dividerAfter: false,
  hung: false,
  canEndTask: true,
  canMove: false,
  canRestart: true,
  customised: false,
  presets: [],
  quick: [],
};

describe("AppMenuPanel", () => {
  beforeEach(() => appMenuAction.mockClear());

  it("asks before ending a task, and only then ends it", () => {
    render(<AppMenuPanel menu={base} />);
    const row = screen.getByRole("button", { name: "End task" });
    fireEvent.click(row);
    expect(appMenuAction).not.toHaveBeenCalled();
    expect(row).toHaveTextContent("Click again to end Code");
    fireEvent.click(row);
    expect(appMenuAction).toHaveBeenCalledWith({ type: "endTask" });
  });

  it("disarms when focus moves away", () => {
    render(<AppMenuPanel menu={base} />);
    const row = screen.getByRole("button", { name: "End task" });
    fireEvent.click(row);
    fireEvent.blur(row);
    expect(row).toHaveTextContent("End task");
    fireEvent.click(row);
    expect(appMenuAction).not.toHaveBeenCalled();
  });

  it("offers End task for any app with windows, apart from Force close", () => {
    const { rerender } = render(<AppMenuPanel menu={{ ...base, canEndTask: false, windows: [] }} />);
    expect(screen.queryByText("End task")).toBeNull();
    rerender(<AppMenuPanel menu={{ ...base, hung: true }} />);
    expect(screen.getByText("End task")).toBeInTheDocument();
    const force = screen.getByText(/Force close/);
    fireEvent.click(force);
    fireEvent.click(force);
    expect(appMenuAction).toHaveBeenCalledWith({ type: "forceClose" });
    expect(appMenuAction).not.toHaveBeenCalledWith({ type: "endTask" });
  });

  it("shows media controls, presets and the user's buttons", () => {
    render(
      <AppMenuPanel
        menu={{
          ...base,
          media: {
            title: "Song",
            artist: "Band",
            playing: true,
            canPlayPause: true,
            canNext: true,
            canPrevious: false,
          },
          presets: [{ id: "p1", name: "Work profile" }],
          quick: [{ id: "q1", name: "Flush DNS" }],
        }}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(appMenuAction).toHaveBeenCalledWith({ type: "media", key: "playPause" });
    fireEvent.click(screen.getByRole("button", { name: "Previous" }));
    expect(appMenuAction).toHaveBeenCalledTimes(1); // not supported by the source
    fireEvent.click(screen.getByText("Work profile"));
    expect(appMenuAction).toHaveBeenCalledWith({ type: "preset", id: "p1" });
    fireEvent.click(screen.getByText("Flush DNS"));
    expect(appMenuAction).toHaveBeenCalledWith({ type: "quick", id: "q1" });
  });

  it("renames inline", () => {
    render(<AppMenuPanel menu={base} />);
    fireEvent.click(screen.getByRole("button", { name: "Rename" }));
    const input = screen.getByRole("textbox", { name: "New name" });
    fireEvent.change(input, { target: { value: "Editor" } });
    fireEvent.submit(input);
    expect(appMenuAction).toHaveBeenCalledWith({ type: "rename", name: "Editor" });
  });

  it("offers hiding for apps and unpinning for the dock's own pins", () => {
    const { rerender } = render(<AppMenuPanel menu={base} />);
    expect(screen.getByRole("button", { name: "Never show on Dock" })).toBeInTheDocument();
    rerender(
      <AppMenuPanel menu={{ ...base, kind: "website", custom: true, windows: [], canEndTask: false }} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Unpin from Dock" }));
    expect(appMenuAction).toHaveBeenCalledWith({ type: "unpin" });
    expect(screen.queryByRole("button", { name: "Never show on Dock" })).toBeNull();
  });
});
