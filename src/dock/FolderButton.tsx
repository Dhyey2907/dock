import { useEffect, useRef, useState, type MouseEvent } from "react";

import type { DockApp } from "../core/generated/DockApp";
import type { DockFolder } from "../core/generated/DockFolder";
import { events, ipc } from "../core/ipc";
import { subscribe } from "../core/ipcFeatures";
import { loadIcon } from "./apps";
import { screenX } from "./appFeatures";
import { useFolderDrag } from "./folderDrag";
import { useGlass } from "./glass/store";

function MiniIcon({ app }: { app: DockApp }) {
  const [icon, setIcon] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    loadIcon(app.icon).then((url) => alive && setIcon(url));
    return () => {
      alive = false;
    };
  }, [app.icon]);
  return icon ? (
    <img className="dock-folder-mini" src={icon} alt="" draggable={false} />
  ) : (
    <span className="dock-folder-mini dock-folder-letter">{app.name.slice(0, 1).toUpperCase()}</span>
  );
}

/** How long the cursor rests on a folder before its popup opens. */
const HOVER_OPEN_MS = 250;
/** How long the popup stays after the cursor leaves, so it can cross the gap into the popup. */
const HOVER_CLOSE_MS = 300;

function report(error: unknown) {
  console.error("folder popup failed", error);
}

/** A folder of apps on the dock: its first four apps in a little grid. Hover (or click) to
 *  open it; it closes once the cursor leaves the folder and its popup. */
export function FolderButton({ folder, apps }: { folder: DockFolder; apps: DockApp[] }) {
  const over = useFolderDrag((s) => s.over);
  const dropping = !!over && "folder" in over && over.folder === folder.id;
  const running = apps.some((a) => a.windows.length > 0);
  const active = apps.some((a) => a.windows.some((w) => w.active));
  const hoverTimer = useRef<number | undefined>(undefined);
  /** This folder's popup is open from hovering. */
  const peeking = useRef(false);
  /** The last hover open, so a close never runs ahead of it. */
  const pending = useRef<Promise<void>>(Promise.resolve());
  useEffect(() => () => window.clearTimeout(hoverTimer.current), []);
  useEffect(
    () =>
      subscribe(() =>
        events.onPanelClosed(() => {
          peeking.current = false;
        }),
      ),
    [],
  );

  const anchorOf = (event: MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return screenX(rect.left + rect.width / 2, useGlass.getState().geometry, event.screenX);
  };
  const open = (event: MouseEvent<HTMLButtonElement>) => {
    window.clearTimeout(hoverTimer.current);
    // A click on the folder hovering opened keeps its popup open.
    peeking.current = false;
    ipc.folderOpen(folder.id, anchorOf(event)).catch(report);
  };
  const onMouseEnter = (event: MouseEvent<HTMLButtonElement>) => {
    window.clearTimeout(hoverTimer.current);
    // Dragging an app onto the folder: leave the popup shut.
    if (useFolderDrag.getState().dragging) return;
    const anchor = anchorOf(event);
    const peek = () => {
      peeking.current = true;
      pending.current = ipc.folderPeek(folder.id, anchor).catch(report);
    };
    // Back from its popup: keep it open at once (cancels the close leaving planned).
    if (peeking.current) peek();
    else hoverTimer.current = window.setTimeout(peek, HOVER_OPEN_MS);
  };
  const onMouseLeave = () => {
    window.clearTimeout(hoverTimer.current);
    if (!peeking.current) return;
    pending.current.then(() => ipc.widgetUnpeek(HOVER_CLOSE_MS)).catch(report);
  };
  return (
    <button
      className="dock-app dock-folder"
      data-folder-id={folder.id}
      data-running={running}
      data-active={active}
      data-drop-target={dropping}
      aria-label={`Folder: ${folder.name}`}
      onClick={open}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      onContextMenu={(e) => {
        e.preventDefault();
        open(e);
      }}
    >
      <span className="dock-folder-grid">
        {apps.slice(0, 4).map((app) => (
          <MiniIcon key={app.key} app={app} />
        ))}
      </span>
      <span className="dock-app-indicator" />
    </button>
  );
}
