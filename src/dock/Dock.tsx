import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { Columns2, PanelBottom, Search, Settings2 } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { AppButton } from "./AppButton";
import { FolderButton } from "./FolderButton";
import { rowItems } from "./folders";
import { ShowDesktopButton } from "./DockExtras";
import { useDragOverRaise, useEmptyDockClicks, useReportButtonRects } from "./dockHooks";
import { useApps } from "./apps";
import { Glass } from "./glass/Glass";
import { ScreenshotThumb } from "./ScreenshotThumb";
import type { Ink } from "./glass/styles";
import { WidgetBar } from "../widgets/WidgetBar";
import { useAutoHide } from "./useAutoHide";
import { fitShrink, lookStyle, roomFor } from "./look";
import { Fluent } from "../core/Fluent";
import { useSystemLook, useWatchSystemLook } from "../core/system";
import { useTooltip } from "./tooltip";

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

/** Windows 11's Start mark: four tiles in the accent blue, lit from the top left. */
function WindowsStartMark() {
  return (
    <svg width="22" height="22" viewBox="0 0 22 22" aria-hidden="true">
      <defs>
        <linearGradient id="start-tile" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#4cc2ff" />
          <stop offset="1" stopColor="#0078d4" />
        </linearGradient>
      </defs>
      <rect x="1" y="1" width="9.4" height="9.4" rx="1" fill="url(#start-tile)" />
      <rect x="11.6" y="1" width="9.4" height="9.4" rx="1" fill="url(#start-tile)" />
      <rect x="1" y="11.6" width="9.4" height="9.4" rx="1" fill="url(#start-tile)" />
      <rect x="11.6" y="11.6" width="9.4" height="9.4" rx="1" fill="url(#start-tile)" />
    </svg>
  );
}

function report(error: unknown) {
  console.error(error);
}

/** A dock button with a Windows-style label on hover. */
function BarButton({
  label,
  className = "dock-icon-button",
  onClick,
  children,
}: {
  label: string;
  className?: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  const tooltip = useTooltip(label);
  return (
    <button className={className} aria-label={label} onClick={onClick} {...tooltip}>
      {children}
    </button>
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
  // Auto-shrink: how much smaller than the size slider the dock draws to fit the screen.
  const [shrink, setShrink] = useState(1);
  // The rise-in plays once, when the dock first appears.
  const [starting, setStarting] = useState(true);
  useEffect(() => {
    const timer = window.setTimeout(() => setStarting(false), 900);
    return () => window.clearTimeout(timer);
  }, []);
  const emptyClicks = useEmptyDockClicks();
  useWatchSystemLook();
  const system = useSystemLook((s) => s.look);
  useDragOverRaise();
  useReportButtonRects(apps, contentRef);

  useLayoutEffect(() => {
    const content = contentRef.current;
    if (!content) return;
    const report = () => {
      ipc
        .dockSetContentWidth(Math.ceil(content.scrollWidth))
        .catch((error) => console.error("failed to report dock width", error));
      setShrink((current) => fitShrink(current, content.scrollWidth, window.screen.width));
    };
    report();
    const observer = new ResizeObserver(report);
    observer.observe(content);
    return () => observer.disconnect();
  }, [loaded]);

  if (!settings) return null;
  const split = settings.style === "split";
  const corners = settings.look.corners / 100;
  const windowsLook = settings.theme === "windows";
  // Apps in the middle of a full-width bar: widgets split around them, like Windows 11.
  const centred = settings.style === "full" && settings.look.centerApps;
  const look = settings.look;
  const style = {
    ...lookStyle(settings, settings.autoShrink && settings.style !== "full" ? shrink : 1),
    ...(windowsLook && system ? { "--system-accent": system.accent } : {}),
  } as CSSProperties;
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
      data-room={roomFor(settings)}
      data-magnify={settings.look.magnify}
      data-startup={starting && settings.look.startupAnimation}
      data-center={look.centerApps}
      data-system-light={windowsLook && (system?.light ?? false)}
      style={style}
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      {/* While tucked away the window is a thin strip; this catches the cursor. */}
      {!revealed && <div className="dock-sliver" data-testid="dock-sliver" />}
      {/* Dragging the bar's background moves the dock; dropping it nearer the top or bottom
          snaps it to that edge. Buttons inside stay clickable. */}
      <div className="dock-content" ref={contentRef} data-tauri-drag-region {...emptyClicks}>
        {!split && (
          <Glass theme={settings.theme} corners={settings.style === "pill" ? corners : 0} onInk={setInk} />
        )}
        {main && centred && (
          <div className="dock-group dock-group-left" data-tauri-drag-region>
            <WidgetBar side="left" />
          </div>
        )}
        <div className="dock-group dock-group-apps" data-tauri-drag-region>
          {split && <Glass theme={settings.theme} corners={corners} onInk={setInk} />}
          <BarButton
            label="Start"
            className="dock-icon-button dock-start"
            onClick={() => ipc.shellStart().catch(report)}
          >
            {windowsLook ? <WindowsStartMark /> : <StartMark />}
          </BarButton>
          {look.search === "box" && (
            <BarButton
              label="Search"
              className="dock-search-box"
              onClick={() => ipc.shellSearch().catch(report)}
            >
              {windowsLook ? <Fluent glyph="search" size={15} /> : <Search size={16} strokeWidth={2} />}
              <span>Search</span>
            </BarButton>
          )}
          {look.search === "icon" && (
            <BarButton label="Search" onClick={() => ipc.shellSearch().catch(report)}>
              {windowsLook ? <Fluent glyph="search" size={18} /> : <Search size={18} strokeWidth={2} />}
            </BarButton>
          )}
          {look.taskView && (
            <BarButton label="Task View" onClick={() => ipc.taskView().catch(report)}>
              {windowsLook ? <Fluent glyph="taskView" size={18} /> : <Columns2 size={18} strokeWidth={1.8} />}
            </BarButton>
          )}
          {!windowsLook && <div className="dock-divider" data-tauri-drag-region />}
          <div className="dock-apps" data-tauri-drag-region>
            {rowItems(apps, settings.apps.folders).map((item) =>
              item.type === "app" ? (
                <AppButton key={item.app.key} app={item.app} />
              ) : (
                <FolderButton key={item.folder.id} folder={item.folder} apps={item.apps} />
              ),
            )}
          </div>
        </div>
        <div className="dock-spacer" data-tauri-drag-region />
        <div className="dock-group dock-group-tray" data-tauri-drag-region>
          {split && <Glass theme={settings.theme} corners={corners} onInk={setInk} />}
          {main && <ScreenshotThumb />}
          {main && <WidgetBar side={centred ? "right" : "all"} />}
          {/* The Windows look keeps the bar like the taskbar: these two are in the dock's
              right-click menu instead. */}
          {!windowsLook && (
            <BarButton
              label={`Switch to the Windows taskbar (bring Dock back with the tray icon${settings.toggleShortcut ? ` or ${settings.toggleShortcut}` : ""})`}
              onClick={() => ipc.switchToWindowsTaskbar().catch(report)}
            >
              <PanelBottom size={18} strokeWidth={1.8} />
            </BarButton>
          )}
          {!windowsLook && (
            <BarButton label="Settings" onClick={() => ipc.openSettings().catch(report)}>
              <Settings2 size={18} strokeWidth={1.8} />
            </BarButton>
          )}
          <ShowDesktopButton />
        </div>
      </div>
    </div>
  );
}
