import { describe, expect, it } from "vitest";

import { shortcutFromKey, shortcutLabel } from "./shortcut";

const press = (
  code: string,
  mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {},
) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("shortcutFromKey", () => {
  it("builds shortcut text from modifiers and a key", () => {
    expect(shortcutFromKey(press("KeyD", { ctrlKey: true, altKey: true }))).toBe("Ctrl+Alt+D");
    expect(shortcutFromKey(press("Space", { metaKey: true, shiftKey: true }))).toBe("Shift+Super+Space");
    expect(shortcutFromKey(press("F9", { altKey: true }))).toBe("Alt+F9");
    expect(shortcutFromKey(press("Digit3", { ctrlKey: true }))).toBe("Ctrl+3");
  });

  it("waits for a usable key with a modifier", () => {
    expect(shortcutFromKey(press("KeyD"))).toBeNull();
    expect(shortcutFromKey(press("ControlLeft", { ctrlKey: true }))).toBeNull();
    expect(shortcutFromKey(press("Escape", { ctrlKey: true }))).toBeNull();
  });
});

describe("shortcutLabel", () => {
  it("names the Windows key and spaces things out", () => {
    expect(shortcutLabel("Shift+Super+Space")).toBe("Shift + Win + Space");
    expect(shortcutLabel("")).toBe("None");
  });
});
