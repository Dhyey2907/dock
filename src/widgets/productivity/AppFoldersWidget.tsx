import { LayoutGrid, Pencil, Plus, X } from "lucide-react";
import { useEffect, useState } from "react";

import { ipc } from "../../core/ipc";
import { useIcon } from "../../core/useIcon";
import type { AppFolders } from "../../core/generated/AppFolders";
import type { FolderApp } from "../../core/generated/FolderApp";
import { matchAll, newId } from "./logic";
import { report, useProductivity } from "./store";

const save = (folders: AppFolders) => ipc.foldersSet(folders).catch(report("save the folders"));

function AppIcon({ app, size }: { app: FolderApp; size: number }) {
  const icon = useIcon(`lnk:${app.path}`);
  return icon ? (
    <img src={icon} width={size} height={size} alt="" draggable={false} />
  ) : (
    <span className="folder-icon-blank" style={{ width: size, height: size }} />
  );
}

export function AppFoldersTile() {
  const first = useProductivity((s) => s.folders.folders[0]);
  const apps = first?.apps.slice(0, 4) ?? [];
  if (apps.length === 0) return <LayoutGrid size={18} strokeWidth={1.8} aria-hidden="true" />;
  return (
    <span className="folder-tile" aria-hidden="true">
      {apps.map((a) => (
        <AppIcon key={a.path} app={a} size={11} />
      ))}
    </span>
  );
}

function Picker({ chosen, onToggle }: { chosen: FolderApp[]; onToggle: (app: FolderApp) => void }) {
  const [all, setAll] = useState<FolderApp[] | null>(null);
  const [query, setQuery] = useState("");
  useEffect(() => {
    ipc.startMenuApps().then(setAll).catch(report("list the apps"));
  }, []);
  const shown = matchAll(all ?? [], query, (a) => [a.name]).slice(0, 80);
  return (
    <div className="folder-picker">
      <input
        className="prod-input"
        autoFocus
        value={query}
        placeholder="Find an app to add"
        onChange={(e) => setQuery(e.target.value)}
      />
      <ul className="prod-list">
        {all == null && <li className="panel-faint">Reading the Start menu…</li>}
        {shown.map((app) => {
          const on = chosen.some((c) => c.path.toLowerCase() === app.path.toLowerCase());
          return (
            <li key={app.path}>
              <button className="prod-row folder-pick" data-active={on} onClick={() => onToggle(app)}>
                <AppIcon app={app} size={18} />
                <span>{app.name}</span>
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

export function AppFoldersPanel() {
  const folders = useProductivity((s) => s.folders);
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const folder = folders.folders.find((f) => f.id === selected) ?? folders.folders[0];

  const update = (change: (apps: FolderApp[]) => FolderApp[], name?: string) => {
    if (!folder) return;
    save({
      folders: folders.folders.map((f) =>
        f.id === folder.id ? { ...f, name: name ?? f.name, apps: change(f.apps) } : f,
      ),
    });
  };

  return (
    <div className="panel-body">
      <div className="prod-head folder-tabs">
        {folders.folders.map((f) => (
          <button
            key={f.id}
            className="prod-chip"
            data-active={f.id === folder?.id}
            onClick={() => setSelected(f.id)}
          >
            {f.name}
          </button>
        ))}
        <button
          className="icon-btn"
          aria-label="New folder"
          title="New folder"
          onClick={() => {
            const id = newId();
            save({
              folders: [...folders.folders, { id, name: `Folder ${folders.folders.length + 1}`, apps: [] }],
            });
            setSelected(id);
            setEditing(true);
          }}
        >
          <Plus size={16} />
        </button>
        {folder && (
          <button
            className="icon-btn"
            aria-label="Edit folder"
            title="Edit folder"
            data-active={editing}
            onClick={() => setEditing((e) => !e)}
          >
            <Pencil size={15} />
          </button>
        )}
      </div>
      {!folder && <p className="panel-faint">Make a folder, then add apps from the Start menu.</p>}
      {folder && editing && (
        <>
          <div className="prod-add">
            <input
              className="prod-input"
              defaultValue={folder.name}
              key={folder.id}
              aria-label="Folder name"
              onBlur={(e) => update((apps) => apps, e.target.value)}
            />
            <button
              className="icon-btn"
              aria-label="Delete folder"
              title="Delete folder"
              onClick={() => {
                save({ folders: folders.folders.filter((f) => f.id !== folder.id) });
                setEditing(false);
              }}
            >
              <X size={16} />
            </button>
          </div>
          <Picker
            chosen={folder.apps}
            onToggle={(app) =>
              update((apps) =>
                apps.some((a) => a.path.toLowerCase() === app.path.toLowerCase())
                  ? apps.filter((a) => a.path.toLowerCase() !== app.path.toLowerCase())
                  : [...apps, app],
              )
            }
          />
        </>
      )}
      {folder && !editing && (
        <ul className="folder-grid">
          {folder.apps.map((app) => (
            <li key={app.path}>
              <button
                className="folder-app"
                title={app.name}
                onClick={() => ipc.folderAppLaunch(app.path).catch(report("start the app"))}
              >
                <AppIcon app={app} size={32} />
                <span>{app.name}</span>
              </button>
            </li>
          ))}
          {folder.apps.length === 0 && <li className="panel-faint">Press the pencil to add apps.</li>}
        </ul>
      )}
    </div>
  );
}
