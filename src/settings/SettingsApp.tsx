import { useEffect, useLayoutEffect, useState } from "react";
import {
  Briefcase,
  CalendarDays,
  Info,
  Keyboard,
  KeyRound,
  LayoutGrid,
  Monitor,
  Palette,
  Search,
  Zap,
  type LucideIcon,
} from "lucide-react";

import { events } from "../core/ipc";
import { AccountsSection } from "./AccountsSettings";
import { AutomationSettings } from "./AutomationSettings";
import { CalendarSettings } from "./CalendarSettings";
import { ShortcutsSection, WorkspacesSection } from "./FeatureSettings";
import {
  parseTarget,
  searchSettings,
  SECTION_LABELS,
  sectionLabel,
  type SettingsEntry,
  type Target,
} from "./searchIndex";
import { AboutSection, AppearanceSection, TaskbarSection } from "./sections";
import { useSettings } from "./store";
import { WidgetSettings } from "./WidgetSettings";

const ICONS: Record<string, LucideIcon> = {
  appearance: Palette,
  widgets: LayoutGrid,
  calendars: CalendarDays,
  automations: Zap,
  shortcuts: Keyboard,
  workspaces: Briefcase,
  accounts: KeyRound,
  taskbar: Monitor,
  about: Info,
};
const SECTIONS = SECTION_LABELS.map((s) => ({ ...s, icon: ICONS[s.id] ?? Info }));

/** The section in the address ("#settings/calendars"). */
function targetFromHash(): Target {
  return parseTarget(window.location.hash.split("/").slice(1).join("/"));
}

export function SettingsApp() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [target, setTarget] = useState<Target>(targetFromHash);
  const [query, setQuery] = useState("");
  const section = target.section;

  // Another part of Dock (the calendar popup, the command bar) can ask for a section.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    events
      .onSettingsSection((id) => setTarget(parseTarget(id)))
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .catch((error) => console.error("failed to listen for sections", error));
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  // Jumping to a control: bring it into view and flash it.
  useLayoutEffect(() => {
    if (!target.control) return;
    const el = document.querySelector<HTMLElement>(`[data-setting="${CSS.escape(target.control)}"]`);
    if (!el) return;
    el.scrollIntoView?.({ block: "center" });
    el.dataset.found = "true";
    const timer = window.setTimeout(() => delete el.dataset.found, 1600);
    return () => window.clearTimeout(timer);
  }, [target]);

  if (!settings) return null;
  const save = (patch: Parameters<typeof update>[0]) =>
    update(patch).catch((error) => console.error("failed to save settings", error));
  const results = query.trim() ? searchSettings(query) : [];
  const go = (entry: SettingsEntry) => {
    setTarget({ section: entry.section, control: entry.id });
    setQuery("");
  };

  return (
    <div className="settings-shell">
      <nav className="settings-nav" aria-label="Settings sections">
        <div className="settings-brand">
          <span className="about-logo" /> Dock
        </div>
        <div className="settings-search">
          <Search size={14} aria-hidden="true" />
          <input
            type="search"
            placeholder="Search settings"
            aria-label="Search settings"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && results[0]) go(results[0]);
              if (e.key === "Escape") setQuery("");
            }}
          />
        </div>
        {query.trim() ? (
          <ul className="settings-results" aria-label="Search results">
            {results.length === 0 && <li className="settings-hint">Nothing matches.</li>}
            {results.map((entry) => (
              <li key={`${entry.section}/${entry.id ?? ""}/${entry.name}`}>
                <button className="settings-nav-item" onClick={() => go(entry)}>
                  <span>
                    {entry.name}
                    {entry.id !== undefined || entry.name !== sectionLabel(entry.section) ? (
                      <span className="settings-hint">{sectionLabel(entry.section)}</span>
                    ) : null}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        ) : (
          SECTIONS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              className="settings-nav-item"
              aria-current={section === id ? "page" : undefined}
              onClick={() => setTarget({ section: id })}
            >
              <Icon size={16} aria-hidden="true" />
              {label}
            </button>
          ))
        )}
      </nav>
      <main className="settings">
        {section === "appearance" && <AppearanceSection settings={settings} save={save} />}
        {section === "widgets" && <WidgetSettings settings={settings} save={save} />}
        {section === "calendars" && <CalendarSettings settings={settings} />}
        {section === "automations" && <AutomationSettings settings={settings} save={save} />}
        {section === "shortcuts" && <ShortcutsSection settings={settings} />}
        {section === "workspaces" && <WorkspacesSection settings={settings} save={save} />}
        {section === "accounts" && <AccountsSection settings={settings} save={save} />}
        {section === "taskbar" && <TaskbarSection settings={settings} save={save} />}
        {section === "about" && <AboutSection />}
      </main>
    </div>
  );
}
