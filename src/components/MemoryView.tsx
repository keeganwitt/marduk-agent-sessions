import { useCallback, useEffect, useState } from "react";

import { useI18n, type I18n } from "../i18n/context";
import { tildePath } from "../lib/format";
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

function storeLabel(store: MemoryStore, homeDir: string, t: I18n["t"]): string {
  if (store.projectDir) return tildePath(store.projectDir, homeDir);
  return t("memory.allProjects", { agent: store.agentName });
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
  const { t } = useI18n();
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
        <button
          type="button"
          onClick={onClose}
          aria-label={t("memory.closeFile")}
        >
          {t("memory.close")}
        </button>
      </header>
      <div className="transcript-body">
        {error && (
          <div className="banner error" role="alert">
            {error}
          </div>
        )}
        {text === null && !error && (
          <p className="muted">{t("memory.loading")}</p>
        )}
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
  const { t } = useI18n();
  return (
    <div className="overlay">
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="trash-memory-title"
      >
        <h2 id="trash-memory-title">
          {t("memory.trash.title", {
            agent: store.agentName,
            target: storeLabel(store, homeDir, t),
          })}
        </h2>
        <p>
          {t("memory.trash.body")}
          {store.projectExists &&
            ` ${t("memory.trash.projectExists", { agent: store.agentName })}`}
        </p>
        <ul className="files">
          {store.files.map((file) => (
            <li key={file.path}>{tildePath(file.path, homeDir)}</li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" onClick={onCancel} disabled={busy}>
            {t("memory.cancel")}
          </button>
          <button
            type="button"
            className="danger"
            onClick={onConfirm}
            disabled={busy}
          >
            {busy ? t("memory.working") : t("memory.moveToTrash")}
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
  const { t, age, dateTime } = useI18n();
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
        t("memory.trashed", {
          agent: store.agentName,
          target: storeLabel(store, homeDir, t),
        }),
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
  if (!stores) return <p className="muted">{t("memory.loadingMemory")}</p>;
  if (stores.length === 0) {
    return <p className="muted">{t("memory.none")}</p>;
  }

  return (
    <div className="memory-view">
      <nav className="memory-list" aria-label={t("memory.navLabel")}>
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
              {storeLabel(store, homeDir, t)}
            </span>
            <span className="memory-list-meta">
              {store.agentName} ·{" "}
              {t("memory.files", { count: store.files.length })}
              {store.problems.length > 0 && (
                <span className="badge warn">
                  {t("memory.warnings", { count: store.problems.length })}
                </span>
              )}
            </span>
          </button>
        ))}
      </nav>
      {current && (
        <section
          className="memory-detail"
          aria-label={t("memory.detailsLabel")}
        >
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
              <h2>{storeLabel(current, homeDir, t)}</h2>
              <p className="muted">
                {current.agentName}
                {current.projectDir &&
                  ` · ${
                    current.sessions > 0
                      ? t("memory.sessionsInProject", {
                          count: current.sessions,
                        })
                      : t("memory.noSessionsLeft")
                  }`}
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
                {t("memory.showFolder")}
              </button>
              {current.canTrash && (
                <button
                  type="button"
                  className="danger"
                  onClick={() => setTrashing(current)}
                >
                  {t("memory.moveToTrashEllipsis")}
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
            <p className="muted">{t("memory.folderEmpty")}</p>
          ) : (
            <table className="memory-files">
              <thead>
                <tr>
                  <th>{t("memory.column.file")}</th>
                  <th>{t("memory.column.type")}</th>
                  <th>{t("memory.column.description")}</th>
                  <th>{t("memory.column.updated")}</th>
                  <th>{t("memory.column.fromSession")}</th>
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
                          title={t("memory.readFile", { path: file.path })}
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
                          file.modified ? dateTime(file.modified) : undefined
                        }
                      >
                        {file.modified ? age(file.modified, now) : "—"}
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
                            title={t("memory.sessionId", {
                              id: file.sessionId,
                            })}
                          >
                            {t("memory.deletedSession")}
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
