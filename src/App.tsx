import { useCallback, useEffect, useMemo, useState } from "react";
import { DeleteDialog } from "./components/DeleteDialog";
import { Filters } from "./components/Filters";
import { SessionTable } from "./components/SessionTable";
import { EMPTY_FILTER, filterSessions } from "./lib/filter";
import { formatBytes } from "./lib/format";
import { deleteSessions, previewDelete, scanSessions } from "./lib/ipc";
import type { DeleteResult, PreviewItem, ScanResponse } from "./lib/types";
import "./App.css";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function ResultSummary({ results, onDismiss }: { results: DeleteResult[]; onDismiss: () => void }) {
  const trashed = results.filter((result) => result.status === "trashed").length;
  const problems = results.filter((result) => result.status !== "trashed");
  return (
    <div className={problems.length > 0 ? "banner warn" : "banner"} role="status">
      <span>
        Moved {trashed} {trashed === 1 ? "session" : "sessions"} to the Trash.
        {problems.length > 0 && ` ${problems.length} could not be moved.`}
      </span>
      {problems.length > 0 && (
        <ul>
          {problems.map((problem) => (
            <li key={problem.uid}>{problem.message}</li>
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
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState(EMPTY_FILTER);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [preview, setPreview] = useState<PreviewItem[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [results, setResults] = useState<DeleteResult[] | null>(null);

  const rescan = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setScan(await scanSessions());
      setScannedAt(new Date());
      setSelected(new Set());
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void rescan();
  }, [rescan]);

  const visible = useMemo(
    () => (scan ? filterSessions(scan.sessions, filter, scannedAt) : []),
    [scan, filter, scannedAt],
  );
  const selectedVisible = visible.filter((session) => selected.has(session.uid));
  const selectedBytes = selectedVisible.reduce((total, session) => total + session.sizeBytes, 0);
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
      setPreview(await previewDelete(selectedVisible.map((session) => session.uid)));
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const confirmDelete = async (uids: string[]) => {
    setDeleting(true);
    try {
      const outcome = await deleteSessions(uids);
      const trashed = new Set(outcome.filter((result) => result.status === "trashed").map((result) => result.uid));
      setScan((current) =>
        current && { ...current, sessions: current.sessions.filter((session) => !trashed.has(session.uid)) },
      );
      setSelected(new Set());
      setResults(outcome);
      setPreview(null);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setDeleting(false);
    }
  };

  const selectedCount = selectedVisible.length;

  return (
    <main className="app">
      <header className="toolbar">
        <h1>Marduk</h1>
        <span className="summary">
          {scan ? `${visible.length} of ${scan.sessions.length} sessions · ${formatBytes(visibleBytes)}` : ""}
        </span>
        <button type="button" onClick={() => void rescan()} disabled={loading}>
          {loading ? "Scanning…" : "Rescan"}
        </button>
        <button
          type="button"
          className="danger"
          disabled={selectedCount === 0 || loading}
          onClick={() => void openPreview()}
        >
          {selectedCount === 0
            ? "Move to Trash…"
            : `Move ${selectedCount} to Trash (${formatBytes(selectedBytes)})…`}
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
      {results && <ResultSummary results={results} onDismiss={() => setResults(null)} />}

      {scan && <Filters agents={scan.agents} filter={filter} onChange={setFilter} />}
      {scan && (
        <div className="table-wrap">
          <SessionTable sessions={visible} selected={selected} now={scannedAt} onToggle={toggle} onSetMany={setMany} />
        </div>
      )}

      {preview && (
        <DeleteDialog
          items={preview}
          busy={deleting}
          onConfirm={(uids) => void confirmDelete(uids)}
          onCancel={() => setPreview(null)}
        />
      )}
    </main>
  );
}
