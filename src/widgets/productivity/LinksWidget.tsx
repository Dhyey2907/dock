import { Folder, Globe, Link2, Plus, X } from "lucide-react";
import { useState } from "react";

import { ipc } from "../../core/ipc";
import type { QuickLink } from "../../core/generated/QuickLink";
import { hostOf, newId } from "./logic";
import { report, useProductivity } from "./store";

const isWeb = (link: QuickLink) => /^https?:\/\//.test(link.target);

export function LinksTile() {
  return <Link2 size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function LinksPanel() {
  const links = useProductivity((s) => s.links);
  const [adding, setAdding] = useState(false);
  const [name, setName] = useState("");
  const [target, setTarget] = useState("");
  const [problem, setProblem] = useState<string | null>(null);

  const add = async () => {
    const before = links.links.length;
    const id = newId();
    try {
      const saved = await ipc.linksSet({ links: [...links.links, { id, name, target }] });
      if (saved.links.length === before) {
        setProblem("Type a web address (example.com) or a folder (C:\\Users\\…).");
        return;
      }
      setName("");
      setTarget("");
      setAdding(false);
      setProblem(null);
    } catch (error) {
      report("save the link")(error);
    }
  };

  return (
    <div className="panel-body">
      <div className="prod-head">
        <h2 className="panel-title">Quick links</h2>
        <button
          className="icon-btn"
          aria-label="Add a link"
          title="Add a link"
          onClick={() => setAdding((a) => !a)}
        >
          <Plus size={16} />
        </button>
      </div>
      {adding && (
        <form
          className="prod-form"
          onSubmit={(e) => {
            e.preventDefault();
            void add();
          }}
        >
          <input
            className="prod-input"
            autoFocus
            value={target}
            placeholder="Web address or folder"
            onChange={(e) => setTarget(e.target.value)}
          />
          <input
            className="prod-input"
            value={name}
            placeholder="Name (optional)"
            onChange={(e) => setName(e.target.value)}
          />
          {problem && <p className="prod-problem">{problem}</p>}
          <button className="panel-button" type="submit">
            Add
          </button>
        </form>
      )}
      <ul className="link-grid">
        {links.links.map((link) => (
          <li key={link.id}>
            <button
              className="link-tile"
              title={link.target}
              onClick={() => ipc.linkOpen(link.target).catch(report("open the link"))}
            >
              {isWeb(link) ? <Globe size={18} /> : <Folder size={18} />}
              <span className="link-name">{link.name === link.target ? hostOf(link.target) : link.name}</span>
            </button>
            <button
              className="icon-btn prod-hover link-remove"
              aria-label={`Remove ${link.name}`}
              onClick={() =>
                ipc
                  .linksSet({ links: links.links.filter((l) => l.id !== link.id) })
                  .catch(report("remove the link"))
              }
            >
              <X size={12} />
            </button>
          </li>
        ))}
      </ul>
      {links.links.length === 0 && !adding && (
        <p className="panel-faint">Add websites and folders you open often.</p>
      )}
    </div>
  );
}
