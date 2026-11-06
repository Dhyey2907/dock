import { Clipboard, Copy, Pin, ScanText, X } from "lucide-react";
import { useState } from "react";

import { ipc } from "../../core/ipc";
import { report, useProductivity } from "./store";

// ---------- Pin image ----------

export function PinImageTile() {
  const pins = useProductivity((s) => s.pins);
  return (
    <span className="tile-row">
      <Pin size={18} strokeWidth={1.8} aria-hidden="true" />
      {pins > 0 && <span className="tile-strong tile-tabular">{pins}</span>}
    </span>
  );
}

export function PinImagePanel() {
  const pins = useProductivity((s) => s.pins);
  const [empty, setEmpty] = useState(false);
  return (
    <div className="panel-body">
      <h2 className="panel-title">Pin a picture</h2>
      <p className="panel-faint">It floats above everything until you close it.</p>
      <div className="prod-actions">
        <button className="panel-button" onClick={() => ipc.snipStart("pin").catch(report("start choosing"))}>
          <Pin size={15} /> Screen area
        </button>
        <button
          className="panel-button prod-secondary"
          onClick={() =>
            ipc
              .pinClipboard()
              .then((pinned) => setEmpty(!pinned))
              .catch(report("pin the clipboard"))
          }
        >
          <Clipboard size={15} /> Clipboard
        </button>
      </div>
      {empty && <p className="prod-problem">There's no picture on the clipboard.</p>}
      {pins > 0 && (
        <button className="panel-link" onClick={() => ipc.pinsCloseAll().catch(report("close the pins"))}>
          <X size={12} /> Close {pins} pinned {pins === 1 ? "picture" : "pictures"}
        </button>
      )}
    </div>
  );
}

// ---------- Copy text from screen ----------

export function ScreenTextTile() {
  return <ScanText size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function ScreenTextPanel() {
  const last = useProductivity((s) => s.screenText);
  const [copied, setCopied] = useState(false);
  return (
    <div className="panel-body">
      <button className="panel-button" onClick={() => ipc.snipStart("text").catch(report("start choosing"))}>
        <ScanText size={15} /> Choose an area
      </button>
      <p className="panel-faint">The text is read on this PC and copied for you.</p>
      {last.problem && <p className="prod-problem">{last.problem}</p>}
      {last.text && (
        <>
          <div className="prod-head">
            <span className="panel-faint">Last copied</span>
            <button
              className="icon-btn"
              aria-label="Copy again"
              title="Copy again"
              onClick={() =>
                ipc
                  .textCopy(last.text)
                  .then(() => setCopied(true))
                  .catch(report("copy"))
              }
            >
              <Copy size={14} />
            </button>
            {copied && <span className="panel-faint">Copied</span>}
          </div>
          <pre className="screen-text">{last.text}</pre>
        </>
      )}
    </div>
  );
}
