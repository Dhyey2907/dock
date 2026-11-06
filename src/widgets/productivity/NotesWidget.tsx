import { Plus, StickyNote, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { ipc } from "../../core/ipc";
import { MEETING_PREFIX, noteTitle, sortNotes } from "./logic";
import { report, useProductivity } from "./store";

/** How long typing pauses before a note is saved. */
const SAVE_AFTER_MS = 400;

/**
 * A text box that saves itself a moment after typing stops, and when it closes. Starts as
 * `initial`; `noteId` null makes a new note on the first save.
 */
export function AutosaveText({
  noteId,
  initial,
  placeholder,
  onSaved,
}: {
  noteId: string | null;
  initial: string;
  placeholder: string;
  onSaved?: (id: string) => void;
}) {
  const [text, setText] = useState(initial);
  const pending = useRef<string | null>(null);
  const timer = useRef<number | undefined>(undefined);
  // Saves run one after another, so a new note is made once and later saves go to it.
  const chain = useRef<Promise<void>>(Promise.resolve());
  const id = useRef(noteId);
  const saved = useRef(onSaved);
  useEffect(() => {
    saved.current = onSaved;
  });

  // Uses only refs, so it never goes stale.
  const flush = useCallback(() => {
    window.clearTimeout(timer.current);
    const next = pending.current;
    pending.current = null;
    if (next === null) return;
    chain.current = chain.current
      .then(async () => {
        const savedId = await ipc.noteSave(id.current, next);
        if (id.current === null) {
          id.current = savedId;
          saved.current?.(savedId);
        }
      })
      .catch(report("save the note"));
  }, []);

  // Save what's typed before closing.
  useEffect(() => flush, [flush]);

  return (
    <textarea
      className="note-text"
      value={text}
      placeholder={placeholder}
      autoFocus
      spellCheck
      onChange={(e) => {
        setText(e.target.value);
        pending.current = e.target.value;
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(flush, SAVE_AFTER_MS);
      }}
      onBlur={flush}
    />
  );
}

export function NotesTile() {
  const notes = useProductivity((s) => s.notes);
  const latest = sortNotes(notes.filter((n) => !n.id.startsWith(MEETING_PREFIX)))[0];
  return (
    <span className="tile-row">
      <StickyNote size={18} strokeWidth={1.8} aria-hidden="true" />
      {latest && <span className="tile-mini prod-tile-text">{noteTitle(latest)}</span>}
    </span>
  );
}

export function NotesPanel() {
  const notes = useProductivity((s) => s.notes);
  const sorted = sortNotes(notes);
  // `null` is a new, unsaved note.
  const [selected, setSelected] = useState<string | null | undefined>(undefined);
  const current = selected === undefined ? sorted[0] : sorted.find((n) => n.id === selected);
  const editingId = selected === undefined ? (current?.id ?? null) : selected;
  // The editor is remade only when another note is picked, not when a new one gets its id.
  const [editorKey, setEditorKey] = useState(0);
  const pick = (id: string | null | undefined) => {
    setSelected(id);
    setEditorKey((k) => k + 1);
  };

  return (
    <div className="panel-body notes-panel">
      <div className="prod-head">
        <h2 className="panel-title">Notes</h2>
        <button className="icon-btn" aria-label="New note" title="New note" onClick={() => pick(null)}>
          <Plus size={16} />
        </button>
        <button
          className="icon-btn"
          aria-label="Delete note"
          title="Delete note"
          disabled={!current || editingId === null}
          onClick={() => {
            if (!current) return;
            ipc.noteDelete(current.id).catch(report("delete the note"));
            pick(undefined);
          }}
        >
          <Trash2 size={16} />
        </button>
      </div>
      <AutosaveText
        key={editorKey}
        noteId={editingId}
        initial={editingId === null ? "" : (current?.text ?? "")}
        placeholder="Type a note. It saves as you go."
        onSaved={(id) => {
          if (editingId === null) setSelected(id);
        }}
      />
      {sorted.length > 1 && (
        <ul className="note-list">
          {sorted.map((n) => (
            <li key={n.id}>
              <button
                className="prod-row"
                data-active={n.id === editingId}
                onClick={() => n.id !== editingId && pick(n.id)}
                title={noteTitle(n)}
              >
                {noteTitle(n)}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
