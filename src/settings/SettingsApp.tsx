import { useEffect, useState } from "react";
import {
  AppWindow,
  CalendarDays,
  Info,
  LayoutGrid,
  Monitor,
  Palette,
  Zap,
  type LucideIcon,
} from "lucide-react";

import { events } from "../core/ipc";
import { AppSettings } from "./AppSettings";
import { AutomationSettings } from "./AutomationSettings";
import { CalendarSettings } from "./CalendarSettings";
import { AboutSection, AppearanceSection, TaskbarSection } from "./sections";
import { useSettings } from "./store";
import { WidgetSettings } from "./WidgetSettings";

const SECTIONS: { id: string; label: string; icon: LucideIcon }[] = [
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "widgets", label: "Widgets", icon: LayoutGrid },
  { id: "apps", label: "Apps", icon: AppWindow },
  { id: "calendars", label: "Calendars", icon: CalendarDays },
  { id: "automations", label: "Automations", icon: Zap },
  { id: "taskbar", label: "Taskbar & startup", icon: Monitor },
  { id: "about", label: "About", icon: Info },
];

/** The section in the address ("#settings/calendars"), Appearance otherwise. */
function sectionFromHash(): string {
  const id = window.location.hash.split("/")[1];
  return SECTIONS.some((s) => s.id === id) ? id : "appearance";
}

export function SettingsApp() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [section, setSection] = useState(sectionFromHash);

  // Another part of Dock (the calendar popup, say) can ask for a section.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    events
      .onSettingsSection((id) => SECTIONS.some((s) => s.id === id) && setSection(id))
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .catch((error) => console.error("failed to listen for sections", error));
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  if (!settings) return null;
  const save = (patch: Parameters<typeof update>[0]) =>
    update(patch).catch((error) => console.error("failed to save settings", error));

  return (
    <div className="settings-shell">
      <nav className="settings-nav" aria-label="Settings sections">
        <div className="settings-brand">
          <span className="about-logo" /> Dock
        </div>
        {SECTIONS.map(({ id, label, icon: Icon }) => (
          <button
            key={id}
            className="settings-nav-item"
            aria-current={section === id ? "page" : undefined}
            onClick={() => setSection(id)}
          >
            <Icon size={16} aria-hidden="true" />
            {label}
          </button>
        ))}
      </nav>
      <main className="settings">
        {section === "appearance" && <AppearanceSection settings={settings} save={save} />}
        {section === "widgets" && <WidgetSettings settings={settings} save={save} />}
        {section === "apps" && <AppSettings settings={settings} save={save} />}
        {section === "calendars" && <CalendarSettings settings={settings} />}
        {section === "automations" && <AutomationSettings settings={settings} save={save} />}
        {section === "taskbar" && <TaskbarSection settings={settings} save={save} />}
        {section === "about" && <AboutSection />}
      </main>
    </div>
  );
}
