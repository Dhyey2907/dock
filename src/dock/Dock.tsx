import { useLayoutEffect, useRef } from "react";
import { Settings2 } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { useAutoHide } from "./useAutoHide";

/** The dock bar. A pill reports its content width so the window can hug it. */
export function Dock() {
  const settings = useSettings((s) => s.settings);
  const contentRef = useRef<HTMLDivElement>(null);
  const { revealed, onEnter, onLeave } = useAutoHide(settings?.autoHide ?? false);
  const loaded = settings !== null;

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
      onMouseEnter={onEnter}
      onMouseLeave={onLeave}
    >
      {/* While tucked away the window is a thin strip; this catches the cursor. */}
      {!revealed && <div className="dock-sliver" data-testid="dock-sliver" />}
      {/* Dragging the bar's background moves the dock; dropping it nearer the top or bottom
          snaps it to that edge. Buttons inside stay clickable. */}
      <div className="dock-content" ref={contentRef} data-tauri-drag-region>
        <button className="dock-logo" aria-label="Dock" title="Dock">
          <span className="dock-logo-mark" />
        </button>
        <div className="dock-spacer" data-tauri-drag-region />
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
