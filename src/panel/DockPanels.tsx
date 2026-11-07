import {
  History,
  Minimize2,
  Monitor,
  Activity,
  PanelBottom,
  Settings2,
  Terminal,
  X,
  FolderOpen,
  File,
} from "lucide-react";

import type { DockMenu } from "../core/generated/DockMenu";
import type { DockMenuAction } from "../core/generated/DockMenuAction";
import type { StackView } from "../core/generated/StackView";
import { ipc } from "../core/ipc";
import { useIcon } from "../core/useIcon";
import { ConfirmRow } from "./AppMenuPanel";

const ICON = { className: "jump-icon", size: 18, strokeWidth: 1.7, "aria-hidden": true } as const;

function report(error: unknown) {
  console.error("popup action failed", error);
}

function act(action: DockMenuAction) {
  ipc.dockMenuAction(action).catch(report);
}

function RecentIcon({ source }: { source: string }) {
  const icon = useIcon(source);
  return icon ? <img className="jump-icon" src={icon} alt="" draggable={false} /> : <History {...ICON} />;
}

/** The dock's own menu: desktop, windows, recently closed apps and the user's buttons. */
export function DockMenuPanel({ menu }: { menu: DockMenu }) {
  return (
    <div className="jump" role="menu" aria-label="Dock">
      <button className="jump-row" onClick={() => act({ type: "showDesktop" })}>
        <Monitor {...ICON} />
        <span className="jump-label">Show desktop</span>
      </button>
      <button className="jump-row" onClick={() => act({ type: "minimizeOthers" })}>
        <Minimize2 {...ICON} />
        <span className="jump-label">Minimize other windows</span>
      </button>
      {menu.windows > 0 && (
        <ConfirmRow
          icon={<X {...ICON} />}
          label="Close all windows"
          confirm={`Click again to close ${menu.windows} window${menu.windows === 1 ? "" : "s"}`}
          onConfirm={() => act({ type: "closeAll" })}
        />
      )}
      <button className="jump-row" onClick={() => act({ type: "taskManager" })}>
        <Activity {...ICON} />
        <span className="jump-label">Task Manager</span>
      </button>
      <button className="jump-row" onClick={() => act({ type: "settings" })}>
        <Settings2 {...ICON} />
        <span className="jump-label">Dock settings</span>
      </button>
      <button className="jump-row" onClick={() => act({ type: "windowsTaskbar" })}>
        <PanelBottom {...ICON} />
        <span className="jump-label">Use the Windows taskbar</span>
      </button>
      {menu.recent.length > 0 && (
        <section className="jump-section">
          <hr className="jump-divider" />
          <h3 className="jump-title">Recently closed</h3>
          {menu.recent.map((r) => (
            <button key={r.key} className="jump-row" onClick={() => act({ type: "reopen", key: r.key })}>
              <RecentIcon source={r.icon} />
              <span className="jump-label">{r.name}</span>
            </button>
          ))}
        </section>
      )}
      {menu.quick.length > 0 && (
        <>
          <hr className="jump-divider" />
          {menu.quick.map((q) => (
            <button key={q.id} className="jump-row" onClick={() => act({ type: "quick", id: q.id })}>
              <Terminal {...ICON} />
              <span className="jump-label">{q.name}</span>
            </button>
          ))}
        </>
      )}
    </div>
  );
}

function StackItem({ name, path, icon }: { name: string; path: string; icon: string }) {
  const url = useIcon(icon);
  return (
    <button className="stack-item" title={path} onClick={() => ipc.stackOpen(path).catch(report)}>
      {url ? (
        <img src={url} alt="" draggable={false} />
      ) : (
        <File size={32} strokeWidth={1.4} aria-hidden="true" />
      )}
      <span>{name}</span>
    </button>
  );
}

/** A pinned folder's files fanned out in a grid, newest first. */
export function StackPanel({ stack }: { stack: StackView }) {
  return (
    <div className="stack" aria-label={stack.name}>
      <h3 className="stack-title">{stack.name}</h3>
      {stack.items.length === 0 ? (
        <p className="stack-empty">This folder is empty.</p>
      ) : (
        <div className="stack-grid">
          {stack.items.map((item) => (
            <StackItem key={item.path} {...item} />
          ))}
        </div>
      )}
      <button className="jump-row stack-open" onClick={() => ipc.stackOpen(stack.path).catch(report)}>
        <FolderOpen {...ICON} />
        <span className="jump-label">Open in File Explorer</span>
      </button>
    </div>
  );
}
