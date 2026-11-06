import { useState, type ReactNode } from "react";

/** A button that asks before it acts: the first click shows "Sure?" with Yes and Cancel. */
export function ConfirmButton({
  label,
  question,
  confirm,
  onConfirm,
  disabled,
}: {
  label: ReactNode;
  question: string;
  confirm: string;
  onConfirm: () => void;
  disabled?: boolean;
}) {
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <button className="tool-btn" disabled={disabled} onClick={() => setAsking(true)}>
        {label}
      </button>
    );
  }
  return (
    <span className="tool-confirm" role="group" aria-label={question}>
      <span className="tool-confirm-question">{question}</span>
      <button
        className="tool-btn tool-btn-danger"
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
      >
        {confirm}
      </button>
      <button className="tool-btn" onClick={() => setAsking(false)}>
        Cancel
      </button>
    </span>
  );
}

/** A one-line note under a list, for errors and empty states. */
export function Note({ children }: { children: ReactNode }) {
  return <p className="tool-note panel-faint">{children}</p>;
}
