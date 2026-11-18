/* eslint-disable react-refresh/only-export-components -- a dev page, not hot-reloaded components */
/**
 * Every widget on one page, drawn the way the dock and its popup draw them, with sample data.
 * `?state=full|empty|fail`, `?h=48` (dock height), `?view=cards|tiles|panels`, `?only=kind`.
 */
import React from "react";
import ReactDOM from "react-dom/client";

import "../../src/design/tokens.css";
import "../../src/index.css";
import "../../src/widgets/widgets.css";
import "../../src/widgets/productivity/productivity.css";
import "../../src/dock/apps.css";
import "../../src/settings/features.css";
import "../../src/widgets/accounts/accounts.css";
import "../../src/widgets/cards/cards.css";
import "../../src/design/windows.css";
import "./gallery.css";

import { WIDGETS } from "../../src/widgets/registry";
import { useWidgets } from "../../src/widgets/store";
import { useProductivity } from "../../src/widgets/productivity/store";
import { useSettings } from "../../src/settings/store";

const params = new URLSearchParams(window.location.search);
const view = params.get("view") ?? "cards";
const only = params.get("only")?.split(",");
const h = Number(params.get("h") ?? 56);

const view_ = view === "panels" ? "panel" : "dock";
document.documentElement.classList.add(`${view_}-window`, "gallery");

void useSettings.getState().load();
void useWidgets.getState().load();
void useProductivity
  .getState()
  .load()
  .catch(() => {});

class Boundary extends React.Component<{ name: string; children: React.ReactNode }, { error?: string }> {
  state: { error?: string } = {};
  static getDerivedStateFromError(e: Error) {
    return { error: e.message };
  }
  render() {
    return this.state.error ? (
      <span className="gallery-error">CRASH: {this.state.error}</span>
    ) : (
      this.props.children
    );
  }
}

const entries = Object.entries(WIDGETS).filter(([kind]) => !only || only.includes(kind));

function Cards() {
  return (
    <div className="gallery-grid">
      {entries
        .filter(([, def]) => def!.Card)
        .map(([kind, def]) => (
          <figure key={kind} className="gallery-item" data-kind={kind}>
            <figcaption>{kind}</figcaption>
            <div className="dock" data-size="medium" style={{ ["--dock-h" as string]: `${h}px`, height: h }}>
              <div className="dock-content dock-bar-sample">
                <div className="dock-widgets">
                  <button className="dock-widget dock-card" data-kind={kind}>
                    <Boundary name={kind}>{React.createElement(def!.Card!)}</Boundary>
                  </button>
                </div>
              </div>
            </div>
          </figure>
        ))}
    </div>
  );
}

function Tiles() {
  return (
    <div className="gallery-grid">
      {entries
        .filter(([, def]) => !def!.cardOnly)
        .map(([kind, def]) => (
          <figure key={kind} className="gallery-item" data-kind={kind}>
            <figcaption>{kind}</figcaption>
            <div className="dock" data-size="medium" style={{ ["--dock-h" as string]: `${h}px`, height: h }}>
              <div className="dock-content dock-bar-sample">
                <div className="dock-widgets">
                  <button className="dock-widget" data-kind={kind}>
                    <Boundary name={kind}>{React.createElement(def!.Tile)}</Boundary>
                  </button>
                </div>
              </div>
            </div>
          </figure>
        ))}
    </div>
  );
}

function Panels() {
  const seen = new Set<unknown>();
  return (
    <div className="gallery-grid gallery-panels">
      {entries
        .filter(([, def]) => !def!.noPanel && !def!.windowsOnly)
        .filter(([, def]) => (seen.has(def!.Panel) ? false : (seen.add(def!.Panel), true)))
        .map(([kind, def]) => (
          <figure key={kind} className="gallery-item" data-kind={kind}>
            <figcaption>{kind}</figcaption>
            <div className="panel" data-theme="dark" data-kind={kind} data-popup="glass">
              <Boundary name={kind}>{React.createElement(def!.Panel)}</Boundary>
            </div>
          </figure>
        ))}
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  view === "tiles" ? <Tiles /> : view === "panels" ? <Panels /> : <Cards />,
);
