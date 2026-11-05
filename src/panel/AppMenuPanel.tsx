import { AppWindow, Folder, X } from "lucide-react";

import type { AppMenu } from "../core/generated/AppMenu";
import type { JumpItem } from "../core/generated/JumpItem";
import type { MenuAction } from "../core/generated/MenuAction";
import { ipc } from "../core/ipc";
import { useIcon } from "../core/useIcon";

function act(action: MenuAction) {
  ipc.appMenuAction(action).catch((error) => console.error("menu action failed", error));
}

function ItemIcon({ source, fallback: Fallback }: { source: string; fallback: typeof Folder }) {
  const icon = useIcon(source);
  return icon ? (
    <img className="jump-icon" src={icon} alt="" draggable={false} />
  ) : (
    <Fallback className="jump-icon" size={18} strokeWidth={1.7} aria-hidden="true" />
  );
}

function JumpRow({ item }: { item: JumpItem }) {
  return (
    <button className="jump-row" title={item.path} onClick={() => act({ type: "open", path: item.path })}>
      <ItemIcon source={item.icon} fallback={Folder} />
      <span className="jump-label">{item.name}</span>
    </button>
  );
}

/** An app's right-click menu, laid out like the Windows taskbar's jump lists. */
export function AppMenuPanel({ menu }: { menu: AppMenu }) {
  const hasItems = menu.sections.some((s) => s.items.length > 0);
  const hasWindows = menu.windows.length > 0;
  return (
    <div className="jump" role="menu" aria-label={menu.name}>
      {menu.sections.map((section) => (
        <section key={section.title} className="jump-section">
          <h3 className="jump-title">{section.title}</h3>
          {section.items.map((item) => (
            <JumpRow key={item.path} item={item} />
          ))}
        </section>
      ))}
      {hasWindows && (
        <>
          {hasItems && <hr className="jump-divider" />}
          {menu.windows.map((w) => (
            <button
              key={w.hwnd}
              className="jump-row"
              data-active={w.active}
              onClick={() => act({ type: "window", hwnd: w.hwnd })}
            >
              <AppWindow className="jump-icon" size={18} strokeWidth={1.7} aria-hidden="true" />
              <span className="jump-label">{w.title || menu.name}</span>
            </button>
          ))}
        </>
      )}
      {(menu.launchable || hasWindows) && (hasItems || hasWindows) && <hr className="jump-divider" />}
      {menu.launchable && (
        <button className="jump-row" onClick={() => act({ type: "launch" })}>
          <ItemIcon source={menu.icon} fallback={AppWindow} />
          <span className="jump-label">{menu.name}</span>
        </button>
      )}
      {hasWindows && (
        <button className="jump-row" onClick={() => act({ type: "close" })}>
          <X className="jump-icon" size={18} strokeWidth={1.7} aria-hidden="true" />
          <span className="jump-label">{menu.windows.length > 1 ? "Close all windows" : "Close window"}</span>
        </button>
      )}
    </div>
  );
}
