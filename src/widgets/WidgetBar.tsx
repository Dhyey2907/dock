import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent,
  type CSSProperties,
  type RefObject,
  type WheelEvent,
} from "react";
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
import { COMPACT_OF } from "./cards/cardRegistry";
import { WidgetBoundary } from "./WidgetBoundary";

/** How far the pointer must move before a press becomes a drag. */
const DRAG_THRESHOLD = 6;
/** How long the mouse rests on a widget before its preview opens, and moving between widgets
 *  while one is showing (quicker, like the taskbar's window previews). */
const PEEK_DELAY_MS = 450;
const PEEK_SWITCH_DELAY_MS = 120;

const visible = (w: WidgetEntry) => w.enabled && WIDGETS[w.kind] !== undefined;

/** Room kept for the "+N" chip, in CSS pixels. */
const MORE_CHIP = 64;

/** Whether a full row turns its later cards into compact squares (off: cards stay expanded). */
const COMPACT_SQUARES = false;

/** The smallest a row's cards shrink to (still readable) before any are hidden. */
const MIN_FIT = 0.9;

interface Fit {
  /** Widgets from here on show as compact squares (the row's length when none do). */
  compactFrom: number;
  /** The scale everything is drawn at. */
  scale: number;
  /** The first widget hidden behind the "+N" chip; null when all show. */
  from: number | null;
}

const FULL: Fit = { compactFrom: Number.MAX_SAFE_INTEGER, scale: 1, from: null };

/**
 * Fit a row of widgets to its room, keeping as much as it can: the first widgets keep
 * their full cards and the later ones turn into compact squares; if even all-compact is too
 * wide, everything shrinks a little (down to MIN_FIT); only then do the last ones hide behind
 * a "+N" chip. Full widths are remembered while shown, so a widget drawn compact or hidden
 * never seems to fit again by mistake. Only for the rows either side of centred apps.
 */
function useFit(
  row: RefObject<HTMLDivElement | null>,
  keys: string[],
  hasCompact: (key: string) => boolean,
  active: boolean,
): Fit {
  const [fit, setFit] = useState<Fit>(FULL);
  const widths = useRef(new Map<string, number>());
  const square = useRef(0);
  const drawn = useRef(1);
  useLayoutEffect(() => {
    drawn.current = fit.scale;
  }, [fit.scale]);
  const signature = keys.join(",");
  const compactable = keys.map((k) => (hasCompact(k) ? "1" : "0")).join("");
  useLayoutEffect(() => {
    const el = row.current;
    if (!el || !active) return;
    const order = signature ? signature.split(",") : [];
    const canCompact = (i: number) => compactable[i] === "1";
    const measure = () => {
      const at = drawn.current || 1;
      for (const child of Array.from(el.querySelectorAll<HTMLElement>(":scope > [data-kind]"))) {
        if (child.dataset.overflow === "true") continue;
        const box = child.getBoundingClientRect();
        if (child.dataset.compact === "true") square.current = box.width / at;
        else widths.current.set(child.dataset.kind ?? "", box.width / at);
        if (!square.current) square.current = box.height / at;
      }
      const gap = (parseFloat(getComputedStyle(el).columnGap) || 0) / at;
      const room = el.clientWidth;
      const full = order.map((key) => widths.current.get(key) ?? 0);
      const small = order.map((_, i) => (canCompact(i) ? square.current : full[i]));
      const gaps = gap * Math.max(0, order.length - 1);
      // The most leading widgets that can keep their full card.
      let next: Fit | null = null;
      for (let k = order.length; k >= 0; k--) {
        const width =
          gaps + full.slice(0, k).reduce((a, b) => a + b, 0) + small.slice(k).reduce((a, b) => a + b, 0);
        if (width <= room) {
          next = { compactFrom: k, scale: 1, from: null };
          break;
        }
      }
      if (!next) {
        const allSmall = gaps + small.reduce((a, b) => a + b, 0);
        if (allSmall * MIN_FIT <= room) {
          next = { compactFrom: 0, scale: Math.floor((room / allSmall) * 1000) / 1000 - 0.005, from: null };
        } else {
          let used = 0;
          let from: number | null = null;
          for (let i = 0; i < order.length; i++) {
            const width = (i ? gap : 0) + small[i];
            const reserve = i < order.length - 1 ? MORE_CHIP + gap * MIN_FIT : 0;
            if ((used + width) * MIN_FIT + reserve > room + 0.5) {
              from = i;
              break;
            }
            used += width;
          }
          next = { compactFrom: 0, scale: MIN_FIT, from };
        }
      }
      const result = next;
      setFit((current) =>
        current.from === result.from &&
        current.compactFrom === result.compactFrom &&
        Math.abs(current.scale - result.scale) < 0.004
          ? current
          : result,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    el.querySelectorAll(":scope > [data-kind]").forEach((child) => observer.observe(child));
    return () => observer.disconnect();
  }, [row, signature, compactable, active]);
  // After drawing: if the row still spills over (a card grew, rounding), give up one more.
  useLayoutEffect(() => {
    const el = row.current;
    if (!el || !active || el.scrollWidth <= el.clientWidth + 1) return;
    const frame = requestAnimationFrame(() =>
      setFit((current) => {
        if (current.compactFrom > 0 && current.compactFrom < keys.length) {
          return { ...current, compactFrom: current.compactFrom - 1 };
        }
        const visible = current.from ?? keys.length;
        return visible > 0 ? { ...current, from: visible - 1 } : current;
      }),
    );
    return () => cancelAnimationFrame(frame);
  });
  return active ? fit : FULL;
}

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
  const row = useRef<HTMLDivElement>(null);
  const fitKeys = (settings?.widgets ?? []).filter(onSide(side)).map((w) => w.kind);
  // Widgets always keep their full cards (compact squares looked cramped); a full row shrinks
  // a little, then the rest go behind the "+N" chip.
  const fit = useFit(
    row,
    fitKeys,
    (kind) => COMPACT_SQUARES && Boolean(COMPACT_OF[kind as WidgetKind]),
    side !== "all",
  );
  const overflowFrom = fit.from;
  // Whether a widget draws as a card (only cards have compact squares).
  const asCardFor = (kind: WidgetKind) => {
    const def = WIDGETS[kind];
    return Boolean(def) && Boolean(def?.cardOnly || (settings?.look.widgetStyle === "cards" && !def?.corner));
  };
  // The compact square a widget shows as, when its row is full and it's past the full ones.
  const compactNow = (kind: WidgetKind, index: number) =>
    COMPACT_SQUARES && index >= fit.compactFrom && asCardFor(kind) ? COMPACT_OF[kind] : undefined;

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

  // Cards for card-only widgets, and, when the style is Cards, for every widget outside the
  // taskbar's corner (the corner keeps the taskbar's own plain look). A widget with no card
  // of its own gets a plain card frame, so the row matches.
  const cardsMode = settings.look.widgetStyle === "cards";
  const asCard = (def: { Card?: unknown; cardOnly?: boolean; corner?: boolean }) =>
    Boolean(def.cardOnly) || (cardsMode && !def.corner);

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
      ref={row}
      className="dock-widgets"
      data-side={side}
      style={{ "--fit": fit.scale } as CSSProperties}
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
            data-overflow={overflowFrom !== null && index >= overflowFrom}
            data-compact={Boolean(compactNow(entry.kind, index))}
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
              {compactNow(entry.kind, index) ? (
                (() => {
                  const Compact = compactNow(entry.kind, index)!;
                  return <Compact />;
                })()
              ) : asCard(def) ? (
                def.Card ? (
                  <def.Card />
                ) : (
                  <span className="card card-tile">
                    <def.Tile />
                  </span>
                )
              ) : (
                <def.Tile />
              )}
            </WidgetBoundary>
          </button>
        );
      })}
      {overflowFrom !== null && (
        <button
          className="dock-more"
          aria-label={`${shown.length - overflowFrom} more widgets`}
          title="Show the widgets that don't fit"
          onClick={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            if (!geometry) return;
            const hidden = shown.slice(overflowFrom).map((w) => w.kind);
            ipc
              .overflowToggle(hidden, geometry.x + (rect.left + rect.width / 2) * geometry.scale)
              .catch((error) => console.error("failed to show the hidden widgets", error));
          }}
        >
          +{shown.length - overflowFrom}
        </button>
      )}
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
