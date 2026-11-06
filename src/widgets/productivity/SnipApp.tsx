import { useEffect, useState, type PointerEvent } from "react";

import { events, ipc } from "../../core/ipc";
import type { SnipBackdrop } from "../../core/generated/SnipBackdrop";
import { boxFractions, type Box } from "./logic";
import { report } from "./store";

/** The area picker: the captured screen, dimmed, with the dragged-out area cut clear. */
export function SnipApp() {
  const [backdrop, setBackdrop] = useState<SnipBackdrop | null>(null);
  const [box, setBox] = useState<Box | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    const load = () => {
      setBox(null);
      ipc
        .snipBackdrop()
        .then((b) => alive && setBackdrop(b))
        .catch(report("load the screen"));
    };
    events
      .onSnipOpen(load)
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .then(load)
      .catch(report("listen for the picker"));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") ipc.snipCancel().catch(report("cancel"));
    };
    window.addEventListener("keydown", onKey);
    return () => {
      alive = false;
      unlisten?.();
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const onDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) {
      ipc.snipCancel().catch(report("cancel"));
      return;
    }
    e.currentTarget.setPointerCapture(e.pointerId);
    setBox({ x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY });
  };
  const onMove = (e: PointerEvent<HTMLDivElement>) => {
    if (box) setBox({ ...box, x1: e.clientX, y1: e.clientY });
  };
  const onUp = () => {
    if (!box) return;
    const f = boxFractions(box, window.innerWidth, window.innerHeight);
    setBox(null);
    setBackdrop(null);
    ipc.snipFinish(f.x, f.y, f.width, f.height).catch(report("use the area"));
  };

  const rect = box && {
    left: Math.min(box.x0, box.x1),
    top: Math.min(box.y0, box.y1),
    width: Math.abs(box.x1 - box.x0),
    height: Math.abs(box.y1 - box.y0),
  };
  return (
    <div
      className="snip"
      style={backdrop ? { backgroundImage: `url(${backdrop.image})` } : undefined}
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onContextMenu={(e) => e.preventDefault()}
    >
      {!rect && <div className="snip-veil" />}
      {rect && <div className="snip-box" style={rect} />}
      {backdrop && !box && (
        <div className="snip-hint">
          {backdrop.mode === "pin" ? "Drag to pin an area" : "Drag over the text to copy"} · Esc to cancel
        </div>
      )}
    </div>
  );
}

/** A pinned picture's window: the picture, draggable, with a close button on hover. */
export function PinApp({ id }: { id: number }) {
  const [image, setImage] = useState<string | null>(null);
  useEffect(() => {
    ipc.pinImage(id).then(setImage).catch(report("load the picture"));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") ipc.pinClose(id).catch(report("close the pin"));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [id]);
  return (
    <div
      className="pin"
      data-tauri-drag-region
      onDoubleClick={() => ipc.pinClose(id).catch(report("close the pin"))}
    >
      {image && <img src={image} alt="Pinned picture" draggable={false} data-tauri-drag-region />}
      <button
        className="pin-close"
        aria-label="Close"
        onClick={() => ipc.pinClose(id).catch(report("close the pin"))}
      >
        ×
      </button>
    </div>
  );
}
