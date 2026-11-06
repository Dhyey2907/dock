import React from "react";
import ReactDOM from "react-dom/client";

import { BubbleApp } from "./bubble/BubbleApp";
import { Dock } from "./dock/Dock";
import { useApps } from "./dock/apps";
import { useGlass } from "./dock/glass/store";
import { isMainDock } from "./core/ipc";
import { DashboardApp } from "./panel/DashboardApp";
import { FloatApp } from "./panel/FloatApp";
import { PanelApp } from "./panel/PanelApp";
import { SettingsApp } from "./settings/SettingsApp";
import { WelcomeApp } from "./settings/WelcomeApp";
import { useSettings } from "./settings/store";
import { useWidgets } from "./widgets/store";
import "./design/tokens.css";
import "./index.css";
import "./widgets/widgets.css";

function report(what: string) {
  return (error: unknown) => console.error(`failed to load ${what}`, error);
}

// One bundle serves every window: the dock (default), the widget popup (#panel), the reminder
// cards (#bubble), settings (#settings) and the first-run welcome (#welcome).
const hash = window.location.hash;
const page = hash.startsWith("#settings")
  ? "settings"
  : hash.startsWith("#panel")
    ? "panel"
    : hash.startsWith("#bubble")
      ? "bubble"
      : hash.startsWith("#welcome")
        ? "welcome"
        : hash.startsWith("#float")
          ? "float"
          : hash.startsWith("#dashboard")
            ? "dashboard"
            : "dock";
document.documentElement.classList.add(`${page}-window`);

useSettings.getState().load().catch(report("settings"));
// Widgets live on the main dock only; docks on other screens show just their screen's windows.
const widgets =
  page === "panel" || page === "float" || page === "dashboard" || (page === "dock" && isMainDock());
if (widgets) useWidgets.getState().load().catch(report("widgets"));
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
    ) : page === "welcome" ? (
      <WelcomeApp />
    ) : page === "float" ? (
      <FloatApp />
    ) : page === "dashboard" ? (
      <DashboardApp />
    ) : (
      <Dock main={isMainDock()} />
    )}
  </React.StrictMode>,
);
