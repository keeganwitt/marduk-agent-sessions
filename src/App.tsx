import { useCallback, useEffect, useMemo, useState } from "react";
import { DeleteDialog } from "./components/DeleteDialog";
import { FilesDialog } from "./components/FilesDialog";
import { Filters } from "./components/Filters";
import { SessionTable } from "./components/SessionTable";
import { EMPTY_FILTER, filterSessions } from "./lib/filter";
import { formatBytes } from "./lib/format";
import { deleteSessions, openProjectDir, previewDelete, revealSessionFile, scanSessions, toggleArchive } from "./lib/ipc";
import { nextSort, sortSessions, type SortKey, type SortState } from "./lib/sort";
import {
  isArchivable,
  isDeletable,
  type ArchiveResult,
  type DeleteResult,
  type PreviewItem,
  type ScanResponse,
  type SessionRow,
} from "./lib/types";
import "./App.css";

interface Notice {
  text: string;
  problems: string[];
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function plural(count: number, word: string): string {
  return `${count} ${count === 1 ? word : `${word}s`}`;
}

function deleteNotice(results: DeleteResult[]): Notice {
  const trashed = results.filter((result) => result.status === "trashed").length;
  const deleted = results.filter((result) => result.status === "deleted").length;
  const problems = results.flatMap((result) =>
    result.status === "refused" || result.status === "failed" ? [result.message] : [],
  );
  const parts = [
    trashed > 0 && `Moved ${plural(trashed, "session")} to the Trash.`,
    deleted > 0 && `Permanently deleted ${plural(deleted, "session")}.`,
    problems.length > 0 && `${plural(problems.length, "session")} couldn't be deleted.`,
  ].filter(Boolean);
  return { text: parts.join(" ") || "Nothing was deleted.", problems };
}

function archiveNotice(results: ArchiveResult[]): Notice {
  const archived = results.filter((result) => result.status === "done" && result.archived).length;
  const restored = results.filter((result) => result.status === "done" && !result.archived).length;
  const problems = results.flatMap((result) => (result.status === "done" ? [] : [result.message]));
  const parts = [
    archived > 0 && `Archived ${plural(archived, "session")}.`,
    restored > 0 && `Unarchived ${plural(restored, "session")}.`,
    problems.length > 0 && `${plural(problems.length, "session")} couldn't be changed.`,
  ].filter(Boolean);
  return { text: parts.join(" ") || "Nothing was changed.", problems };
}

function NoticeBanner({ notice, onDismiss }: { notice: Notice; onDismiss: () => void }) {
  return (
    <div className={notice.problems.length > 0 ? "banner warn" : "banner"} role="status">
      <span>{notice.text}</span>
      {notice.problems.length > 0 && (
        <ul>
          {notice.problems.map((problem, index) => (
            <li key={index}>{problem}</li>
          ))}
        </ul>
      )}
      <button type="button" onClick={onDismiss}>
        Dismiss
      </button>
    </div>
  );
}

export default function App() {
  const [scan, setScan] = useState<ScanResponse | null>(null);
  const [scannedAt, setScannedAt] = useState(() => new Date());
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState(EMPTY_FILTER);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [preview, setPreview] = useState<PreviewItem[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [sort, setSort] = useState<SortState | null>(null);
  const [filesFor, setFilesFor] = useState<SessionRow | null>(null);

  const rescan = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setScan(await scanSessions());
      setScannedAt(new Date());
      setSelected(new Set());
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  const visible = useMemo(
    () => (scan ? sortSessions(filterSessions(scan.sessions, filter, scannedAt), sort) : []),
    [scan, filter, scannedAt, sort],
  );
  const selectedVisible = visible.filter((session) => selected.has(session.uid));
  const toDelete = selectedVisible.filter(isDeletable);
  const toArchive = selectedVisible.filter((session) => isArchivable(session) && !session.archived);
  const toUnarchive = selectedVisible.filter((session) => isArchivable(session) && session.archived);
  const deleteBytes = toDelete.reduce((total, session) => total + session.sizeBytes, 0);
  const visibleBytes = visible.reduce((total, session) => total + session.sizeBytes, 0);
  const incompleteAgents = scan?.agents.filter((agent) => !agent.complete) ?? [];

  const toggle = (uid: string) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(uid)) next.delete(uid);
      else next.add(uid);
      return next;
    });

  const setMany = (uids: string[], select: boolean) =>
    setSelected((current) => {
      const next = new Set(current);
      for (const uid of uids) {
        if (select) next.add(uid);
        else next.delete(uid);
      }
      return next;
    });

  const openPreview = async () => {
    setError(null);
    try {
      setPreview(await previewDelete(toDelete.map((session) => session.uid)));
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const confirmDelete = async (uids: string[]) => {
    setDeleting(true);
    try {
      const outcome = await deleteSessions(uids);
      const removed = new Set(
        outcome.filter((result) => result.status === "trashed" || result.status === "deleted").map((r) => r.uid),
      );
      setScan((current) =>
        current && { ...current, sessions: current.sessions.filter((session) => !removed.has(session.uid)) },
      );
      setSelected(new Set());
      setNotice(deleteNotice(outcome));
      setPreview(null);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setDeleting(false);
    }
  };

  const runArchive = async (uids: string[]) => {
    setBusy(true);
    setError(null);
    try {
      const outcome = await toggleArchive(uids);
      setNotice(archiveNotice(outcome));
    } catch (caught) {
      setError(errorMessage(caught));
    }
    await rescan();
  };

  const reveal = async (path: string) => {
    try {
      await revealSessionFile(path);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const openProject = async (session: SessionRow) => {
    setError(null);
    try {
      await openProjectDir(session.uid);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const closeFiles = useCallback(() => setFilesFor(null), []);

  return (
    <main className="app">
      <header className="toolbar">
        <h1>Marduk</h1>
        <span className="summary">
          {scan ? `${visible.length} of ${scan.sessions.length} sessions · ${formatBytes(visibleBytes)}` : ""}
        </span>
        <button type="button" onClick={() => void rescan()} disabled={busy}>
          {busy ? "Working…" : "Rescan"}
        </button>
        {toArchive.length > 0 && (
          <button type="button" disabled={busy} onClick={() => void runArchive(toArchive.map((s) => s.uid))}>
            Archive {toArchive.length}
          </button>
        )}
        {toUnarchive.length > 0 && (
          <button type="button" disabled={busy} onClick={() => void runArchive(toUnarchive.map((s) => s.uid))}>
            Unarchive {toUnarchive.length}
          </button>
        )}
        <button type="button" className="danger" disabled={toDelete.length === 0 || busy} onClick={() => void openPreview()}>
          {toDelete.length === 0 ? "Delete…" : `Delete ${toDelete.length} (${formatBytes(deleteBytes)})…`}
        </button>
      </header>

      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}
      {incompleteAgents.map((agent) => (
        <div key={agent.id} className="banner warn" role="status">
          Some {agent.name} sessions couldn't be read: {agent.errors.slice(0, 3).join("; ")}
          {agent.errors.length > 3 && ` (and ${agent.errors.length - 3} more)`}
        </div>
      ))}
      {notice && <NoticeBanner notice={notice} onDismiss={() => setNotice(null)} />}

      {scan && <Filters agents={scan.agents} filter={filter} onChange={setFilter} />}
      {scan && (
        <div className="table-wrap">
          <SessionTable
            sessions={visible}
            selected={selected}
            now={scannedAt}
            sort={sort}
            onSort={(key: SortKey) => setSort((current) => nextSort(current, key))}
            onToggle={toggle}
            onSetMany={setMany}
            onOpenProject={(session) => void openProject(session)}
            onShowFiles={setFilesFor}
          />
        </div>
      )}

      {filesFor && <FilesDialog session={filesFor} onReveal={(path) => void reveal(path)} onClose={closeFiles} />}

      {preview && (
        <DeleteDialog
          items={preview}
          busy={deleting}
          onConfirm={(uids) => void confirmDelete(uids)}
          onCancel={() => setPreview(null)}
          onReveal={(path) => void reveal(path)}
        />
      )}
    </main>
  );
}
