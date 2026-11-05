import React from "react";
import ReactDOM from "react-dom/client";

import { Dock } from "./dock/Dock";
import { useApps } from "./dock/apps";
import { useGlass } from "./dock/glass/store";
import { SettingsApp } from "./settings/SettingsApp";
import { useSettings } from "./settings/store";
import "./design/tokens.css";
import "./index.css";

useSettings
  .getState()
  .load()
  .catch((error) => console.error("failed to load settings", error));

// One bundle serves both windows: the dock (default) and settings (#settings).
const isSettingsWindow = window.location.hash.startsWith("#settings");
if (!isSettingsWindow) {
  useApps
    .getState()
    .load()
    .catch((error) => console.error("failed to load apps", error));
  useGlass
    .getState()
    .load()
    .catch((error) => console.error("failed to load the glass", error));
}
document.documentElement.classList.add(isSettingsWindow ? "settings-window" : "dock-window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{isSettingsWindow ? <SettingsApp /> : <Dock />}</React.StrictMode>,
);
