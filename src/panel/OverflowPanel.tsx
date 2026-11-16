import type { WidgetKind } from "../core/generated/WidgetKind";
import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { WIDGETS } from "../widgets/registry";
import { WidgetBoundary } from "../widgets/WidgetBoundary";

function report(error: unknown) {
  console.error("overflow action failed", error);
}

/** Where this flyout sits, as a screen x for the popup that replaces it. */
function anchor() {
  return (window.screenX + window.innerWidth / 2) * window.devicePixelRatio;
}

/** The widgets that don't fit on the dock, as live cards; click one for its popup. */
export function OverflowPanel({ kinds }: { kinds: WidgetKind[] }) {
  const cardsMode = useSettings((s) => s.settings?.look.widgetStyle === "cards");
  return (
    <div className="panel-body card-panel overflow-panel">
      <div className="overflow-head">
        <h2 className="panel-title">More widgets</h2>
        <button className="panel-link" onClick={() => ipc.galleryToggle(anchor()).catch(report)}>
          Add or remove…
        </button>
      </div>
      <div className="overflow-grid">
        {kinds.map((kind) => {
          const def = WIDGETS[kind];
          if (!def) return null;
          const open = () => {
            if (def.native) ipc.trayPress(def.native).catch(report);
            else if (!def.noPanel) ipc.panelToggle(kind, anchor()).catch(report);
          };
          return (
            <div
              key={kind}
              className="overflow-item"
              role="button"
              tabIndex={0}
              title={def.label}
              onClick={open}
              onKeyDown={(e) => {
                if (e.key === "Enter") open();
              }}
            >
              <WidgetBoundary name={def.label}>
                {def.Card && (cardsMode || def.cardOnly) ? (
                  <def.Card />
                ) : (
                  <span className="card card-tile overflow-tile">
                    <def.Tile />
                    <span className="card-label">{def.label}</span>
                  </span>
                )}
              </WidgetBoundary>
            </div>
          );
        })}
      </div>
    </div>
  );
}
