import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Layers, Pin, X } from "lucide-react";

import { ipc } from "../core/ipc";
import { useSettings } from "../settings/store";
import { WIDGETS } from "../widgets/registry";
import { floatKind } from "./float";

function report(error: unknown) {
  console.error("floating widget action failed", error);
}

/** A widget in its own window: popped out of the dock (on top) or kept on the desktop. */
export function FloatApp() {
  const kind = floatKind(window.location.hash);
  const theme = useSettings((s) => s.settings?.theme);
  const floating = useSettings((s) => s.settings?.floating.find((f) => f.kind === kind));
  const popupStyle = useSettings((s) => s.settings?.look.popupStyle);

  // Remember where it was dragged to, once the drag settles.
  useEffect(() => {
    if (!kind) return;
    let timer: number | undefined;
    let unlisten: (() => void) | undefined;
    let alive = true;
    getCurrentWindow()
      .onMoved(({ payload }) => {
        window.clearTimeout(timer);
        timer = window.setTimeout(() => ipc.floatingMoved(kind, payload.x, payload.y).catch(report), 400);
      })
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      .catch(report);
    return () => {
      alive = false;
      window.clearTimeout(timer);
      unlisten?.();
    };
  }, [kind]);

  const def = kind ? WIDGETS[kind] : undefined;
  if (!kind || !def) return null;
  const onTop = floating?.onTop ?? true;
  return (
    <div
      className="panel float"
      data-light={theme === "white"}
      data-kind={kind}
      data-popup={popupStyle ?? "glass"}
    >
      <header className="float-head" data-tauri-drag-region>
        <span className="float-title" data-tauri-drag-region>
          {def.label}
        </span>
        <button
          className="float-button"
          title={onTop ? "Put on the desktop, behind windows" : "Keep on top of windows"}
          aria-label={onTop ? "Put on the desktop" : "Keep on top"}
          onClick={() => ipc.floatingShow(kind, !onTop).catch(report)}
        >
          {onTop ? <Layers size={14} /> : <Pin size={14} />}
        </button>
        <button
          className="float-button"
          title="Close"
          aria-label="Close"
          onClick={() => ipc.floatingClose(kind).catch(report)}
        >
          <X size={15} />
        </button>
      </header>
      <div className="float-body">
        <def.Panel />
      </div>
    </div>
  );
}
