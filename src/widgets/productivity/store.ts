import { create } from "zustand";

import { events, ipc } from "../../core/ipc";
import type { AppFolders } from "../../core/generated/AppFolders";
import type { ClaudeActivity } from "../../core/generated/ClaudeActivity";
import type { KanbanBoard } from "../../core/generated/KanbanBoard";
import type { Note } from "../../core/generated/Note";
import type { QuickLinks } from "../../core/generated/QuickLinks";
import type { ScreenText } from "../../core/generated/ScreenText";
import type { SongsToday } from "../../core/generated/SongsToday";
import type { TimeLog } from "../../core/generated/TimeLog";
import type { Todo } from "../../core/generated/Todo";

interface Data {
  notes: Note[];
  todos: Todo[];
  kanban: KanbanBoard;
  timelog: TimeLog;
  links: QuickLinks;
  folders: AppFolders;
  songs: SongsToday;
  claude: ClaudeActivity;
  pins: number;
  screenText: ScreenText;
}

interface ProductivityState extends Data {
  loaded: boolean;
  load: () => Promise<void>;
}

/** The productivity widgets' saved and live data, shared by the dock's tiles and the popup. */
export const useProductivity = create<ProductivityState>((set) => ({
  notes: [],
  todos: [],
  kanban: { cards: [] },
  timelog: { projects: [], entries: [] },
  links: { links: [] },
  folders: { folders: [] },
  songs: { day: null, songs: [] },
  claude: { sessions: [], running: 0 },
  pins: 0,
  screenText: { text: "" },
  loaded: false,
  load: async () => {
    // Listen first so a change while loading isn't missed; events win over the first read.
    const seen = new Set<keyof Data>();
    const mark =
      <K extends keyof Data>(key: K) =>
      (value: Data[K]) => {
        seen.add(key);
        set({ [key]: value } as Pick<Data, K>);
      };
    await Promise.all([
      events.onNotes(mark("notes")),
      events.onTodos(mark("todos")),
      events.onKanban(mark("kanban")),
      events.onTimelog(mark("timelog")),
      events.onLinks(mark("links")),
      events.onFolders(mark("folders")),
      events.onSongs(mark("songs")),
      events.onClaudeActivity(mark("claude")),
      events.onPins(mark("pins")),
      events.onScreenText(mark("screenText")),
    ]);
    const [notes, todos, kanban, timelog, links, folders, songs, pins, screenText, claude] =
      await Promise.all([
        ipc.notesGet(),
        ipc.todosGet(),
        ipc.kanbanGet(),
        ipc.timelogGet(),
        ipc.linksGet(),
        ipc.foldersGet(),
        ipc.songsGet(),
        ipc.pinsCount(),
        ipc.screenTextLast(),
        ipc.claudeSessionsGet(),
      ]);
    const first: Partial<Data> = {
      notes,
      todos,
      kanban,
      timelog,
      links,
      folders,
      songs,
      pins,
      screenText,
      claude,
    };
    const fresh = Object.fromEntries(Object.entries(first).filter(([key]) => !seen.has(key as keyof Data)));
    set({ ...fresh, loaded: true });
  },
}));

/** Report a failed call without breaking the page. */
export function report(what: string) {
  return (error: unknown) => console.error(`failed to ${what}`, error);
}
