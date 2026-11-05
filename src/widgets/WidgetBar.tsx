import { useEffect, useRef, useState, type PointerEvent } from "react";

import { events, ipc } from "../core/ipc";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import type { WidgetKind } from "../core/generated/WidgetKind";
import { useGlass } from "../dock/glass/store";
import { useSettings } from "../settings/store";
import { reorderVisible, slotAt } from "./logic";
import { WIDGETS } from "./registry";

/** How far the pointer must move before a press becomes a drag. */
const DRAG_THRESHOLD = 6;

const visible = (w: WidgetEntry) => w.enabled && WIDGETS[w.kind] !== undefined;

interface Drag {
  kind: WidgetKind;
  from: number;
  startX: number;
  midpoints: number[];
  to: number;
  moved: boolean;
}

/** The dock's widgets, in order. Click one for its popup; drag one sideways to move it. */
export function WidgetBar() {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const geometry = useGlass((s) => s.geometry);
  const [open, setOpen] = useState<WidgetKind | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const tiles = useRef(new Map<WidgetKind, HTMLButtonElement>());
  const suppressClick = useRef(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    events
      .onPanelClosed(() => setOpen(null))
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .catch((error) => console.error("failed to listen for popups", error));
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  if (!settings) return null;
  const shown = settings.widgets.filter(visible);
  if (shown.length === 0) return null;
  // While dragging, show the order the drop would give.
  const order = drag?.moved ? reorderVisible(shown, () => true, drag.from, drag.to) : shown;

  const toggle = (kind: WidgetKind) => {
    const tile = tiles.current.get(kind);
    if (!tile || !geometry) return;
    const rect = tile.getBoundingClientRect();
    const anchor = geometry.x + (rect.left + rect.width / 2) * geometry.scale;
    setOpen((current) => (current === kind ? null : kind));
    ipc.panelToggle(kind, anchor).catch((error) => console.error("failed to open the popup", error));
  };

  const onPointerDown = (kind: WidgetKind, index: number) => (e: PointerEvent<HTMLButtonElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    const midpoints = shown.map((w) => {
      const r = tiles.current.get(w.kind)?.getBoundingClientRect();
      return r ? r.left + r.width / 2 : 0;
    });
    setDrag({ kind, from: index, startX: e.clientX, midpoints, to: index, moved: false });
  };

  const onPointerMove = (e: PointerEvent<HTMLButtonElement>) => {
    if (!drag) return;
    const moved = drag.moved || Math.abs(e.clientX - drag.startX) > DRAG_THRESHOLD;
    if (!moved) return;
    const to = slotAt(e.clientX, drag.midpoints);
    if (moved !== drag.moved || to !== drag.to) setDrag({ ...drag, moved, to });
  };

  const onPointerUp = () => {
    if (!drag) return;
    if (drag.moved) {
      // The click that follows a drag must not open the popup.
      suppressClick.current = true;
      if (drag.to !== drag.from) {
        update({ widgets: reorderVisible(settings.widgets, visible, drag.from, drag.to) }).catch((error) =>
          console.error("failed to save the widget order", error),
        );
      }
    }
    setDrag(null);
  };

  return (
    <div className="dock-widgets" data-dragging={Boolean(drag?.moved)}>
      {order.map((entry) => {
        const def = WIDGETS[entry.kind];
        if (!def) return null;
        const index = shown.findIndex((w) => w.kind === entry.kind);
        return (
          <button
            key={entry.kind}
            ref={(el) => {
              if (el) tiles.current.set(entry.kind, el);
              else tiles.current.delete(entry.kind);
            }}
            className="dock-widget"
            data-kind={entry.kind}
            data-open={open === entry.kind}
            data-lifted={drag?.moved && drag.kind === entry.kind}
            aria-label={def.label}
            title={def.label}
            onPointerDown={onPointerDown(entry.kind, index)}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={() => setDrag(null)}
            onClick={() => {
              if (suppressClick.current) {
                suppressClick.current = false;
                return;
              }
              toggle(entry.kind);
            }}
          >
            <def.Tile />
          </button>
        );
      })}
    </div>
  );
}
