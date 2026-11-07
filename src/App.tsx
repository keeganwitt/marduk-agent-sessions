import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { DeleteDialog } from "./components/DeleteDialog";
import { FilesDialog } from "./components/FilesDialog";
import { HelpDialog } from "./components/HelpDialog";
import { MemoryView } from "./components/MemoryView";
import { LabelMenu } from "./components/LabelMenu";
import { SettingsDialog } from "./components/SettingsDialog";
import { TranscriptPanel } from "./components/TranscriptPanel";
import { Filters } from "./components/Filters";
import { ColumnsMenu, SessionTable } from "./components/SessionTable";
import {
  EMPTY_FILTER,
  agentTypeOptions,
  isOptionalFilter,
  modelOptions,
  projectOptions,
  sessionMatcher,
  type OptionalFilter,
} from "./lib/filter";
import { rangeBetween } from "./lib/selection";
import { applyTheme, isTheme } from "./lib/theme";
import { nextColor } from "./lib/labels";
import { buildTree } from "./lib/tree";
import { useI18n, type I18n } from "./i18n/context";
import {
  applyLabels,
  createLabel,
  deleteSessions,
  getColumns,
  getTheme,
  getUiPref,
  listLabels,
  memoryLeftBehind,
  openProjectDir,
  previewDelete,
  revealSessionFile,
  saveColumns,
  saveUiPref,
  scanSessions,
  toggleArchive,
  trashMemory,
} from "./lib/ipc";
import {
  loadColumnPrefs,
  parseColumnPrefs,
  saveColumnPrefs,
  type ColumnPrefs,
} from "./lib/columns";
import {
  DEFAULT_SORT,
  nextSort,
  type SortKey,
  type SortState,
} from "./lib/sort";
import {
  isArchivable,
  isDeletable,
  isSelectable,
  type ArchiveResult,
  type DeleteResult,
  type PreviewItem,
  type Label,
  type MemoryStore,
  type ScanProgress,
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

function deleteNotice(results: DeleteResult[], t: I18n["t"]): Notice {
  const trashed = results.filter(
    (result) => result.status === "trashed",
  ).length;
  const deleted = results.filter(
    (result) => result.status === "deleted",
  ).length;
  const problems = results.flatMap((result) =>
    result.status === "refused" || result.status === "failed"
      ? [result.message]
      : [],
  );
  const parts = [
    trashed > 0 && t("app.notice.trashed", { count: trashed }),
    deleted > 0 && t("app.notice.deleted", { count: deleted }),
    problems.length > 0 &&
      t("app.notice.deleteFailed", { count: problems.length }),
  ].filter(Boolean);
  return {
    text: parts.join(" ") || t("app.notice.nothingDeleted"),
    problems,
  };
}

function archiveNotice(results: ArchiveResult[], t: I18n["t"]): Notice {
  const archived = results.filter(
    (result) => result.status === "done" && result.archived,
  ).length;
  const restored = results.filter(
    (result) => result.status === "done" && !result.archived,
  ).length;
  const problems = results.flatMap((result) =>
    result.status === "done" ? [] : [result.message],
  );
  const parts = [
    archived > 0 && t("app.notice.archived", { count: archived }),
    restored > 0 && t("app.notice.unarchived", { count: restored }),
    problems.length > 0 &&
      t("app.notice.changeFailed", { count: problems.length }),
  ].filter(Boolean);
  return {
    text: parts.join(" ") || t("app.notice.nothingChanged"),
    problems,
  };
}

function NoticeBanner({
  notice,
  onDismiss,
}: {
  notice: Notice;
  onDismiss: () => void;
}) {
  const { t } = useI18n();
  return (
    <div
      className={notice.problems.length > 0 ? "banner warn" : "banner"}
      role="status"
    >
      <span>{notice.text}</span>
      {notice.problems.length > 0 && (
        <ul>
          {notice.problems.map((problem, index) => (
            <li key={index}>{problem}</li>
          ))}
        </ul>
      )}
      <button type="button" onClick={onDismiss}>
        {t("app.dismiss")}
      </button>
    </div>
  );
}

function BusyIndicator({
  label,
  progress,
  overlay = false,
}: {
  label: string;
  progress: ScanProgress | null;
  overlay?: boolean;
}) {
  const { t } = useI18n();
  const percent =
    progress && progress.total > 0
      ? Math.min(100, Math.floor((progress.done / progress.total) * 100))
      : null;
  return (
    <div
      className={overlay ? "busy overlay-busy" : "busy"}
      role="status"
      aria-live="polite"
    >
      {percent === null ? (
        <span className="spinner" aria-hidden="true" />
      ) : (
        <progress
          className="scan-progress"
          max={100}
          value={percent}
          aria-label={t("app.scanProgress")}
        />
      )}
      <span>
        {progress?.agent
          ? t("app.scanningAgent", { agent: progress.agent })
          : label}
        {percent !== null && ` ${percent}%`}
      </span>
    </div>
  );
}

export default function App() {
  const { t, bytes } = useI18n();
  const [scan, setScan] = useState<ScanResponse | null>(null);
  const [scannedAt, setScannedAt] = useState(() => new Date());
  const [activity, setActivity] = useState<string | null>(() =>
    t("app.scanning"),
  );
  const busy = activity !== null;
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState(EMPTY_FILTER);
  const [addedFilters, setAddedFilters] = useState<OptionalFilter[]>([]);
  useEffect(() => {
    getUiPref("filters").then(
      (saved) =>
        Array.isArray(saved) && setAddedFilters(saved.filter(isOptionalFilter)),
      () => {},
    );
  }, []);
  const changeAddedFilters = useCallback((added: OptionalFilter[]) => {
    setAddedFilters(added);
    void saveUiPref("filters", added).catch(() => {});
  }, []);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [preview, setPreview] = useState<PreviewItem[] | null>(null);
  const [memoryOffers, setMemoryOffers] = useState<MemoryStore[]>([]);
  const [view, setView] = useState<"sessions" | "memory">("sessions");
  const [deleting, setDeleting] = useState(false);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [sort, setSort] = useState<SortState>(DEFAULT_SORT);
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [filesFor, setFilesFor] = useState<SessionRow | null>(null);
  const [transcriptFor, setTranscriptFor] = useState<SessionRow | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [labels, setLabels] = useState<Label[]>([]);
  const [labelMenuOpen, setLabelMenuOpen] = useState(false);
  const [columns, setColumns] = useState<ColumnPrefs>(loadColumnPrefs);

  const columnsLoaded = useRef(false);
  useEffect(() => {
    getColumns()
      .then(
        (saved) => {
          if (saved === null || saved === undefined) {
            void saveColumns(loadColumnPrefs()).catch(() => {});
          } else {
            setColumns(parseColumnPrefs(saved));
          }
        },
        () => {},
      )
      .finally(() => {
        columnsLoaded.current = true;
      });
  }, []);
  // Resizing changes widths continuously; only the settled width is written to disk.
  useEffect(() => {
    saveColumnPrefs(columns);
    if (!columnsLoaded.current) return;
    const timer = setTimeout(
      () => void saveColumns(columns).catch(() => {}),
      400,
    );
    return () => clearTimeout(timer);
  }, [columns]);
  const labelsById = useMemo(
    () => new Map(labels.map((label) => [label.id, label])),
    [labels],
  );

  const refreshLabels = useCallback(async () => {
    try {
      setLabels(await listLabels());
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }, []);

  /** Shows a scan's result. The "Scanning" state must already be showing, as it is on startup. */
  const finishScan = useCallback(
    (request: Promise<ScanResponse>) =>
      request
        .then(
          async (result) => {
            setScan(result);
            setScannedAt(new Date());
            setSelected(new Set());
            await refreshLabels();
          },
          (caught: unknown) => setError(errorMessage(caught)),
        )
        .finally(() => {
          setActivity(null);
          setProgress(null);
        }),
    [refreshLabels],
  );

  const rescan = useCallback(async () => {
    setActivity(t("app.scanning"));
    setError(null);
    setProgress(null);
    await finishScan(scanSessions(setProgress));
  }, [finishScan, t]);

  useEffect(() => {
    void finishScan(scanSessions(setProgress));
  }, [finishScan]);

  useEffect(() => {
    getTheme().then(
      (theme) => isTheme(theme) && applyTheme(theme),
      () => {},
    );
  }, []);

  const projects = useMemo(() => projectOptions(scan?.sessions ?? []), [scan]);
  const models = useMemo(() => modelOptions(scan?.sessions ?? []), [scan]);
  const agentTypes = useMemo(
    () => agentTypeOptions(scan?.sessions ?? []),
    [scan],
  );
  const matcher = useMemo(
    () => sessionMatcher(filter, scannedAt),
    [filter, scannedAt],
  );
  const matching = useMemo(
    () => (scan ? scan.sessions.filter(matcher) : []),
    [scan, matcher],
  );
  const rows = useMemo(
    () => (scan ? buildTree(scan.sessions, matcher, sort, expanded) : []),
    [scan, matcher, sort, expanded],
  );
  const visible = rows.map((row) => row.session);
  const selectedVisible = visible.filter((session) =>
    selected.has(session.uid),
  );
  const toDelete = selectedVisible.filter(isDeletable);
  const toArchive = selectedVisible.filter(
    (session) => isArchivable(session) && !session.archived,
  );
  const toUnarchive = selectedVisible.filter(
    (session) => isArchivable(session) && session.archived,
  );
  const deleteBytes = toDelete.reduce(
    (total, session) => total + session.sizeBytes,
    0,
  );
  const matchingBytes = matching.reduce(
    (total, session) => total + session.sizeBytes,
    0,
  );
  const incompleteAgents =
    scan?.agents.filter((agent) => !agent.complete) ?? [];

  const toggleExpanded = (uid: string) =>
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(uid)) next.delete(uid);
      else next.add(uid);
      return next;
    });

  const selectionAnchor = useRef<string | null>(null);

  const toggle = (uid: string, extend: boolean) => {
    const select = !selected.has(uid);
    const range = extend
      ? rangeBetween(
          visible.map((session) => session.uid),
          selectionAnchor.current,
          uid,
        )
      : [uid];
    const selectable = new Set(
      visible.filter(isSelectable).map((session) => session.uid),
    );
    selectionAnchor.current = uid;
    setSelected((current) => {
      const next = new Set(current);
      for (const id of range) {
        if (!selectable.has(id)) continue;
        if (select) next.add(id);
        else next.delete(id);
      }
      return next;
    });
  };

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
      const items = await previewDelete(toDelete.map((session) => session.uid));
      const ready = items
        .filter((item) => item.refusedReason === null)
        .map((item) => item.uid);
      setMemoryOffers(
        ready.length > 0 ? await memoryLeftBehind(ready).catch(() => []) : [],
      );
      setPreview(items);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const confirmDelete = async (uids: string[], memoryFolders: string[]) => {
    setDeleting(true);
    try {
      const outcome = await deleteSessions(uids);
      const removed = new Set(
        outcome
          .filter(
            (result) =>
              result.status === "trashed" || result.status === "deleted",
          )
          .map((r) => r.uid),
      );
      setScan(
        (current) =>
          current && {
            ...current,
            sessions: current.sessions.filter(
              (session) => !removed.has(session.uid),
            ),
          },
      );
      setSelected(new Set());
      const memoryProblems: string[] = [];
      let memoryTrashed = 0;
      for (const folder of memoryFolders) {
        try {
          await trashMemory(folder, true);
          memoryTrashed += 1;
        } catch (caught) {
          memoryProblems.push(errorMessage(caught));
        }
      }
      const result = deleteNotice(outcome, t);
      setNotice({
        text: [
          result.text,
          memoryTrashed === 1 && t("app.notice.memoryOne"),
          memoryTrashed > 1 &&
            t("app.notice.memoryMany", { count: memoryTrashed }),
        ]
          .filter(Boolean)
          .join(" "),
        problems: [...result.problems, ...memoryProblems],
      });
      setPreview(null);
      // Agent deletes can take related sessions with them (OpenCode deletes child sessions).
      if (outcome.some((result) => result.status === "deleted")) await rescan();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setDeleting(false);
    }
  };

  const runArchive = async (uids: string[]) => {
    setActivity(t("app.updatingArchive"));
    setError(null);
    try {
      const outcome = await toggleArchive(uids);
      setNotice(archiveNotice(outcome, t));
    } catch (caught) {
      setError(errorMessage(caught));
    }
    await rescan();
  };

  const labelSelected = async (add: number[], remove: number[]) => {
    const uids = new Set(selectedVisible.map((session) => session.uid));
    setLabelMenuOpen(false);
    try {
      await applyLabels([...uids], add, remove);
      setScan(
        (current) =>
          current && {
            ...current,
            sessions: current.sessions.map((session) =>
              uids.has(session.uid)
                ? {
                    ...session,
                    labelIds: [
                      ...new Set([...session.labelIds, ...add]),
                    ].filter((id) => !remove.includes(id)),
                  }
                : session,
            ),
          },
      );
      await refreshLabels();
    } catch (caught) {
      setError(errorMessage(caught));
    }
  };

  const newLabel = async (name: string): Promise<Label | null> => {
    try {
      const label = await createLabel(name, nextColor(labels));
      setLabels((current) =>
        [...current, label].sort((a, b) => a.name.localeCompare(b.name)),
      );
      return label;
    } catch (caught) {
      setError(errorMessage(caught));
      return null;
    }
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
  const closeTranscript = useCallback(() => setTranscriptFor(null), []);

  return (
    <main className="app">
      <header className="toolbar">
        <h1 className="visually-hidden">Marduk</h1>
        <div className="view-tabs" role="tablist" aria-label={t("app.view")}>
          {(
            [
              ["sessions", t("app.view.sessions")],
              ["memory", t("app.view.memory")],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={view === id}
              onClick={() => setView(id)}
            >
              {label}
            </button>
          ))}
        </div>
        <span className="summary">
          {scan && view === "sessions"
            ? t("app.summary", {
                shown: matching.length,
                count: scan.sessions.length,
                size: bytes(matchingBytes),
              })
            : ""}
        </span>
        {scan && view === "sessions" && (
          <input
            type="search"
            className="toolbar-search"
            aria-label={t("table.filter.search")}
            placeholder={t("table.filter.searchPlaceholder")}
            value={filter.query}
            onChange={(event) =>
              setFilter((current) => ({
                ...current,
                query: event.target.value,
              }))
            }
          />
        )}
        <button type="button" onClick={() => setHelpOpen(true)}>
          {t("app.help")}
        </button>
        <button type="button" onClick={() => setSettingsOpen(true)}>
          {t("app.settings")}
        </button>
        {view === "sessions" && (
          <div className="menu-anchor">
            <button
              type="button"
              aria-haspopup="dialog"
              aria-expanded={labelMenuOpen}
              disabled={selectedVisible.length === 0 || busy}
              onClick={() => setLabelMenuOpen((open) => !open)}
            >
              {t("app.labels")}
            </button>
            {labelMenuOpen && (
              <LabelMenu
                labels={labels}
                selected={selectedVisible}
                onApply={(add, remove) => void labelSelected(add, remove)}
                onCreate={newLabel}
                onClose={() => setLabelMenuOpen(false)}
              />
            )}
          </div>
        )}
        <button type="button" onClick={() => void rescan()} disabled={busy}>
          {t("app.rescan")}
        </button>
        {view === "sessions" && (
          <>
            {toArchive.length > 0 && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void runArchive(toArchive.map((s) => s.uid))}
              >
                {t("app.archive", { count: toArchive.length })}
              </button>
            )}
            {toUnarchive.length > 0 && (
              <button
                type="button"
                disabled={busy}
                onClick={() => void runArchive(toUnarchive.map((s) => s.uid))}
              >
                {t("app.unarchive", { count: toUnarchive.length })}
              </button>
            )}
            <button
              type="button"
              className="danger"
              disabled={toDelete.length === 0 || busy}
              onClick={() => void openPreview()}
            >
              {toDelete.length === 0
                ? t("app.delete")
                : t("app.deleteCount", {
                    count: toDelete.length,
                    size: bytes(deleteBytes),
                  })}
            </button>
          </>
        )}
      </header>

      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}
      {incompleteAgents.map((agent) => (
        <div key={agent.id} className="banner warn" role="status">
          {t("app.unreadable", {
            agent: agent.name,
            errors: agent.errors.slice(0, 3).join("; "),
          })}
          {agent.errors.length > 3 &&
            t("app.unreadableMore", { count: agent.errors.length - 3 })}
        </div>
      ))}
      {notice && (
        <NoticeBanner notice={notice} onDismiss={() => setNotice(null)} />
      )}

      {scan && view === "sessions" && (
        <div className="filter-bar">
          <Filters
            agents={scan.agents}
            labels={labels}
            projects={projects}
            models={models}
            agentTypes={agentTypes}
            homeDir={scan.homeDir}
            filter={filter}
            onChange={setFilter}
            added={addedFilters}
            onAddedChange={changeAddedFilters}
          />
          <ColumnsMenu columns={columns} onColumnsChange={setColumns} />
        </div>
      )}
      {!scan && activity && (
        <BusyIndicator label={activity} progress={progress} />
      )}
      {scan && view === "memory" && (
        <MemoryView
          sessions={scan.sessions}
          homeDir={scan.homeDir}
          now={scannedAt}
          reloadKey={scan}
          onOpenSession={setTranscriptFor}
        />
      )}
      {scan && view === "sessions" && (
        <div className="table-area" aria-busy={busy}>
          {activity && (
            <BusyIndicator label={activity} progress={progress} overlay />
          )}
          <div className="table-wrap">
            <SessionTable
              rows={rows}
              columns={columns}
              onColumnsChange={setColumns}
              labelsById={labelsById}
              onToggleExpand={toggleExpanded}
              homeDir={scan.homeDir}
              selected={selected}
              now={scannedAt}
              sort={sort}
              onSort={(key: SortKey) =>
                setSort((current) => nextSort(current, key))
              }
              onToggle={toggle}
              onSetMany={setMany}
              onOpenProject={(session) => void openProject(session)}
              onFilterProject={(project) =>
                setFilter((current) => ({ ...current, project }))
              }
              onShowFiles={setFilesFor}
              onOpenTranscript={setTranscriptFor}
            />
          </div>
        </div>
      )}

      {helpOpen && (
        <HelpDialog
          homeDir={scan?.homeDir ?? ""}
          onClose={() => setHelpOpen(false)}
        />
      )}
      {settingsOpen && (
        <SettingsDialog
          onClose={(changed) => {
            setSettingsOpen(false);
            if (changed) void rescan();
          }}
        />
      )}
      {transcriptFor && (
        <TranscriptPanel
          key={transcriptFor.uid}
          session={
            scan?.sessions.find(
              (session) => session.uid === transcriptFor.uid,
            ) ?? transcriptFor
          }
          labelsById={labelsById}
          onClose={closeTranscript}
        />
      )}
      {filesFor && (
        <FilesDialog
          session={filesFor}
          homeDir={scan?.homeDir ?? ""}
          onReveal={(path) => void reveal(path)}
          onClose={closeFiles}
        />
      )}

      {preview && (
        <DeleteDialog
          items={preview}
          memory={memoryOffers}
          busy={deleting}
          onConfirm={(uids, folders) => void confirmDelete(uids, folders)}
          onCancel={() => setPreview(null)}
          onReveal={(path) => void reveal(path)}
          homeDir={scan?.homeDir ?? ""}
        />
      )}
    </main>
  );
}
