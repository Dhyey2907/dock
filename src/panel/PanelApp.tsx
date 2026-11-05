import { useEffect, useState } from "react";

import { events, ipc } from "../core/ipc";
import type { WidgetKind } from "../core/generated/WidgetKind";
import { useSettings } from "../settings/store";
import { WIDGETS } from "../widgets/registry";

/** The widget popup window: shows whichever widget the dock asked for. */
export function PanelApp() {
  const [kind, setKind] = useState<WidgetKind | null>(null);
  const theme = useSettings((s) => s.settings?.theme);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let alive = true;
    let told = false;
    events
      .onPanelOpen(({ kind }) => {
        told = true;
        setKind(kind);
      })
      .then((fn) => (alive ? (unlisten = fn) : fn()))
      // The first open is announced before this page has loaded: ask what's open.
      .then(() => ipc.panelCurrent())
      .then((current) => {
        if (alive && !told && current) setKind(current);
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

  const def = kind ? WIDGETS[kind] : undefined;
  return (
    <div className="panel" data-light={theme === "white"} data-kind={kind ?? undefined}>
      {def && <def.Panel />}
    </div>
  );
}
