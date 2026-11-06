import { useState } from "react";

import { ipc } from "../../core/ipc";
import type { CaptureKind } from "../../core/generated/CaptureKind";
import { report } from "./store";

/** Quick capture, opened by its shortcut from anywhere: type, Enter, gone. */
export function CapturePanel() {
  const [kind, setKind] = useState<CaptureKind>("todo");
  const [text, setText] = useState("");
  const save = () => {
    ipc.captureSave(kind, text).catch(report("save"));
    setText("");
  };
  return (
    <form
      className="panel-body capture-panel"
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <div className="prod-segments" role="radiogroup" aria-label="Save as">
        {(["todo", "note"] as const).map((k) => (
          <button
            key={k}
            type="button"
            role="radio"
            aria-checked={kind === k}
            data-active={kind === k}
            onClick={() => setKind(k)}
          >
            {k === "todo" ? "To-do" : "Note"}
          </button>
        ))}
      </div>
      <textarea
        className="note-text"
        autoFocus
        value={text}
        placeholder={kind === "todo" ? "Something to do…" : "A quick note…"}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          // Enter saves; Shift+Enter starts a new line in a note. Tab switches kind.
          if (e.key === "Enter" && !(e.shiftKey && kind === "note")) {
            e.preventDefault();
            save();
          } else if (e.key === "Tab") {
            e.preventDefault();
            setKind((k) => (k === "todo" ? "note" : "todo"));
          }
        }}
      />
      <p className="panel-faint prod-hint">Enter saves · Tab switches · Esc closes</p>
    </form>
  );
}
