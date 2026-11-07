import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { events, ipc } from "../core/ipc";
import type { PanelContent } from "../core/generated/PanelContent";
import { useSettings } from "../settings/store";
import { useSystemLook, useWatchSystemLook } from "../core/system";
import { CapturePanel } from "../widgets/productivity/CapturePanel";
import { WIDGETS } from "../widgets/registry";
import { AppMenuPanel } from "./AppMenuPanel";
import { GalleryPanel } from "./GalleryPanel";
import { FolderPanel } from "./FolderPanel";
import { PictureInPicture2 } from "lucide-react";
import { DockMenuPanel, StackPanel } from "./DockPanels";

/** The popup window: a widget's popup or an app's menu, whichever the dock asked for. */
export function PanelApp() {
  const [content, setContent] = useState<PanelContent | null>(null);
  const theme = useSettings((s) => s.settings?.theme);
  const popupStyle = useSettings((s) => s.settings?.look.popupStyle);
  useWatchSystemLook();
  const system = useSystemLook((s) => s.look);

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

  // Menus are sized from an estimate; measure the real content and fit the popup to it.
  const root = useRef<HTMLDivElement>(null);
  const isMenu = content?.type === "app" || content?.type === "dock";
  useLayoutEffect(() => {
    const menu = root.current?.querySelector<HTMLElement>(".jump");
    if (!isMenu || !menu) return;
    const fit = () => {
      // The extent of everything in the menu (its groups have no box of their own), from the
      // menu's top, plus its bottom padding.
      const range = document.createRange();
      range.selectNodeContents(menu);
      const contents = range.getBoundingClientRect();
      const top = menu.getBoundingClientRect().top;
      const padding = parseFloat(getComputedStyle(menu).paddingBottom) || 0;
      const height = contents.bottom - top + menu.scrollTop + padding;
      if (contents.height > 0 && Math.abs(height - window.innerHeight) > 2) {
        ipc.panelFit(height).catch((error) => console.error("failed to fit the menu", error));
      }
    };
    fit();
    // Again once icons and fonts have settled.
    const later = window.setTimeout(fit, 150);
    const observer = new MutationObserver(fit);
    observer.observe(menu, { childList: true, subtree: true });
    return () => {
      window.clearTimeout(later);
      observer.disconnect();
    };
  }, [content, isMenu]);

  const kind = content?.type === "widget" ? content.kind : undefined;
  const def = kind ? WIDGETS[kind] : undefined;
  return (
    <div
      ref={root}
      className="panel"
      data-light={theme === "white"}
      data-theme={theme}
      data-system-light={theme === "windows" && (system?.light ?? false)}
      style={system ? ({ "--system-accent": system.accent } as React.CSSProperties) : undefined}
      data-kind={kind ?? content?.type}
      data-popup={popupStyle ?? "glass"}
      onMouseEnter={() => ipc.panelHover(true).catch(() => {})}
      onMouseLeave={() => ipc.panelHover(false).catch(() => {})}
      onPointerDownCapture={() => ipc.panelPin().catch(() => {})}
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
      {content?.type === "folder" && <FolderPanel key={content.folder.id} folder={content.folder} />}
      {content?.type === "capture" && <CapturePanel />}
      {content?.type === "app" && <AppMenuPanel key={content.menu.key} menu={content.menu} />}
      {content?.type === "stack" && <StackPanel stack={content.stack} />}
      {content?.type === "dock" && <DockMenuPanel menu={content.menu} />}
    </div>
  );
}
