import { useEffect, useState } from "react";

import { ipc } from "./ipc";

/** Icons already fetched, by icon source; `null` when the source has none. */
const icons = new Map<string, string | null>();

/**
 * The icon for `source` (an `app_icon` source such as "aumid:…" or "path:…") as a data URL, or
 * null while loading or when there is none. Pass `wanted` false to skip fetching.
 */
export function useIcon(source: string | undefined, wanted = true): string | null {
  const [, setLoaded] = useState(0);
  const key = wanted && source ? source : null;
  useEffect(() => {
    if (!key || icons.has(key)) return;
    let live = true;
    ipc
      .appIcon(key)
      .then((icon) => {
        icons.set(key, icon);
        if (live) setLoaded((n) => n + 1);
      })
      .catch(() => icons.set(key, null));
    return () => {
      live = false;
    };
  }, [key]);
  return key ? (icons.get(key) ?? null) : null;
}
