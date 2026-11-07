import { useState, type ReactNode } from "react";

import { ipc } from "../../core/ipc";
import type { CardSettings } from "../../core/generated/CardSettings";
import { useSettings } from "../../settings/store";
import { useLoaded } from "../useLoaded";
import { useStopwatch } from "./clockStore";
import { bytes, duration } from "./format";
import { Bars, Meter } from "./parts";

function report(error: unknown) {
  console.error("card action failed", error);
}

/** A popup's title and body. */
function Panel({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="panel-body card-panel">
      <h2 className="panel-title">{title}</h2>
      {children}
    </div>
  );
}

/** A text field that saves into the card settings when you leave it (or press Enter). */
function Field({
  label,
  value,
  placeholder,
  onSave,
}: {
  label: string;
  value: string;
  placeholder?: string;
  onSave: (value: string) => void;
}) {
  const [draft, setDraft] = useState(value);
  return (
    <label className="card-field">
      <span className="panel-faint">{label}</span>
      <input
        value={draft}
        placeholder={placeholder}
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => draft !== value && onSave(draft)}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
    </label>
  );
}

function useCards(): [CardSettings | undefined, (change: Partial<CardSettings>) => void] {
  const cards = useSettings((s) => s.settings?.cards);
  const update = useSettings((s) => s.update);
  return [cards, (change) => cards && update({ cards: { ...cards, ...change } }).catch(report)];
}

const list = (text: string) =>
  text
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);

/** World clocks: which cities. */
export function WorldClocksPanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="World clocks">
      <Field
        label="Time zones, separated by commas"
        value={cards.worldClocks.join(", ")}
        placeholder="America/New_York, Europe/London, Asia/Tokyo"
        onSave={(v) => save({ worldClocks: list(v) })}
      />
      <p className="panel-faint">Use the zone's name, like Asia/Kolkata or Australia/Sydney. Up to three.</p>
    </Panel>
  );
}

/** Stocks: which tickers. */
export function StocksPanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="Stocks">
      <Field
        label="Tickers, separated by commas"
        value={cards.stocks.join(", ")}
        placeholder="AAPL, NVDA, RELIANCE.NS"
        onSave={(v) => save({ stocks: list(v) })}
      />
      <p className="panel-faint">
        The first one is the single-stock cards'. Indian shares end in .NS or .BO.
      </p>
    </Panel>
  );
}

/** Metric: which coin. */
export function MetricPanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="Live price">
      <Field
        label="Coin (CoinGecko id)"
        value={cards.metric}
        placeholder="bitcoin"
        onSave={(v) => save({ metric: v })}
      />
      <p className="panel-faint">For example bitcoin, ethereum or solana.</p>
    </Panel>
  );
}

/** Currency: which pair, and a converter. */
export function CurrencyPanel() {
  const [cards, save] = useCards();
  const { data } = useLoaded(ipc.currencyRate);
  const [amount, setAmount] = useState("1");
  if (!cards) return null;
  const value = Number(amount);
  return (
    <Panel title="Currency">
      <div className="card-field-row">
        <Field label="From" value={cards.currencyFrom} onSave={(v) => save({ currencyFrom: v })} />
        <Field label="To" value={cards.currencyTo} onSave={(v) => save({ currencyTo: v })} />
      </div>
      <label className="card-field">
        <span className="panel-faint">Amount</span>
        <input value={amount} inputMode="decimal" onChange={(e) => setAmount(e.target.value)} />
      </label>
      {data && Number.isFinite(value) && (
        <p className="panel-big">
          {(value * data.rate).toLocaleString(undefined, { maximumFractionDigits: 2 })} {data.to}
        </p>
      )}
      {data?.date && <p className="panel-faint">Rate from {data.date}.</p>}
    </Panel>
  );
}

/** GitHub Activity: whose grid. */
export function GithubPanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="GitHub activity">
      <Field
        label="GitHub username"
        value={cards.githubUser}
        placeholder="octocat"
        onSave={(v) => save({ githubUser: v })}
      />
      <p className="panel-faint">Public contributions only; no sign-in needed.</p>
    </Panel>
  );
}

/** LeetCode: whose stats. */
export function LeetcodePanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="LeetCode">
      <Field label="LeetCode username" value={cards.leetcodeUser} onSave={(v) => save({ leetcodeUser: v })} />
    </Panel>
  );
}

/** Timer: its length. */
export function TimerPanel() {
  const [cards, save] = useCards();
  if (!cards) return null;
  return (
    <Panel title="Timer">
      <Field
        label="Minutes"
        value={String(cards.timerMinutes)}
        onSave={(v) => {
          const minutes = Math.round(Number(v));
          if (minutes > 0) save({ timerMinutes: minutes });
        }}
      />
    </Panel>
  );
}

/** Stopwatch: its laps. */
export function StopwatchPanel() {
  const laps = useStopwatch((s) => s.laps);
  const time = (ms: number) =>
    `${Math.floor(ms / 60000)}:${String(Math.floor((ms / 1000) % 60)).padStart(2, "0")}.${Math.floor((ms % 1000) / 100)}`;
  return (
    <Panel title="Laps">
      {laps.length === 0 && <p className="panel-faint">Press Lap on the card while it runs.</p>}
      <ol className="card-panel-list">
        {laps.map((lap, i) => (
          <li key={i}>
            <span>Lap {i + 1}</span>
            <span className="card-tabular">{time(lap - (laps[i - 1] ?? 0))}</span>
            <span className="panel-faint card-tabular">{time(lap)}</span>
          </li>
        ))}
      </ol>
    </Panel>
  );
}

/** Screen time: today per app, and the week. */
export function ScreenTimePanel() {
  const { data } = useLoaded(ipc.screenTime, 60_000);
  const top = data?.apps[0]?.minutes ?? 1;
  const days = Array.from({ length: 7 }, (_, i) => {
    const d = new Date();
    d.setDate(d.getDate() - 6 + i);
    return d.toLocaleDateString(undefined, { weekday: "narrow" });
  });
  return (
    <Panel title={`Screen time · ${duration(data?.minutes ?? 0)}`}>
      <ul className="card-panel-list">
        {(data?.apps ?? []).map((app) => (
          <li key={app.name}>
            <span className="card-panel-name">{app.name}</span>
            <Meter value={(app.minutes / top) * 100} width={90} />
            <span className="panel-faint card-tabular">{duration(app.minutes)}</span>
          </li>
        ))}
        {data && data.apps.length === 0 && (
          <li className="panel-faint">Counting starts now; check back in a few minutes.</li>
        )}
      </ul>
      <div className="card-week-bars">
        <Bars values={data?.week ?? []} width={260} height={50} mark={6} />
        <div className="card-week-labels">
          {days.map((d, i) => (
            <span key={i}>{d}</span>
          ))}
        </div>
      </div>
    </Panel>
  );
}

function FileList({
  read,
  title,
  empty,
}: {
  read: () => ReturnType<typeof ipc.recentFiles>;
  title: string;
  empty: string;
}) {
  const { data } = useLoaded(read, 30_000);
  return (
    <Panel title={title}>
      <ul className="card-panel-list">
        {(data ?? []).map((file) => (
          <li key={file.path}>
            <button className="card-panel-file" onClick={() => ipc.openListedFile(file.path).catch(report)}>
              {file.thumbnail && <img src={file.thumbnail} alt="" />}
              <span className="card-panel-name">{file.name}</span>
              <span className="panel-faint">{new Date(file.modified * 1000).toLocaleDateString()}</span>
            </button>
          </li>
        ))}
        {data?.length === 0 && <li className="panel-faint">{empty}</li>}
      </ul>
    </Panel>
  );
}

export function RecentFilesPanel() {
  return <FileList read={ipc.recentFiles} title="Recently opened" empty="Nothing opened lately." />;
}

export function DownloadsPanel() {
  return <FileList read={ipc.downloadsList} title="Downloads" empty="Downloads is empty." />;
}

export function ScreenshotsPanel() {
  return <FileList read={ipc.screenshotsList} title="Screenshots" empty="No screenshots yet." />;
}

/** Storage: every drive. */
export function StoragePanel() {
  const { data } = useLoaded(ipc.drives, 60_000);
  return (
    <Panel title="Storage">
      <ul className="card-panel-list">
        {(data ?? []).map((d) => {
          const used = d.total ? ((d.total - d.free) / d.total) * 100 : 0;
          return (
            <li key={d.name}>
              <span className="card-panel-name">{d.name}</span>
              <Meter value={used} width={110} color={used > 90 ? "#ff453a" : "#0a84ff"} />
              <span className="panel-faint">
                {bytes(d.free)} free of {bytes(d.total)}
              </span>
            </li>
          );
        })}
      </ul>
    </Panel>
  );
}

/** Search: the web, or a folder by its path. */
export function WebSearchPanel() {
  const [query, setQuery] = useState("");
  return (
    <Panel title="Search">
      <input
        className="card-search-input"
        autoFocus
        value={query}
        placeholder="Search the web, or paste a folder path"
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && query.trim()) {
            ipc
              .webSearch(query)
              .then(() => ipc.panelClose())
              .catch(report);
          }
        }}
      />
      <p className="panel-faint">Press Enter. A path like C:\Users opens in File Explorer.</p>
    </Panel>
  );
}

/** Codex: the plan's windows in detail. */
export function CodexPanel() {
  const { data } = useLoaded(ipc.codexUsage, 60_000);
  const label = (minutes: number) =>
    minutes >= 28 * 1440 ? "Monthly" : minutes >= 7 * 1440 ? "Weekly" : `${Math.round(minutes / 60)}-hour`;
  return (
    <Panel title="ChatGPT (Codex) limits">
      {!data && (
        <p className="panel-faint">Codex hasn't been used on this PC, so there's nothing to read yet.</p>
      )}
      {data && <p className="panel-faint">Plan: {data.plan || "unknown"} · read from Codex's own files.</p>}
      {[data?.primary, data?.secondary].map(
        (w, i) =>
          w && (
            <div key={i} className="card-panel-limit">
              <span>{label(w.windowMinutes)}</span>
              <Meter value={w.usedPercent} width={150} />
              <span className="card-tabular">{Math.round(w.usedPercent)}%</span>
              <span className="panel-faint">resets {new Date(w.resetsAt * 1000).toLocaleString()}</span>
            </div>
          ),
      )}
    </Panel>
  );
}
