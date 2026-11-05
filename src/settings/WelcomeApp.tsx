import { useState } from "react";
import { ArrowRight, Check } from "lucide-react";

import type { Edge } from "../core/generated/Edge";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import { ipc } from "../core/ipc";
import { WIDGETS } from "../widgets/registry";
import { Choice, StyleCards } from "./controls";
import { useSettings } from "./store";

const STEPS = ["Welcome", "Look", "Widgets"] as const;

/** First run: what Dock does to the taskbar, a style, and which widgets to start with. */
export function WelcomeApp() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [step, setStep] = useState(0);
  if (!settings) return null;
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

      <footer className="welcome-footer">
        {step > 0 ? (
          <button className="settings-link" onClick={() => setStep(step - 1)}>
            Back
          </button>
        ) : (
          <button className="settings-link" onClick={() => finish(false)}>
            Skip
          </button>
        )}
        {step < STEPS.length - 1 ? (
          <button className="welcome-next" onClick={() => setStep(step + 1)}>
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
