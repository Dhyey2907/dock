import { useState, type ReactNode } from "react";
import {
  AppWindow,
  CircleAlert,
  EyeOff,
  Folder,
  ImageIcon,
  MonitorUp,
  OctagonX,
  Pause,
  Pencil,
  PinOff,
  Play,
  RefreshCw,
  Rocket,
  RotateCcw,
  SeparatorVertical,
  SkipBack,
  SkipForward,
  Terminal,
  X,
} from "lucide-react";

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

const ICON = { className: "jump-icon", size: 18, strokeWidth: 1.7, "aria-hidden": true } as const;

/** A row that asks before doing something that can't be undone: the first click arms it. */
export function ConfirmRow({
  icon,
  label,
  confirm,
  onConfirm,
}: {
  icon: ReactNode;
  label: string;
  confirm: string;
  onConfirm: () => void;
}) {
  const [armed, setArmed] = useState(false);
  return (
    <button
      className="jump-row"
      data-danger={armed}
      onClick={() => (armed ? onConfirm() : setArmed(true))}
      onBlur={() => setArmed(false)}
    >
      {icon}
      <span className="jump-label">{armed ? confirm : label}</span>
    </button>
  );
}

function Tool({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button className="jump-tool" aria-label={label} title={label} onClick={onClick}>
      {children}
    </button>
  );
}

/** The row of small buttons at the bottom: rename, icon, divider, screen, restart, hide. */
function Toolbar({ menu }: { menu: AppMenu }) {
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(menu.name);
  if (renaming) {
    return (
      <form
        className="jump-rename"
        onSubmit={(e) => {
          e.preventDefault();
          act({ type: "rename", name });
        }}
      >
        <input
          aria-label="New name"
          value={name}
          autoFocus
          maxLength={60}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              setRenaming(false);
            }
          }}
        />
        <button type="submit" className="jump-tool-text">
          Save
        </button>
      </form>
    );
  }
  return (
    <div className="jump-toolbar" role="toolbar" aria-label="App actions">
      <Tool label="Rename" onClick={() => setRenaming(true)}>
        <Pencil size={16} />
      </Tool>
      <Tool label="Change icon" onClick={() => act({ type: "changeIcon" })}>
        <ImageIcon size={16} />
      </Tool>
      {menu.customised && (
        <Tool label="Original name and icon" onClick={() => act({ type: "resetLook" })}>
          <RotateCcw size={16} />
        </Tool>
      )}
      {menu.pinned && (
        <Tool
          label={menu.dividerAfter ? "Remove divider" : "Add divider after"}
          onClick={() => act({ type: "toggleDivider" })}
        >
          <SeparatorVertical size={16} />
        </Tool>
      )}
      {menu.canMove && (
        <Tool label="Move to other screen" onClick={() => act({ type: "moveToScreen" })}>
          <MonitorUp size={16} />
        </Tool>
      )}
      {menu.canRestart && (
        <Tool label="Restart" onClick={() => act({ type: "restart" })}>
          <RefreshCw size={16} />
        </Tool>
      )}
      {menu.custom ? (
        <Tool label="Unpin from Dock" onClick={() => act({ type: "unpin" })}>
          <PinOff size={16} />
        </Tool>
      ) : (
        menu.kind === "app" && (
          <Tool label="Never show on Dock" onClick={() => act({ type: "hide" })}>
            <EyeOff size={16} />
          </Tool>
        )
      )}
    </div>
  );
}

/** What the app is playing, with its controls. */
function Media({ menu }: { menu: AppMenu }) {
  const media = menu.media;
  if (!media) return null;
  return (
    <div className="jump-media">
      <span className="jump-media-text">
        <strong>{media.title || "Playing"}</strong>
        {media.artist && <span>{media.artist}</span>}
      </span>
      <span className="jump-media-buttons">
        <Tool label="Previous" onClick={() => media.canPrevious && act({ type: "media", key: "previous" })}>
          <SkipBack size={16} />
        </Tool>
        <Tool
          label={media.playing ? "Pause" : "Play"}
          onClick={() => media.canPlayPause && act({ type: "media", key: "playPause" })}
        >
          {media.playing ? <Pause size={16} /> : <Play size={16} />}
        </Tool>
        <Tool label="Next" onClick={() => media.canNext && act({ type: "media", key: "next" })}>
          <SkipForward size={16} />
        </Tool>
      </span>
    </div>
  );
}

/** An app's right-click menu, laid out like the Windows taskbar's jump lists. */
export function AppMenuPanel({ menu }: { menu: AppMenu }) {
  const hasItems = menu.sections.some((s) => s.items.length > 0);
  const hasWindows = menu.windows.length > 0;
  const groups: ReactNode[] = [];
  if (menu.media) groups.push(<Media key="media" menu={menu} />);
  if (hasWindows)
    groups.push(
      menu.windows.map((w) => (
        <button
          key={w.hwnd}
          className="jump-row"
          data-active={w.active}
          onClick={() => act({ type: "window", hwnd: w.hwnd })}
        >
          <AppWindow {...ICON} />
          <span className="jump-label">
            {w.title || menu.name}
            {w.hung ? " (Not responding)" : ""}
          </span>
        </button>
      )),
    );
  const actions: ReactNode[] = [];
  if (menu.launchable)
    actions.push(
      <button key="launch" className="jump-row" onClick={() => act({ type: "launch" })}>
        <ItemIcon source={menu.icon} fallback={AppWindow} />
        <span className="jump-label">{menu.name}</span>
      </button>,
    );
  for (const preset of menu.presets)
    actions.push(
      <button
        key={`p-${preset.id}`}
        className="jump-row"
        onClick={() => act({ type: "preset", id: preset.id })}
      >
        <Rocket {...ICON} />
        <span className="jump-label">{preset.name}</span>
      </button>,
    );
  for (const quick of menu.quick)
    actions.push(
      <button key={`q-${quick.id}`} className="jump-row" onClick={() => act({ type: "quick", id: quick.id })}>
        <Terminal {...ICON} />
        <span className="jump-label">{quick.name}</span>
      </button>,
    );
  if (menu.hung)
    actions.push(
      <ConfirmRow
        key="force"
        icon={<CircleAlert {...ICON} />}
        label="Not responding · Force close"
        confirm="Click again to force it closed"
        onConfirm={() => act({ type: "forceClose" })}
      />,
    );
  if (menu.canEndTask)
    actions.push(
      <ConfirmRow
        key="end"
        icon={<OctagonX {...ICON} />}
        label="End task"
        confirm={`Click again to end ${menu.name}`}
        onConfirm={() => act({ type: "endTask" })}
      />,
    );
  if (hasWindows)
    actions.push(
      <button key="close" className="jump-row" onClick={() => act({ type: "close" })}>
        <X {...ICON} />
        <span className="jump-label">{menu.windows.length > 1 ? "Close all windows" : "Close window"}</span>
      </button>,
    );
  if (actions.length) groups.push(actions);
  groups.push(<Toolbar key="tools" menu={menu} />);

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
      {groups.map((group, i) => (
        <div key={i} className="jump-group">
          {(i > 0 || hasItems) && <hr className="jump-divider" />}
          {group}
        </div>
      ))}
    </div>
  );
}
