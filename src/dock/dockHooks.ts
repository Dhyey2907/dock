import { useEffect, useRef, type MouseEvent, type RefObject } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import type { DockApp } from "../core/generated/DockApp";
import { ipc } from "../core/ipc";
import { useGlass } from "./glass/store";
import {
  HOLD_DELAY_MS,
  RAISE_DELAY_MS,
  appKeyAt,
  buttonRects,
  hoverTimer,
  isEmptySpot,
  screenX,
} from "./appFeatures";

function report(error: unknown) {
  console.error("dock action failed", error);
}

/**
 * Handlers for the dock's empty background: a right-click opens the dock's own menu; a press
 * is reported (the backend tells double-clicks and press-and-hold apart, since a press there
 * may also start moving the dock, which swallows the release).
 */
export function useEmptyDockClicks() {
  const holdTimer = useRef<number | undefined>(undefined);
  useEffect(() => () => window.clearTimeout(holdTimer.current), []);
  return {
    onMouseDown: (event: MouseEvent) => {
      if (event.button !== 0 || !isEmptySpot(event.target)) return;
      ipc.dockPress().catch(report);
      window.clearTimeout(holdTimer.current);
      holdTimer.current = window.setTimeout(() => ipc.dockHold().catch(report), HOLD_DELAY_MS);
    },
    onContextMenu: (event: MouseEvent) => {
      if (!isEmptySpot(event.target)) return;
      event.preventDefault();
      ipc.dockMenu(screenX(event.clientX, useGlass.getState().geometry, event.screenX)).catch(report);
    },
  };
}

/** Files dragged over an app's icon for a moment bring its window forward. */
export function useDragOverRaise() {
  useEffect(() => {
    const timer = hoverTimer(RAISE_DELAY_MS, (key) => ipc.appRaise(key).catch(report));
    let unlisten: (() => void) | undefined;
    let alive = true;
    try {
      getCurrentWebview()
        .onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === "over" || payload.type === "enter") {
            const ratio = window.devicePixelRatio || 1;
            timer.over(appKeyAt(payload.position.x / ratio, payload.position.y / ratio));
          } else {
            timer.clear();
          }
        })
        .then((fn) => (alive ? (unlisten = fn) : fn()))
        .catch(report);
    } catch {
      // Outside Tauri (tests).
    }
    return () => {
      alive = false;
      timer.clear();
      unlisten?.();
    };
  }, []);
}

/** Tell the backend where the app buttons are (windows minimize into them). */
export function useReportButtonRects(apps: DockApp[], ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const container = ref.current;
    if (!container) return;
    let frame = 0;
    const send = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => ipc.appRects(buttonRects()).catch(report));
    };
    send();
    const observer = new ResizeObserver(send);
    observer.observe(container);
    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [apps, ref]);
}
