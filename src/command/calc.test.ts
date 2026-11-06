import { describe, expect, it } from "vitest";

import { calculate, formatResult } from "./calc";

describe("calculate", () => {
  it("follows the usual order of operations", () => {
    expect(calculate("2 + 3 * 4")).toBe(14);
    expect(calculate("(2 + 3) * 4")).toBe(20);
    expect(calculate("10 - 4 - 3")).toBe(3);
    expect(calculate("100 / 10 / 2")).toBe(5);
  });

  it("does powers right to left and binds unary minus looser than powers", () => {
    expect(calculate("2 ^ 3 ^ 2")).toBe(512);
    expect(calculate("2**10")).toBe(1024);
    expect(calculate("-2^2")).toBe(-4);
    expect(calculate("(-2)^2")).toBe(4);
  });

  it("knows percent, typed symbols, thousands and implicit multiplication", () => {
    expect(calculate("200 * 15%")).toBe(30);
    expect(calculate("6 × 7")).toBe(42);
    expect(calculate("9 ÷ 3")).toBe(3);
    expect(calculate("1,200 + 1")).toBe(1201);
    expect(calculate("2(3 + 4)")).toBe(14);
    expect(calculate("2pi")).toBeCloseTo(2 * Math.PI);
    expect(calculate("1e3 + 1")).toBe(1001);
    expect(calculate("3 + 4 =")).toBe(7);
  });

  it("has functions and constants", () => {
    expect(calculate("sqrt(16)")).toBe(4);
    expect(calculate("sqrt 9")).toBe(3);
    expect(calculate("log(1000)")).toBe(3);
    expect(calculate("ln(e)")).toBe(1);
    expect(calculate("abs(-3) + round(2.6)")).toBe(6);
    expect(calculate("cos(0)")).toBe(1);
  });

  it("isn't fooled by things that aren't sums", () => {
    expect(calculate("")).toBeNull();
    expect(calculate("2024")).toBeNull();
    expect(calculate("e")).toBeNull();
    expect(calculate("notepad")).toBeNull();
    expect(calculate("2 +")).toBeNull();
    expect(calculate("(1 + 2")).toBeNull();
    expect(calculate("1 / 0")).toBeNull();
    expect(calculate("alert(1)")).toBeNull();
    expect(calculate("2 $ 3")).toBeNull();
  });
});

describe("formatResult", () => {
  it("drops float noise and trailing zeros", () => {
    expect(formatResult(0.1 + 0.2)).toBe("0.3");
    expect(formatResult(14)).toBe("14");
    expect(formatResult(1 / 3)).toBe("0.333333333333");
    expect(formatResult(-0)).toBe("0");
    expect(formatResult(2 ** 60)).toBe("1.152922e+18");
  });
});
