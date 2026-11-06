import { useEffect, useState } from "react";

import { events, ipc } from "../core/ipc";
import type { PanelContent } from "../core/generated/PanelContent";
import { useSettings } from "../settings/store";
import { CapturePanel } from "../widgets/productivity/CapturePanel";
import { WIDGETS } from "../widgets/registry";
import { AppMenuPanel } from "./AppMenuPanel";
import { GalleryPanel } from "./GalleryPanel";
import { PictureInPicture2 } from "lucide-react";
import { DockMenuPanel, StackPanel } from "./DockPanels";

/** The popup window: a widget's popup or an app's menu, whichever the dock asked for. */
export function PanelApp() {
  const [content, setContent] = useState<PanelContent | null>(null);
  const theme = useSettings((s) => s.settings?.theme);
  const popupStyle = useSettings((s) => s.settings?.look.popupStyle);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    let told = false;
    events
      .onPanelOpen((next) => {
        told = true;
        setContent(next);
      })
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      // The first open is announced before this page has loaded: ask what's open.
      .then(() => ipc.panelCurrent())
      .then((current) => {
        if (alive && !told && current) setContent(current);
      })
      .catch((error) => console.error("failed to listen for the popup", error));
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") ipc.panelClose().catch((error) => console.error(error));
    };
    window.addEventListener("keydown", onKey);
    return () => {
      alive = false;
      unlisten?.();
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const kind = content?.type === "widget" ? content.kind : undefined;
  const def = kind ? WIDGETS[kind] : undefined;
  return (
    <div
      className="panel"
      data-light={theme === "white"}
      data-kind={kind ?? content?.type}
      data-popup={popupStyle ?? "glass"}
    >
      {def && <def.Panel />}
      {def && (
        <button
          className="panel-detach"
          title="Pop out: keep this widget in its own window"
          aria-label="Pop out"
          onClick={() => ipc.panelDetach().catch((error) => console.error("failed to pop out", error))}
        >
          <PictureInPicture2 size={14} />
        </button>
      )}
      {content?.type === "gallery" && <GalleryPanel />}
      {content?.type === "capture" && <CapturePanel />}
      {content?.type === "app" && <AppMenuPanel key={content.menu.key} menu={content.menu} />}
      {content?.type === "stack" && <StackPanel stack={content.stack} />}
      {content?.type === "dock" && <DockMenuPanel menu={content.menu} />}
    </div>
  );
}
