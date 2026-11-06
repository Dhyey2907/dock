import { ipc } from "../../core/ipc";
import { report } from "./store";

/** Text dropped on the dock: plain text, or a link's address. Files and pictures aren't text. */
export function droppedText(data: Pick<DataTransfer, "types" | "getData">): string | null {
  if (data.types.includes("Files")) return null;
  const text = data.getData("text/plain") || data.getData("text/uri-list");
  return text.trim() ? text.trim() : null;
}

/**
 * Let text be dropped anywhere on the main dock to make a note. The page shows
 * `data-text-drop` on <html> while text is over it.
 */
export function enableTextDrop(): () => void {
  const root = document.documentElement;
  const accepts = (e: DragEvent) => Boolean(e.dataTransfer && !e.dataTransfer.types.includes("Files"));
  const over = (e: DragEvent) => {
    if (!accepts(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    root.dataset.textDrop = "true";
  };
  const leave = (e: DragEvent) => {
    // Only when leaving the window, not moving between its elements.
    if (e.relatedTarget == null) delete root.dataset.textDrop;
  };
  const drop = (e: DragEvent) => {
    delete root.dataset.textDrop;
    if (!e.dataTransfer) return;
    const text = droppedText(e.dataTransfer);
    if (text == null) return;
    e.preventDefault();
    ipc.noteSave(null, text).catch(report("save the dropped text"));
  };
  window.addEventListener("dragenter", over);
  window.addEventListener("dragover", over);
  window.addEventListener("dragleave", leave);
  window.addEventListener("drop", drop);
  return () => {
    window.removeEventListener("dragenter", over);
    window.removeEventListener("dragover", over);
    window.removeEventListener("dragleave", leave);
    window.removeEventListener("drop", drop);
  };
}
