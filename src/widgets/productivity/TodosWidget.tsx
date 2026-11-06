import { Check, ListChecks, X } from "lucide-react";
import { useState } from "react";

import { ipc } from "../../core/ipc";
import type { TodoChange } from "../../core/generated/TodoChange";
import { sortTodos, todosLeft } from "./logic";
import { report, useProductivity } from "./store";

const change = (c: TodoChange) => ipc.todoChange(c).catch(report("update the to-dos"));

export function TodosTile() {
  const left = todosLeft(useProductivity((s) => s.todos));
  return (
    <span className="tile-row">
      <ListChecks size={18} strokeWidth={1.8} aria-hidden="true" />
      {left > 0 && <span className="tile-strong tile-tabular">{left}</span>}
    </span>
  );
}

export function TodosPanel() {
  const todos = sortTodos(useProductivity((s) => s.todos));
  const [draft, setDraft] = useState("");
  const left = todosLeft(todos);
  const done = todos.length - left;

  return (
    <div className="panel-body">
      <div className="prod-head">
        <h2 className="panel-title">To-do</h2>
        <span className="panel-faint">{left === 0 ? "All done" : `${left} left`}</span>
      </div>
      <form
        className="prod-add"
        onSubmit={(e) => {
          e.preventDefault();
          if (draft.trim()) change({ kind: "add", text: draft });
          setDraft("");
        }}
      >
        <input
          className="prod-input"
          value={draft}
          autoFocus
          placeholder="Add a to-do and press Enter"
          onChange={(e) => setDraft(e.target.value)}
        />
      </form>
      <ul className="prod-list">
        {todos.map((t) => (
          <li key={t.id} className="todo-row" data-done={t.done}>
            <button
              className="todo-check"
              role="checkbox"
              aria-checked={t.done}
              aria-label={t.text}
              onClick={() => change({ kind: "setDone", id: t.id, done: !t.done })}
            >
              {t.done && <Check size={12} strokeWidth={3} />}
            </button>
            <input
              className="todo-text"
              defaultValue={t.text}
              aria-label="To-do"
              onBlur={(e) => {
                if (e.target.value !== t.text) change({ kind: "rename", id: t.id, text: e.target.value });
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
              }}
            />
            <button
              className="icon-btn prod-hover"
              aria-label="Delete"
              onClick={() => change({ kind: "delete", id: t.id })}
            >
              <X size={14} />
            </button>
          </li>
        ))}
      </ul>
      {todos.length === 0 && <p className="panel-faint">Nothing to do yet.</p>}
      {done > 0 && (
        <button className="panel-link" onClick={() => change({ kind: "clearDone" })}>
          Clear {done} done
        </button>
      )}
    </div>
  );
}
