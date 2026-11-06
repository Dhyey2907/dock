import type { AppsSettings } from "../core/generated/AppsSettings";
import type { DockApp } from "../core/generated/DockApp";
import type { DockWindow } from "../core/generated/DockWindow";

/** The backend's defaults for the apps settings. */
export const defaultApps: AppsSettings = {
  folders: [],
  customPins: [],
  overrides: [],
  hidden: [],
  iconPack: "",
  sortByUse: false,
  separators: [],
  numberShortcuts: true,
  launchBounce: true,
  attentionFlash: true,
  previews: true,
  altTab: false,
  edgeSnap: true,
  minimizeAnimation: true,
  showDesktopButton: true,
  peek: true,
  doubleClick: "showDesktop",
  pressAndHold: "nothing",
  launchPresets: [],
  quickActions: [],
  reopenOnLastScreen: true,
  rearrangeOnUnplug: true,
};

export function dockWindow(hwnd: number, extra: Partial<DockWindow> = {}): DockWindow {
  return { hwnd, title: `w${hwnd}`, active: false, minimized: false, hung: false, flashing: false, ...extra };
}

export function dockApp(key: string, extra: Partial<DockApp> = {}): DockApp {
  return {
    key,
    name: key,
    pinned: true,
    icon: "",
    windows: [],
    kind: "app",
    attention: false,
    hung: false,
    dividerAfter: false,
    ...extra,
  };
}
