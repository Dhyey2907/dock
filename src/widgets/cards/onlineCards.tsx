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
import type { Quote } from "../../core/generated/Quote";
import { useLoaded } from "../useLoaded";
import { Card, CardButton, DotGrid, IconTile, Meter, Spark } from "./parts";
import { bytes } from "./format";

const readQuotes = () => ipc.stockQuotes();

function change(q: Quote) {
  return q.previousClose ? ((q.price - q.previousClose) / q.previousClose) * 100 : 0;
}

function Change({ value }: { value: number }) {
  return (
    <span className={value >= 0 ? "card-green" : "card-red"}>
      {value >= 0 ? "▲" : "▼"} {Math.abs(value).toFixed(2)}%
    </span>
  );
}

const money = (n: number, currency = "USD") =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency,
    maximumFractionDigits: n >= 1000 ? 0 : 2,
  }).format(n);

/** Stock: one ticker's price and daily change. */
export function StockCard() {
  const { data, failed } = useLoaded(readQuotes, 120_000);
  const q = data?.[0];
  return (
    <Card>
      <span className="card-col">
        <span className="card-label">{q?.symbol ?? "…"}</span>
        <span className="card-big card-tabular">{q ? money(q.price, q.currency) : failed ? "—" : "…"}</span>
      </span>
      {q && <Change value={change(q)} />}
    </Card>
  );
}

/** Stock Chart: one ticker with today's line. */
export function StockChartCard() {
  const { data } = useLoaded(readQuotes, 120_000);
  const q = data?.[0];
  const up = q ? change(q) >= 0 : true;
  return (
    <Card className="card-wide card-chart">
      <span className="card-row card-between">
        <span className="card-label">{q?.symbol ?? "…"}</span>
        <span className="card-col card-end">
          <span className="card-label card-tabular">{q ? money(q.price, q.currency) : "…"}</span>
          {q && <Change value={change(q)} />}
        </span>
      </span>
      <Spark points={q?.points ?? []} width={200} height={22} color={up ? "#30d158" : "#ff453a"} fill />
    </Card>
  );
}

/** Stock Watchlist: three tickers at a glance. */
export function StockWatchlistCard() {
  const { data } = useLoaded(readQuotes, 120_000);
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
        {!data && <span className="card-faint">Loading quotes…</span>}
      </span>
    </Card>
  );
}

const readCoin = () => ipc.coinPrice();

/** Metric: a live price, such as BTC/USD. */
export function MetricCard() {
  const { data, failed } = useLoaded(readCoin, 120_000);
  return (
    <Card>
      <IconTile color="#f7931a">
        <Bitcoin strokeWidth={2.4} />
      </IconTile>
      <span className="card-col">
        <span className="card-label card-dim">{data?.id ?? "bitcoin"}</span>
        <span className="card-big card-tabular">{data ? money(data.usd) : failed ? "—" : "…"}</span>
      </span>
      {data && <Change value={data.change24h} />}
    </Card>
  );
}

const readGithub = () => ipc.githubContributions();

/** GitHub Activity: the contribution grid and streak. */
export function GithubActivityCard() {
  const { data, failed } = useLoaded(readGithub, 900_000);
  if (!data) {
    return (
      <Card>
        <GitBranch />
        <span className="card-faint">{failed ? "Add your GitHub username" : "Loading…"}</span>
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
  const { data, failed } = useLoaded(readLeetcode, 900_000);
  return (
    <Card>
      <IconTile color="#ffa116">
        <Code2 strokeWidth={2.4} />
      </IconTile>
      {data ? (
        <span className="card-col">
          <span className="card-big">{data.total} solved</span>
          <span className="card-row card-gap card-small">
            <span className="card-green">{data.easy}</span>
            <span className="card-yellow">{data.medium}</span>
            <span className="card-red">{data.hard}</span>
          </span>
        </span>
      ) : (
        <span className="card-faint">{failed ? "Add your LeetCode username" : "Loading…"}</span>
      )}
    </Card>
  );
}

// ---------- Apps and files ----------

const readRecent = () => ipc.recentFiles();

/** Recently Opened: the files you used last. */
export function RecentFilesCard() {
  const { data } = useLoaded(readRecent, 60_000);
  const latest = data?.[0];
  return (
    <Card>
      <IconTile color="#5e5ce6">
        <FileText strokeWidth={2.2} />
      </IconTile>
      <span className="card-col card-text">
        <span className="card-label card-dim">Recent</span>
        <span className="card-line">{latest?.name ?? "Nothing yet"}</span>
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
  const { data } = useLoaded(readDrives, 300_000);
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
          <span className="card-faint">{drive ? `${bytes(drive.free)} free` : "…"}</span>
        </span>
        <Meter value={used} color={used > 90 ? "#ff453a" : "#0a84ff"} width={100} />
      </span>
    </Card>
  );
}

const readShots = () => ipc.screenshotsList();

/** Recent Screenshots: the newest ones as pictures. */
export function ScreenshotsCard() {
  const { data } = useLoaded(readShots, 60_000);
  return (
    <Card>
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
  const { data } = useLoaded(readDownloads, 30_000);
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
          <span className="card-line">Nothing yet</span>
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
