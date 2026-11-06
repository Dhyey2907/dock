import { SquareKanban, X } from "lucide-react";
import { useRef, useState, type PointerEvent } from "react";

import { ipc } from "../../core/ipc";
import type { KanbanBoard } from "../../core/generated/KanbanBoard";
import type { KanbanColumn } from "../../core/generated/KanbanColumn";
import { COLUMNS, moveCard, newId, slotFor } from "./logic";
import { report, useProductivity } from "./store";

/** How far the pointer must move before a press becomes a drag. */
const DRAG_THRESHOLD = 5;

const save = (board: KanbanBoard) => ipc.kanbanSet(board).catch(report("save the board"));

interface Drag {
  id: string;
  startX: number;
  startY: number;
  x: number;
  y: number;
  moved: boolean;
  /** Where it would land. */
  column?: KanbanColumn;
  slot?: number;
}

export function KanbanTile() {
  const cards = useProductivity((s) => s.kanban.cards);
  const doing = cards.filter((c) => c.column === "doing").length;
  return (
    <span className="tile-row">
      <SquareKanban size={18} strokeWidth={1.8} aria-hidden="true" />
      {doing > 0 && <span className="tile-strong tile-tabular">{doing}</span>}
    </span>
  );
}

export function KanbanPanel() {
  const board = useProductivity((s) => s.kanban);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [draft, setDraft] = useState("");
  const [editing, setEditing] = useState<string | null>(null);
  const columns = useRef(new Map<KanbanColumn, HTMLElement>());

  const target = (x: number, y: number, id: string) => {
    for (const [column, el] of columns.current) {
      const r = el.getBoundingClientRect();
      if (x >= r.left && x <= r.right) {
        const mids = [...el.querySelectorAll<HTMLElement>("[data-card]")]
          .filter((c) => c.dataset.card !== id)
          .map((c) => {
            const cr = c.getBoundingClientRect();
            return cr.top + cr.height / 2;
          });
        return { column, slot: slotFor(y, mids) };
      }
    }
    return {};
  };

  const onDown = (id: string) => (e: PointerEvent<HTMLElement>) => {
    if (e.button !== 0 || editing) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ id, startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, moved: false });
  };
  const onMove = (e: PointerEvent<HTMLElement>) => {
    if (!drag) return;
    const moved = drag.moved || Math.hypot(e.clientX - drag.startX, e.clientY - drag.startY) > DRAG_THRESHOLD;
    if (!moved) return;
    setDrag({ ...drag, moved, x: e.clientX, y: e.clientY, ...target(e.clientX, e.clientY, drag.id) });
  };
  const onUp = () => {
    if (drag?.moved && drag.column != null && drag.slot != null) {
      save(moveCard(board, drag.id, drag.column, drag.slot));
    }
    setDrag(null);
  };

  return (
    <div className="panel-body kanban-panel">
      <div className="kanban" data-dragging={Boolean(drag?.moved)}>
        {COLUMNS.map((col) => {
          const cards = board.cards.filter((c) => c.column === col.id);
          return (
            <section
              key={col.id}
              className="kanban-col"
              data-target={drag?.moved && drag.column === col.id}
              ref={(el) => {
                if (el) columns.current.set(col.id, el);
                else columns.current.delete(col.id);
              }}
            >
              <h3 className="kanban-title">
                {col.label} <span className="panel-faint">{cards.length}</span>
              </h3>
              <ul className="kanban-cards">
                {cards.map((card, i) => {
                  const lifted = drag?.moved && drag.id === card.id;
                  const gapBefore =
                    drag?.moved &&
                    drag.column === col.id &&
                    drag.slot === cards.filter((c) => c.id !== drag.id).indexOf(card) &&
                    !lifted;
                  return (
                    <li
                      key={card.id}
                      data-card={card.id}
                      className="kanban-card"
                      data-lifted={lifted}
                      data-gap={gapBefore}
                      style={
                        lifted
                          ? { translate: `${drag.x - drag.startX}px ${drag.y - drag.startY}px` }
                          : undefined
                      }
                      onPointerDown={onDown(card.id)}
                      onPointerMove={onMove}
                      onPointerUp={onUp}
                      onPointerCancel={() => setDrag(null)}
                      onDoubleClick={() => setEditing(card.id)}
                      aria-posinset={i + 1}
                    >
                      {editing === card.id ? (
                        <input
                          className="prod-input"
                          defaultValue={card.text}
                          autoFocus
                          onBlur={(e) => {
                            setEditing(null);
                            const text = e.target.value.trim();
                            save({
                              cards: text
                                ? board.cards.map((c) => (c.id === card.id ? { ...c, text } : c))
                                : board.cards.filter((c) => c.id !== card.id),
                            });
                          }}
                          onKeyDown={(e) => {
                            if (e.key === "Enter") e.currentTarget.blur();
                            e.stopPropagation();
                          }}
                        />
                      ) : (
                        <>
                          <span className="kanban-text">{card.text}</span>
                          <button
                            className="icon-btn prod-hover kanban-delete"
                            aria-label="Delete card"
                            onPointerDown={(e) => e.stopPropagation()}
                            onClick={() => save({ cards: board.cards.filter((c) => c.id !== card.id) })}
                          >
                            <X size={12} />
                          </button>
                        </>
                      )}
                    </li>
                  );
                })}
              </ul>
              {col.id === "todo" && (
                <form
                  onSubmit={(e) => {
                    e.preventDefault();
                    const text = draft.trim();
                    if (text) save({ cards: [...board.cards, { id: newId(), text, column: "todo" }] });
                    setDraft("");
                  }}
                >
                  <input
                    className="prod-input kanban-add"
                    value={draft}
                    placeholder="+ Add a card"
                    onChange={(e) => setDraft(e.target.value)}
                  />
                </form>
              )}
            </section>
          );
        })}
      </div>
      <p className="panel-faint prod-hint">Drag cards between columns · double-click to edit</p>
    </div>
  );
}
