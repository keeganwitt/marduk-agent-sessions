import { useState } from "react";

import { tildePath } from "../lib/format";
import type { MemoryStore, PreviewItem } from "../lib/types";
import { FileList } from "./FileList";

interface Props {
  items: PreviewItem[];
  /** Memory no session would use after this delete, offered for the Trash too. */
  memory?: MemoryStore[];
  busy: boolean;
  onConfirm: (uids: string[], memoryFolders: string[]) => void;
  onCancel: () => void;
  onReveal: (path: string) => void;
  homeDir: string;
}

function plural(count: number, word: string): string {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}

export function DeleteDialog({
  items,
  memory = [],
  busy,
  onConfirm,
  onCancel,
  onReveal,
  homeDir,
}: Props) {
  const [memoryChosen, setMemoryChosen] = useState<ReadonlySet<string>>(
    new Set(),
  );
  const toggleMemory = (folder: string, chosen: boolean) =>
    setMemoryChosen((current) => {
      const next = new Set(current);
      if (chosen) next.add(folder);
      else next.delete(folder);
      return next;
    });
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
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-title"
      >
        <h2 id="delete-title">{title}</h2>
        {toTrash.length > 0 && (
          <p>
            {plural(toTrash.length, "session")} will be moved to the system
            Trash. You can restore them from there.
          </p>
        )}
        {permanent.length > 0 && (
          <p className="permanent-warning" role="alert">
            {plural(permanent.length, "session")} will be{" "}
            <strong>permanently deleted</strong> by the agent's own command and
            can't be restored.
          </p>
        )}
        {ready.length > 0 && (
          <ul className="preview-list">
            {ready.map((item) => (
              <li key={item.uid}>
                <div className="preview-title">
                  <strong>{item.title ?? item.uid}</strong>
                  {item.agentName && (
                    <span className="muted"> · {item.agentName}</span>
                  )}
                  {item.permanent && (
                    <span className="badge danger">Permanent</span>
                  )}
                </div>
                {item.command && (
                  <code className="command">{item.command}</code>
                )}
                {item.warning && <p className="item-warning">{item.warning}</p>}
                <FileList
                  paths={item.files}
                  homeDir={homeDir}
                  onReveal={onReveal}
                />
              </li>
            ))}
          </ul>
        )}
        {memory.length > 0 && ready.length > 0 && (
          <div className="memory-offers">
            <h3>Project memory</h3>
            {memory.map((store) => (
              <div key={store.folder} className="memory-offer">
                <label>
                  <input
                    type="checkbox"
                    checked={memoryChosen.has(store.folder)}
                    onChange={(event) =>
                      toggleMemory(store.folder, event.target.checked)
                    }
                  />
                  Also move {store.agentName}'s memory for{" "}
                  <code>
                    {store.projectDir
                      ? tildePath(store.projectDir, homeDir)
                      : tildePath(store.folder, homeDir)}
                  </code>{" "}
                  to the Trash ({plural(store.files.length, "file")})
                </label>
                <p className="muted">
                  {store.projectExists === false
                    ? "Its project folder no longer exists, and this deletes the project's last session."
                    : `This deletes the project's last session, but ${store.agentName} would still use this memory the next time you work there.`}
                </p>
              </div>
            ))}
          </div>
        )}
        {refused.length > 0 && (
          <div className="refused">
            <h3>Skipped ({refused.length})</h3>
            <ul>
              {refused.map((item) => (
                <li key={item.uid}>
                  <strong>{item.title ?? item.uid}</strong>:{" "}
                  {item.refusedReason}
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
            onClick={() =>
              onConfirm(
                ready.map((item) => item.uid),
                memory
                  .map((store) => store.folder)
                  .filter((folder) => memoryChosen.has(folder)),
              )
            }
            disabled={busy || ready.length === 0}
          >
            {busy
              ? "Working…"
              : permanent.length > 0
                ? "Delete"
                : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}
