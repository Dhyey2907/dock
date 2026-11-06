import { useEffect, useState } from "react";
import { X } from "lucide-react";

import type { SwitcherState } from "../core/generated/SwitcherState";
import type { SwitchItem } from "../core/generated/SwitchItem";
import { events, ipc } from "../core/ipc";
import { useIcon } from "../core/useIcon";
import { useSettings } from "../settings/store";

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
    <span className="preview-fallback" />
  );
}

/** Live pictures of an app's windows; the pictures themselves are drawn by Windows into the
 *  areas these cards leave for them. */
export function PreviewApp() {
  const content = useContent(listenPreview, previewNow, previewClosed);
  const theme = useSettings((s) => s.settings?.theme);
  if (!content) return null;
  return (
    <div
      className="preview"
      data-light={theme === "white"}
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
            <span>
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

const listenSwitcher = events.onSwitcherState;
const switcherNow = () => ipc.switcherCurrent();

function SwitchCell({ item, selected }: { item: SwitchItem; selected: boolean }) {
  return (
    <button
      className="switcher-item"
      data-selected={selected}
      title={item.title}
      onClick={() => ipc.switcherPick(item.hwnd).catch(report)}
    >
      <AppIcon source={item.icon} size={44} />
      <span>{item.appName}</span>
    </button>
  );
}

/** The dock's own Alt+Tab switcher. */
export function SwitcherApp() {
  const state = useContent<SwitcherState>(listenSwitcher, switcherNow);
  const theme = useSettings((s) => s.settings?.theme);
  if (!state) return null;
  const chosen = state.items[state.selected];
  return (
    <div className="switcher" data-light={theme === "white"}>
      <div className="switcher-grid">
        {state.items.map((item, i) => (
          <SwitchCell key={item.hwnd} item={item} selected={i === state.selected} />
        ))}
      </div>
      <div className="switcher-caption">{chosen?.title || chosen?.appName}</div>
    </div>
  );
}
