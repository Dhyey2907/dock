import { useEffect, useState } from "react";
import { create } from "zustand";

import type { InputLanguage } from "./generated/InputLanguage";
import type { SystemClock } from "./generated/SystemClock";
import type { SystemLook } from "./generated/SystemLook";
import { ipc } from "./ipc";
import { useSettings } from "../settings/store";

/** Windows' accent colour and taskbar mode, read when asked and kept here. */
export const useSystemLook = create<{ look: SystemLook | null; refresh: () => void }>((set) => ({
  look: null,
  refresh: () => {
    ipc
      .systemLook()
      .then((look) => set({ look }))
      .catch((error) => console.error("failed to read Windows' look", error));
  },
}));

/** Keep the Windows look fresh: now, when a window regains focus, and every half minute (a
 *  mode or accent change in Windows Settings shows up within that). */
export function useWatchSystemLook() {
  useEffect(() => {
    const { refresh } = useSystemLook.getState();
    refresh();
    const timer = window.setInterval(refresh, 30_000);
    window.addEventListener("focus", refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, []);
}

/** The dock is in the Windows 11 theme. */
export function useWindowsLook(): boolean {
  return useSettings((s) => s.settings?.theme === "windows");
}

/** The time and date written as the taskbar writes them, updated on the minute. */
export function useSystemClock(): SystemClock | null {
  const [clock, setClock] = useState<SystemClock | null>(null);
  useEffect(() => {
    let timer: number | undefined;
    const tick = () => {
      ipc
        .systemClock()
        .then(setClock)
        .catch((error) => console.error("failed to read the clock", error));
      // Next update just after the minute turns.
      timer = window.setTimeout(tick, 60_000 - (Date.now() % 60_000) + 200);
    };
    tick();
    return () => window.clearTimeout(timer);
  }, []);
  return clock;
}

/** The keyboard language, as the taskbar shows it; checked every couple of seconds (it follows
 *  the window in front). */
export function useInputLanguage(): InputLanguage | null {
  const [language, setLanguage] = useState<InputLanguage | null>(null);
  useEffect(() => {
    const read = () =>
      ipc
        .inputLanguage()
        .then((next) =>
          setLanguage((current) =>
            current && current.language === next.language && current.region === next.region ? current : next,
          ),
        )
        .catch((error) => console.error("failed to read the input language", error));
    read();
    const timer = window.setInterval(read, 2_000);
    return () => window.clearInterval(timer);
  }, []);
  return language;
}
