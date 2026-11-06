import { rank } from "../command/rank";

/** A section of the Settings window. */
export interface SettingsSection {
  id: string;
  label: string;
}

/** A section or control that settings search (and the command bar) can find. */
export interface SettingsEntry {
  section: string;
  /** The control's `data-setting` id within the section; absent for the section itself. */
  id?: string;
  name: string;
  keywords?: string[];
}

export const SECTION_LABELS: SettingsSection[] = [
  { id: "appearance", label: "Appearance" },
  { id: "widgets", label: "Widgets" },
  { id: "apps", label: "Apps" },
  { id: "calendars", label: "Calendars" },
  { id: "automations", label: "Automations" },
  { id: "shortcuts", label: "Shortcuts" },
  { id: "workspaces", label: "Workspaces & screenshots" },
  { id: "accounts", label: "Accounts" },
  { id: "taskbar", label: "Taskbar & startup" },
  { id: "about", label: "About" },
];

const CONTROLS: SettingsEntry[] = [
  {
    section: "appearance",
    id: "dockSize",
    name: "Dock size",
    keywords: ["bigger", "smaller", "scale", "height"],
  },
  {
    section: "appearance",
    id: "transparency",
    name: "Transparency",
    keywords: ["see-through", "opacity", "glass"],
  },
  { section: "appearance", id: "blur", name: "Blur", keywords: ["frosted", "glass"] },
  { section: "appearance", id: "magnify", name: "Magnify on hover", keywords: ["zoom", "grow", "icons"] },
  { section: "appearance", id: "corners", name: "Corners", keywords: ["round", "radius", "pill"] },
  {
    section: "apps",
    name: "Apps",
    keywords: ["pin", "website", "stack", "icon", "rename", "hide", "alt tab", "preview"],
  },
  {
    section: "appearance",
    id: "theme",
    name: "Dock style",
    keywords: ["theme", "glass", "dark", "light", "black", "white", "midnight", "colour", "color"],
  },
  { section: "appearance", id: "edge", name: "Position", keywords: ["top", "bottom", "edge", "move"] },
  { section: "appearance", id: "style", name: "Shape", keywords: ["pill", "full width", "floating"] },
  {
    section: "appearance",
    id: "size",
    name: "Size",
    keywords: ["bigger", "smaller", "small", "large", "height"],
  },
  {
    section: "appearance",
    id: "docksOn",
    name: "Docks on",
    keywords: ["every screen", "monitor", "display", "multiple"],
  },
  { section: "appearance", id: "screen", name: "Screen", keywords: ["monitor", "display", "main dock"] },
  { section: "widgets", name: "Turn widgets on or off", keywords: ["order", "enable", "disable", "reorder"] },
  {
    section: "calendars",
    name: "Add a calendar",
    keywords: ["ical", "google calendar", "outlook", "icloud", "link"],
  },
  { section: "automations", name: "Meeting mode", keywords: ["reminder", "pause music"] },
  { section: "automations", name: "Claude limit warnings", keywords: ["usage", "ai"] },
  { section: "automations", name: "Focus music", keywords: ["timer", "pomodoro"] },
  { section: "automations", name: "Sunset dark mode", keywords: ["night", "theme"] },
  { section: "automations", name: "Quiet hours", keywords: ["do not disturb", "silence", "reminders"] },
  {
    section: "shortcuts",
    id: "toggle",
    name: "Show or hide the dock shortcut",
    keywords: ["hotkey", "keyboard", "ctrl alt d"],
  },
  {
    section: "shortcuts",
    id: "commandBar",
    name: "Command bar shortcut",
    keywords: ["hotkey", "keyboard", "ctrl space", "search"],
  },
  {
    section: "shortcuts",
    id: "quickCapture",
    name: "Quick capture shortcut",
    keywords: ["hotkey", "keyboard", "note"],
  },
  {
    section: "workspaces",
    id: "save",
    name: "Save a workspace",
    keywords: ["open apps", "layout", "windows", "restore"],
  },
  { section: "accounts", id: "anthropic", name: "Ask Claude API key", keywords: ["anthropic", "ai", "key"] },
  {
    section: "accounts",
    id: "spotify",
    name: "Spotify sign-in",
    keywords: ["music", "playlists", "client id"],
  },
  { section: "accounts", id: "notion", name: "Notion tasks", keywords: ["token", "database", "todo"] },
  { section: "accounts", id: "google", name: "Google Tasks sign-in", keywords: ["todo", "client id"] },
  {
    section: "accounts",
    id: "github",
    name: "GitHub Actions",
    keywords: ["ci", "token", "repos", "workflow"],
  },
  {
    section: "workspaces",
    id: "screenshot",
    name: "Latest screenshot on the dock",
    keywords: ["snip", "print screen", "drag"],
  },
  { section: "taskbar", id: "taskbarMode", name: "Windows taskbar", keywords: ["hide", "peek", "dock only"] },
  { section: "taskbar", id: "autoHide", name: "Auto-hide the dock", keywords: ["slide away", "hide"] },
  {
    section: "taskbar",
    id: "launchAtStartup",
    name: "Launch at sign-in",
    keywords: ["startup", "boot", "autostart"],
  },
  { section: "taskbar", id: "quit", name: "Quit Dock", keywords: ["exit", "close"] },
  { section: "about", name: "Version and privacy", keywords: ["data", "about"] },
];

/** Every section and control that search can find. */
export const SETTINGS_INDEX: SettingsEntry[] = [
  ...SECTION_LABELS.map((s) => ({ section: s.id, name: s.label })),
  ...CONTROLS,
];

/** The best matches for `query`. */
export function searchSettings(query: string, limit = 8): SettingsEntry[] {
  return rank(query, SETTINGS_INDEX, limit);
}

/** Every entry, for searches that rank them along with other things. */
export function searchSettingsAll(): SettingsEntry[] {
  return SETTINGS_INDEX;
}

/** The name of a section, for showing where a result lives. */
export function sectionLabel(id: string): string {
  return SECTION_LABELS.find((s) => s.id === id)?.label ?? id;
}

/** A place in Settings: a section, and maybe one control in it. */
export interface Target {
  section: string;
  control?: string;
}

/** "taskbar/autoHide" → the Taskbar section's auto-hide row; unknown sections → Appearance. */
export function parseTarget(text: string | undefined): Target {
  const [section, control] = (text ?? "").split("/");
  return SECTION_LABELS.some((s) => s.id === section) ? { section, control } : { section: "appearance" };
}
