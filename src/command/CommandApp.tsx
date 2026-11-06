import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import {
  AppWindow,
  Briefcase,
  Calculator,
  FileText,
  LayoutGrid,
  Search,
  Settings2,
  SquareCheck,
  StickyNote,
  Zap,
  type LucideIcon,
} from "lucide-react";

import type { IndexHit } from "../core/generated/IndexHit";
import type { InstalledApp } from "../core/generated/InstalledApp";
import type { NoteHit } from "../core/generated/NoteHit";
import type { Workspace } from "../core/generated/Workspace";
import { ipc } from "../core/ipc";
import { errorText, featureEvents, featuresIpc, subscribe } from "../core/ipcFeatures";
import { useIcon } from "../core/useIcon";
import { useSettings } from "../settings/store";
import { WIDGETS } from "../widgets/registry";
import { buildResults, type CommandItem } from "./items";

/** Wait this long after typing stops before asking the search index. */
const FILE_DELAY = 220;

const KIND_ICONS: Record<CommandItem["kind"], LucideIcon> = {
  calc: Calculator,
  app: AppWindow,
  file: FileText,
  setting: Settings2,
  widget: LayoutGrid,
  note: StickyNote,
  workspace: Briefcase,
  action: Zap,
};

function ItemIcon({ item }: { item: CommandItem }) {
  const icon = useIcon(item.kind === "app" ? item.icon : undefined);
  if (icon) return <img className="command-icon" src={icon} alt="" />;
  const Icon = item.kind === "note" && item.todo ? SquareCheck : KIND_ICONS[item.kind];
  return (
    <span className="command-icon">
      <Icon size={18} strokeWidth={1.8} aria-hidden="true" />
    </span>
  );
}

async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Older WebView2 without clipboard permission: the classic way.
    const area = document.createElement("textarea");
    area.value = text;
    document.body.appendChild(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
}

/** The command bar: type to find anything; arrows to choose; Enter to run; Esc to close. */
export function CommandApp() {
  const theme = useSettings((s) => s.settings?.theme);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [apps, setApps] = useState<InstalledApp[]>([]);
  const [notes, setNotes] = useState<NoteHit[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [files, setFiles] = useState<{ query: string; hits: IndexHit[] }>({ query: "", hits: [] });
  const [naming, setNaming] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLUListElement>(null);

  // Each time it opens: a clean box, and fresh lists.
  useEffect(() => {
    const reload = () => {
      setQuery("");
      setSelected(0);
      setNaming(false);
      setMessage(null);
      setFiles({ query: "", hits: [] });
      input.current?.focus();
      featuresIpc.commandApps().then(setApps).catch(console.error);
      featuresIpc.commandNotes().then(setNotes).catch(console.error);
      featuresIpc.workspacesList().then(setWorkspaces).catch(console.error);
    };
    reload();
    const stopShown = subscribe(() => featureEvents.onCommandShown(reload));
    const stopWorkspaces = subscribe(() => featureEvents.onWorkspacesChanged(setWorkspaces));
    return () => {
      stopShown();
      stopWorkspaces();
    };
  }, []);

  // Files come from the search index, a moment after typing stops.
  useEffect(() => {
    const q = query.trim();
    if (naming || q.length < 2) return;
    let live = true;
    const timer = window.setTimeout(() => {
      featuresIpc
        .commandFiles(q)
        .then((hits) => live && setFiles({ query: q, hits }))
        .catch(console.error);
    }, FILE_DELAY);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [query, naming]);

  const widgets = useMemo(
    () =>
      Object.entries(WIDGETS).map(([kind, def]) => ({
        kind: kind as keyof typeof WIDGETS,
        label: def!.label,
        description: def!.description,
      })),
    [],
  );
  const items = useMemo(
    () =>
      naming
        ? []
        : buildResults(query, {
            apps,
            notes,
            workspaces,
            widgets,
            files: files.query === query.trim() ? files.hits : [],
          }),
    [naming, query, apps, notes, workspaces, widgets, files],
  );
  const current = Math.min(selected, Math.max(0, items.length - 1));

  useEffect(() => {
    list.current?.querySelector(`[data-index="${current}"]`)?.scrollIntoView?.({ block: "nearest" });
  }, [current]);

  const fail = (error: unknown) => setMessage(errorText(error, "That didn't work. Try again."));
  const hide = () => featuresIpc.commandHide().catch(console.error);

  const run = async (item: CommandItem) => {
    try {
      switch (item.kind) {
        case "calc":
          await copy(item.value);
          setMessage(`Copied ${item.value}`);
          return;
        case "note":
          await copy(item.text);
          setMessage("Copied to the clipboard");
          return;
        case "app":
          return await featuresIpc.commandLaunchApp(item.id);
        case "file":
          return await featuresIpc.commandOpenFile(item.path);
        case "setting":
          return await featuresIpc.commandOpenSettings(item.target);
        case "widget":
          return await featuresIpc.commandOpenWidget(item.widget);
        case "workspace":
          return await featuresIpc.workspaceRestore(item.name);
        case "action":
          if (item.action === "saveWorkspace") {
            setNaming(true);
            setQuery("");
            setMessage(null);
          } else if (item.action === "openSettings") {
            await featuresIpc.commandOpenSettings("appearance");
          } else {
            await hide();
            await ipc.switchToWindowsTaskbar();
          }
      }
    } catch (error) {
      fail(error);
    }
  };

  const saveWorkspace = async () => {
    if (!query.trim()) return;
    try {
      const saved = await featuresIpc.workspaceSave(query);
      setNaming(false);
      setQuery("");
      setMessage(`Saved “${saved.name}” with ${saved.apps.length} windows`);
    } catch (error) {
      fail(error);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      if (naming) setNaming(false);
      else if (query) setQuery("");
      else hide();
    } else if (e.key === "ArrowDown" && items.length) {
      e.preventDefault();
      setSelected((current + 1) % items.length);
    } else if (e.key === "ArrowUp" && items.length) {
      e.preventDefault();
      setSelected((current - 1 + items.length) % items.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (naming) saveWorkspace();
      else if (items[current]) run(items[current]);
    }
  };

  return (
    <div className="command" data-light={theme === "white"}>
      <div className="command-box">
        <Search size={20} aria-hidden="true" />
        <input
          ref={input}
          autoFocus
          spellCheck={false}
          value={query}
          placeholder={
            naming
              ? "Name this workspace, then press Enter"
              : "Search apps, files, settings, notes… or type a sum"
          }
          aria-label={naming ? "Workspace name" : "Search"}
          aria-controls="command-results"
          aria-activedescendant={items[current] ? `command-item-${current}` : undefined}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(0);
            setMessage(null);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      {message && (
        <p className="command-message" role="status">
          {message}
        </p>
      )}
      {naming ? (
        <p className="command-hint">
          Saves every open app window, with its place on screen. Restore it from here or from Settings.
        </p>
      ) : (
        <ul className="command-list" id="command-results" role="listbox" ref={list}>
          {items.length === 0 && query.trim() && <li className="command-hint">Nothing found.</li>}
          {items.map((item, i) => (
            <li
              key={item.key}
              id={`command-item-${i}`}
              data-index={i}
              role="option"
              aria-selected={i === current}
              className="command-item"
              data-kind={item.kind}
              onMouseMove={() => i !== current && setSelected(i)}
              onClick={() => run(item)}
            >
              <ItemIcon item={item} />
              <span className="command-text">
                <span className="command-title">{item.title}</span>
                <span className="command-sub">{item.subtitle}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
      <footer className="command-footer">
        <span>↑↓ choose</span>
        <span>Enter {naming ? "save" : "open"}</span>
        <span>Esc {naming || query ? "back" : "close"}</span>
      </footer>
    </div>
  );
}
