import { useState } from "react";
import { Minus } from "lucide-react";

import type { FolderView } from "../core/generated/FolderView";
import { ipc } from "../core/ipc";
import { useIcon } from "../core/useIcon";
import { removeFromFolder } from "../dock/folders";
import { useSettings } from "../settings/store";

function report(error: unknown) {
  console.error("folder action failed", error);
}

function FolderItem({ dock, app }: { dock: string; app: FolderView["apps"][number] }) {
  const icon = useIcon(app.icon);
  const settings = useSettings((s) => s.settings);
  const takeOut = () => {
    if (!settings) return;
    ipc.dockFoldersSet(removeFromFolder(settings.apps.folders, app.key)).catch(report);
  };
  return (
    <div className="folder-item">
      <button
        className="folder-app"
        title={app.name}
        onClick={() => ipc.folderAppClick(dock, app.key).catch(report)}
        onContextMenu={(event) => {
          event.preventDefault();
          // Screen pixels: the page's are scaled by the screen's DPI.
          const anchor = event.screenX * window.devicePixelRatio;
          ipc.folderAppMenu(dock, app.key, anchor).catch(report);
        }}
      >
        {icon ? (
          <img src={icon} alt="" draggable={false} />
        ) : (
          <span className="folder-letter">{app.name.slice(0, 1).toUpperCase()}</span>
        )}
        <span className="folder-app-name">{app.name}</span>
        {app.running && <span className="folder-running" aria-label="Running" />}
      </button>
      <button
        className="folder-remove"
        title="Take out of the folder"
        aria-label={`Take ${app.name} out`}
        onClick={takeOut}
      >
        <Minus size={11} strokeWidth={3} />
      </button>
    </div>
  );
}

/** A dock folder opened: its apps in a grid, with its name to rename. */
export function FolderPanel({ folder }: { folder: FolderView }) {
  const settings = useSettings((s) => s.settings);
  const [name, setName] = useState(folder.name);
  const saveName = () => {
    const next = name.trim() || "Folder";
    if (!settings || next === folder.name) return;
    const folders = settings.apps.folders.map((f) => (f.id === folder.id ? { ...f, name: next } : f));
    ipc.dockFoldersSet(folders).catch(report);
  };
  const ungroup = () => {
    if (!settings) return;
    const folders = settings.apps.folders.filter((f) => f.id !== folder.id);
    ipc.dockFoldersSet(folders).catch(report);
  };
  return (
    <div className="folder">
      <header className="folder-head">
        <input
          className="folder-name"
          value={name}
          maxLength={30}
          aria-label="Folder name"
          onChange={(e) => setName(e.target.value)}
          onBlur={saveName}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
        <button className="folder-ungroup" title="Put the apps back on the dock" onClick={ungroup}>
          Ungroup
        </button>
      </header>
      <div className="folder-grid">
        {folder.apps.map((app) => (
          <FolderItem key={app.key} dock={folder.dock} app={app} />
        ))}
      </div>
    </div>
  );
}
