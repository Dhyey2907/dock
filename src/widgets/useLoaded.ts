import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Read something once when a popup opens (and again on `reload`, or every `every` ms while it
 * stays open). Stale answers from an earlier read are dropped. `read` must be stable (a module
 * function or an `ipc` wrapper). A change of `key` (the setting the read depends on, such as a
 * username) reads again at once instead of waiting for the next `every`.
 */
export function useLoaded<T>(read: () => Promise<T>, every?: number, key?: string) {
  const [data, setData] = useState<T | undefined>(undefined);
  const [failed, setFailed] = useState(false);
  const [loading, setLoading] = useState(true);
  const latest = useRef(0);

  const run = useCallback(() => {
    const ticket = ++latest.current;
    read()
      .then((value) => {
        if (ticket !== latest.current) return;
        setData(value);
        setFailed(false);
      })
      .catch((error) => {
        if (ticket !== latest.current) return;
        console.error(error);
        setFailed(true);
      })
      .finally(() => {
        if (ticket === latest.current) setLoading(false);
      });
  }, [read]);

  const reload = useCallback(() => {
    setLoading(true);
    run();
  }, [run]);

  useEffect(() => {
    const counter = latest;
    run();
    const timer = every ? window.setInterval(run, every) : undefined;
    return () => {
      // Answers still on their way belong to a closed popup.
      counter.current++;
      window.clearInterval(timer);
    };
  }, [run, every, key]);

  return { data, setData, failed, loading, reload };
}
