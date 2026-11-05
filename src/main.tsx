import React from "react";
import ReactDOM from "react-dom/client";

import { BubbleApp } from "./bubble/BubbleApp";
import { Dock } from "./dock/Dock";
import { useApps } from "./dock/apps";
import { useGlass } from "./dock/glass/store";
import { PanelApp } from "./panel/PanelApp";
import { SettingsApp } from "./settings/SettingsApp";
import { useSettings } from "./settings/store";
import { useWidgets } from "./widgets/store";
import "./design/tokens.css";
import "./index.css";
import "./widgets/widgets.css";

function report(what: string) {
  return (error: unknown) => console.error(`failed to load ${what}`, error);
}

// One bundle serves every window: the dock (default), the widget popup (#panel), the reminder
// cards (#bubble) and settings.
const hash = window.location.hash;
const page = hash.startsWith("#settings")
  ? "settings"
  : hash.startsWith("#panel")
    ? "panel"
    : hash.startsWith("#bubble")
      ? "bubble"
      : "dock";
document.documentElement.classList.add(`${page}-window`);

useSettings.getState().load().catch(report("settings"));
if (page === "dock" || page === "panel") useWidgets.getState().load().catch(report("widgets"));
if (page === "dock") {
  useApps.getState().load().catch(report("apps"));
  useGlass.getState().load().catch(report("the glass"));
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {page === "settings" ? (
      <SettingsApp />
    ) : page === "panel" ? (
      <PanelApp />
    ) : page === "bubble" ? (
      <BubbleApp />
    ) : (
      <Dock />
    )}
  </React.StrictMode>,
);
