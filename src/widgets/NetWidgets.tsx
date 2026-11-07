import { Flame, Star, Wifi, WifiOff } from "lucide-react";

import { Fluent } from "../core/Fluent";
import { useWindowsLook } from "../core/system";
import { ipc } from "../core/ipc";
import { compactCount, signalWord } from "./systemLogic";
import { Note } from "./systemShared";
import { useLoaded } from "./useLoaded";
import { useWidgets } from "./store";

// ---------- Wi-Fi details ----------

export function WifiTile() {
  const network = useWidgets((s) => s.network);
  const windowsLook = useWindowsLook();
  if (windowsLook) return <Fluent glyph={network?.connected ? "wifi" : "offline"} />;
  const Icon = network?.connected ? Wifi : WifiOff;
  return <Icon size={18} strokeWidth={1.9} aria-hidden="true" />;
}

function Fact({ label, value }: { label: string; value?: string }) {
  return (
    <div className="tool-fact">
      <span className="panel-faint">{label}</span>
      <span className="tool-tabular">{value ?? "—"}</span>
    </div>
  );
}

export function WifiPanel() {
  const { data } = useLoaded(ipc.wifiDetails);
  // One request each time the popup opens; never polled.
  const { data: publicIp, loading: askingPublic } = useLoaded(ipc.publicIp);
  const signal = data?.signal;
  return (
    <div className="panel-body">
      <h2 className="panel-title">{data?.wifi ? "Wi-Fi" : "Network"}</h2>
      <div className="panel-big tool-ellipsis">
        {data ? (data.connected ? (data.name ?? "Connected") : "Offline") : "—"}
      </div>
      {data?.connected && !data.internet && <span className="panel-faint">No internet access</span>}
      <div className="tool-facts">
        {data?.wifi && (
          <Fact label="Signal" value={signal != null ? `${signalWord(signal)} · ${signal}%` : undefined} />
        )}
        {data?.wifi && (
          <Fact label="Link speed" value={data.linkMbps ? `${data.linkMbps} Mbps` : undefined} />
        )}
        <Fact label="Local address" value={data?.localIp} />
        <Fact
          label="Public address"
          value={publicIp ?? (askingPublic ? "Checking…" : data?.internet ? "Unavailable" : undefined)}
        />
      </div>
      <button className="panel-link" onClick={() => ipc.openWindowsSettings("network")}>
        Network settings
      </button>
    </div>
  );
}

// ---------- GitHub trending ----------

export function GithubTrendingTile() {
  return <Flame size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function GithubTrendingPanel() {
  const { data, loading } = useLoaded(ipc.githubTrending);
  const repos = data?.repos ?? [];
  return (
    <div className="panel-body">
      <div className="tool-head">
        <h2 className="panel-title">Trending on GitHub</h2>
        {data?.stale && repos.length > 0 && <span className="panel-faint">Offline · earlier list</span>}
      </div>
      {loading && !data && <Note>Loading today's list…</Note>}
      {data && repos.length === 0 && <Note>Can't reach GitHub right now.</Note>}
      <ul className="tool-list tool-scroll">
        {repos.map((repo) => (
          <li key={repo.repo}>
            <button
              className="tool-repo"
              onClick={() => ipc.openUrl(`https://github.com/${repo.repo}`).catch((e) => console.error(e))}
            >
              <span className="tool-name">{repo.repo}</span>
              {repo.description && <span className="tool-repo-text panel-faint">{repo.description}</span>}
              <span className="tool-repo-meta panel-faint">
                {repo.language && <span>{repo.language}</span>}
                <span>
                  <Star size={11} aria-hidden="true" /> {compactCount(repo.stars)}
                </span>
                {repo.starsToday > 0 && <span>+{compactCount(repo.starsToday)} today</span>}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
