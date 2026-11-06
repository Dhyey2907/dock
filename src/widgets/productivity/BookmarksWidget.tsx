import { Bookmark as BookmarkIcon } from "lucide-react";
import { useEffect, useState } from "react";

import { ipc } from "../../core/ipc";
import type { Bookmark } from "../../core/generated/Bookmark";
import { hostOf, matchAll } from "./logic";
import { report } from "./store";

export function BookmarksTile() {
  return <BookmarkIcon size={18} strokeWidth={1.8} aria-hidden="true" />;
}

export function BookmarksPanel() {
  const [marks, setMarks] = useState<Bookmark[] | null>(null);
  const [query, setQuery] = useState("");
  useEffect(() => {
    // Read fresh each time the popup opens: the browsers change the files themselves.
    ipc.bookmarksGet().then(setMarks).catch(report("read the bookmarks"));
  }, []);
  const shown = matchAll(marks ?? [], query, (b) => [b.name, b.url, b.folder]).slice(0, 100);

  return (
    <div className="panel-body">
      <input
        className="prod-input"
        autoFocus
        value={query}
        placeholder="Search Chrome and Edge bookmarks"
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && shown[0]) ipc.linkOpen(shown[0].url).catch(report("open the bookmark"));
        }}
      />
      <ul className="prod-list">
        {shown.map((b, i) => (
          <li key={`${b.browser}-${i}-${b.url}`}>
            <button
              className="prod-row prod-row-two"
              title={b.url}
              onClick={() => ipc.linkOpen(b.url).catch(report("open the bookmark"))}
            >
              <span className="prod-row-main">{b.name}</span>
              <span className="panel-faint prod-row-sub">
                {hostOf(b.url)} · {b.browser === "edge" ? "Edge" : "Chrome"}
                {b.folder ? ` · ${b.folder}` : ""}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {marks?.length === 0 && <p className="panel-faint">No Chrome or Edge bookmarks found on this PC.</p>}
      {marks && marks.length > 0 && shown.length === 0 && <p className="panel-faint">No bookmark matches.</p>}
    </div>
  );
}
