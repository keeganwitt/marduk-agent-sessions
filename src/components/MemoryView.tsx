import { useCallback, useEffect, useState } from "react";

import { formatAge, tildePath } from "../lib/format";
import {
  listMemory,
  readMemoryFile,
  revealMemory,
  trashMemory,
} from "../lib/ipc";
import type { MemoryFile, MemoryStore, SessionRow } from "../lib/types";
import { Markdown } from "./Markdown";

interface Props {
  sessions: SessionRow[];
  homeDir: string;
  now: Date;
  /** Changes when a rescan should reload the list. */
  reloadKey: unknown;
  onOpenSession: (session: SessionRow) => void;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function storeLabel(store: MemoryStore, homeDir: string): string {
  if (store.projectDir) return tildePath(store.projectDir, homeDir);
  return `${store.agentName} (all projects)`;
}

function plural(count: number, word: string): string {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}

function FilePanel({
  file,
  homeDir,
  onClose,
}: {
  file: MemoryFile;
  homeDir: string;
  onClose: () => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    readMemoryFile(file.path).then(setText, (caught: unknown) =>
      setError(errorMessage(caught)),
    );
  }, [file.path]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <aside
      className="transcript"
      role="dialog"
      aria-modal="false"
      aria-labelledby="memory-file-title"
    >
      <header className="transcript-header">
        <div className="transcript-heading">
          <h2 id="memory-file-title">{file.title}</h2>
          <p className="muted">{tildePath(file.path, homeDir)}</p>
        </div>
        <button type="button" onClick={onClose} aria-label="Close memory file">
          Close
        </button>
      </header>
      <div className="transcript-body">
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
        {text === null && !error && <p className="muted">Loading…</p>}
        {text !== null && (
          <article className="message memory-text">
            <Markdown text={text} />
          </article>
        )}
      </div>
    </aside>
  );
}

function TrashDialog({
  store,
  homeDir,
  busy,
  onConfirm,
  onCancel,
}: {
  store: MemoryStore;
  homeDir: string;
  busy: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="overlay">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="trash-memory-title"
      >
        <h2 id="trash-memory-title">
          Move {store.agentName} memory for {storeLabel(store, homeDir)} to the
          Trash?
        </h2>
        <p>
          The whole memory folder goes to the Trash, where you can restore it.
          {store.projectExists &&
            ` The project folder still exists, so ${store.agentName} would start its memory over the next time you work there.`}
        </p>
        <ul className="files">
          {store.files.map((file) => (
            <li key={file.path}>{tildePath(file.path, homeDir)}</li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            type="button"
            className="danger"
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? "Working…" : "Move to Trash"}
          </button>
        </div>
      </div>
    </div>
  );
}

export function MemoryView({
  sessions,
  homeDir,
  now,
  reloadKey,
  onOpenSession,
}: Props) {
  const [stores, setStores] = useState<MemoryStore[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [openFile, setOpenFile] = useState<MemoryFile | null>(null);
  const [trashing, setTrashing] = useState<MemoryStore | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    listMemory().then(
      (loaded) => {
        setStores(loaded);
        setError(null);
      },
      (caught: unknown) => setError(errorMessage(caught)),
    );
  }, []);

  useEffect(() => {
    load();
  }, [load, reloadKey]);

  const sessionById = new Map(
    sessions.map((session) => [`${session.agent}:${session.id}`, session]),
  );
  const current =
    stores?.find((store) => store.folder === selected) ?? stores?.[0] ?? null;

  const trash = async (store: MemoryStore) => {
    setBusy(true);
    try {
      await trashMemory(store.folder, false);
      setNotice(
        `Moved ${store.agentName} memory for ${storeLabel(store, homeDir)} to the Trash.`,
      );
      setTrashing(null);
      setSelected(null);
      load();
    } catch (caught) {
      setError(errorMessage(caught));
      setTrashing(null);
    } finally {
      setBusy(false);
    }
  };

  if (error && !stores) {
    return (
      <div className="banner error" role="alert">
        {error}
      </div>
    );
  }
  if (!stores) return <p className="muted">Loading memory…</p>;
  if (stores.length === 0) {
    return (
      <p className="muted">
        No agent memory found. Claude Code keeps memory per project, and Codex
        keeps it only when its memories feature has been turned on.
      </p>
    );
  }

  return (
    <div className="memory-view">
      <nav className="memory-list" aria-label="Memory">
        {stores.map((store) => (
          <button
            key={store.folder}
            type="button"
            aria-current={store === current ? "true" : undefined}
            onClick={() => {
              setSelected(store.folder);
              setOpenFile(null);
            }}
          >
            <span className="memory-list-title">
              {storeLabel(store, homeDir)}
            </span>
            <span className="memory-list-meta">
              {store.agentName} · {plural(store.files.length, "file")}
              {store.problems.length > 0 && (
                <span className="badge warn">
                  {plural(store.problems.length, "warning")}
                </span>
              )}
            </span>
          </button>
        ))}
      </nav>
      {current && (
        <section className="memory-detail" aria-label="Memory details">
          {notice && (
            <div className="banner" role="status">
              {notice}
            </div>
          )}
          {error && (
            <div className="banner error" role="alert">
              {error}
            </div>
          )}
          <header className="memory-header">
            <div>
              <h2>{storeLabel(current, homeDir)}</h2>
              <p className="muted">
                {current.agentName}
                {current.projectDir &&
                  (current.sessions > 0
                    ? ` · ${plural(current.sessions, "session")} in this project`
                    : " · No sessions left in this project")}
              </p>
            </div>
            <div className="memory-actions">
              <button
                type="button"
                onClick={() =>
                  void revealMemory(current.folder).catch((caught: unknown) =>
                    setError(errorMessage(caught)),
                  )
                }
              >
                Show folder
              </button>
              {current.canTrash && (
                <button
                  type="button"
                  className="danger"
                  onClick={() => setTrashing(current)}
                >
                  Move to Trash…
                </button>
              )}
            </div>
          </header>
          {current.note && <p className="banner info">{current.note}</p>}
          {current.problems.length > 0 && (
            <ul className="banner warn memory-problems">
              {current.problems.map((problem) => (
                <li key={problem}>{problem}</li>
              ))}
            </ul>
          )}
          {current.files.length === 0 ? (
            <p className="muted">This memory folder is empty.</p>
          ) : (
            <table className="memory-files">
              <thead>
                <tr>
                  <th>File</th>
                  <th>Type</th>
                  <th>Description</th>
                  <th>Updated</th>
                  <th>From session</th>
                </tr>
              </thead>
              <tbody>
                {current.files.map((file) => {
                  const source = file.sessionId
                    ? sessionById.get(`${current.agent}:${file.sessionId}`)
                    : undefined;
                  return (
                    <tr key={file.path}>
                      <td>
                        <button
                          type="button"
                          className="link"
                          title={`Read ${file.path}`}
                          onClick={() => setOpenFile(file)}
                        >
                          {file.title}
                        </button>
                      </td>
                      <td>
                        {file.kind && (
                          <span className="badge">{file.kind}</span>
                        )}
                      </td>
                      <td className="muted">{file.description}</td>
                      <td
                        className="muted"
                        title={
                          file.modified
                            ? new Date(file.modified).toLocaleString()
                            : undefined
                        }
                      >
                        {file.modified ? formatAge(file.modified, now) : "—"}
                      </td>
                      <td>
                        {source ? (
                          <button
                            type="button"
                            className="link"
                            onClick={() => onOpenSession(source)}
                          >
                            {source.title ?? source.id}
                          </button>
                        ) : file.sessionId ? (
                          <span
                            className="muted"
                            title={`Session ${file.sessionId}`}
                          >
                            Deleted session
                          </span>
                        ) : (
                          <span className="muted">—</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          )}
        </section>
      )}
      {openFile && (
        <FilePanel
          key={openFile.path}
          file={openFile}
          homeDir={homeDir}
          onClose={() => setOpenFile(null)}
        />
      )}
      {trashing && (
        <TrashDialog
          store={trashing}
          homeDir={homeDir}
          busy={busy}
          onConfirm={() => void trash(trashing)}
          onCancel={() => setTrashing(null)}
        />
      )}
    </div>
  );
}
