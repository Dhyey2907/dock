import type { DockApp } from "../core/generated/DockApp";
import type { DockFolder } from "../core/generated/DockFolder";

/** What the app row shows: an app on its own, or a folder of apps in the first one's place. */
export type RowItem = { type: "app"; app: DockApp } | { type: "folder"; folder: DockFolder; apps: DockApp[] };

/** The app row with folders folded in: each folder sits where its first app on the dock was. */
export function rowItems(apps: DockApp[], folders: DockFolder[]): RowItem[] {
  const folderOf = new Map<string, DockFolder>();
  for (const folder of folders) for (const key of folder.keys) folderOf.set(key, folder);
  const placed = new Set<string>();
  const items: RowItem[] = [];
  for (const app of apps) {
    const folder = folderOf.get(app.key);
    if (!folder) {
      items.push({ type: "app", app });
      continue;
    }
    if (placed.has(folder.id)) continue;
    placed.add(folder.id);
    const members = folder.keys
      .map((key) => apps.find((a) => a.key === key))
      .filter((a): a is DockApp => a !== undefined);
    items.push({ type: "folder", folder, apps: members });
  }
  return items;
}

/** What an app was dropped on: another app (by key) or a folder (by id). */
export type DropTarget = { app: string } | { folder: string };

/**
 * The folders after dropping the app `key` on `target`: onto a folder (or an app in one) adds it
 * there; onto an app on its own makes a new folder of the two. The app leaves any folder it was in.
 */
export function dropIntoFolder(
  folders: DockFolder[],
  key: string,
  target: DropTarget,
  newId: string,
): DockFolder[] {
  if ("app" in target && target.app === key) return folders;
  const into =
    "folder" in target
      ? folders.find((f) => f.id === target.folder)
      : folders.find((f) => f.keys.includes(target.app));
  if (into?.keys.includes(key)) return folders;
  const without = folders
    .map((f) => ({ ...f, keys: f.keys.filter((k) => k !== key) }))
    .filter((f) => f.keys.length > 0);
  if (into) return without.map((f) => (f.id === into.id ? { ...f, keys: [...f.keys, key] } : f));
  if ("folder" in target) return folders;
  return [...without, { id: newId, name: "Folder", keys: [target.app, key] }];
}

/** The folders without `key`; a folder left with one app stops being a folder. */
export function removeFromFolder(folders: DockFolder[], key: string): DockFolder[] {
  return folders
    .map((f) => ({ ...f, keys: f.keys.filter((k) => k !== key) }))
    .filter((f) => f.keys.length > 1);
}
