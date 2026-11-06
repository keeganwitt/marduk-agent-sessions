import type { PreviewItem } from "../lib/types";

interface Props {
  items: PreviewItem[];
  busy: boolean;
  onConfirm: (uids: string[]) => void;
  onCancel: () => void;
}

export function DeleteDialog({ items, busy, onConfirm, onCancel }: Props) {
  const ready = items.filter((item) => item.refusedReason === null);
  const refused = items.filter((item) => item.refusedReason !== null);
  const fileCount = ready.reduce((total, item) => total + item.files.length, 0);

  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="delete-title">
        <h2 id="delete-title">
          Move {ready.length} {ready.length === 1 ? "session" : "sessions"} to the Trash?
        </h2>
        <p>
          {fileCount} {fileCount === 1 ? "item" : "items"} will be moved to the system Trash. You can restore them from
          there.
        </p>
        {ready.length > 0 && (
          <details>
            <summary>Show files</summary>
            <ul className="files">
              {ready.flatMap((item) => item.files.map((file) => <li key={`${item.uid}:${file}`}>{file}</li>))}
            </ul>
          </details>
        )}
        {refused.length > 0 && (
          <div className="refused">
            <h3>Skipped ({refused.length})</h3>
            <ul>
              {refused.map((item) => (
                <li key={item.uid}>
                  <strong>{item.title ?? item.uid}</strong>: {item.refusedReason}
                </li>
              ))}
            </ul>
          </div>
        )}
        <div className="actions">
          <button type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="danger"
            onClick={() => onConfirm(ready.map((item) => item.uid))}
            disabled={busy || ready.length === 0}
          >
            {busy ? "Moving…" : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}
