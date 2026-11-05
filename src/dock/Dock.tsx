import { useLayoutEffect, useRef, useState } from "react";
import { PanelBottom, Search, Settings2 } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { AppButton } from "./AppButton";
import { useApps } from "./apps";
import { Glass } from "./glass/Glass";
import type { Ink } from "./glass/styles";
import { WidgetBar } from "../widgets/WidgetBar";
import { useAutoHide } from "./useAutoHide";

/** Four squares, like the Windows key. */
function StartMark() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <rect x="1" y="1" width="7.4" height="7.4" rx="1.4" />
      <rect x="9.6" y="1" width="7.4" height="7.4" rx="1.4" />
      <rect x="1" y="9.6" width="7.4" height="7.4" rx="1.4" />
      <rect x="9.6" y="9.6" width="7.4" height="7.4" rx="1.4" />
    </svg>
  );
}

/**
 * The dock bar. A pill reports its content width so the window can hug it. `main` is the dock
 * with the widgets; docks on other screens (`main` false) leave them out.
 */
export function Dock({ main = true }: { main?: boolean }) {
  const settings = useSettings((s) => s.settings);
  const apps = useApps((s) => s.apps);
  const contentRef = useRef<HTMLDivElement>(null);
  const { revealed, onEnter, onLeave } = useAutoHide(settings?.autoHide ?? false);
  const loaded = settings !== null;
  // Light or dark text, following the wallpaper behind the glass.
  const [ink, setInk] = useState<Ink>("light");

  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const report = () => {
      ipc
        .dockSetContentWidth(Math.ceil(content.scrollWidth))
        .catch((error) => console.error("failed to report dock width", error));
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(content);
    return () => observer.disconnect();
  }, [loaded]);

  if (!settings) return null;
  return (
    <div
      className="dock"
      data-theme={settings.theme}
      data-style={settings.style}
      data-edge={settings.edge}
      data-size={settings.size}
      data-revealed={revealed}
      data-auto-hide={settings.autoHide}
      data-ink={ink}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      {/* While tucked away the window is a thin strip; this catches the cursor. */}
      {!revealed && <div className="dock-sliver" data-testid="dock-sliver" />}
      {/* Dragging the bar's background moves the dock; dropping it nearer the top or bottom
          snaps it to that edge. Buttons inside stay clickable. */}
      <div className="dock-content" ref={contentRef} data-tauri-drag-region>
        <Glass theme={settings.theme} rounded={settings.style === "pill"} onInk={setInk} />
        <button
          className="dock-icon-button dock-start"
          aria-label="Start"
          title="Start"
          onClick={() => ipc.shellStart().catch((error) => console.error(error))}
        >
          <StartMark />
        </button>
        <button
          className="dock-icon-button"
          aria-label="Search"
          title="Search"
          onClick={() => ipc.shellSearch().catch((error) => console.error(error))}
        >
          <Search size={18} strokeWidth={2} />
        </button>
        <div className="dock-divider" data-tauri-drag-region />
        <div className="dock-apps" data-tauri-drag-region>
          {apps.map((app) => (
            <AppButton key={app.key} app={app} />
          ))}
        </div>
        <div className="dock-spacer" data-tauri-drag-region />
        {main && <WidgetBar />}
        <button
          className="dock-icon-button"
          aria-label="Switch to the Windows taskbar"
          title={`Switch to the Windows taskbar
Bring Dock back with the tray icon${settings?.toggleShortcut ? ` or ${settings.toggleShortcut}` : ""}`}
          onClick={() => ipc.switchToWindowsTaskbar().catch((error) => console.error(error))}
        >
          <PanelBottom size={18} strokeWidth={1.8} />
        </button>
        <button
          className="dock-icon-button"
          aria-label="Settings"
          title="Settings"
          onClick={() => ipc.openSettings().catch((error) => console.error(error))}
        >
          <Settings2 size={18} strokeWidth={1.8} />
        </button>
      </div>
    </div>
  );
}
