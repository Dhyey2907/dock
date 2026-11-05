import { describe, expect, it, vi } from "vitest";

import type { DockApp } from "../core/generated/DockApp";

const app = (key: string): DockApp => ({ key, name: key, pinned: true, icon: "", windows: [] });

let push: ((apps: DockApp[]) => void) | undefined;
let resolveList: ((apps: DockApp[]) => void) | undefined;
vi.mock("../core/ipc", () => ({
  events: {
    onAppsChanged: async (handler: (apps: DockApp[]) => void) => {
      push = handler;
      return () => {};
    },
  },
  ipc: {
    appsList: () => new Promise<DockApp[]>((resolve) => (resolveList = resolve)),
    appIcon: async () => null,
  },
}));

const { useApps } = await import("./apps");

describe("useApps.load", () => {
  it("keeps a change that arrives while the first list is still loading", async () => {
    const loading = useApps.getState().load();
    await vi.waitFor(() => expect(resolveList).toBeDefined());
    // The backend finishes reading pins and pushes the full list...
    push?.([app("chrome"), app("claude")]);
    // ...then the older reply (read before the pins) arrives.
    resolveList?.([app("claude")]);
    await loading;
    expect(useApps.getState().apps.map((a) => a.key)).toEqual(["chrome", "claude"]);
  });
});
