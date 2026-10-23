import { useLayoutEffect, useRef } from "react";
import { Settings2 } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";

/** The dock bar. A pill reports its content width so the window can hug it. */
export function Dock() {
  const settings = useSettings((s) => s.settings);
  const contentRef = useRef<HTMLDivElement>(null);

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
  }, []);

  if (!settings) return null;
  return (
    <div
      className="dock"
      data-theme={settings.theme}
      data-style={settings.style}
      data-edge={settings.edge}
      data-size={settings.size}
    >
      <div className="dock-content" ref={contentRef}>
        <button className="dock-logo" aria-label="Dock" title="Dock">
          <span className="dock-logo-mark" />
        </button>
        <div className="dock-spacer" />
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
