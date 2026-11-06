import { create } from "zustand";

import type { DropTarget } from "./folders";

interface FolderDrag {
  /** The app being dragged along the dock, by key. */
  dragging: string | null;
  /** What it would drop on now. */
  over: DropTarget | null;
  set: (dragging: string | null, over: DropTarget | null) => void;
}

/** An app dragged onto another app (or a folder) on the dock, to group them in a folder. */
export const useFolderDrag = create<FolderDrag>((set) => ({
  dragging: null,
  over: null,
  set: (dragging, over) => set({ dragging, over }),
}));

/** The app or folder under a point of the dock page. */
export function dropTargetAt(x: number, y: number): DropTarget | null {
  const element = document.elementFromPoint?.(x, y)?.closest<HTMLElement>("[data-app-key], [data-folder-id]");
  if (!element) return null;
  if (element.dataset.folderId) return { folder: element.dataset.folderId };
  return element.dataset.appKey ? { app: element.dataset.appKey } : null;
}
