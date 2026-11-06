import React from "react";
import ReactDOM from "react-dom/client";

import { BubbleApp } from "./bubble/BubbleApp";
import { Dock } from "./dock/Dock";
import { useApps } from "./dock/apps";
import { useGlass } from "./dock/glass/store";
import { isMainDock } from "./core/ipc";
import { PanelApp } from "./panel/PanelApp";
import { PreviewApp, SwitcherApp } from "./preview/PreviewApp";
import { initAppFeatures, useLaunching } from "./dock/appFeatures";
import { SettingsApp } from "./settings/SettingsApp";
import { WelcomeApp } from "./settings/WelcomeApp";
import { useSettings } from "./settings/store";
import { useWidgets } from "./widgets/store";
import "./design/tokens.css";
import "./index.css";
import "./widgets/widgets.css";
import "./dock/apps.css";

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
        : hash.startsWith("#preview")
          ? "preview"
          : hash.startsWith("#switcher")
            ? "switcher"
            : "dock";
document.documentElement.classList.add(`${page}-window`);

useSettings.getState().load().catch(report("settings"));
// Widgets live on the main dock only; docks on other screens show just their screen's windows.
const widgets = page === "panel" || (page === "dock" && isMainDock());
if (widgets) useWidgets.getState().load().catch(report("widgets"));
if (page === "dock") {
  useApps.getState().load().catch(report("apps"));
  useGlass.getState().load().catch(report("the glass"));
  initAppFeatures(() => useApps.getState().apps).catch(report("app features"));
  useApps.subscribe((s) => useLaunching.getState().settle(s.apps));
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
    ) : page === "preview" ? (
      <PreviewApp />
    ) : page === "switcher" ? (
      <SwitcherApp />
    ) : (
      <Dock main={isMainDock()} />
    )}
  </React.StrictMode>,
);
