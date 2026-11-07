import type { ComponentType } from "react";

import type { WidgetKind } from "../../core/generated/WidgetKind";
import { ClaudePanel } from "../ClaudeWidget";
import { CalendarPanel } from "../CalendarWidget";
import { ClockPanel } from "../ClockWidget";
import { MusicPanel } from "../MusicWidget";
import { StatusPanel } from "../StatusWidget";
import { SystemPanel } from "../SystemWidget";
import { WeatherPanel } from "../WeatherWidget";
import { ClaudeCard, ClaudePlanCard, CodexCard } from "./aiCards";
import {
  CodexPanel,
  CurrencyPanel,
  DownloadsPanel,
  GithubPanel,
  LeetcodePanel,
  MetricPanel,
  RecentFilesPanel,
  ScreenshotsPanel,
  ScreenTimePanel,
  StocksPanel,
  StopwatchPanel,
  StoragePanel,
  TimerPanel,
  WebSearchPanel,
  WorldClocksPanel,
} from "./cardPanels";
import {
  AnalogClassicCard,
  AnalogDarkCard,
  AnalogMinimalCard,
  BigClockCard,
  ClockWeatherCard,
  DateCard,
  DayNumberCard,
  FlipClockCard,
  SegmentClockCard,
  SevenDaysCard,
  TodayCard,
  WorldClockListCard,
  WorldClocksCard,
  WorldTimeCard,
} from "./clockCards";
import { NowPlayingBarCard, NowPlayingCard, NowPlayingCompactCard, NowPlayingMiniCard } from "./mediaCards";
import {
  DownloadsCard,
  GithubActivityCard,
  LeetcodeCard,
  MetricCard,
  RecentFilesCard,
  RecycleBinCard,
  ScreenCaptureCard,
  ScreenshotsCard,
  StockCard,
  StockChartCard,
  StockWatchlistCard,
  StorageCard,
  WebSearchCard,
} from "./onlineCards";
import {
  AudioOutputCard,
  BatteryCard,
  BrightnessCard,
  CpuCard,
  LanguageCard,
  NetSpeedCard,
  QuickTogglesCard,
  RamCard,
  SystemStatsCard,
  TemperatureCard,
  ThemeToggleCard,
  TrayCard,
} from "./systemCards";
import {
  CalendarCard,
  CurrencyCard,
  DayProgressCard,
  DayTimelineCard,
  MonthProgressCard,
  NotesCard,
  PomodoroCard,
  ScreenTimeCard,
  StopwatchCard,
  TimerCard,
  TodosCard,
  YearProgressCard,
} from "./timeCards";
import {
  HourlyCard,
  SunriseSunsetCard,
  TodayForecastCard,
  WeatherCard,
  WeatherDetailsCard,
  WeatherForecastCard,
  WeatherStatusCard,
} from "./weatherCards";

/** The card look of widgets that also have a plain tile (used with "Widget style: Cards"). */
export const CARD_OF: Partial<Record<WidgetKind, ComponentType>> = {
  claude: ClaudeCard,
  music: NowPlayingCard,
  system: SystemStatsCard,
  focus: PomodoroCard,
  todos: TodosCard,
  notes: NotesCard,
  calendar: CalendarCard,
  weather: WeatherCard,
  language: LanguageCard,
  hiddenIcons: TrayCard,
  brightness: BrightnessCard,
};

export type CardCategory =
  "ai" | "productivity" | "system" | "media" | "weather" | "clocks" | "online" | "files";

/** A widget that only comes as a card. */
export interface CardWidget {
  label: string;
  description: string;
  category: CardCategory;
  Card: ComponentType;
  /** Its popup: details or its choices; none means clicking it does nothing. */
  Panel?: ComponentType;
}

/** Every card-only widget, Lumabar's no-setup set. */
export const CARD_WIDGETS: Partial<Record<WidgetKind, CardWidget>> = {
  claudePlan: {
    label: "Claude Plan Usage",
    category: "ai",
    description: "Session and weekly Claude use, plus your most-used model.",
    Card: ClaudePlanCard,
    Panel: ClaudePanel,
  },
  codexUsage: {
    label: "ChatGPT Usage",
    category: "ai",
    description: "Your ChatGPT plan's Codex limits, read from Codex's files on this PC.",
    Card: CodexCard,
    Panel: CodexPanel,
  },
  dayTimeline: {
    label: "Day Timeline",
    category: "productivity",
    description: "Your whole day on one line: time on the PC, focus rounds, meetings, sunrise and sunset.",
    Card: DayTimelineCard,
    Panel: ScreenTimePanel,
  },
  screenTime: {
    label: "Screen Time",
    category: "productivity",
    description: "Today's time at the PC hour by hour, and every app's.",
    Card: ScreenTimeCard,
    Panel: ScreenTimePanel,
  },
  timer: {
    label: "Timer",
    category: "productivity",
    description: "A countdown that starts in one click.",
    Card: TimerCard,
    Panel: TimerPanel,
  },
  stopwatch: {
    label: "Stopwatch",
    category: "productivity",
    description: "Start, pause and lap from the dock; the popup keeps every lap.",
    Card: StopwatchCard,
    Panel: StopwatchPanel,
  },
  currency: {
    label: "Currency",
    category: "productivity",
    description: "Convert between currencies at today's rate.",
    Card: CurrencyCard,
    Panel: CurrencyPanel,
  },
  cpu: {
    label: "CPU",
    category: "system",
    description: "Processor load as a ring.",
    Card: CpuCard,
    Panel: SystemPanel,
  },
  ram: {
    label: "RAM",
    category: "system",
    description: "Memory in use as a ring.",
    Card: RamCard,
    Panel: SystemPanel,
  },
  temperature: {
    label: "Temperature",
    category: "system",
    description: "The PC's temperature as a gauge, with processor and memory load.",
    Card: TemperatureCard,
    Panel: SystemPanel,
  },
  netSpeed: {
    label: "Network",
    category: "system",
    description: "Download and upload speed with a live chart.",
    Card: NetSpeedCard,
  },
  battery: {
    label: "Battery",
    category: "system",
    description: "Charge as a filling gauge, with the charging state.",
    Card: BatteryCard,
    Panel: StatusPanel,
  },
  quickToggles: {
    label: "Quick Toggles",
    category: "system",
    description: "Wi-Fi, Bluetooth and airplane mode.",
    Card: QuickTogglesCard,
  },
  themeToggle: {
    label: "Theme Toggle",
    category: "system",
    description: "Switch Windows between light and dark.",
    Card: ThemeToggleCard,
  },
  dayProgress: {
    label: "Day Progress",
    category: "system",
    description: "How much of today is behind you.",
    Card: DayProgressCard,
  },
  monthProgress: {
    label: "Month Progress",
    category: "system",
    description: "How much of the month is behind you.",
    Card: MonthProgressCard,
  },
  yearProgress: {
    label: "Year Progress",
    category: "system",
    description: "The year as a grid of dots, filling up.",
    Card: YearProgressCard,
  },
  audioOutput: {
    label: "Audio Output",
    category: "media",
    description: "Volume for your speakers or headphones.",
    Card: AudioOutputCard,
    Panel: StatusPanel,
  },
  nowPlayingCompact: {
    label: "Now Playing (compact)",
    category: "media",
    description: "Title, artist and play/pause in a smaller card.",
    Card: NowPlayingCompactCard,
    Panel: MusicPanel,
  },
  nowPlayingMini: {
    label: "Now Playing Mini",
    category: "media",
    description: "One round play button with the track's progress.",
    Card: NowPlayingMiniCard,
    Panel: MusicPanel,
  },
  nowPlayingBar: {
    label: "Now Playing Bar",
    category: "media",
    description: "The track with a progress bar and skip.",
    Card: NowPlayingBarCard,
    Panel: MusicPanel,
  },
  metric: {
    label: "Metric",
    category: "online",
    description: "A live price, such as BTC/USD.",
    Card: MetricCard,
    Panel: MetricPanel,
  },
  githubActivity: {
    label: "GitHub Activity",
    category: "online",
    description: "Your contribution grid and streak (by username).",
    Card: GithubActivityCard,
    Panel: GithubPanel,
  },
  leetcode: {
    label: "LeetCode Stats",
    category: "online",
    description: "Problems solved, by difficulty.",
    Card: LeetcodeCard,
    Panel: LeetcodePanel,
  },
  stock: {
    label: "Stock",
    category: "online",
    description: "One ticker's price and daily change.",
    Card: StockCard,
    Panel: StocksPanel,
  },
  stockChart: {
    label: "Stock Chart",
    category: "online",
    description: "One ticker with today's line.",
    Card: StockChartCard,
    Panel: StocksPanel,
  },
  stockWatchlist: {
    label: "Stock Watchlist",
    category: "online",
    description: "Three tickers at a glance.",
    Card: StockWatchlistCard,
    Panel: StocksPanel,
  },
  recentFiles: {
    label: "Recently Opened",
    category: "files",
    description: "The files you used last.",
    Card: RecentFilesCard,
    Panel: RecentFilesPanel,
  },
  recycleBin: {
    label: "Recycle Bin",
    category: "files",
    description: "What the bin holds, and a click to empty it.",
    Card: RecycleBinCard,
  },
  webSearch: {
    label: "Search",
    category: "files",
    description: "Search the web, or type a folder path to open it.",
    Card: WebSearchCard,
    Panel: WebSearchPanel,
  },
  storage: {
    label: "Storage",
    category: "files",
    description: "Free space on your drive, with a usage bar.",
    Card: StorageCard,
    Panel: StoragePanel,
  },
  screenshots: {
    label: "Recent Screenshots",
    category: "files",
    description: "Your newest screenshots as pictures.",
    Card: ScreenshotsCard,
    Panel: ScreenshotsPanel,
  },
  downloads: {
    label: "Downloads",
    category: "files",
    description: "The newest file in Downloads, one click to open.",
    Card: DownloadsCard,
    Panel: DownloadsPanel,
  },
  screenCapture: {
    label: "Screenshot Capture",
    category: "files",
    description: "Capture the screen, a window or an area.",
    Card: ScreenCaptureCard,
  },
  weatherDetails: {
    label: "Weather Details",
    category: "weather",
    description: "Wind, rain and the day's range.",
    Card: WeatherDetailsCard,
    Panel: WeatherPanel,
  },
  weatherForecast: {
    label: "Weather Forecast",
    category: "weather",
    description: "The next four days.",
    Card: WeatherForecastCard,
    Panel: WeatherPanel,
  },
  todayForecast: {
    label: "Today & Forecast",
    category: "weather",
    description: "Today next to the next three days.",
    Card: TodayForecastCard,
    Panel: WeatherPanel,
  },
  weatherStatus: {
    label: "Weather Status",
    category: "weather",
    description: "The conditions in words, with your town.",
    Card: WeatherStatusCard,
    Panel: WeatherPanel,
  },
  hourly: {
    label: "Hourly",
    category: "weather",
    description: "The next six hours.",
    Card: HourlyCard,
    Panel: WeatherPanel,
  },
  sunriseSunset: {
    label: "Sunrise & Sunset",
    category: "weather",
    description: "Where the sun is between sunrise and sunset.",
    Card: SunriseSunsetCard,
    Panel: WeatherPanel,
  },
  bigClock: {
    label: "Clock",
    category: "clocks",
    description: "Large, readable time with its seconds and the date.",
    Card: BigClockCard,
    Panel: ClockPanel,
  },
  flipClock: {
    label: "Flip Clock",
    category: "clocks",
    description: "Split-flap digits that turn over every minute.",
    Card: FlipClockCard,
    Panel: ClockPanel,
  },
  worldClocks: {
    label: "World Clocks",
    category: "clocks",
    description: "Three small analog faces for the cities you work with.",
    Card: WorldClocksCard,
    Panel: WorldClocksPanel,
  },
  analogClassic: {
    label: "Analog Classic",
    category: "clocks",
    description: "A white analog face with a sweeping second hand.",
    Card: AnalogClassicCard,
    Panel: ClockPanel,
  },
  analogDark: {
    label: "Analog Classic Dark",
    category: "clocks",
    description: "The same classic face, in black.",
    Card: AnalogDarkCard,
    Panel: ClockPanel,
  },
  sevenDays: {
    label: "7 Days",
    category: "clocks",
    description: "This week as a strip of days, with today's temperature.",
    Card: SevenDaysCard,
    Panel: ClockPanel,
  },
  worldClockList: {
    label: "World Clock",
    category: "clocks",
    description: "Three cities in a list, each with its hour difference.",
    Card: WorldClockListCard,
    Panel: WorldClocksPanel,
  },
  worldTime: {
    label: "World Time",
    category: "clocks",
    description: "One more city in big digits, with its offset.",
    Card: WorldTimeCard,
    Panel: WorldClocksPanel,
  },
  today: {
    label: "Today",
    category: "clocks",
    description: "Weekday, date, time and weather in a single card.",
    Card: TodayCard,
    Panel: CalendarPanel,
  },
  date: {
    label: "Date",
    category: "clocks",
    description: "The full date, set large.",
    Card: DateCard,
    Panel: ClockPanel,
  },
  dayNumber: {
    label: "Day Number",
    category: "clocks",
    description: "Today's date as a calendar page.",
    Card: DayNumberCard,
    Panel: ClockPanel,
  },
  analogMinimal: {
    label: "Analog Minimal",
    category: "clocks",
    description: "Hands and hour ticks on a dark face, nothing else.",
    Card: AnalogMinimalCard,
    Panel: ClockPanel,
  },
  segmentClock: {
    label: "Segment Clock",
    category: "clocks",
    description: "The time as a seven-segment display, under the weekday and date.",
    Card: SegmentClockCard,
    Panel: ClockPanel,
  },
  clockWeather: {
    label: "Clock & Weather",
    category: "clocks",
    description: "Time, date and temperature as three colour blocks.",
    Card: ClockWeatherCard,
    Panel: WeatherPanel,
  },
};
