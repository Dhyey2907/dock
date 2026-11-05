import { describe, expect, it, vi } from "vitest";

import type { DockApp } from "../core/generated/DockApp";

const app = (key: string): DockApp => ({ key, name: key, pinned: true, icon: "", windows: [] });

let push: ((apps: DockApp[]) => void) | undefined;
let resolveList: ((apps: DockApp[]) => void) | undefined;
const appIcon = vi.fn(async (_source: string): Promise<string | null> => null);
vi.mock("../core/ipc", () => ({
  events: {
    onAppsChanged: async (handler: (apps: DockApp[]) => void) => {
      push = handler;
      return () => {};
    },
  },
  ipc: {
    appsList: () => new Promise<DockApp[]>((resolve) => (resolveList = resolve)),
    appIcon: (source: string) => appIcon(source),
  },
}));

const { loadIcon, useApps } = await import("./apps");

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

describe("loadIcon", () => {
  it("keeps icons that loaded, and asks again after a miss", async () => {
    appIcon.mockResolvedValueOnce(null).mockResolvedValueOnce("data:image/png;base64,AA");
    expect(await loadIcon("exe:c:/app.exe")).toBeNull();
    expect(await loadIcon("exe:c:/app.exe")).toBe("data:image/png;base64,AA");
    // Now kept: no third request.
    expect(await loadIcon("exe:c:/app.exe")).toBe("data:image/png;base64,AA");
    expect(appIcon).toHaveBeenCalledTimes(2);
  });
});
