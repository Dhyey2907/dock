import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  CircleCheck,
  ExternalLink,
  GitBranch,
  ListMusic,
  ListTodo,
  LoaderCircle,
  MessagesSquare,
  Play,
  RefreshCw,
  Send,
  Square,
  type LucideIcon,
} from "lucide-react";
import { create } from "zustand";

import type { AccountStatus } from "../../core/generated/AccountStatus";
import type { GithubState } from "../../core/generated/GithubState";
import type { RunState } from "../../core/generated/RunState";
import { ipc } from "../../core/ipc";
import { errorText, featureEvents, featuresIpc } from "../../core/ipcFeatures";
import { useSettings } from "../../settings/store";
import { useAccounts, watchAccounts } from "./status";

const report = (error: unknown) => console.error("account widget", error);

/** Account status for a popup, read again each time the popup comes into focus. */
function useStatus(): AccountStatus | null {
  const status = useAccounts((s) => s.status);
  useEffect(() => watchAccounts(), []);
  return status;
}

/** A widget that isn't set up yet: what it needs, and the way to Settings. */
function SetupNeeded({
  icon: Icon,
  title,
  text,
  place,
}: {
  icon: LucideIcon;
  title: string;
  text: string;
  place: string;
}) {
  return (
    <div className="panel-body panel-empty account-setup">
      <Icon size={28} strokeWidth={1.6} />
      <h2 className="panel-title">{title}</h2>
      <p className="panel-faint">{text}</p>
      <button
        className="panel-button"
        onClick={() => {
          ipc.panelClose().catch(report);
          ipc.openSettings(`accounts/${place}`).catch(report);
        }}
      >
        Set up in Settings
      </button>
    </div>
  );
}

function Problem({ text, retry }: { text: string; retry?: () => void }) {
  return (
    <p className="account-problem" role="alert">
      {text}{" "}
      {retry && (
        <button className="panel-link account-retry" onClick={retry}>
          Try again
        </button>
      )}
    </p>
  );
}

/** Load something when the popup opens, with loading and error states. */
function useLoad<T>(load: () => Promise<T>, enabled: boolean) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(enabled);
  const loadRef = useRef(load);
  useEffect(() => {
    loadRef.current = load;
  });
  // Only sets state once the answer is back.
  const fetchNow = useCallback(() => {
    loadRef
      .current()
      .then((value) => {
        setData(value);
        setError(null);
      })
      .catch((e) => setError(errorText(e, "That couldn't be loaded. Try again.")))
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => {
    if (enabled) fetchNow();
  }, [enabled, fetchNow]);
  const reload = () => {
    setLoading(true);
    setError(null);
    fetchNow();
  };
  return { data, setData, error, setError, loading, reload };
}

function Header({ title, loading, onRefresh }: { title: string; loading?: boolean; onRefresh?: () => void }) {
  return (
    <div className="account-head">
      <h2 className="panel-title">{title}</h2>
      {onRefresh && (
        <button className="account-icon-btn" aria-label="Refresh" title="Refresh" onClick={onRefresh}>
          {loading ? <LoaderCircle size={15} className="spin" /> : <RefreshCw size={15} />}
        </button>
      )}
    </div>
  );
}

function Scroll({ children }: { children: ReactNode }) {
  return <ul className="account-list">{children}</ul>;
}

// ---------- Ask Claude ----------

interface Turn {
  id: number;
  question: string;
  answer: string;
  done: boolean;
  error?: string;
}

interface AskState {
  turns: Turn[];
  listening: boolean;
  listen: () => void;
  ask: (question: string) => Promise<void>;
}

/** The conversation survives the popup closing (the popup window is reused). */
const useAsk = create<AskState>((set, get) => ({
  turns: [],
  listening: false,
  listen: () => {
    if (get().listening) return;
    set({ listening: true });
    featureEvents
      .onAskClaude((chunk) =>
        set({
          turns: get().turns.map((t) =>
            t.id !== chunk.id
              ? t
              : {
                  ...t,
                  answer: t.answer + (chunk.text ?? ""),
                  done: t.done || chunk.done,
                  error: chunk.error ?? t.error,
                },
          ),
        }),
      )
      .catch(report);
  },
  ask: async (question) => {
    const id = await featuresIpc.askClaude(question);
    set({ turns: [...get().turns, { id, question, answer: "", done: false }].slice(-6) });
  },
}));

export function AskClaudeTile() {
  return <MessagesSquare size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function AskClaudePanel() {
  const status = useStatus();
  const { turns, listen, ask } = useAsk();
  const [question, setQuestion] = useState("");
  const [error, setError] = useState<string | null>(null);
  const bottom = useRef<HTMLDivElement>(null);
  useEffect(() => listen(), [listen]);
  const last = turns[turns.length - 1];
  useEffect(() => bottom.current?.scrollIntoView?.({ block: "end" }), [last?.answer]);

  if (!status) return null;
  if (!status.anthropic) {
    return (
      <SetupNeeded
        icon={MessagesSquare}
        title="Ask Claude"
        text="Paste your Anthropic API key in Settings to ask Claude questions from the dock. Use is billed to your key."
        place="anthropic"
      />
    );
  }
  const busy = last !== undefined && !last.done;
  const send = () => {
    if (!question.trim() || busy) return;
    setError(null);
    ask(question)
      .then(() => setQuestion(""))
      .catch((e) => setError(errorText(e)));
  };

  return (
    <div className="panel-body ask-panel">
      <Header title="Ask Claude" />
      <div className="ask-thread">
        {turns.length === 0 && (
          <p className="panel-faint">Ask anything. Answers stream in as Claude writes them.</p>
        )}
        {turns.map((t) => (
          <div key={t.id} className="ask-turn">
            <p className="ask-question">{t.question}</p>
            <p className="ask-answer">
              {t.answer}
              {!t.done && !t.answer && <LoaderCircle size={14} className="spin" />}
            </p>
            {t.error && <Problem text={t.error} />}
          </div>
        ))}
        <div ref={bottom} />
      </div>
      {error && <Problem text={error} />}
      <div className="ask-input">
        <textarea
          rows={2}
          value={question}
          placeholder="Ask a question…"
          aria-label="Question"
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        />
        {busy ? (
          <button
            className="account-icon-btn"
            aria-label="Stop"
            title="Stop"
            onClick={() => featuresIpc.askClaudeStop().catch(report)}
          >
            <Square size={15} />
          </button>
        ) : (
          <button
            className="account-icon-btn"
            aria-label="Ask"
            title="Ask"
            disabled={!question.trim()}
            onClick={send}
          >
            <Send size={15} />
          </button>
        )}
      </div>
      <p className="panel-faint account-footnote">Claude Sonnet 5.5 · billed to your Anthropic API key</p>
    </div>
  );
}

// ---------- Spotify ----------

export function SpotifyTile() {
  return <ListMusic size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function SpotifyPanel() {
  const status = useStatus();
  const ready = Boolean(status?.spotify);
  const { data, error, setError, loading, reload } = useLoad(featuresIpc.spotifyPlaylists, ready);
  const [playing, setPlaying] = useState<string | null>(null);
  if (!status) return null;
  if (!ready) {
    return (
      <SetupNeeded
        icon={ListMusic}
        title="Spotify playlists"
        text="Add your Spotify app's client ID and sign in, in Settings → Accounts."
        place="spotify"
      />
    );
  }
  const play = (uri: string) => {
    setError(null);
    featuresIpc
      .spotifyPlay(uri)
      .then(() => setPlaying(uri))
      .catch((e) => setError(errorText(e)));
  };
  return (
    <div className="panel-body">
      <Header title="Your playlists" loading={loading} onRefresh={reload} />
      {error && <Problem text={error} retry={data ? undefined : reload} />}
      {data && data.length === 0 && <p className="panel-faint">No playlists yet.</p>}
      <Scroll>
        {data?.map((p) => (
          <li key={p.id} className="account-row" data-on={playing === p.uri}>
            <button className="account-row-main" onClick={() => play(p.uri)} title={`Play ${p.name}`}>
              <Play size={14} />
              <span className="account-row-text">
                <span>{p.name}</span>
                <span className="panel-faint">
                  {p.owner}
                  {p.tracks != null ? ` · ${p.tracks} songs` : ""}
                </span>
              </span>
            </button>
            <button
              className="account-icon-btn"
              aria-label={`Open ${p.name} in Spotify`}
              title="Open in Spotify"
              onClick={() => featuresIpc.accountOpenLink(p.link).catch(report)}
            >
              <ExternalLink size={13} />
            </button>
          </li>
        ))}
      </Scroll>
    </div>
  );
}

// ---------- Task lists (Notion and Google Tasks) ----------

interface TaskLike {
  key: string;
  title: string;
  detail?: string;
  link?: string;
}

function TaskList({
  title,
  load,
  complete,
}: {
  title: string;
  load: () => Promise<TaskLike[]>;
  complete: (task: TaskLike) => Promise<void>;
}) {
  const { data, setData, error, setError, loading, reload } = useLoad(load, true);
  const [ticking, setTicking] = useState<string | null>(null);
  const tick = (task: TaskLike) => {
    setTicking(task.key);
    setError(null);
    complete(task)
      .then(() => setData((list) => list?.filter((t) => t.key !== task.key) ?? null))
      .catch((e) => setError(errorText(e, "That task couldn't be ticked off.")))
      .finally(() => setTicking(null));
  };
  return (
    <div className="panel-body">
      <Header title={title} loading={loading} onRefresh={reload} />
      {error && <Problem text={error} retry={data ? undefined : reload} />}
      {data && data.length === 0 && (
        <p className="panel-faint account-done">
          <CircleCheck size={16} /> Nothing left to do.
        </p>
      )}
      <Scroll>
        {data?.map((task) => (
          <li key={task.key} className="account-row">
            <label className="account-row-main account-task">
              <input
                type="checkbox"
                checked={ticking === task.key}
                disabled={ticking !== null}
                onChange={() => tick(task)}
                aria-label={`Done: ${task.title}`}
              />
              <span className="account-row-text">
                <span>{task.title}</span>
                {task.detail && <span className="panel-faint">{task.detail}</span>}
              </span>
            </label>
            {task.link && (
              <button
                className="account-icon-btn"
                aria-label={`Open ${task.title}`}
                title="Open"
                onClick={() => featuresIpc.accountOpenLink(task.link!).catch(report)}
              >
                <ExternalLink size={13} />
              </button>
            )}
          </li>
        ))}
      </Scroll>
    </div>
  );
}

export function NotionTile() {
  return <ListTodo size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function NotionPanel() {
  const status = useStatus();
  const database = useSettings((s) => s.settings?.accounts.notionDatabaseId ?? "");
  if (!status) return null;
  if (!status.notion || !database) {
    return (
      <SetupNeeded
        icon={ListTodo}
        title="Notion tasks"
        text="Add a Notion integration token and your tasks database in Settings → Accounts."
        place="notion"
      />
    );
  }
  return (
    <TaskList
      title="Notion tasks"
      load={async () =>
        (await featuresIpc.notionTasks()).map((t) => ({
          key: t.id,
          title: t.title,
          link: t.link || undefined,
        }))
      }
      complete={(task) => featuresIpc.notionComplete(task.key)}
    />
  );
}

export function GoogleTasksTile() {
  return <CircleCheck size={18} strokeWidth={1.8} aria-hidden="true" />;
}

const dueText = (day: string) =>
  `Due ${new Date(`${day}T12:00:00`).toLocaleDateString(navigator.language || "en", { day: "numeric", month: "short" })}`;

export function GoogleTasksPanel() {
  const status = useStatus();
  if (!status) return null;
  if (!status.google) {
    return (
      <SetupNeeded
        icon={CircleCheck}
        title="Google Tasks"
        text="Add your Google client ID and sign in, in Settings → Accounts."
        place="google"
      />
    );
  }
  return (
    <TaskList
      title="Google Tasks"
      load={async () =>
        (await featuresIpc.googleTasks()).map((t) => ({
          key: `${t.listId}\n${t.id}`,
          title: t.title,
          detail: [t.list, t.due ? dueText(t.due) : ""].filter(Boolean).join(" · "),
        }))
      }
      complete={(task) => {
        const [listId, id] = task.key.split("\n");
        return featuresIpc.googleComplete(listId, id);
      }}
    />
  );
}

// ---------- GitHub Actions ----------

const useGithub = create<{ state: GithubState | null; started: boolean; start: () => void }>((set, get) => ({
  state: null,
  started: false,
  start: () => {
    if (get().started) return;
    set({ started: true });
    featureEvents.onGithub((state) => set({ state })).catch(report);
    featuresIpc
      .githubCurrent()
      .then((state) => get().state === null && set({ state }))
      .catch(report);
  },
}));

/** The worst state across repos, for the tile's dot. */
function overall(state: GithubState | null): RunState | null {
  const states = state?.runs.map((r) => r.state).filter((s): s is RunState => s != null) ?? [];
  if (states.length === 0) return null;
  for (const s of ["failed", "running", "passed"] as const) if (states.includes(s)) return s;
  return "other";
}

const STATE_TEXT: Record<RunState, string> = {
  running: "Running",
  passed: "Passed",
  failed: "Failed",
  other: "Cancelled or skipped",
};

export function GithubActionsTile() {
  const state = useGithub((s) => s.state);
  const start = useGithub((s) => s.start);
  useEffect(() => start(), [start]);
  const worst = overall(state);
  return (
    <span className="tile-row">
      <GitBranch size={17} strokeWidth={1.8} aria-hidden="true" />
      {worst && <span className="run-dot" data-state={worst} aria-label={STATE_TEXT[worst]} />}
    </span>
  );
}

const ago = (iso: string) => {
  const minutes = Math.max(0, Math.round((Date.now() - Date.parse(iso)) / 60_000));
  if (minutes < 60) return `${minutes} min ago`;
  const hours = Math.round(minutes / 60);
  return hours < 48 ? `${hours} h ago` : `${Math.round(hours / 24)} days ago`;
};

export function GithubActionsPanel() {
  const status = useStatus();
  const repos = useSettings((s) => s.settings?.accounts.githubRepos ?? []);
  const state = useGithub((s) => s.state);
  const start = useGithub((s) => s.start);
  const ready = Boolean(status?.github) && repos.length > 0;
  const [loading, setLoading] = useState(ready);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => start(), [start]);
  const fetchNow = useCallback(() => {
    featuresIpc
      .githubRefresh()
      .then((next) => {
        useGithub.setState({ state: next });
        setError(null);
      })
      .catch((e) => setError(errorText(e)))
      .finally(() => setLoading(false));
  }, []);
  useEffect(() => {
    if (ready) fetchNow();
  }, [ready, fetchNow]);
  const refresh = () => {
    setLoading(true);
    fetchNow();
  };

  if (!status) return null;
  if (!ready) {
    return (
      <SetupNeeded
        icon={GitBranch}
        title="GitHub Actions"
        text="Add a GitHub token and the repos to watch in Settings → Accounts."
        place="github"
      />
    );
  }
  return (
    <div className="panel-body">
      <Header title="GitHub Actions" loading={loading} onRefresh={refresh} />
      {error && <Problem text={error} />}
      <Scroll>
        {(state?.runs ?? []).map((run) => (
          <li key={run.repo} className="account-row">
            <button
              className="account-row-main"
              disabled={!run.link}
              onClick={() => run.link && featuresIpc.accountOpenLink(run.link).catch(report)}
              title={run.link ? "Open the run on GitHub" : undefined}
            >
              <span className="run-dot" data-state={run.state ?? "other"} />
              <span className="account-row-text">
                <span>{run.repo}</span>
                <span className="panel-faint">
                  {run.problem ??
                    [
                      run.state && STATE_TEXT[run.state],
                      run.workflow,
                      run.branch,
                      run.updated && ago(run.updated),
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                </span>
              </span>
            </button>
          </li>
        ))}
      </Scroll>
    </div>
  );
}
