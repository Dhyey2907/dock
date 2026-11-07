import { useEffect, useRef } from "react";

import { ipc } from "../core/ipc";
import { useGlass } from "./glass/store";

/** How long the mouse rests on a button before its label shows, like the taskbar's. */
const TOOLTIP_DELAY_MS = 600;

/**
 * A Windows-style label for a dock button (instead of the browser's own tooltip): spread the
 * returned handlers on the button. `text` empty shows nothing.
 */
export function useTooltip(text: string) {
  const timer = useRef<number | undefined>(undefined);
  const shown = useRef(false);
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const hide = () => {
    window.clearTimeout(timer.current);
    if (shown.current) {
      shown.current = false;
      ipc.previewHide(0).catch(() => {});
    }
  };
  return {
    onMouseEnter: (event: React.MouseEvent<HTMLElement>) => {
      const element = event.currentTarget;
      window.clearTimeout(timer.current);
      if (!text) return;
      timer.current = window.setTimeout(() => {
        const geometry = useGlass.getState().geometry;
        if (!geometry) return;
        const rect = element.getBoundingClientRect();
        shown.current = true;
        ipc
          .tooltipShow(text, geometry.x + (rect.left + rect.width / 2) * geometry.scale)
          .catch((error) => console.error("failed to show a tooltip", error));
      }, TOOLTIP_DELAY_MS);
    },
    onMouseLeave: hide,
    onMouseDown: hide,
  };
}
