import { useState, type ReactNode } from "react";
import { Check } from "lucide-react";

import { ipc } from "../core/ipc";
import type { Theme } from "../core/generated/Theme";
import { shortcutFromKey, shortcutLabel } from "./shortcut";

export function Choice<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: [T, string][];
  onChange: (value: T) => void;
  label?: string;
}) {
  return (
    <div className="choice" role="radiogroup" aria-label={label}>
      {options.map(([option, text]) => (
        <button
          key={option}
          role="radio"
          aria-checked={value === option}
          className={value === option ? "choice-on" : undefined}
          onClick={() => onChange(option)}
        >
          {text}
        </button>
      ))}
    </div>
  );
}

/**
 * A slider that shows its value while dragging and saves once you let go, so dragging doesn't
 * rewrite the settings file on every step.
 */
export function Slider({
  value,
  min,
  max,
  step = 1,
  label,
  format = (v) => `${v}%`,
  onCommit,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  label: string;
  format?: (value: number) => string;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState<number | null>(null);
  const shown = draft ?? value;
  const commit = () => {
    if (draft !== null && draft !== value) onCommit(draft);
    setDraft(null);
  };
  return (
    <span className="slider">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={shown}
        aria-label={label}
        onChange={(e) => setDraft(Number(e.target.value))}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      <span className="slider-value">{format(shown)}</span>
    </span>
  );
}

export function Row({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div className="settings-row">
      <span>
        {label}
        {hint && <span className="settings-hint">{hint}</span>}
      </span>
      {children}
    </div>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      className="toggle"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-knob" />
    </button>
  );
}

const STYLES: [Theme, string][] = [
  ["glass", "Glass"],
  ["iosGlass", "iOS Glass"],
  ["midnight", "Midnight"],
  ["white", "White"],
  ["black", "Black"],
  ["clear", "Clear"],
];

/** The six dock styles as small previews. */
export function StyleCards({ value, onChange }: { value: Theme; onChange: (theme: Theme) => void }) {
  return (
    <div className="style-cards" role="radiogroup" aria-label="Dock style">
      {STYLES.map(([theme, name]) => (
        <button
          key={theme}
          role="radio"
          aria-checked={value === theme}
          className="style-card"
          onClick={() => onChange(theme)}
        >
          <span className="style-scene">
            <span className="dock style-preview" data-theme={theme}>
              <span className="style-pill">
                <span className="style-dot" />
                <span className="style-dot" />
                <span className="style-dot" />
              </span>
            </span>
            {value === theme && (
              <span className="style-check">
                <Check size={12} strokeWidth={3} />
              </span>
            )}
          </span>
          <span className="style-name">{name}</span>
        </button>
      ))}
    </div>
  );
}

/** Press a key combination to make it the shortcut; it's checked before it's saved. */
export function ShortcutField({ value }: { value: string }) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = (text: string) => {
    setRecording(false);
    ipc
      .shortcutSet(text)
      .then(() => setError(null))
      .catch((e: { kind?: string; message?: string }) =>
        setError(
          e?.kind === "invalid_input"
            ? (e.message ?? "").replace(/^invalid input: /, "")
            : "That shortcut couldn't be set.",
        ),
      );
  };
  return (
    <span className="shortcut-field">
      <button
        className="shortcut-box"
        data-recording={recording}
        onClick={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          if (!recording) return;
          e.preventDefault();
          if (e.key === "Escape") return setRecording(false);
          const text = shortcutFromKey(e);
          if (text) save(text);
        }}
      >
        {recording ? "Press keys…" : shortcutLabel(value)}
      </button>
      {value && (
        <button className="settings-link" onClick={() => save("")}>
          Clear
        </button>
      )}
      {error && (
        <span className="settings-error" role="alert">
          {error}
        </span>
      )}
    </span>
  );
}
