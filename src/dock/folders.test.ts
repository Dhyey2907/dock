import { describe, expect, it } from "vitest";

import type { DockApp } from "../core/generated/DockApp";
import type { DockFolder } from "../core/generated/DockFolder";
import { dropIntoFolder, removeFromFolder, rowItems } from "./folders";

const app = (key: string) => ({ key, name: key, windows: [] }) as unknown as DockApp;
const folder = (id: string, keys: string[]): DockFolder => ({ id, name: "Coding", keys });

describe("dock folders", () => {
  it("puts a folder where its first app was", () => {
    const items = rowItems([app("a"), app("b"), app("c"), app("d")], [folder("f", ["c", "b"])]);
    expect(items.map((i) => (i.type === "app" ? i.app.key : `folder:${i.folder.id}`))).toEqual([
      "a",
      "folder:f",
      "d",
    ]);
    const f = items[1];
    expect(f.type === "folder" && f.apps.map((a) => a.key)).toEqual(["c", "b"]);
  });

  it("makes a folder from two apps, and adds to one", () => {
    const made = dropIntoFolder([], "b", { app: "a" }, "new");
    expect(made).toEqual([{ id: "new", name: "Folder", keys: ["a", "b"] }]);
    expect(dropIntoFolder(made, "c", { folder: "new" }, "x")[0].keys).toEqual(["a", "b", "c"]);
    expect(dropIntoFolder(made, "c", { app: "a" }, "x")[0].keys).toEqual(["a", "b", "c"]);
  });

  it("moves an app between folders and ignores dropping on itself", () => {
    const folders = [folder("f", ["a", "b"]), folder("g", ["c", "d"])];
    const moved = dropIntoFolder(folders, "b", { folder: "g" }, "x");
    expect(moved.find((f) => f.id === "f")?.keys).toEqual(["a"]);
    expect(moved.find((f) => f.id === "g")?.keys).toEqual(["c", "d", "b"]);
    expect(dropIntoFolder(folders, "a", { app: "a" }, "x")).toBe(folders);
  });

  it("dissolves a folder left with one app", () => {
    expect(removeFromFolder([folder("f", ["a", "b"])], "a")).toEqual([]);
    expect(removeFromFolder([folder("f", ["a", "b", "c"])], "a")[0].keys).toEqual(["b", "c"]);
  });
});
