import type { FileHit } from "../core/generated/FileHit";
import type { InstalledApp } from "../core/generated/InstalledApp";
import type { NoteHit } from "../core/generated/NoteHit";
import type { WidgetKind } from "../core/generated/WidgetKind";
import type { Workspace } from "../core/generated/Workspace";
import { searchSettingsAll, sectionLabel } from "../settings/searchIndex";
import { calculate, formatResult } from "./calc";
import { rank } from "./rank";

export type CommandAction = "saveWorkspace" | "openSettings" | "windowsTaskbar";

/** One line in the command bar's list. */
export type CommandItem = { key: string; title: string; subtitle: string } & (
  | { kind: "calc"; value: string }
  | { kind: "app"; id: string; icon: string }
  | { kind: "file"; path: string }
  | { kind: "setting"; target: string }
  | { kind: "widget"; widget: WidgetKind }
  | { kind: "note"; text: string; todo: boolean }
  | { kind: "workspace"; name: string }
  | { kind: "action"; action: CommandAction }
);

export interface Sources {
  apps: InstalledApp[];
  notes: NoteHit[];
  workspaces: Workspace[];
  files: FileHit[];
  widgets: { kind: WidgetKind; label: string; description: string }[];
}

const ACTIONS: { action: CommandAction; title: string; subtitle: string; keywords: string[] }[] = [
  {
    action: "saveWorkspace",
    title: "Save open apps as a workspace…",
    subtitle: "Workspace",
    keywords: ["save", "workspace", "layout", "windows"],
  },
  { action: "openSettings", title: "Dock settings", subtitle: "Dock", keywords: ["preferences", "options"] },
  {
    action: "windowsTaskbar",
    title: "Switch to the Windows taskbar",
    subtitle: "Dock",
    keywords: ["hide dock", "toggle"],
  },
];

/** Shown before anything is typed: workspaces first, then the actions. */
function startingList(sources: Sources): CommandItem[] {
  return [
    ...sources.workspaces.slice(0, 5).map(workspaceItem),
    ...ACTIONS.map((a) => ({
      key: `action:${a.action}`,
      kind: "action" as const,
      action: a.action,
      title: a.title,
      subtitle: a.subtitle,
    })),
  ];
}

function workspaceItem(w: Workspace): CommandItem {
  return {
    key: `workspace:${w.name}`,
    kind: "workspace",
    name: w.name,
    title: `Restore “${w.name}”`,
    subtitle: `Workspace · ${w.apps.length} ${w.apps.length === 1 ? "window" : "windows"}`,
  };
}

/** Everything that matches `query`, best first: a sum's answer, then ranked results, then files. */
export function buildResults(query: string, sources: Sources, limit = 30): CommandItem[] {
  const q = query.trim();
  if (!q) return startingList(sources);
  const items: CommandItem[] = [];
  const value = calculate(q);
  if (value !== null) {
    const text = formatResult(value);
    items.push({
      key: "calc",
      kind: "calc",
      value: text,
      title: `= ${text}`,
      subtitle: "Press Enter to copy",
    });
  }

  type Candidate = { name: string; keywords?: string[]; boost: number; item: CommandItem };
  const candidates: Candidate[] = [
    ...sources.apps.map((a) => ({
      name: a.name,
      boost: 12,
      item: {
        key: `app:${a.id}`,
        kind: "app" as const,
        id: a.id,
        icon: a.icon,
        title: a.name,
        subtitle: "App",
      },
    })),
    ...sources.workspaces.map((w) => ({
      name: w.name,
      keywords: ["workspace", "restore"],
      boost: 10,
      item: workspaceItem(w),
    })),
    ...sources.widgets.map((w) => ({
      name: w.label,
      keywords: ["widget", ...w.description.split(/\W+/).filter((x) => x.length > 3)],
      boost: 4,
      item: {
        key: `widget:${w.kind}`,
        kind: "widget" as const,
        widget: w.kind,
        title: w.label,
        subtitle: "Widget",
      },
    })),
    ...searchSettingsAll().map((s) => ({
      name: s.name,
      keywords: s.keywords,
      boost: 0,
      item: {
        key: `setting:${s.section}/${s.id ?? ""}/${s.name}`,
        kind: "setting" as const,
        target: s.id ? `${s.section}/${s.id}` : s.section,
        title: s.name,
        subtitle: `Settings · ${sectionLabel(s.section)}`,
      },
    })),
    ...ACTIONS.map((a) => ({
      name: a.title,
      keywords: a.keywords,
      boost: 6,
      item: {
        key: `action:${a.action}`,
        kind: "action" as const,
        action: a.action,
        title: a.title,
        subtitle: a.subtitle,
      },
    })),
    ...sources.notes.map((n) => ({
      name: n.text.split("\n")[0],
      keywords: n.text.split(/\s+/).slice(0, 40),
      boost: -4,
      item: {
        key: `${n.kind}:${n.id}:${n.text.slice(0, 20)}`,
        kind: "note" as const,
        text: n.text,
        todo: n.kind === "todo",
        title: n.text.split("\n")[0].slice(0, 120),
        subtitle: n.kind === "todo" ? (n.done ? "To-do · done" : "To-do") : "Note",
      },
    })),
  ];
  items.push(...rank(q, candidates, limit).map((c) => c.item));
  const seen = new Set(items.map((i) => i.key));
  for (const f of sources.files) {
    const key = `file:${f.path}`;
    if (seen.has(key)) continue;
    seen.add(key);
    items.push({ key, kind: "file", path: f.path, title: f.name, subtitle: f.folder });
  }
  return items.slice(0, limit + sources.files.length);
}
