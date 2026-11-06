import { useState } from "react";
import { Activity, Clock3, Globe, RefreshCw, Server, Usb } from "lucide-react";

import { ipc } from "../core/ipc";
import type { TopApp } from "../core/generated/TopApp";
import { ConfirmButton, Note } from "./systemShared";
import { useLoaded } from "./useLoaded";
import { formatBytes, formatUptime, shortUptime, usedPercent } from "./systemLogic";
import { useNow } from "./useNow";

// ---------- Top apps ----------

export function TopAppsTile() {
  return <Activity size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function TopAppsPanel() {
  const { data, reload } = useLoaded(ipc.topApps, 3000);
  const [sort, setSort] = useState<"cpu" | "memory">("cpu");
  const [problem, setProblem] = useState<string | null>(null);
  const apps = sort === "cpu" ? data?.byCpu : data?.byMemory;

  const end = (app: TopApp) => {
    setProblem(null);
    ipc
      .processEnd(app.pids, app.name)
      .then(reload)
      .catch(() => setProblem(`Couldn't end ${app.name}. It may need administrator rights.`));
  };

  return (
    <div className="panel-body">
      <div className="tool-head">
        <h2 className="panel-title">Top apps</h2>
        <span className="tool-tabs" role="tablist">
          <button role="tab" aria-selected={sort === "cpu"} onClick={() => setSort("cpu")}>
            Processor
          </button>
          <button role="tab" aria-selected={sort === "memory"} onClick={() => setSort("memory")}>
            Memory
          </button>
        </span>
      </div>
      {!apps ? (
        <Note>Measuring…</Note>
      ) : (
        <ul className="tool-list">
          {apps.map((app) => (
            <li key={app.name} className="tool-row">
              <span className="tool-text">
                <span className="tool-name">{app.name}</span>
                <span className="panel-faint">
                  {app.cpu.toFixed(1)}% CPU · {formatBytes(app.memory)}
                  {app.pids.length > 1 ? ` · ${app.pids.length} processes` : ""}
                </span>
              </span>
              {!app.protected && (
                <ConfirmButton
                  label="End task"
                  question={`End ${app.name}?`}
                  confirm="End"
                  onConfirm={() => end(app)}
                />
              )}
            </li>
          ))}
        </ul>
      )}
      {problem && <Note>{problem}</Note>}
    </div>
  );
}

// ---------- Local dev servers ----------

export function DevServersTile() {
  return <Server size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function DevServersPanel() {
  const { data, reload, loading } = useLoaded(ipc.devServers);
  const [problem, setProblem] = useState<string | null>(null);
  return (
    <div className="panel-body">
      <div className="tool-head">
        <h2 className="panel-title">Local servers</h2>
        <button className="icon-btn" aria-label="Refresh" disabled={loading} onClick={reload}>
          <RefreshCw size={14} />
        </button>
      </div>
      {data && data.length === 0 && <Note>Nothing is listening on this PC right now.</Note>}
      <ul className="tool-list">
        {data?.map((server) => (
          <li key={server.port} className="tool-row">
            <span className="tool-text">
              <span className="tool-name tool-tabular">localhost:{server.port}</span>
              <span className="panel-faint">
                {server.process}
                {server.public ? " · open to your network" : ""}
              </span>
            </span>
            <button
              className="icon-btn"
              aria-label={`Open localhost:${server.port} in the browser`}
              title="Open in browser"
              onClick={() => ipc.openUrl(`http://localhost:${server.port}`).catch((e) => console.error(e))}
            >
              <Globe size={14} />
            </button>
            {!server.protected && (
              <ConfirmButton
                label="Stop"
                question={`Stop ${server.process}?`}
                confirm="Stop"
                onConfirm={() => {
                  setProblem(null);
                  ipc
                    .devServerStop(server.port, server.pid)
                    .then(reload)
                    .catch(() => setProblem(`Couldn't stop ${server.process}.`));
                }}
              />
            )}
          </li>
        ))}
      </ul>
      {problem && <Note>{problem}</Note>}
    </div>
  );
}

// ---------- USB drives ----------

export function UsbDrivesTile() {
  return <Usb size={18} strokeWidth={1.9} aria-hidden="true" />;
}

export function UsbDrivesPanel() {
  const { data, reload } = useLoaded(ipc.usbDrives);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const eject = (letter: string, label: string) => {
    setBusy(letter);
    setMessage(null);
    ipc
      .usbEject(letter)
      .then((result) => {
        if (result.result === "ejected") setMessage(`${label} (${letter}:) is safe to remove.`);
        else if (result.result === "inUse")
          setMessage(
            `${label} is in use${result.by ? ` by ${result.by}` : ""}. Close its files and try again.`,
          );
        else setMessage(`${label} couldn't be ejected.`);
      })
      .catch(() => setMessage(`${label} couldn't be ejected.`))
      .finally(() => {
        setBusy(null);
        reload();
      });
  };

  return (
    <div className="panel-body">
      <h2 className="panel-title">USB drives</h2>
      {data && data.length === 0 && !message && <Note>No USB drives are plugged in.</Note>}
      <ul className="tool-list">
        {data?.map((drive) => (
          <li key={drive.letter} className="tool-row">
            <span className="tool-text">
              <span className="tool-name">
                {drive.label} ({drive.letter}:)
              </span>
              <span className="panel-faint">
                {drive.total > 0
                  ? `${formatBytes(drive.free)} free of ${formatBytes(drive.total)}`
                  : "No media"}
              </span>
              {drive.total > 0 && (
                <span className="meter-track">
                  <span
                    className="meter-fill"
                    data-high={usedPercent(drive.total, drive.free) >= 90}
                    style={{ width: `${usedPercent(drive.total, drive.free)}%` }}
                  />
                </span>
              )}
            </span>
            <button
              className="tool-btn"
              disabled={busy !== null}
              onClick={() => eject(drive.letter, drive.label)}
            >
              {busy === drive.letter ? "Ejecting…" : "Eject"}
            </button>
          </li>
        ))}
      </ul>
      {message && <Note>{message}</Note>}
    </div>
  );
}

// ---------- Uptime and updates ----------

const readBoot = () =>
  ipc.uptimeGet().then((s) => ({ boot: Date.now() - s.seconds * 1000, restartPending: s.restartPending }));

/** Boot time (ms since epoch), so the clock counts on by itself; re-read every half hour for a
 * newly pending restart. */
function useBootTime() {
  return useLoaded(readBoot, 30 * 60_000).data;
}

export function UptimeTile() {
  const state = useBootTime();
  const now = useNow(60_000);
  return (
    <span className="tile-row">
      <Clock3 size={14} aria-hidden="true" />
      <span className="tile-mini">{state ? shortUptime((now - state.boot) / 1000) : "—"}</span>
      {state?.restartPending && <span className="tool-dot" aria-label="Restart pending" />}
    </span>
  );
}

export function UptimePanel() {
  const state = useBootTime();
  const now = useNow(60_000);
  return (
    <div className="panel-body">
      <h2 className="panel-title">Uptime</h2>
      <div className="panel-big">{state ? formatUptime((now - state.boot) / 1000) : "—"}</div>
      <span className="panel-faint">
        {state
          ? `Since ${new Date(state.boot).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}`
          : ""}
      </span>
      {state?.restartPending ? (
        <button className="panel-button tool-start" onClick={() => ipc.openWindowsSettings("update")}>
          Restart to finish updates
        </button>
      ) : (
        state && <span className="panel-faint">No updates are waiting for a restart.</span>
      )}
    </div>
  );
}
