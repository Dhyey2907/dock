import { useEffect, useRef, useState, type MouseEvent, type PointerEvent } from "react";

import { useGlass } from "./glass/store";
import { ipc } from "../core/ipc";
import type { DockApp } from "../core/generated/DockApp";
import { useSettings } from "../settings/store";
import { loadIcon } from "./apps";
import {
  DRAG_THRESHOLD,
  PREVIEW_DELAY_MS,
  PREVIEW_SWITCH_DELAY_MS,
  movedPast,
  screenX,
  useLaunching,
  usePreviewOpen,
} from "./appFeatures";

/** Waits before asking again for an icon that didn't load. */
const RETRY_DELAYS = [5_000, 15_000, 45_000];

function report(error: unknown) {
  console.error("app action failed", error);
}

/** The screen x of a button's centre, where its popups open. */
function anchorOf(element: HTMLElement, fallback: number) {
  const rect = element.getBoundingClientRect();
  return screenX(rect.left + rect.width / 2, useGlass.getState().geometry, fallback);
}

/** One app on the dock: click to open, bring forward or minimize; middle-click for a new
 *  window; right-click for its windows and actions; hover for its window previews; drag it to
 *  a screen edge to snap its window there. */
export function AppButton({ app }: { app: DockApp }) {
  const [icon, setIcon] = useState<string | null>(null);
  const apps = useSettings((s) => s.settings?.apps);
  const launch = useLaunching((s) => s.launches[app.key]);
  const previewOpen = usePreviewOpen((s) => s.open);
  const hoverTimer = useRef<number | undefined>(undefined);
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);
  const dragged = useRef(false);

  useEffect(() => {
    let alive = true;
    let timer: number | undefined;
    // An icon can fail to load for a moment (an app mid-update); try again a few times.
    const attempt = (n: number) =>
      loadIcon(app.icon).then((url) => {
        if (!alive) return;
        setIcon(url);
        if (!url && n < RETRY_DELAYS.length) timer = window.setTimeout(() => attempt(n + 1), RETRY_DELAYS[n]);
      });
    attempt(0);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [app.icon]);

  useEffect(() => () => window.clearTimeout(hoverTimer.current), []);

  const active = app.windows.some((w) => w.active);
  const running = app.windows.length > 0;
  // The launch store forgets a launch once a window appears or it times out.
  const launching = (apps?.launchBounce ?? true) && !!launch && app.windows.length <= launch.windows;
  const attention = (apps?.attentionFlash ?? true) && app.attention;
  const base = active ? (app.windows.find((w) => w.active)?.title ?? app.name) : app.name;
  const title = app.hung ? `${base} (Not responding)` : base;

  const onAuxClick = (event: MouseEvent) => {
    if (event.button === 1) ipc.appLaunch(app.key).catch(report);
  };
  const onContextMenu = (event: MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    window.clearTimeout(hoverTimer.current);
    ipc.appMenu(app.key, anchorOf(event.currentTarget, event.screenX)).catch(report);
  };
  const onClick = (event: MouseEvent<HTMLButtonElement>) => {
    window.clearTimeout(hoverTimer.current);
    if (dragged.current) {
      dragged.current = false;
      return;
    }
    ipc.appClick(app.key, anchorOf(event.currentTarget, event.screenX)).catch(report);
  };
  const onMouseEnter = (event: MouseEvent<HTMLButtonElement>) => {
    if (!running || apps?.previews === false) return;
    const element = event.currentTarget;
    window.clearTimeout(hoverTimer.current);
    hoverTimer.current = window.setTimeout(
      () => ipc.appPreviews(app.key, anchorOf(element, 0)).catch(report),
      previewOpen ? PREVIEW_SWITCH_DELAY_MS : PREVIEW_DELAY_MS,
    );
  };
  const onMouseLeave = () => {
    window.clearTimeout(hoverTimer.current);
    if (running) ipc.previewHide(300).catch(report);
  };
  const onPointerDown = (event: PointerEvent) => {
    if (event.button === 0) press.current = { x: event.clientX, y: event.clientY, dragging: false };
  };
  const onPointerMove = (event: PointerEvent) => {
    const start = press.current;
    if (!start || start.dragging || (event.buttons & 1) === 0) return;
    if (!running || apps?.edgeSnap === false) return;
    if (movedPast(start, { x: event.clientX, y: event.clientY }, DRAG_THRESHOLD)) {
      start.dragging = true;
      dragged.current = true;
      window.clearTimeout(hoverTimer.current);
      ipc.appDragOut(app.key).catch(report);
    }
  };
  const onPointerUp = () => {
    press.current = null;
  };

  return (
    <>
      <button
        className="dock-app"
        data-app-key={app.key}
        data-kind={app.kind}
        data-running={running}
        data-active={active}
        data-many={app.windows.length > 1}
        data-launching={launching}
        data-attention={attention}
        data-hung={app.hung}
        aria-label={app.name}
        title={title}
        onClick={onClick}
        onAuxClick={onAuxClick}
        onContextMenu={onContextMenu}
        onMouseEnter={onMouseEnter}
        onMouseLeave={onMouseLeave}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
      >
        {icon ? (
          <img className="dock-app-icon" src={icon} alt="" draggable={false} />
        ) : (
          <span className="dock-app-fallback">{app.name.slice(0, 1).toUpperCase()}</span>
        )}
        {app.hung && <span className="dock-app-badge" aria-label="Not responding" />}
        <span className="dock-app-indicator" />
      </button>
      {app.dividerAfter && <span className="dock-separator" role="separator" data-tauri-drag-region />}
    </>
  );
}
