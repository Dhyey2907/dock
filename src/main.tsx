import React from "react";
import ReactDOM from "react-dom/client";

import { BubbleApp } from "./bubble/BubbleApp";
import { Dock } from "./dock/Dock";
import { useApps } from "./dock/apps";
import { useGlass } from "./dock/glass/store";
import { isMainDock } from "./core/ipc";
import { PanelApp } from "./panel/PanelApp";
import { SettingsApp } from "./settings/SettingsApp";
import { WelcomeApp } from "./settings/WelcomeApp";
import { useSettings } from "./settings/store";
import { useWidgets } from "./widgets/store";
import { PinApp, SnipApp } from "./widgets/productivity/SnipApp";
import { useProductivity } from "./widgets/productivity/store";
import { enableTextDrop } from "./widgets/productivity/textDrop";
import "./design/tokens.css";
import "./index.css";
import "./widgets/widgets.css";
import "./widgets/productivity/productivity.css";

function report(what: string) {
  return (error: unknown) => console.error(`failed to load ${what}`, error);
}

// One bundle serves every window: the dock (default), the widget popup (#panel), the reminder
// cards (#bubble), settings (#settings), the first-run welcome (#welcome), the screen-area
// picker (#snip) and pinned pictures (#pin-<n>).
const hash = window.location.hash;
const page = hash.startsWith("#settings")
  ? "settings"
  : hash.startsWith("#panel")
    ? "panel"
    : hash.startsWith("#bubble")
      ? "bubble"
      : hash.startsWith("#welcome")
        ? "welcome"
        : hash.startsWith("#snip")
          ? "snip"
          : hash.startsWith("#pin-")
            ? "pin"
            : "dock";
document.documentElement.classList.add(`${page}-window`);

useSettings.getState().load().catch(report("settings"));
// Widgets live on the main dock only; docks on other screens show just their screen's windows.
const widgets = page === "panel" || (page === "dock" && isMainDock());
if (widgets) {
  useWidgets.getState().load().catch(report("widgets"));
  useProductivity.getState().load().catch(report("productivity widgets"));
}
// Text dropped on the main dock becomes a note.
if (page === "dock" && isMainDock()) enableTextDrop();
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
    ) : page === "snip" ? (
      <SnipApp />
    ) : page === "pin" ? (
      <PinApp id={Number(hash.slice("#pin-".length))} />
    ) : (
      <Dock main={isMainDock()} />
    )}
  </React.StrictMode>,
);
