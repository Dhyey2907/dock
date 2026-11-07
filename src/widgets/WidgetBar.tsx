import { useEffect, useRef, useState, type PointerEvent, type WheelEvent } from "react";
import { ChevronRight } from "lucide-react";
import { useWidgets } from "./store";

import { events, ipc } from "../core/ipc";
import { featureEvents, subscribe } from "../core/ipcFeatures";
import type { WidgetEntry } from "../core/generated/WidgetEntry";
import type { WidgetKind } from "../core/generated/WidgetKind";
import { useGlass } from "../dock/glass/store";
import { useSettings } from "../settings/store";
import { reorderVisible, slotAt } from "./logic";
import { WIDGETS } from "./registry";
import { WidgetBoundary } from "./WidgetBoundary";

/** How far the pointer must move before a press becomes a drag. */
const DRAG_THRESHOLD = 6;
/** How long the mouse rests on a widget before its preview opens, and moving between widgets
 *  while one is showing (quicker, like the taskbar's window previews). */
const PEEK_DELAY_MS = 450;
const PEEK_SWITCH_DELAY_MS = 120;

const visible = (w: WidgetEntry) => w.enabled && WIDGETS[w.kind] !== undefined;

/** Which widgets a bar shows: all of them, the taskbar corner's (right of the apps), or the
 *  others (left of them, where Windows 11 has its Widgets button). */
export type WidgetSide = "all" | "left" | "right";

function onSide(side: WidgetSide) {
  return (w: WidgetEntry) =>
    visible(w) && (side === "all" || (side === "right") === Boolean(WIDGETS[w.kind]?.corner));
}

interface Drag {
  kind: WidgetKind;
  from: number;
  startX: number;
  midpoints: number[];
  to: number;
  moved: boolean;
}

/** The dock's widgets, in order. Click one for its popup; drag one sideways to move it. */
export function WidgetBar({ side = "all" }: { side?: WidgetSide }) {
  const settings = useSettings((s) => s.settings);
  const update = useSettings((s) => s.update);
  const geometry = useGlass((s) => s.geometry);
  const [open, setOpen] = useState<WidgetKind | "gallery" | null>(null);
  const galleryButton = useRef<HTMLButtonElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const tiles = useRef(new Map<WidgetKind, HTMLButtonElement>());
  const suppressClick = useRef(false);
  const peekTimer = useRef<number | undefined>(undefined);
  /** The widget whose popup is showing as a hover preview. */
  const peeking = useRef<WidgetKind | null>(null);
  useEffect(() => () => window.clearTimeout(peekTimer.current), []);

  useEffect(() => {
    const stops: (() => void)[] = [];
    let alive = true;
    const keep = (fn: () => void) => (alive ? stops.push(fn) : fn());
    events
      .onPanelClosed(() => {
        peeking.current = null;
        setOpen(null);
      })
      .then(keep)
      .then(() => events.onPanelPinned(() => (peeking.current = null)))
      .then(keep)
      .catch((error) => console.error("failed to listen for popups", error));
    return () => {
      alive = false;
      stops.forEach((fn) => fn());
    };
  }, []);

  // The command bar asks for a widget's popup (opened unless it already is).
  useEffect(
    () =>
      subscribe(() =>
        featureEvents.onWidgetOpen((kind) => {
          const tile = tiles.current.get(kind);
          const where = useGlass.getState().geometry;
          if (open === kind || !tile || !where) return;
          const rect = tile.getBoundingClientRect();
          setOpen(kind);
          ipc
            .panelToggle(kind, where.x + (rect.left + rect.width / 2) * where.scale)
            .catch((error) => console.error("failed to open the popup", error));
        }),
      ),
    [open],
  );

  if (!settings) return null;
  const shows = onSide(side);
  const shown = settings.widgets.filter(shows);
  // While dragging, show the order the drop would give.
  const order = drag?.moved ? reorderVisible(shown, () => true, drag.from, drag.to) : shown;

  const anchorOf = (kind: WidgetKind) => {
    const tile = tiles.current.get(kind);
    if (!tile || !geometry) return null;
    const rect = tile.getBoundingClientRect();
    return geometry.x + (rect.left + rect.width / 2) * geometry.scale;
  };

  // Windows' own flyout for this widget, when it stands for a taskbar button and that's chosen.
  const nativeFor = (kind: WidgetKind) => {
    const def = WIDGETS[kind];
    if (!def?.native) return undefined;
    return def.windowsOnly || settings.look.nativeFlyouts ? def.native : undefined;
  };

  const toggle = (kind: WidgetKind) => {
    window.clearTimeout(peekTimer.current);
    const native = nativeFor(kind);
    if (native) {
      // A preview of the dock's popup gives way to Windows' flyout.
      if (peeking.current) ipc.panelClose().catch(() => {});
      peeking.current = null;
      ipc.trayPress(native).catch((error) => console.error("failed to open Windows' flyout", error));
      return;
    }
    // A card with no popup: its own controls do the work.
    if (WIDGETS[kind]?.noPanel) return;
    const anchor = anchorOf(kind);
    if (anchor === null) return;
    // A click on the widget being previewed keeps its popup open.
    const wasPeek = peeking.current === kind;
    peeking.current = null;
    setOpen((current) => (wasPeek ? kind : current === kind ? null : kind));
    ipc.panelToggle(kind, anchor).catch((error) => console.error("failed to open the popup", error));
  };

  // Cards for card-only widgets, and for every widget with a card when the style is Cards.
  const asCard = (def: { Card?: unknown; cardOnly?: boolean }) =>
    Boolean(def.Card) && (def.cardOnly || settings.look.widgetStyle === "cards");

  const hoverPreviews = settings.look.widgetHover && !drag;
  const onEnter = (kind: WidgetKind) => () => {
    if (!hoverPreviews || WIDGETS[kind]?.windowsOnly || WIDGETS[kind]?.noPanel) return;
    window.clearTimeout(peekTimer.current);
    // Another popup (not a preview) is open: leave it.
    if (open !== null && peeking.current === null) return;
    peekTimer.current = window.setTimeout(
      () => {
        const anchor = anchorOf(kind);
        if (anchor === null) return;
        peeking.current = kind;
        setOpen(kind);
        ipc.widgetPeek(kind, anchor).catch((error) => console.error("failed to preview the widget", error));
      },
      peeking.current ? PEEK_SWITCH_DELAY_MS : PEEK_DELAY_MS,
    );
  };
  const onLeave = () => {
    window.clearTimeout(peekTimer.current);
    if (peeking.current) ipc.widgetUnpeek(300).catch((error) => console.error(error));
  };

  const onWheel = (kind: WidgetKind) => (e: WheelEvent<HTMLButtonElement>) => {
    if (kind !== "status") return;
    const up = e.deltaY < 0;
    if (e.shiftKey) {
      ipc
        .quickGet()
        .then((q) => {
          if (q.brightness == null) return;
          const next = Math.max(0, Math.min(100, q.brightness + (up ? 5 : -5)));
          return ipc.quickSetBrightness(next);
        })
        .catch((error) => console.error("failed to change brightness", error));
      return;
    }
    const level = useWidgets.getState().sound?.level ?? 0.5;
    const next = Math.max(0, Math.min(1, Math.round((level + (up ? 0.04 : -0.04)) * 100) / 100));
    ipc.volumeSet(next).catch((error) => console.error("failed to change the volume", error));
  };

  const toggleGallery = () => {
    const button = galleryButton.current;
    if (!button || !geometry) return;
    const rect = button.getBoundingClientRect();
    const anchor = geometry.x + (rect.left + rect.width / 2) * geometry.scale;
    setOpen((current) => (current === "gallery" ? null : "gallery"));
    ipc.galleryToggle(anchor).catch((error) => console.error("failed to open the widget gallery", error));
  };

  const onPointerDown = (kind: WidgetKind, index: number) => (e: PointerEvent<HTMLButtonElement>) => {
    if (e.button !== 0) return;
    window.clearTimeout(peekTimer.current);
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
        update({ widgets: reorderVisible(settings.widgets, shows, drag.from, drag.to) }).catch((error) =>
          console.error("failed to save the widget order", error),
        );
      }
    }
    setDrag(null);
  };

  return (
    <div
      className="dock-widgets"
      data-side={side}
      data-dragging={Boolean(drag?.moved)}
      onWheel={(e) => {
        // The mouse wheel scrolls a crowded widget row sideways (the Status widget uses it
        // for volume instead).
        const row = e.currentTarget;
        const onStatus = (e.target as HTMLElement).closest('[data-kind="status"]');
        if (!onStatus && row.scrollWidth > row.clientWidth) row.scrollLeft += e.deltaY;
      }}
    >
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
            className={asCard(def) ? "dock-widget dock-card" : "dock-widget"}
            data-kind={entry.kind}
            data-size={entry.size}
            data-open={open === entry.kind}
            data-lifted={drag?.moved && drag.kind === entry.kind}
            aria-label={def.label}
            title={hoverPreviews && !def.windowsOnly ? undefined : def.label}
            onMouseEnter={onEnter(entry.kind)}
            onMouseLeave={onLeave}
            // Pressing anything in a widget (a card's own buttons too) cancels a coming preview.
            onPointerDownCapture={() => window.clearTimeout(peekTimer.current)}
            onPointerDown={onPointerDown(entry.kind, index)}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerCancel={() => setDrag(null)}
            onWheel={onWheel(entry.kind)}
            onClick={() => {
              if (suppressClick.current) {
                suppressClick.current = false;
                return;
              }
              toggle(entry.kind);
            }}
          >
            <WidgetBoundary name={def.label}>
              {asCard(def) && def.Card ? <def.Card /> : <def.Tile />}
            </WidgetBoundary>
          </button>
        );
      })}
      {side !== "left" && (
        <button
          ref={galleryButton}
          className="dock-gallery-button"
          data-open={open === "gallery"}
          aria-label="Add widgets"
          title="Add widgets"
          onClick={toggleGallery}
        >
          <ChevronRight size={16} strokeWidth={2.2} />
        </button>
      )}
    </div>
  );
}
