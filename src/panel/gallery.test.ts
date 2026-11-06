import { describe, expect, it } from "vitest";

import type { WidgetEntry } from "../core/generated/WidgetEntry";
import type { WidgetDefinition } from "../widgets/registry";
import { galleryGroups } from "./gallery";

const def = (label: string, category?: WidgetDefinition["category"]): WidgetDefinition => ({
  label,
  description: `${label} things`,
  category,
  Tile: () => null,
  Panel: () => null,
});

const entry = (kind: WidgetEntry["kind"], enabled = false): WidgetEntry => ({
  kind,
  enabled,
  size: "normal",
});

describe("galleryGroups", () => {
  const defs = {
    clock: def("Clock", "info"),
    focus: def("Focus timer", "productivity"),
    music: def("Music", "media"),
  };
  const widgets = [entry("music", true), entry("clock"), entry("focus"), entry("calendar")];

  it("groups by category in a fixed order and skips widgets this version lacks", () => {
    const groups = galleryGroups(widgets, defs, "");
    expect(groups.map((g) => g.category)).toEqual(["productivity", "info", "media"]);
    expect(groups.flatMap((g) => g.entries.map((e) => e.kind))).not.toContain("calendar");
  });

  it("filters by name or description, ignoring case", () => {
    expect(galleryGroups(widgets, defs, "TIMER").flatMap((g) => g.entries.map((e) => e.kind))).toEqual([
      "focus",
    ]);
    expect(galleryGroups(widgets, defs, "music things")[0].entries[0].kind).toBe("music");
    expect(galleryGroups(widgets, defs, "nothing like this")).toEqual([]);
  });
});
