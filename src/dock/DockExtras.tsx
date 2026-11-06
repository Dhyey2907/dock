import { useEffect, useRef } from "react";
import { Monitor } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";

function report(error: unknown) {
  console.error("dock action failed", error);
}

/** Show the desktop; hovering a moment peeks at it. */
export function ShowDesktopButton() {
  const apps = useSettings((s) => s.settings?.apps);
  const timer = useRef<number | undefined>(undefined);
  const peeking = useRef(false);
  const stopPeek = () => {
    window.clearTimeout(timer.current);
    if (peeking.current) {
      peeking.current = false;
      ipc.desktopPeek(false).catch(report);
    }
  };
  useEffect(() => stopPeek, []);
  if (apps && !apps.showDesktopButton) return null;
  return (
    <button
      className="dock-icon-button dock-show-desktop"
      aria-label="Show desktop"
      title="Show desktop"
      onMouseEnter={() => {
        if (apps?.peek === false) return;
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => {
          peeking.current = true;
          ipc.desktopPeek(true).catch(report);
        }, 500);
      }}
      onMouseLeave={stopPeek}
      onClick={() => {
        stopPeek();
        ipc.showDesktop().catch(report);
      }}
    >
      <Monitor size={17} strokeWidth={1.8} />
    </button>
  );
}
