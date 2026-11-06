import { LayoutGrid } from "lucide-react";

import { useSettings } from "../settings/store";
import { WIDGETS } from "../widgets/registry";

/** Every widget on the dock at once, each as its full popup, in a grid. */
export function DashboardApp() {
  const settings = useSettings((s) => s.settings);
  if (!settings) return null;
  const shown = settings.widgets.filter((w) => w.enabled && WIDGETS[w.kind]);
  return (
    <main className="dashboard panel" data-light={settings.theme === "white"}>
      <header className="dashboard-head">
        <LayoutGrid size={18} aria-hidden="true" />
        <h1>Dashboard</h1>
        <span className="dashboard-sub">Everything on your dock, at a glance</span>
      </header>
      {shown.length === 0 ? (
        <p className="dashboard-empty">Add widgets with the › button on the dock, and they show up here.</p>
      ) : (
        <div className="dashboard-grid">
          {shown.map((entry) => {
            const def = WIDGETS[entry.kind]!;
            return (
              <section key={entry.kind} className="dashboard-card" data-kind={entry.kind}>
                <h2 className="dashboard-card-title">{def.label}</h2>
                <div className="dashboard-card-body">
                  <def.Panel />
                </div>
              </section>
            );
          })}
        </div>
      )}
    </main>
  );
}
