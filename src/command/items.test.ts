import { describe, expect, it } from "vitest";

import { buildResults, type Sources } from "./items";

const sources: Sources = {
  apps: [
    { id: "Microsoft.WindowsNotepad_8wekyb3d8bbwe!App", name: "Notepad", icon: "aumid:np" },
    { id: "Spotify.exe", name: "Spotify", icon: "aumid:sp" },
  ],
  notes: [
    { kind: "note", id: "1", text: "Notepad tips\nUse tabs", done: false },
    { kind: "todo", id: "2", text: "Renew passport", done: false },
  ],
  workspaces: [{ name: "Coding", saved: "2026-11-06T09:00:00Z", apps: [] }],
  files: [{ path: "C:\\Users\\a\\notes.txt", name: "notes.txt", folder: "C:\\Users\\a" }],
  widgets: [{ kind: "spotify", label: "Spotify playlists", description: "Play your playlists." }],
};

describe("buildResults", () => {
  it("starts with workspaces and actions before anything is typed", () => {
    const items = buildResults("", sources);
    expect(items[0]).toMatchObject({ kind: "workspace", name: "Coding" });
    expect(items.some((i) => i.kind === "action")).toBe(true);
  });

  it("puts an app ahead of a note and a setting with the same words", () => {
    const items = buildResults("notepad", sources);
    expect(items[0]).toMatchObject({ kind: "app", title: "Notepad" });
    expect(items.findIndex((i) => i.kind === "note")).toBeGreaterThan(0);
  });

  it("shows a sum's answer first", () => {
    const items = buildResults("12*12", sources);
    expect(items[0]).toMatchObject({ kind: "calc", value: "144" });
  });

  it("finds settings, widgets, to-dos and workspaces", () => {
    expect(buildResults("auto-hide", sources)[0]).toMatchObject({
      kind: "setting",
      target: "taskbar/autoHide",
    });
    expect(buildResults("spotify", { ...sources, files: [] }).map((i) => i.kind)).toEqual([
      "app",
      "widget",
      "setting",
    ]);
    expect(buildResults("passport", sources)[0]).toMatchObject({ kind: "note", todo: true });
    expect(buildResults("coding", sources)[0]).toMatchObject({ kind: "workspace" });
  });

  it("adds files from the index after the rest", () => {
    const items = buildResults("notes", sources);
    expect(items[items.length - 1]).toMatchObject({ kind: "file", title: "notes.txt" });
  });
});
