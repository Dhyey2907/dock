import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

import type { AccountStatus } from "./generated/AccountStatus";
import type { AskChunk } from "./generated/AskChunk";
import type { IndexHit } from "./generated/IndexHit";
import type { GithubState } from "./generated/GithubState";
import type { GoogleTask } from "./generated/GoogleTask";
import type { HotkeyAction } from "./generated/HotkeyAction";
import type { InstalledApp } from "./generated/InstalledApp";
import type { KeyAccount } from "./generated/KeyAccount";
import type { LatestScreenshot } from "./generated/LatestScreenshot";
import type { NoteHit } from "./generated/NoteHit";
import type { NotionTask } from "./generated/NotionTask";
import type { OAuthProvider } from "./generated/OAuthProvider";
import type { Playlist } from "./generated/Playlist";
import type { Settings } from "./generated/Settings";
import type { WidgetKind } from "./generated/WidgetKind";
import type { Workspace } from "./generated/Workspace";

/** Typed wrappers for the command bar, workspaces, screenshot, hotkeys and account widgets. */
export const featuresIpc = {
  hotkeySet: (action: HotkeyAction, shortcut: string) => invoke<Settings>("hotkey_set", { action, shortcut }),

  commandToggle: () => invoke<void>("command_toggle"),
  commandHide: () => invoke<void>("command_hide"),
  commandApps: () => invoke<InstalledApp[]>("command_apps"),
  commandFiles: (query: string) => invoke<IndexHit[]>("command_files", { query }),
  commandNotes: () => invoke<NoteHit[]>("command_notes"),
  commandLaunchApp: (id: string) => invoke<void>("command_launch_app", { id }),
  commandOpenFile: (path: string) => invoke<void>("command_open_file", { path }),
  commandOpenWidget: (kind: WidgetKind) => invoke<void>("command_open_widget", { kind }),
  /** `section` may name a control too: "taskbar/autoHide". */
  commandOpenSettings: (section: string) => invoke<void>("command_open_settings", { section }),

  workspacesList: () => invoke<Workspace[]>("workspaces_list"),
  workspaceSave: (name: string) => invoke<Workspace>("workspace_save", { name }),
  workspaceRestore: (name: string) => invoke<void>("workspace_restore", { name }),
  workspaceRemove: (name: string) => invoke<void>("workspace_remove", { name }),

  screenshotLatest: () => invoke<LatestScreenshot | null>("screenshot_latest"),
  screenshotOpen: (path: string) => invoke<void>("screenshot_open", { path }),
  screenshotDrag: (path: string) => invoke<void>("screenshot_drag", { path }),
  screenshotDismiss: () => invoke<void>("screenshot_dismiss"),

  accountStatus: () => invoke<AccountStatus>("account_status"),
  accountKeySet: (account: KeyAccount, value: string) =>
    invoke<AccountStatus>("account_key_set", { account, value }),
  accountConnect: (provider: OAuthProvider) => invoke<AccountStatus>("account_connect", { provider }),
  accountDisconnect: (provider: OAuthProvider) => invoke<AccountStatus>("account_disconnect", { provider }),
  accountOpenLink: (url: string) => invoke<void>("account_open_link", { url }),
  askClaude: (question: string) => invoke<number>("ask_claude", { question }),
  askClaudeStop: () => invoke<void>("ask_claude_stop"),
  spotifyPlaylists: () => invoke<Playlist[]>("spotify_playlists"),
  spotifyPlay: (uri: string) => invoke<void>("spotify_play", { uri }),
  notionTasks: () => invoke<NotionTask[]>("notion_tasks"),
  notionComplete: (id: string) => invoke<void>("notion_complete", { id }),
  googleTasks: () => invoke<GoogleTask[]>("google_tasks"),
  googleComplete: (listId: string, id: string) => invoke<void>("google_complete", { listId, id }),
  githubCurrent: () => invoke<GithubState>("github_current"),
  githubRefresh: () => invoke<GithubState>("github_refresh"),
};

function on<T>(name: string) {
  return (handler: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (event) => handler(event.payload));
}

function onThisWindow<T>(name: string) {
  return (handler: (payload: T) => void): Promise<UnlistenFn> =>
    getCurrentWebviewWindow().listen<T>(name, (event) => handler(event.payload));
}

export const featureEvents = {
  onCommandShown: onThisWindow<null>("command-shown"),
  onWidgetOpen: onThisWindow<WidgetKind>("widget-open"),
  onLatestScreenshot: onThisWindow<LatestScreenshot | null>("latest-screenshot"),
  onWorkspacesChanged: on<Workspace[]>("workspaces-changed"),
  onAskClaude: on<AskChunk>("ask-claude"),
  onGithub: on<GithubState>("widget-github"),
};

/** A backend error as a sentence for people: input problems carry their own words. */
export function errorText(error: unknown, fallback = "Something went wrong. Try again."): string {
  const kind = (error as { kind?: string })?.kind;
  const message = (error as { message?: string })?.message ?? "";
  return kind === "invalid_input" ? message.replace(/^invalid input: /, "") : fallback;
}

/** Subscribe in an effect, safely: unsubscribes even if the effect ends before `listen` resolves. */
export function subscribe(start: () => Promise<UnlistenFn>): () => void {
  let unlisten: UnlistenFn | undefined;
  let alive = true;
  start()
    .then((fn) => (alive ? (unlisten = fn) : fn()))
    .catch((error) => console.error("failed to listen", error));
  return () => {
    alive = false;
    unlisten?.();
  };
}
