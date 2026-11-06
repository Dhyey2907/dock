import { useEffect, useState } from "react";
import {
  BatteryLow,
  CalendarClock,
  Download,
  MoonStar,
  RotateCcw,
  Sparkles,
  Timer,
  X,
  type LucideIcon,
} from "lucide-react";

import { events, ipc } from "../core/ipc";
import type { Bubble } from "../core/generated/Bubble";
import type { BubbleKind } from "../core/generated/BubbleKind";
import { useSettings } from "../settings/store";

const ICONS: Record<BubbleKind, LucideIcon> = {
  meeting: CalendarClock,
  claude: Sparkles,
  focus: Timer,
  theme: MoonStar,
  bluetooth: BatteryLow,
  download: Download,
  claudeReset: RotateCcw,
};

/** The reminder cards window: the automations' cards, newest nearest the dock. */
export function BubbleApp() {
  const [cards, setCards] = useState<Bubble[]>([]);
  const settings = useSettings((s) => s.settings);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    let told = false;
    events
      .onBubbles((list) => {
        told = true;
        setCards(list);
      })
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      // The first cards may be sent before this page has loaded.
      .then(() => ipc.bubblesGet())
      .then((list) => {
        if (alive && !told) setCards(list.slice(-3));
      })
      .catch((error) => console.error("failed to listen for reminders", error));
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const onTop = settings?.edge === "top";
  // Newest nearest the dock: at the bottom of the stack for a bottom dock.
  const ordered = onTop ? [...cards].reverse() : cards;
  return (
    <div className="bubbles" data-top={onTop} data-light={settings?.theme === "white"}>
      {ordered.map((card) => {
        const Icon = ICONS[card.kind];
        return (
          <div key={card.id} className="bubble-card" data-kind={card.kind} role="status">
            <span className="bubble-icon">
              <Icon size={18} aria-hidden="true" />
            </span>
            <span className="bubble-text">
              <span className="bubble-title">{card.title}</span>
              <span className="bubble-body">{card.body}</span>
            </span>
            {card.kind === "download" && (
              <button
                className="bubble-action"
                onClick={() => ipc.bubbleOpen(card.id).catch((error) => console.error(error))}
              >
                Open
              </button>
            )}
            <button
              className="bubble-close"
              aria-label="Dismiss"
              onClick={() => ipc.bubbleDismiss(card.id).catch((error) => console.error(error))}
            >
              <X size={14} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
