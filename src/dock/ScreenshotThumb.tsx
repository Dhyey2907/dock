import { useEffect, useRef, useState, type PointerEvent } from "react";
import { X } from "lucide-react";

import type { LatestScreenshot } from "../core/generated/LatestScreenshot";
import { featureEvents, featuresIpc, subscribe } from "../core/ipcFeatures";

/** How far the pointer moves before a press becomes a drag. */
const DRAG_THRESHOLD = 4;

/**
 * The newest screenshot, for two minutes after it's taken: drag it into any app as a file, or
 * click to open it.
 */
export function ScreenshotThumb() {
  const [shot, setShot] = useState<LatestScreenshot | null>(null);
  const press = useRef<{ x: number; y: number; dragged: boolean } | null>(null);

  useEffect(() => {
    let told = false;
    const stop = subscribe(() =>
      featureEvents.onLatestScreenshot((next) => {
        told = true;
        setShot(next);
      }),
    );
    featuresIpc
      .screenshotLatest()
      .then((current) => !told && setShot(current))
      .catch((error) => console.error("failed to read the screenshot", error));
    return stop;
  }, []);

  if (!shot) return null;
  const onPointerDown = (e: PointerEvent) => {
    if (e.button === 0) press.current = { x: e.clientX, y: e.clientY, dragged: false };
  };
  const onPointerMove = (e: PointerEvent) => {
    const p = press.current;
    if (!p || p.dragged || (e.buttons & 1) === 0) return;
    if (Math.hypot(e.clientX - p.x, e.clientY - p.y) < DRAG_THRESHOLD) return;
    p.dragged = true;
    // Windows takes over the drag from here while the button stays down.
    featuresIpc.screenshotDrag(shot.path).catch((error) => console.error("failed to drag", error));
  };
  const onPointerUp = () => {
    const p = press.current;
    press.current = null;
    if (p && !p.dragged) featuresIpc.screenshotOpen(shot.path).catch((error) => console.error(error));
  };

  return (
    <div className="dock-shot">
      <button
        className="dock-shot-thumb"
        aria-label="Latest screenshot: drag it out, or click to open"
        title="Latest screenshot — drag into any app, or click to open"
        draggable={false}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={() => (press.current = null)}
      >
        <img src={shot.thumb} alt="" draggable={false} />
      </button>
      <button
        className="dock-shot-close"
        aria-label="Hide the screenshot"
        title="Hide"
        onClick={() => featuresIpc.screenshotDismiss().catch((error) => console.error(error))}
      >
        <X size={10} strokeWidth={3} />
      </button>
    </div>
  );
}
