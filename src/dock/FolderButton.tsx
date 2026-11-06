import { useEffect, useState, type MouseEvent } from "react";

import type { DockApp } from "../core/generated/DockApp";
import type { DockFolder } from "../core/generated/DockFolder";
import { ipc } from "../core/ipc";
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

/** A folder of apps on the dock: its first four apps in a little grid. Click to open it. */
export function FolderButton({ folder, apps }: { folder: DockFolder; apps: DockApp[] }) {
  const over = useFolderDrag((s) => s.over);
  const dropping = !!over && "folder" in over && over.folder === folder.id;
  const running = apps.some((a) => a.windows.length > 0);
  const active = apps.some((a) => a.windows.some((w) => w.active));
  const open = (event: MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    const anchor = screenX(rect.left + rect.width / 2, useGlass.getState().geometry, event.screenX);
    ipc.folderOpen(folder.id, anchor).catch((error) => console.error("failed to open the folder", error));
  };
  return (
    <button
      className="dock-app dock-folder"
      data-folder-id={folder.id}
      data-running={running}
      data-active={active}
      data-drop-target={dropping}
      aria-label={`Folder: ${folder.name}`}
      title={`${folder.name}: ${apps.map((a) => a.name).join(", ")}`}
      onClick={open}
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
