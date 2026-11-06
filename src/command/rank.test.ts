import { describe, expect, it } from "vitest";

import { fold, rank, score } from "./rank";

const names = (items: { name: string }[]) => items.map((i) => i.name);

describe("score", () => {
  it("puts exact, then start, then word-start, then middle matches first", () => {
    const exact = score("code", "Code");
    const start = score("code", "Codex");
    const wordStart = score("code", "Visual Studio Code");
    const middle = score("code", "Barcode Reader");
    expect(exact).toBeGreaterThan(start);
    expect(start).toBeGreaterThan(wordStart);
    expect(wordStart).toBeGreaterThan(middle);
    expect(middle).toBeGreaterThan(0);
  });

  it("finds initials and letters in order", () => {
    expect(score("vsc", "Visual Studio Code")).toBeGreaterThan(0);
    expect(score("ntpd", "Notepad")).toBeGreaterThan(0);
    expect(score("vsc", "Visual Studio Code")).toBeGreaterThan(score("ntpd", "Notepad"));
  });

  it("needs every word, and uses keywords", () => {
    expect(score("dock size", "Size", ["dock", "bigger", "smaller"])).toBeGreaterThan(0);
    expect(score("dark", "Style", ["theme", "dark", "light"])).toBeGreaterThan(0);
    expect(score("dark zebra", "Style", ["dark"])).toBe(0);
    expect(score("xyz", "Notepad")).toBe(0);
    expect(score("   ", "Notepad")).toBe(0);
  });

  it("ignores case and accents", () => {
    expect(fold("Café")).toBe("cafe");
    expect(score("CAFE", "Café Finder")).toBeGreaterThan(0);
  });
});

describe("rank", () => {
  const apps = [
    { name: "Microsoft Edge" },
    { name: "Notepad" },
    { name: "Notepad++" },
    { name: "OneNote" },
    { name: "Notion" },
  ];

  it("orders by score and drops misses", () => {
    expect(names(rank("note", apps))).toEqual(["Notepad", "Notepad++", "OneNote"]);
    expect(names(rank("not", apps))).toEqual(["Notion", "Notepad", "Notepad++", "OneNote"]);
  });

  it("applies boosts and limits", () => {
    const items = [
      { name: "Appearance", boost: 0 },
      { name: "Apple Music", boost: 20 },
    ];
    expect(names(rank("app", items))[0]).toBe("Apple Music");
    expect(rank("note", apps, 1)).toHaveLength(1);
  });

  it("keeps the given order on ties", () => {
    const twins = [{ name: "Alpha One" }, { name: "Alpha Two" }];
    expect(names(rank("alpha", twins))).toEqual(["Alpha One", "Alpha Two"]);
  });
});
