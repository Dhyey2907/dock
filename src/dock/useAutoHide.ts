import { useCallback, useEffect, useRef, useState } from "react";

import { events, ipc } from "../core/ipc";

/** How long the cursor may be off an auto-hiding dock before it tucks away. */
export const HIDE_DELAY_MS = 700;

/**
 * Auto-hide, driven by the cursor entering and leaving the window (no polling): while tucked
 * away the window is a thin strip along the edge, and touching it brings the dock out.
 */
export function useAutoHide(enabled: boolean) {
  const [revealed, setRevealed] = useState(true);
  const hideTimer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    events
      .onDockRevealed(setRevealed)
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .catch((error) => console.error("failed to listen for auto-hide", error));
    ipc
      .dockIsRevealed()
      .then((value) => alive && setRevealed(value))
      .catch((error) => console.error("failed to read auto-hide state", error));
    return () => {
      alive = false;
      unlisten?.();
      window.clearTimeout(hideTimer.current);
    };
  }, [enabled]);

  const onEnter = useCallback(() => {
    if (!enabled) return;
    window.clearTimeout(hideTimer.current);
    ipc.dockReveal(true).catch((error) => console.error("failed to reveal dock", error));
  }, [enabled]);

  const onLeave = useCallback(() => {
    if (!enabled) return;
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      ipc.dockReveal(false).catch((error) => console.error("failed to hide dock", error));
    }, HIDE_DELAY_MS);
  }, [enabled]);

  return { revealed: !enabled || revealed, onEnter, onLeave };
}
