import { useState } from "react";
import {
  Bitcoin,
  Camera,
  Code2,
  Download,
  FileText,
  GitBranch,
  Globe,
  HardDrive,
  Image as ImageIcon,
  Search,
  Trash2,
} from "lucide-react";

import { ipc } from "../../core/ipc";
import { useSettings } from "../../settings/store";
import { useLoaded } from "../useLoaded";
import { Card, CardButton, DotGrid, IconTile, Meter, Spark } from "./parts";
import { bytes } from "./format";
import { change, COINS, coinLabel, money } from "./quotes";

const readQuotes = () => ipc.stockQuotes();

/** The tickers from Settings, so a change in the popup reads again at once. */
function useQuotes() {
  const tickers = useSettings((s) => s.settings?.cards.stocks.join(",") ?? "");
  const loaded = useLoaded(readQuotes, 120_000, tickers);
  return { ...loaded, tickers: tickers ? tickers.split(",") : [] };
}

/** What a stock card says while it has no price: loading, no ticker, or couldn't reach it. */
function quoteNote(failed: boolean, tickers: string[]) {
  if (tickers.length === 0) return "Add a ticker";
  return failed ? "Couldn't get prices" : "Loading…";
}

function Change({ value }: { value: number }) {
  return (
    <span className={value >= 0 ? "card-green" : "card-red"}>
      {value >= 0 ? "▲" : "▼"} {Math.abs(value).toFixed(2)}%
    </span>
  );
}

/** Stock: one ticker's price and daily change. */
export function StockCard() {
  const { data, failed, tickers } = useQuotes();
  const q = data?.[0];
  return (
    <Card>
      <span className="card-col">
        <span className="card-label">{q?.symbol ?? tickers[0] ?? "Stock"}</span>
        {q ? (
          <span className="card-big card-tabular">{money(q.price, q.currency)}</span>
        ) : (
          <span className="card-faint">{quoteNote(failed, tickers)}</span>
        )}
      </span>
      {q && <Change value={change(q)} />}
    </Card>
  );
}

/** Stock Chart: one ticker with today's line. */
export function StockChartCard() {
  const { data, failed, tickers } = useQuotes();
  const q = data?.[0];
  const up = q ? change(q) >= 0 : true;
  return (
    <Card className="card-wide card-chart">
      <span className="card-row card-between">
        <span className="card-label">{q?.symbol ?? tickers[0] ?? "Stock"}</span>
        <span className="card-col card-end">
          {q ? (
            <>
              <span className="card-label card-tabular">{money(q.price, q.currency)}</span>
              <Change value={change(q)} />
            </>
          ) : (
            <span className="card-faint">{quoteNote(failed, tickers)}</span>
          )}
        </span>
      </span>
      <Spark points={q?.points ?? []} width={200} height={22} color={up ? "#30d158" : "#ff453a"} fill />
    </Card>
  );
}

/** Stock Watchlist: three tickers at a glance. */
export function StockWatchlistCard() {
  const { data, failed, tickers } = useQuotes();
  return (
    <Card>
      <span className="card-col card-list">
        {(data ?? []).map((q) => (
          <span key={q.symbol} className="card-row card-between card-list-row">
            <span className="card-small card-label">{q.symbol}</span>
            <span className="card-small card-tabular">{money(q.price, q.currency)}</span>
            <span className="card-small">
              <Change value={change(q)} />
            </span>
          </span>
        ))}
        {(!data || data.length === 0) && (
          <span className="card-faint">
            {data ? "No prices for those tickers" : quoteNote(failed, tickers)}
          </span>
        )}
      </span>
    </Card>
  );
}

const readCoin = () => ipc.coinPrice();

/** Metric: a live price, such as BTC/USD. */
export function MetricCard() {
  const coin = useSettings((s) => s.settings?.cards.metric || "bitcoin");
  const { data, failed } = useLoaded(readCoin, 120_000, coin);
  const id = data?.id ?? coin;
  const known = COINS[id];
  return (
    <Card>
      <IconTile color={known?.color ?? "#3a3a3c"}>
        {id === "bitcoin" ? (
          <Bitcoin strokeWidth={2.4} />
        ) : (
          <span className="card-mark card-coin-mark">{(known?.symbol ?? id).slice(0, 1).toUpperCase()}</span>
        )}
      </IconTile>
      <span className="card-col">
        <span className="card-label card-dim">{coinLabel(id)} · USD</span>
        {data ? (
          <span className="card-big card-tabular">{money(data.usd)}</span>
        ) : (
          <span className="card-faint">{failed ? "Couldn't get the price" : "Loading…"}</span>
        )}
      </span>
      {data && <Change value={data.change24h} />}
    </Card>
  );
}

const readGithub = () => ipc.githubContributions();

/** GitHub Activity: the contribution grid and streak. */
export function GithubActivityCard() {
  const user = useSettings((s) => s.settings?.cards.githubUser ?? "");
  const { data, failed } = useLoaded(readGithub, 900_000, user);
  if (!data || !user) {
    return (
      <Card>
        <GitBranch />
        <span className="card-faint">
          {!user ? "Add your GitHub username" : failed ? `Couldn't reach GitHub for ${user}` : "Loading…"}
        </span>
      </Card>
    );
  }
  return (
    <Card className="card-github">
      <span className="card-col">
        <span className="card-row card-gap">
          <span className="card-small">
            <b>{data.today}</b> today
          </span>
          <span className="card-small">
            <b>{data.week}</b> week
          </span>
          <span className="card-small">
            🔥 <b>{data.streak}</b> streak
          </span>
        </span>
        <DotGrid
          total={data.levels.length}
          columns={Math.ceil(data.levels.length / 7)}
          levels={data.levels}
          square
          cell={5}
        />
      </span>
    </Card>
  );
}

const readLeetcode = () => ipc.leetcodeStats();

/** LeetCode Stats: problems solved. */
export function LeetcodeCard() {
  const user = useSettings((s) => s.settings?.cards.leetcodeUser ?? "");
  const { data, failed } = useLoaded(readLeetcode, 900_000, user);
  return (
    <Card>
      <IconTile color="#ffa116">
        <Code2 strokeWidth={2.4} />
      </IconTile>
      {data && user ? (
        <span className="card-col">
          <span className="card-big">{data.total} solved</span>
          <span className="card-row card-gap card-small">
            <span className="card-green">{data.easy}</span>
            <span className="card-yellow">{data.medium}</span>
            <span className="card-red">{data.hard}</span>
          </span>
        </span>
      ) : (
        <span className="card-faint">
          {!user ? "Add your LeetCode username" : failed ? `Couldn't reach LeetCode for ${user}` : "Loading…"}
        </span>
      )}
    </Card>
  );
}

// ---------- Apps and files ----------

const readRecent = () => ipc.recentFiles();

/** Recently Opened: the files you used last. */
export function RecentFilesCard() {
  const { data, failed } = useLoaded(readRecent, 60_000);
  const latest = data?.[0];
  return (
    <Card>
      <IconTile color="#5e5ce6">
        <FileText strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">Recent</span>
        <span className="card-line">
          {latest?.name ?? (data ? "Nothing yet" : failed ? "Couldn't read" : "…")}
        </span>
      </span>
    </Card>
  );
}

const readBin = () => ipc.recycleBin();

/** Recycle Bin: what it holds, and a click (twice, to be sure) to empty it. */
export function RecycleBinCard() {
  const { data, reload } = useLoaded(readBin, 30_000);
  const [confirm, setConfirm] = useState(false);
  const empty = !data || data.items === 0;
  return (
    <Card>
      <IconTile color={empty ? "#636366" : "#8e8e93"}>
        <Trash2 strokeWidth={2.2} />
      </IconTile>
      <span className="card-col">
        <span className="card-label">
          {empty ? "Empty" : `${data.items} item${data.items === 1 ? "" : "s"}`}
        </span>
        <span className="card-faint">{empty ? "Recycle Bin" : bytes(data.bytes)}</span>
      </span>
      {!empty && (
        <CardButton
          label={confirm ? "Click again to delete for good" : "Empty the Recycle Bin"}
          className={confirm ? "card-pill card-danger" : "card-pill"}
          onPress={() => {
            if (!confirm) {
              setConfirm(true);
              window.setTimeout(() => setConfirm(false), 4000);
              return;
            }
            setConfirm(false);
            ipc.recycleBinEmpty().then(reload).catch(console.error);
          }}
        >
          {confirm ? "Sure?" : "Empty"}
        </CardButton>
      )}
    </Card>
  );
}

/** Search: the web, or a folder by its path (opens in the popup). */
export function WebSearchCard() {
  return (
    <Card className="card-search">
      <Search />
      <span className="card-faint">Search the web</span>
      <Globe className="card-faint" />
    </Card>
  );
}

const readDrives = () => ipc.drives();

/** Storage: free space on the system drive. */
export function StorageCard() {
  const { data, failed } = useLoaded(readDrives, 300_000);
  const drive = data?.find((d) => d.name === "C:") ?? data?.[0];
  const used = drive && drive.total ? ((drive.total - drive.free) / drive.total) * 100 : 0;
  return (
    <Card>
      <IconTile color="#64d2ff">
        <HardDrive strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-grow">
        <span className="card-row card-between">
          <span className="card-label">{drive?.name ?? "Drive"}</span>
          <span className="card-faint">
            {drive ? `${bytes(drive.free)} free` : failed ? "Couldn't read" : "…"}
          </span>
        </span>
        <Meter value={used} color={used > 90 ? "#ff453a" : "#0a84ff"} width={100} />
      </span>
    </Card>
  );
}

const readShots = () => ipc.screenshotsList();

/** Recent Screenshots: the newest ones as pictures. */
export function ScreenshotsCard() {
  const { data, failed } = useLoaded(readShots, 60_000);
  return (
    <Card>
      {!data && (
        <>
          <ImageIcon className="card-faint" />
          <span className="card-faint">{failed ? "Couldn't read screenshots" : "Loading…"}</span>
        </>
      )}
      {(data ?? []).map((shot) => (
        <CardButton
          key={shot.path}
          label={shot.name}
          className="card-thumb"
          onPress={() => ipc.openListedFile(shot.path).catch(console.error)}
        >
          {shot.thumbnail ? <img src={shot.thumbnail} alt="" draggable={false} /> : <ImageIcon />}
        </CardButton>
      ))}
      {data?.length === 0 && <span className="card-faint">No screenshots yet</span>}
    </Card>
  );
}

const readDownloads = () => ipc.downloadsList();

/** Downloads: the newest file, one click to open. */
export function DownloadsCard() {
  const { data, failed } = useLoaded(readDownloads, 30_000);
  const latest = data?.[0];
  return (
    <Card>
      <IconTile color="#30d158">
        <Download strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">Downloads</span>
        {latest ? (
          <CardButton
            label={`Open ${latest.name}`}
            className="card-link"
            onPress={() => ipc.openListedFile(latest.path).catch(console.error)}
          >
            <span className="card-line">{latest.name}</span>
          </CardButton>
        ) : (
          <span className="card-line">{data ? "Nothing yet" : failed ? "Couldn't read" : "…"}</span>
        )}
      </span>
    </Card>
  );
}

/** Screenshot Capture: the screen, a window or an area. */
export function ScreenCaptureCard() {
  return (
    <Card>
      <CardButton
        label="Capture an area"
        className="card-round"
        onPress={() => ipc.screenCapture().catch(console.error)}
      >
        <Camera />
      </CardButton>
      <span className="card-label">Capture</span>
    </Card>
  );
}
