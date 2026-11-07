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
import { dropTargetAt, useFolderDrag } from "./folderDrag";
import { dropIntoFolder } from "./folders";

/** How long the mouse rests on an app that isn't running before its name shows. */
const TOOLTIP_DELAY_MS = 600;

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
export function AppButton({ app, inFolder = false }: { app: DockApp; inFolder?: boolean }) {
  const [icon, setIcon] = useState<string | null>(null);
  const apps = useSettings((s) => s.settings?.apps);
  const launch = useLaunching((s) => s.launches[app.key]);
  const previewOpen = usePreviewOpen((s) => s.open);
  const hoverTimer = useRef<number | undefined>(undefined);
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(null);
  const dragged = useRef(false);
  const tooltipShown = useRef(false);

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
    const element = event.currentTarget;
    window.clearTimeout(hoverTimer.current);
    // Not running (or previews off): just its name, the way the taskbar labels its buttons.
    if (!running || apps?.previews === false) {
      hoverTimer.current = window.setTimeout(() => {
        tooltipShown.current = true;
        ipc.tooltipShow(title, anchorOf(element, 0)).catch(report);
      }, TOOLTIP_DELAY_MS);
      return;
    }
    hoverTimer.current = window.setTimeout(
      () => ipc.appPreviews(app.key, anchorOf(element, 0)).catch(report),
      previewOpen ? PREVIEW_SWITCH_DELAY_MS : PREVIEW_DELAY_MS,
    );
  };
  const onMouseLeave = () => {
    window.clearTimeout(hoverTimer.current);
    if (running) ipc.previewHide(300).catch(report);
    else if (tooltipShown.current) ipc.previewHide(0).catch(report);
    tooltipShown.current = false;
  };
  const folderDrag = useFolderDrag();
  const onPointerDown = (event: PointerEvent<HTMLButtonElement>) => {
    if (event.button !== 0) return;
    press.current = { x: event.clientX, y: event.clientY, dragging: false };
    // Keep getting moves while the pointer is held, even off the dock.
    event.currentTarget.setPointerCapture?.(event.pointerId);
  };
  const onPointerMove = (event: PointerEvent<HTMLButtonElement>) => {
    const start = press.current;
    if (!start || (event.buttons & 1) === 0) return;
    if (!start.dragging) {
      if (!movedPast(start, { x: event.clientX, y: event.clientY }, DRAG_THRESHOLD)) return;
      start.dragging = true;
      dragged.current = true;
      window.clearTimeout(hoverTimer.current);
    }
    // Dragged up off the dock: snap the app's window to a screen edge instead.
    const offDock = event.clientY < -12 || event.clientY > window.innerHeight + 12;
    if (offDock) {
      useFolderDrag.getState().set(null, null);
      press.current = null;
      event.currentTarget.releasePointerCapture?.(event.pointerId);
      if (running && apps?.edgeSnap !== false) ipc.appDragOut(app.key).catch(report);
      return;
    }
    // Along the dock: over another app or a folder, it would go into a folder with it.
    const over = dropTargetAt(event.clientX, event.clientY);
    const self = over && "app" in over && over.app === app.key;
    useFolderDrag.getState().set(app.key, self ? null : over);
  };
  const onPointerUp = () => {
    const { dragging, over } = useFolderDrag.getState();
    press.current = null;
    useFolderDrag.getState().set(null, null);
    const settings = useSettings.getState().settings;
    if (!dragging || !over || !settings) return;
    const folders = dropIntoFolder(settings.apps.folders, dragging, over, `folder-${Date.now()}`);
    if (folders !== settings.apps.folders) {
      ipc.dockFoldersSet(folders).catch(report);
    }
  };
  const isDragging = folderDrag.dragging === app.key;
  const isDropTarget = !!folderDrag.over && "app" in folderDrag.over && folderDrag.over.app === app.key;

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
        data-dragging={isDragging}
        data-drop-target={isDropTarget}
        data-in-folder={inFolder}
        aria-label={app.name}
        data-title={title}
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
