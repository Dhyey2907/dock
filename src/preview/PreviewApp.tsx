import { useEffect, useState } from "react";
import { AppWindow, X } from "lucide-react";

import { events, ipc } from "../core/ipc";
import { useIcon } from "../core/useIcon";
import { useSettings } from "../settings/store";
import { useSystemLook, useWatchSystemLook } from "../core/system";

/** The theme attributes shared by the preview and tooltip pages. */
function useThemeAttrs() {
  const theme = useSettings((s) => s.settings?.theme);
  useWatchSystemLook();
  const system = useSystemLook((s) => s.look);
  return {
    "data-light": theme === "white",
    "data-theme": theme,
    "data-system-light": theme === "windows" && (system?.light ?? false),
  };
}

function report(error: unknown) {
  console.error("preview action failed", error);
}

/** Listen for a page's content, and ask for it once (the first show comes before load). */
function useContent<T>(
  listen: (handler: (value: T) => void) => Promise<() => void>,
  current: () => Promise<T | null>,
  closed?: (handler: () => void) => Promise<() => void>,
) {
  const [content, setContent] = useState<T | null>(null);
  useEffect(() => {
    let alive = true;
    let told = false;
    const stops: (() => void)[] = [];
    const keep = (fn: () => void) => (alive ? stops.push(fn) : fn());
    listen((value) => {
      told = true;
      setContent(value);
    })
      .then(keep)
      .then(() => closed?.(() => setContent(null)).then(keep))
      .then(() => current())
      .then((value) => {
        if (alive && !told && value) setContent(value);
      })
      .catch(report);
    return () => {
      alive = false;
      stops.forEach((fn) => fn());
    };
  }, [listen, current, closed]);
  return content;
}

const listenPreview = events.onPreviewOpen;
const previewNow = () => ipc.previewCurrent();
const previewClosed = (handler: () => void) => events.onPreviewClosed(handler);

function AppIcon({ source, size }: { source: string; size: number }) {
  const icon = useIcon(source);
  return icon ? (
    <img src={icon} alt="" width={size} height={size} draggable={false} />
  ) : (
    // No icon yet (or none at all): a plain window glyph, never an empty block.
    <AppWindow className="preview-fallback" size={size} strokeWidth={size > 24 ? 1.5 : 1.75} aria-hidden />
  );
}

/** Live pictures of an app's windows; the pictures themselves are drawn by Windows into the
 *  areas these cards leave for them. */
export function PreviewApp() {
  const content = useContent(listenPreview, previewNow, previewClosed);
  const attrs = useThemeAttrs();
  if (!content) return null;
  if (content.tooltip) {
    return (
      <div className="preview preview-tooltip" {...attrs}>
        {content.tooltip}
      </div>
    );
  }
  return (
    <div
      className="preview"
      {...attrs}
      onMouseEnter={() => ipc.previewHover(true).catch(report)}
      onMouseLeave={() => ipc.previewHover(false).catch(report)}
    >
      {content.cards.map((card) => (
        <div
          key={card.hwnd}
          className="preview-card"
          role="button"
          tabIndex={0}
          data-active={card.active}
          style={{ left: card.x, top: card.y, width: card.width, height: card.height }}
          onClick={() => ipc.previewActivate(card.hwnd).catch(report)}
        >
          <div className="preview-title">
            <AppIcon source={content.icon} size={16} />
            <span className="preview-name">
              {card.title || content.name}
              {card.hung ? " (Not responding)" : ""}
            </span>
            <button
              className="preview-close"
              aria-label="Close window"
              onClick={(e) => {
                e.stopPropagation();
                ipc.previewCloseWindow(card.hwnd).catch(report);
              }}
            >
              <X size={14} />
            </button>
          </div>
          {card.minimized && (
            <div
              className="preview-minimized"
              style={{
                left: card.thumbX - card.x,
                top: card.thumbY - card.y,
                width: card.thumbWidth,
                height: card.thumbHeight,
              }}
            >
              <AppIcon source={content.icon} size={40} />
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
