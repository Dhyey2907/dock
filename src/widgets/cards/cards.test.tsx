import { act, render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

/** Sample answers for every call a card makes; anything else resolves to nothing. */
const answers: Record<string, unknown> = {
  screenTime: {
    minutes: 98,
    hours: Array.from({ length: 24 }, (_, i) => (i === 10 ? 40 : i % 5)),
    apps: [{ name: "Chrome", minutes: 50 }],
    week: [60, 0, 30, 90, 10, 0, 98],
  },
  codexUsage: {
    plan: "plus",
    primary: { usedPercent: 34, windowMinutes: 300, resetsAt: 1_900_000_000 },
    secondary: { usedPercent: 21, windowMinutes: 10080, resetsAt: 1_900_000_000 },
    seenAt: 1,
  },
  netSpeed: { down: 150_000, up: 61_000 },
  cpuTemperature: 56,
  drives: [{ name: "C:", total: 500e9, free: 120e9 }],
  recycleBin: { items: 3, bytes: 4_000_000 },
  recentFiles: [{ name: "Plan.docx", path: "C:\\x\\Plan.docx.lnk", modified: 1_800_000_000 }],
  downloadsList: [{ name: "setup.exe", path: "C:\\Downloads\\setup.exe", modified: 1_800_000_000 }],
  screenshotsList: [
    {
      name: "Shot.png",
      path: "C:\\Shots\\Shot.png",
      modified: 1_800_000_000,
      thumbnail: "data:image/png;base64,AAAA",
    },
  ],
  stockQuotes: [
    { symbol: "AAPL", price: 190.5, previousClose: 188, currency: "USD", points: [188, 189, 190.5] },
    { symbol: "NVDA", price: 120, previousClose: 125, currency: "USD", points: [125, 121, 120] },
  ],
  coinPrice: { id: "bitcoin", usd: 68000, change24h: -1.2 },
  currencyRate: { from: "USD", to: "INR", rate: 83.2, date: "2026-11-07" },
  githubContributions: {
    user: "me",
    levels: Array.from({ length: 105 }, (_, i) => i % 5),
    today: 6,
    week: 37,
    streak: 23,
  },
  leetcodeStats: { user: "me", easy: 50, medium: 30, hard: 5, total: 85 },
  windowsTheme: "dark",
  quickGet: { wifi: true, bluetooth: false, brightness: 70 },
  inputLanguage: { language: "ENG", region: "IN", several: true },
};

const ipc = new Proxy({}, { get: (_t, name: string) => vi.fn(async () => answers[name] ?? null) });
vi.mock("../../core/ipc", () => ({ ipc, events: {} }));

const { useWidgets } = await import("../store");
const { useProductivity } = await import("../productivity/store");
const { useSettings } = await import("../../settings/store");
const { CARD_OF, CARD_WIDGETS } = await import("./cardRegistry");

describe("card widgets", () => {
  useWidgets.setState({
    claude: {
      window: {
        startedAt: "2026-11-07T10:00:00Z",
        resetsAt: "2099-01-01T00:00:00Z",
        tokens: 1000,
        messages: 4,
        percent: 64,
        models: [{ model: "claude-opus-5-5", tokens: 900 }],
      },
      todayTokens: 5000,
      weekTokens: 40000,
      usualLimit: 10000,
      found: true,
    },
    stats: { cpu: 9, ram: 69, battery: 80, charging: true },
    sound: { level: 0.46, muted: false, device: "HD 350BT" },
    focus: { phase: "focus", endsAt: "2099-01-01T00:00:00Z", phaseSeconds: 1500, completed: 4 },
    weather: {
      status: "ready",
      temperature: 29,
      condition: "Clear",
      icon: "sun",
      place: "Ahmedabad",
      sunrise: "2026-11-07T06:48",
      sunset: "2026-11-07T17:58",
      days: [
        { date: "2026-11-07", high: 35, low: 23, icon: "sun" },
        { date: "2026-11-08", high: 34, low: 22, icon: "cloud-sun" },
        { date: "2026-11-09", high: 33, low: 22, icon: "rain" },
        { date: "2026-11-10", high: 33, low: 21, icon: "sun" },
        { date: "2026-11-11", high: 32, low: 21, icon: "storm" },
      ],
      hours: [{ time: "2026-11-07T14:00", temperature: 31, icon: "sun" }],
      windKmh: 12,
      rainChance: 10,
      unit: "celsius",
    },
    music: {
      sourceApp: "Spotify",
      sourceIcon: "",
      title: "Afterglow",
      artist: "Lumen",
      album: "",
      status: "playing",
      positionMs: 30000,
      durationMs: 200000,
      positionUpdatedAt: new Date().toISOString(),
      rate: 1,
      capabilities: { playPause: true, next: true, previous: true, seek: true },
    },
    calendar: { events: [], calendars: [], sample: false },
  } as never);
  useProductivity.setState({
    todos: [
      { id: "1", text: "Ship the cards", done: false, created: "" },
      { id: "2", text: "Test", done: true, created: "" },
    ],
    notes: [{ id: "n", text: "Buy milk\nand eggs", updated: "2026-11-07" }],
  } as never);
  useSettings.setState({
    settings: {
      clock24h: false,
      look: { widgetStyle: "cards" },
      cards: {
        worldClocks: ["America/New_York", "Europe/London", "Asia/Tokyo"],
        stocks: ["AAPL", "NVDA"],
        metric: "bitcoin",
        currencyFrom: "USD",
        currencyTo: "INR",
        githubUser: "me",
        leetcodeUser: "me",
        timerMinutes: 10,
      },
    },
  } as never);

  const every = [
    ...Object.entries(CARD_WIDGETS).map(([kind, def]) => [kind, def!.Card] as const),
    ...Object.entries(CARD_OF).map(([kind, Card]) => [`${kind} (card)`, Card!] as const),
  ];

  it.each(every)("%s draws without errors", async (_kind, Card) => {
    const { container } = render(<Card />);
    // Let their first reads answer.
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });
    expect(container.querySelector(".card")).not.toBeNull();
    expect(container.textContent).not.toMatch(/undefined|NaN/);
  });

  it("covers the whole no-setup set", () => {
    expect(Object.keys(CARD_WIDGETS).length).toBeGreaterThanOrEqual(54);
  });
});
