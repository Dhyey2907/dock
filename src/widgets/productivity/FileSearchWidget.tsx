import { FolderOpen, Search } from "lucide-react";
import { useEffect, useState } from "react";

import { ipc } from "../../core/ipc";
import { useIcon } from "../../core/useIcon";
import type { FileHit } from "../../core/generated/FileHit";
import { report } from "./store";

/** Wait this long after typing before asking the index. */
const SEARCH_AFTER_MS = 150;

function FileIcon({ path }: { path: string }) {
  const icon = useIcon(`path:${path}`);
  return icon ? <img src={icon} width={20} height={20} alt="" /> : <span className="folder-icon-blank" />;
}

export function FileSearchTile() {
  return <Search size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function FileSearchPanel() {
  const [query, setQuery] = useState("");
  const [found, setHits] = useState<FileHit[]>([]);
  const [lastQuery, setSearched] = useState("");
  const [problem, setProblem] = useState(false);

  useEffect(() => {
    const q = query.trim();
    if (!q) return;
    let live = true;
    const timer = window.setTimeout(() => {
      ipc
        .fileSearch(q)
        .then((found) => {
          if (!live) return;
          setHits(found);
          setSearched(q);
          setProblem(false);
        })
        .catch((error) => {
          if (live) setProblem(true);
          report("search files")(error);
        });
    }, SEARCH_AFTER_MS);
    return () => {
      live = false;
      window.clearTimeout(timer);
    };
  }, [query]);

  // Cleared the box: show nothing, whatever the last search found.
  const hits = query.trim() ? found : [];
  const searched = query.trim() ? lastQuery : "";

  const open = (hit: FileHit, reveal: boolean) =>
    ipc.fileOpen(hit.path, reveal).catch(report("open the file"));

  return (
    <div className="panel-body">
      <input
        className="prod-input"
        autoFocus
        value={query}
        placeholder="Search files and folders"
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && hits[0]) open(hits[0], e.ctrlKey);
        }}
      />
      <ul className="prod-list">
        {hits.map((hit) => (
          <li key={hit.path} className="file-row">
            <button className="prod-row prod-row-icon" title={hit.path} onClick={() => open(hit, false)}>
              <FileIcon path={hit.path} />
              <span className="prod-row-two">
                <span className="prod-row-main">{hit.name}</span>
                <span className="panel-faint prod-row-sub">{hit.path}</span>
              </span>
            </button>
            <button
              className="icon-btn prod-hover"
              aria-label="Show in folder"
              title="Show in folder"
              onClick={() => open(hit, true)}
            >
              <FolderOpen size={14} />
            </button>
          </li>
        ))}
      </ul>
      {problem && (
        <p className="prod-problem">Windows Search isn't answering. Is the Windows Search service on?</p>
      )}
      {!problem && searched && hits.length === 0 && (
        <p className="panel-faint">Nothing found for “{searched}”.</p>
      )}
      {!query && (
        <p className="panel-faint prod-hint">Uses the Windows Search index · Ctrl+Enter shows in folder</p>
      )}
    </div>
  );
}
