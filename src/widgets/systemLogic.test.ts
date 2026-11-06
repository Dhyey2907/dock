import { describe, expect, it } from "vitest";

import {
  compactCount,
  formatBytes,
  formatUptime,
  intervalLabel,
  shortUptime,
  signalWord,
  usedPercent,
  WALLPAPER_INTERVALS,
} from "./systemLogic";

describe("uptime", () => {
  it("reads in the largest sensible units", () => {
    expect(formatUptime(30)).toBe("0 min");
    expect(formatUptime(8 * 60)).toBe("8 min");
    expect(formatUptime(5 * 3600 + 12 * 60)).toBe("5 h 12 min");
    expect(formatUptime(2 * 3600)).toBe("2 h");
    expect(formatUptime(3 * 86400 + 4 * 3600 + 59)).toBe("3 d 4 h");
    expect(formatUptime(86400)).toBe("1 d");
  });

  it("has a short form for the tile", () => {
    expect(shortUptime(59)).toBe("0m");
    expect(shortUptime(3 * 3600)).toBe("3h");
    expect(shortUptime(4 * 86400)).toBe("4d");
  });
});

describe("sizes and counts", () => {
  it("formats bytes", () => {
    expect(formatBytes(512)).toBe("512 bytes");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(64 * 1024 ** 3)).toBe("64.0 GB");
    expect(formatBytes(250 * 1024 ** 2)).toBe("250 MB");
  });

  it("shortens big counts", () => {
    expect(compactCount(950)).toBe("950");
    expect(compactCount(7785)).toBe("7.8k");
    expect(compactCount(2000)).toBe("2k");
    expect(compactCount(123_456)).toBe("123k");
  });

  it("works out how full a drive is", () => {
    expect(usedPercent(100, 25)).toBe(75);
    expect(usedPercent(0, 0)).toBe(0);
    expect(usedPercent(100, 150)).toBe(0);
  });
});

describe("labels", () => {
  it("names signal strength", () => {
    expect(signalWord(95)).toBe("Excellent");
    expect(signalWord(60)).toBe("Good");
    expect(signalWord(45)).toBe("Fair");
    expect(signalWord(10)).toBe("Weak");
  });

  it("names every wallpaper interval", () => {
    expect(WALLPAPER_INTERVALS.map(intervalLabel)).toEqual([
      "Only when I click",
      "Every 5 minutes",
      "Every 15 minutes",
      "Every 30 minutes",
      "Every hour",
      "Every 3 hours",
      "Every day",
    ]);
  });
});
