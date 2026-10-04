// An inline confirmation for something that cannot be undone (A11Y-1).
// The safe choice has the focus and Escape picks it, so a keyboard user is
// never left at the top of the page; the question is read with it.
import { useId } from "react";

export function Confirm({
  question,
  confirm,
  keep,
  danger = false,
  onConfirm,
  onKeep,
}: {
  question: string;
  confirm: string;
  keep: string;
  danger?: boolean;
  onConfirm: () => void;
  onKeep: () => void;
}) {
  const id = useId();
  return (
    <div
      className="confirm"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onKeep();
        }
      }}
    >
      <span id={id}>{question}</span>
      <button type="button" className={danger ? "danger" : "primary"} onClick={onConfirm}>
        {confirm}
      </button>
      <button type="button" autoFocus aria-describedby={id} onClick={onKeep}>
        {keep}
      </button>
    </div>
  );
}
