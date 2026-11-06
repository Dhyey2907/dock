import { useState } from "react";
import {
  ArrowRight,
  Briefcase,
  Check,
  Command,
  Image,
  KeyRound,
  LayoutGrid,
  PanelBottom,
  type LucideIcon,
} from "lucide-react";

import type { Edge } from "../core/generated/Edge";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import { ipc } from "../core/ipc";
import { WIDGETS } from "../widgets/registry";
import { Choice, StyleCards } from "./controls";
import { shortcutLabel } from "./shortcut";
import { useSettings } from "./store";

const STEPS = ["Welcome", "Look", "Widgets", "Tour"] as const;
const TOUR_STEP = STEPS.indexOf("Tour");

interface Slide {
  icon: LucideIcon;
  title: string;
  text: (keys: { toggle: string; command: string }) => string;
}

/** The tour: one main feature per card. */
const TOUR: Slide[] = [
  {
    icon: Command,
    title: "The command bar",
    text: ({ command }) =>
      `Press ${command} anywhere to find apps, files, settings, notes and workspaces. Type a sum like 18% * 240 to see the answer; Enter copies it.`,
  },
  {
    icon: LayoutGrid,
    title: "Widgets",
    text: () =>
      "Click a widget on the dock for more, drag it sideways to move it, and turn widgets on or off in Settings → Widgets.",
  },
  {
    icon: Briefcase,
    title: "Workspaces",
    text: () =>
      "Save the apps you have open, and where their windows are, under a name. Restore it later from the command bar or Settings.",
  },
  {
    icon: Image,
    title: "Your latest screenshot",
    text: () =>
      "After Win+PrtScn or the Snipping Tool saves a picture, it sits on the dock for two minutes. Drag it straight into an email or chat.",
  },
  {
    icon: KeyRound,
    title: "Your accounts",
    text: () =>
      "Ask Claude, Spotify, Notion, Google Tasks and GitHub Actions widgets work with your own keys and sign-ins, kept in Windows Credential Manager. Set them up in Settings → Accounts.",
  },
  {
    icon: PanelBottom,
    title: "Back to the taskbar any time",
    text: ({ toggle }) =>
      `${toggle} or the tray icon switches between Dock and the Windows taskbar. Every shortcut can be changed in Settings → Shortcuts.`,
  },
];

/** First run: what Dock does to the taskbar, a style, and which widgets to start with. */
export function WelcomeApp() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [step, setStep] = useState(0);
  const [slide, setSlide] = useState(0);
  if (!settings) return null;
  const keys = {
    toggle: settings.toggleShortcut ? shortcutLabel(settings.toggleShortcut) : "The tray icon",
    command: settings.hotkeys.commandBar
      ? shortcutLabel(settings.hotkeys.commandBar)
      : "the command bar shortcut",
  };
  const onTour = step === TOUR_STEP;
  const lastSlide = slide === TOUR.length - 1;
  const back = () => (onTour && slide > 0 ? setSlide(slide - 1) : setStep(step - 1));
  const next = () => (onTour ? setSlide(Math.min(slide + 1, TOUR.length - 1)) : setStep(step + 1));
  const Slide = TOUR[slide];
  const save = (patch: Parameters<typeof update>[0]) =>
    update(patch).catch((error) => console.error("failed to save settings", error));
  const finish = (openSettings: boolean) =>
    ipc.onboardingDone(openSettings).catch((error) => console.error("failed to finish the welcome", error));
  const toggle = (entry: WidgetEntry) =>
    save({
      widgets: settings.widgets.map((w) => (w.kind === entry.kind ? { ...w, enabled: !w.enabled } : w)),
    });

  return (
    <main className="welcome">
      <ol className="welcome-steps" aria-label="Steps">
        {STEPS.map((name, i) => (
          <li key={name} data-state={i < step ? "done" : i === step ? "now" : "next"}>
            {i < step ? <Check size={12} strokeWidth={3} /> : i + 1} {name}
          </li>
        ))}
      </ol>

      {step === 0 && (
        <section className="welcome-page">
          <span className="about-logo welcome-logo" />
          <h1>Your taskbar, made smarter</h1>
          <p>
            Dock sits where the taskbar was: your pinned apps and open windows, plus live widgets — music,
            weather, your calendar, Claude usage — and reminders when they matter.
          </p>
          <p className="welcome-note">
            While Dock runs, the Windows taskbar auto-hides (touch the bottom edge to see it). Quit Dock from
            its tray icon or Settings and the taskbar comes straight back.
          </p>
        </section>
      )}

      {step === 1 && (
        <section className="welcome-page">
          <h1>Pick a look</h1>
          <p>The glass styles show your wallpaper through the dock. You can change this any time.</p>
          <StyleCards value={settings.theme} onChange={(theme) => save({ theme })} />
          <div className="welcome-row">
            <span>Position</span>
            <Choice<Edge>
              label="Position"
              value={settings.edge}
              options={[
                ["bottom", "Bottom"],
                ["top", "Top"],
              ]}
              onChange={(edge) => save({ edge })}
            />
          </div>
        </section>
      )}

      {step === 2 && (
        <section className="welcome-page">
          <h1>Choose your widgets</h1>
          <p>They appear on the right of the dock. Click one for more; drag it to move it.</p>
          <ul className="welcome-widgets">
            {settings.widgets
              .filter((w) => WIDGETS[w.kind])
              .map((entry) => {
                const def = WIDGETS[entry.kind]!;
                return (
                  <li key={entry.kind}>
                    <label>
                      <input type="checkbox" checked={entry.enabled} onChange={() => toggle(entry)} />
                      <span>
                        <strong>{def.label}</strong>
                        <span className="settings-hint">{def.description}</span>
                      </span>
                    </label>
                  </li>
                );
              })}
          </ul>
        </section>
      )}

      {onTour && (
        <section className="welcome-page">
          <h1>A quick tour</h1>
          <p>
            {slide + 1} of {TOUR.length}
          </p>
          <div className="tour-card" aria-live="polite">
            <span className="tour-icon">
              <Slide.icon size={22} />
            </span>
            <span>
              <h2>{Slide.title}</h2>
              <p>{Slide.text(keys)}</p>
            </span>
          </div>
          <div className="tour-dots" role="group" aria-label="Tour steps">
            {TOUR.map((t, i) => (
              <button
                key={t.title}
                aria-label={t.title}
                aria-current={i === slide ? "step" : undefined}
                onClick={() => setSlide(i)}
              />
            ))}
          </div>
        </section>
      )}

      <footer className="welcome-footer">
        {step > 0 ? (
          <button className="settings-link" onClick={back}>
            Back
          </button>
        ) : (
          <button className="settings-link" onClick={() => finish(false)}>
            Skip
          </button>
        )}
        {step < STEPS.length - 1 || !lastSlide ? (
          <button className="welcome-next" onClick={next}>
            Next <ArrowRight size={16} />
          </button>
        ) : (
          <span className="welcome-actions">
            <button className="settings-link" onClick={() => finish(true)}>
              More settings
            </button>
            <button className="welcome-next" onClick={() => finish(false)}>
              Done <Check size={16} />
            </button>
          </span>
        )}
      </footer>
    </main>
  );
}
