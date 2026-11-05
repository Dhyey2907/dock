import { useEffect, useState, type MouseEvent } from "react";

import { ipc } from "../core/ipc";
import type { DockApp } from "../core/generated/DockApp";
import { loadIcon } from "./apps";

function report(error: unknown) {
  console.error("app action failed", error);
}

/** One app on the dock: click to open, bring forward or minimize; middle-click for a new
 *  window; right-click for its windows and actions. */
export function AppButton({ app }: { app: DockApp }) {
  const [icon, setIcon] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    loadIcon(app.icon).then((url) => alive && setIcon(url));
    return () => {
      alive = false;
    };
  }, [app.icon]);

  const active = app.windows.some((w) => w.active);
  const running = app.windows.length > 0;
  const title = active ? (app.windows.find((w) => w.active)?.title ?? app.name) : app.name;

  const onAuxClick = (event: MouseEvent) => {
    if (event.button === 1) ipc.appLaunch(app.key).catch(report);
  };
  const onContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    ipc.appMenu(app.key).catch(report);
  };

  return (
    <button
      className="dock-app"
      data-running={running}
      data-active={active}
      data-many={app.windows.length > 1}
      aria-label={app.name}
      title={title}
      onClick={() => ipc.appClick(app.key).catch(report)}
      onAuxClick={onAuxClick}
      onContextMenu={onContextMenu}
    >
      {icon ? (
        <img className="dock-app-icon" src={icon} alt="" draggable={false} />
      ) : (
        <span className="dock-app-fallback">{app.name.slice(0, 1).toUpperCase()}</span>
      )}
      <span className="dock-app-indicator" />
    </button>
  );
}
