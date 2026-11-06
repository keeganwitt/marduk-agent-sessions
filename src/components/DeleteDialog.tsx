import type { PreviewItem } from "../lib/types";

interface Props {
  items: PreviewItem[];
  busy: boolean;
  onConfirm: (uids: string[]) => void;
  onCancel: () => void;
  onReveal: (path: string) => void;
}

function plural(count: number, word: string): string {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}

export function DeleteDialog({ items, busy, onConfirm, onCancel, onReveal }: Props) {
  const ready = items.filter((item) => item.refusedReason === null);
  const refused = items.filter((item) => item.refusedReason !== null);
  const toTrash = ready.filter((item) => !item.permanent);
  const permanent = ready.filter((item) => item.permanent);
  const title =
    permanent.length === 0
      ? `Move ${plural(ready.length, "session")} to the Trash?`
      : `Delete ${plural(ready.length, "session")}?`;

  return (
    <div className="overlay">
      <div className="dialog" role="dialog" aria-modal="true" aria-labelledby="delete-title">
        <h2 id="delete-title">{title}</h2>
        {toTrash.length > 0 && (
          <p>{plural(toTrash.length, "session")} will be moved to the system Trash. You can restore them from there.</p>
        )}
        {permanent.length > 0 && (
          <p className="permanent-warning" role="alert">
            {plural(permanent.length, "session")} will be <strong>permanently deleted</strong> by the agent's own
            command and can't be restored.
          </p>
        )}
        {ready.length > 0 && (
          <ul className="preview-list">
            {ready.map((item) => (
              <li key={item.uid}>
                <div className="preview-title">
                  <strong>{item.title ?? item.uid}</strong>
                  {item.agentName && <span className="muted"> · {item.agentName}</span>}
                  {item.permanent && <span className="badge danger">Permanent</span>}
                </div>
                {item.command && <code className="command">{item.command}</code>}
                <ul className="files">
                  {item.files.map((file) => (
                    <li key={file}>
                      <span className="path">{file}</span>
                      <button type="button" className="link" onClick={() => onReveal(file)}>
                        Show in folder
                      </button>
                    </li>
                  ))}
                </ul>
              </li>
            ))}
          </ul>
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
            {busy ? "Working…" : permanent.length > 0 ? "Delete" : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}
