import { useEffect, useMemo, useState } from "react";
import { Check, LayoutGrid, Monitor, Plus, Search, Undo2 } from "lucide-react";

import type { WidgetEntry } from "../core/generated/WidgetEntry";
import type { WidgetKind } from "../core/generated/WidgetKind";
import type { WidgetSize } from "../core/generated/WidgetSize";
import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { CATEGORY_LABELS, WIDGETS, type WidgetCategory } from "../widgets/registry";
import { galleryGroups } from "./gallery";

const SIZES: [WidgetSize, string][] = [
  ["small", "S"],
  ["normal", "M"],
  ["large", "L"],
];

function report(error: unknown) {
  console.error("gallery change failed", error);
}

/** The widget gallery the dock's › button opens: every widget, with Add/Remove and a size. */
export function GalleryPanel() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const [query, setQuery] = useState("");
  const [canUndo, setCanUndo] = useState(false);

  // Refresh "Undo" whenever the settings change (from here or another window).
  useEffect(() => {
    let alive = true;
    ipc
      .settingsCanUndo()
      .then((can) => alive && setCanUndo(can))
      .catch(report);
    return () => {
      alive = false;
    };
  }, [settings]);

  const groups = useMemo(
    () => (settings ? galleryGroups(settings.widgets, WIDGETS, query) : []),
    [settings, query],
  );
  if (!settings) return null;

  const change = (kind: WidgetKind, patch: Partial<WidgetEntry>) =>
    update({ widgets: settings.widgets.map((w) => (w.kind === kind ? { ...w, ...patch } : w)) }).catch(
      report,
    );
  const added = settings.widgets.filter((w) => w.enabled && WIDGETS[w.kind]).length;

  return (
    <div className="gallery">
      <header className="gallery-head">
        <div>
          <h2 className="gallery-title">Widgets</h2>
          <p className="gallery-sub">{added} on your dock</p>
        </div>
        <div className="gallery-actions">
          <button
            className="gallery-undo"
            title="All your widgets in one window"
            onClick={() => ipc.dashboardOpen().catch(report)}
          >
            <LayoutGrid size={15} aria-hidden="true" />
            Dashboard
          </button>
          <button
            className="gallery-undo"
            disabled={!canUndo}
            title="Undo the last layout change"
            onClick={() => ipc.settingsUndoLayout().catch(report)}
          >
            <Undo2 size={15} aria-hidden="true" />
            Undo
          </button>
        </div>
      </header>
      <label className="gallery-search">
        <Search size={15} aria-hidden="true" />
        <input
          type="search"
          placeholder="Search widgets"
          value={query}
          autoFocus
          onChange={(e) => setQuery(e.target.value)}
          aria-label="Search widgets"
        />
      </label>
      <div className="gallery-list">
        {groups.length === 0 && <p className="gallery-empty">No widgets match "{query}".</p>}
        {groups.map(({ category, entries }) => (
          <section key={category} className="gallery-group">
            <h3 className="gallery-group-title">{CATEGORY_LABELS[category as WidgetCategory]}</h3>
            {entries.map((entry) => {
              const def = WIDGETS[entry.kind]!;
              return (
                <div key={entry.kind} className="gallery-item" data-added={entry.enabled}>
                  <div className="gallery-preview" aria-hidden="true">
                    <def.Tile />
                  </div>
                  <div className="gallery-text">
                    <span className="gallery-name">{def.label}</span>
                    <span className="gallery-desc">{def.description}</span>
                    {entry.enabled && (
                      <div className="gallery-sizes" role="radiogroup" aria-label={`${def.label} size`}>
                        {SIZES.map(([size, text]) => (
                          <button
                            key={size}
                            role="radio"
                            aria-checked={entry.size === size}
                            title={size === "small" ? "Small" : size === "large" ? "Large" : "Medium"}
                            onClick={() => change(entry.kind, { size })}
                          >
                            {text}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                  <button
                    className="gallery-desktop"
                    title="Put on the desktop"
                    aria-label={`Put ${def.label} on the desktop`}
                    onClick={() => ipc.floatingShow(entry.kind, false).catch(report)}
                  >
                    <Monitor size={14} />
                  </button>
                  <button
                    className="gallery-toggle"
                    data-on={entry.enabled}
                    aria-label={entry.enabled ? `Remove ${def.label}` : `Add ${def.label}`}
                    onClick={() => change(entry.kind, { enabled: !entry.enabled })}
                  >
                    {entry.enabled ? <Check size={15} /> : <Plus size={15} />}
                    {entry.enabled ? "Added" : "Add"}
                  </button>
                </div>
              );
            })}
          </section>
        ))}
      </div>
    </div>
  );
}
