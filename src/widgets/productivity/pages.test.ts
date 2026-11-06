import { describe, expect, it } from "vitest";

import { boxFractions } from "./logic";
import { droppedText } from "./textDrop";

const transfer = (types: string[], data: Record<string, string>) => ({
  types,
  getData: (type: string) => data[type] ?? "",
});

describe("text dropped on the dock", () => {
  it("takes plain text, or a link", () => {
    expect(droppedText(transfer(["text/plain"], { "text/plain": "  buy milk " }))).toBe("buy milk");
    expect(droppedText(transfer(["text/uri-list"], { "text/uri-list": "https://x.dev" }))).toBe(
      "https://x.dev",
    );
  });
  it("ignores files and empty text", () => {
    expect(droppedText(transfer(["Files", "text/plain"], { "text/plain": "a.txt" }))).toBeNull();
    expect(droppedText(transfer(["text/plain"], { "text/plain": "  " }))).toBeNull();
  });
});

describe("area picker", () => {
  it("turns a box dragged any way into window fractions", () => {
    expect(boxFractions({ x0: 300, y0: 200, x1: 100, y1: 100 }, 1000, 500)).toEqual({
      x: 0.1,
      y: 0.2,
      width: 0.2,
      height: 0.2,
    });
  });
});
