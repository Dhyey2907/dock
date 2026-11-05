/** The parts of a key press that make a shortcut. */
export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

/** The key's name as Dock's shortcut text uses it, or null for keys a shortcut can't use. */
function keyName(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  if (code === "Space") return "Space";
  return null;
}

/**
 * Turn a key press into shortcut text ("Ctrl+Alt+D"). Null until a usable key is pressed with at
 * least one modifier — a bare key would fire while typing anywhere.
 */
export function shortcutFromKey(press: KeyPress): string | null {
  const key = keyName(press.code);
  if (!key) return null;
  const mods = [
    press.ctrlKey && "Ctrl",
    press.altKey && "Alt",
    press.shiftKey && "Shift",
    press.metaKey && "Super",
  ].filter(Boolean);
  if (mods.length === 0) return null;
  return [...mods, key].join("+");
}

/** Shortcut text as shown to people ("Super" is the Windows key). */
export function shortcutLabel(text: string): string {
  return text ? text.replace("Super", "Win").split("+").join(" + ") : "None";
}
