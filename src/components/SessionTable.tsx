import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import {
  COLUMNS,
  DEFAULT_PREFS,
  isHidden,
  layoutWidths,
  resize,
  setHidden,
  visibleColumns,
  type ColumnDef,
  type ColumnId,
  type ColumnPrefs,
} from "../lib/columns";
import { NO_PROJECT } from "../lib/filter";
import { formatAge, formatBytes, shortenPath } from "../lib/format";
import type { SortKey, SortState } from "../lib/sort";
import type { TreeRow } from "../lib/tree";
import { LabelChips } from "./LabelChip";
import { isSelectable, type Label, type SessionRow } from "../lib/types";

interface Props {
  rows: TreeRow[];
  labelsById: ReadonlyMap<number, Label>;
  onToggleExpand: (uid: string) => void;
  homeDir: string;
  selected: ReadonlySet<string>;
  now: Date;
  sort: SortState | null;
  onSort: (key: SortKey) => void;
  /** `extend` is true for shift-clicks, which select a range. */
  onToggle: (uid: string, extend: boolean) => void;
  onSetMany: (uids: string[], selected: boolean) => void;
  onOpenProject: (session: SessionRow) => void;
  onFilterProject: (project: string) => void;
  onShowFiles: (session: SessionRow) => void;
  onOpenTranscript: (session: SessionRow) => void;
  columns: ColumnPrefs;
  onColumnsChange: Dispatch<SetStateAction<ColumnPrefs>>;
}

function StatusBadges({
  session,
  nested,
}: {
  session: SessionRow;
  nested: boolean;
}) {
  return (
    <>
      {session.running && (
        <span
          className="badge running"
          title="Written to in the last few minutes, so it can't be changed."
        >
          Running
        </span>
      )}
      {session.parentId && session.parentMissing && (
        <span
          className="badge warn"
          title={`Started by session ${session.parentId}, which no longer exists`}
        >
          Parent deleted
        </span>
      )}
      {session.parentId && !session.parentMissing && !nested && (
        <span
          className="badge muted"
          title={`Started by session ${session.parentId}`}
        >
          Subagent
        </span>
      )}
      {session.archived && <span className="badge archived">Archived</span>}
      {session.readOnlyReason && !session.partOfParent && (
        <span className="badge muted" title={session.readOnlyReason}>
          Read-only
        </span>
      )}
    </>
  );
}

function SortableHeader({
  label,
  sortKey,
  sort,
  onSort,
  numeric = false,
}: {
  label: string;
  sortKey: SortKey;
  sort: SortState | null;
  onSort: (key: SortKey) => void;
  numeric?: boolean;
}) {
  const direction = sort?.key === sortKey ? sort.direction : null;
  return (
    <button
      type="button"
      className={numeric ? "sort num" : "sort"}
      onClick={() => onSort(sortKey)}
    >
      {label}
      <span className="sort-indicator" aria-hidden="true">
        {direction === "asc" ? "▲" : direction === "desc" ? "▼" : ""}
      </span>
    </button>
  );
}

function ariaSortFor(
  sort: SortState | null,
  key: SortKey | null,
): "ascending" | "descending" | "none" | undefined {
  if (key === null) return undefined;
  if (sort?.key !== key) return "none";
  return sort.direction === "asc" ? "ascending" : "descending";
}

const SORT_KEYS: Partial<Record<ColumnId, SortKey>> = {
  model: "model",
  session: "title",
  agent: "agent",
  project: "project",
  created: "created",
  lastActivity: "lastActivity",
  size: "size",
};

const CHECK_WIDTH = 36;
const ACTIONS_WIDTH = 64;
const KEYBOARD_STEP = 16;

/** Drag to resize; double-click resets; arrow keys resize when focused. */
/**
 * The width of the table's scroll container, without its scrollbar; null until measured.
 * `onResize` runs whenever it changes (window resize, moving to another screen).
 */
function useContainerWidth(onResize: () => void) {
  const [width, setWidth] = useState<number | null>(null);
  const observer = useRef<ResizeObserver | null>(null);
  const measure = useCallback(
    (table: HTMLTableElement | null) => {
      observer.current?.disconnect();
      const container = table?.parentElement;
      if (!container || typeof ResizeObserver === "undefined") return;
      const update = () => {
        setWidth(container.clientWidth);
        onResize();
      };
      observer.current = new ResizeObserver(update);
      observer.current.observe(container);
      update();
    },
    [onResize],
  );
  return [width, measure] as const;
}

function ResizeHandle({
  column,
  width,
  onColumnsChange,
}: {
  column: ColumnDef;
  width: number;
  onColumnsChange: Dispatch<SetStateAction<ColumnPrefs>>;
}) {
  const drag = useRef<{ startX: number; startWidth: number } | null>(null);

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      if (!drag.current) return;
      const next =
        drag.current.startWidth + event.clientX - drag.current.startX;
      onColumnsChange((prefs) => resize(prefs, column.id, next));
    };
    const onUp = () => {
      if (!drag.current) return;
      drag.current = null;
      document.body.classList.remove("resizing-column");
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [column.id, onColumnsChange]);

  return (
    <span
      className="resize-handle"
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${column.label} column`}
      aria-valuenow={width}
      aria-valuemin={column.minWidth}
      tabIndex={0}
      onPointerDown={(event) => {
        event.preventDefault();
        drag.current = { startX: event.clientX, startWidth: width };
        document.body.classList.add("resizing-column");
      }}
      onDoubleClick={() =>
        onColumnsChange((prefs) => resize(prefs, column.id, null))
      }
      onKeyDown={(event) => {
        const step =
          event.key === "ArrowRight"
            ? KEYBOARD_STEP
            : event.key === "ArrowLeft"
              ? -KEYBOARD_STEP
              : 0;
        if (step === 0) return;
        event.preventDefault();
        onColumnsChange((prefs) => resize(prefs, column.id, width + step));
      }}
    />
  );
}

export function ColumnsMenu({
  columns,
  onColumnsChange,
}: {
  columns: ColumnPrefs;
  onColumnsChange: Dispatch<SetStateAction<ColumnPrefs>>;
}) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      if (!anchor.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    window.addEventListener("pointerdown", onPointer);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointerdown", onPointer);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="menu-anchor columns-anchor" ref={anchor}>
      <button
        type="button"
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        Columns ▾
      </button>
      {open && (
        <div className="columns-menu" role="group" aria-label="Show columns">
          {COLUMNS.filter((column) => column.hideable).map((column) => (
            <label key={column.id}>
              <input
                type="checkbox"
                checked={!isHidden(columns, column)}
                onChange={(event) =>
                  onColumnsChange((prefs) =>
                    setHidden(prefs, column.id, !event.target.checked),
                  )
                }
              />
              {column.label}
            </label>
          ))}
          <button
            type="button"
            className="link"
            onClick={() => onColumnsChange(DEFAULT_PREFS)}
          >
            Reset widths and columns
          </button>
        </div>
      )}
    </div>
  );
}

function DateCell({ value, now }: { value: string | null; now: Date }) {
  if (value === null) return <td className="num muted">—</td>;
  return (
    <td className="num" title={new Date(value).toLocaleString()}>
      {formatAge(value, now)}
    </td>
  );
}

function ProjectCell({
  session,
  dir,
  homeDir,
  onOpenProject,
  onFilterProject,
}: {
  session: SessionRow;
  dir: string;
  homeDir: string;
  onOpenProject: (session: SessionRow) => void;
  onFilterProject: (project: string) => void;
}) {
  const chat = session.projectless;
  return (
    <div className="project-cell">
      <button
        type="button"
        className={chat ? "link no-project" : "link"}
        title={
          chat ? `No project. Open its chat folder, ${dir}` : `Open ${dir}`
        }
        aria-label={`Open ${chat ? "chat" : "project"} folder ${dir}`}
        onClick={() => onOpenProject(session)}
      >
        {chat ? "No project" : shortenPath(dir, homeDir)}
      </button>
      <button
        type="button"
        className="filter-project"
        title={
          chat
            ? "Show only sessions without a project"
            : "Show only this project"
        }
        aria-label={
          chat
            ? "Show only sessions without a project"
            : `Show only sessions in ${dir}`
        }
        onClick={() => onFilterProject(chat ? NO_PROJECT : dir)}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true">
          <path
            d="M2.5 3.5h11L9.25 8.5v4L6.75 13.5v-5z"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.4"
            strokeLinejoin="round"
          />
        </svg>
      </button>
    </div>
  );
}

export function SessionTable(props: Props) {
  const {
    rows,
    labelsById,
    onToggleExpand,
    homeDir,
    selected,
    now,
    sort,
    onSort,
    onToggle,
    onSetMany,
    onOpenProject,
    onFilterProject,
    onShowFiles,
    onOpenTranscript,
    columns,
    onColumnsChange,
  } = props;
  const [fit, setFit] = useState(true);
  const fitAgain = useCallback(() => setFit(true), []);
  const [available, measureContainer] = useContainerWidth(fitAgain);
  /** Dragging Session means the user picked its width, even if the table then overflows. */
  const changeColumns = useCallback(
    (change: SetStateAction<ColumnPrefs>, column: ColumnId) => {
      if (column === "session") setFit(false);
      onColumnsChange(change);
    },
    [onColumnsChange],
  );
  const selectable = rows
    .map((row) => row.session)
    .filter(isSelectable)
    .map((session) => session.uid);
  const allSelected =
    selectable.length > 0 && selectable.every((uid) => selected.has(uid));
  const shown = visibleColumns(columns);
  const widths = layoutWidths(
    columns,
    available,
    CHECK_WIDTH + ACTIONS_WIDTH,
    fit,
  );
  const widthOf = (column: ColumnDef) => widths.get(column.id)!;
  const tableWidth =
    CHECK_WIDTH +
    ACTIONS_WIDTH +
    shown.reduce((total, column) => total + widthOf(column), 0);

  if (rows.length === 0) {
    return <p className="empty">No sessions match these filters.</p>;
  }

  const cell = (column: ColumnDef, row: TreeRow) => {
    const { session, depth, childCount, expanded } = row;
    const label = session.title ?? session.id;
    switch (column.id) {
      case "session":
        return (
          <td
            key={column.id}
            className="title"
            style={{ paddingLeft: 10 + depth * 18 }}
          >
            <div className="title-cell">
              {childCount > 0 ? (
                <button
                  type="button"
                  className="expander"
                  aria-expanded={expanded}
                  aria-label={`${expanded ? "Hide" : "Show"} ${childCount} subagent ${childCount === 1 ? "session" : "sessions"} of ${label}`}
                  onClick={() => onToggleExpand(session.uid)}
                >
                  <svg viewBox="0 0 16 16" aria-hidden="true">
                    <path
                      d="M6 3.5 10.5 8 6 12.5"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.8"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                </button>
              ) : (
                <span className="expander-spacer" aria-hidden="true" />
              )}
              <button
                type="button"
                className="link title-link"
                title={`${label}\nRead transcript (${session.id})`}
                onClick={() => onOpenTranscript(session)}
              >
                {label}
              </button>
              <StatusBadges session={session} nested={depth > 0} />
              {childCount > 0 && (
                <button
                  type="button"
                  className="child-count"
                  tabIndex={-1}
                  aria-hidden="true"
                  onClick={() => onToggleExpand(session.uid)}
                >
                  {childCount} {childCount === 1 ? "subagent" : "subagents"}
                </button>
              )}
            </div>
          </td>
        );
      case "labels":
        return (
          <td
            key={column.id}
            className="labels"
            title={
              session.labelIds
                .map((id) => labelsById.get(id)?.name)
                .filter(Boolean)
                .join(", ") || undefined
            }
          >
            <LabelChips ids={session.labelIds} labelsById={labelsById} />
          </td>
        );
      case "agent":
        return (
          <td key={column.id} title={session.agentName}>
            {session.agentName}
          </td>
        );
      case "model": {
        const current = session.models[session.models.length - 1];
        return (
          <td key={column.id} title={session.models.join(", ") || undefined}>
            {current ?? <span className="muted">—</span>}
            {session.models.length > 1 && (
              <span className="muted"> +{session.models.length - 1}</span>
            )}
          </td>
        );
      }
      case "project":
        return (
          <td key={column.id} className="project">
            {session.projectDir ? (
              <ProjectCell
                session={session}
                dir={session.projectDir}
                homeDir={homeDir}
                onOpenProject={onOpenProject}
                onFilterProject={onFilterProject}
              />
            ) : (
              <span className="muted">—</span>
            )}
          </td>
        );
      case "created":
        return <DateCell key={column.id} value={session.created} now={now} />;
      case "lastActivity":
        return (
          <DateCell key={column.id} value={session.lastActivity} now={now} />
        );
      case "size":
        return (
          <td key={column.id} className="num">
            {formatBytes(session.sizeBytes)}
          </td>
        );
    }
  };

  return (
    <table
      className="sessions"
      style={{ width: tableWidth }}
      ref={measureContainer}
    >
      <colgroup>
        <col style={{ width: CHECK_WIDTH }} />
        {shown.map((column) => (
          <col key={column.id} style={{ width: widthOf(column) }} />
        ))}
        <col style={{ width: ACTIONS_WIDTH }} />
      </colgroup>
      <thead>
        <tr>
          <th className="check">
            <input
              type="checkbox"
              aria-label="Select all sessions shown"
              checked={allSelected}
              disabled={selectable.length === 0}
              onChange={() => onSetMany(selectable, !allSelected)}
            />
          </th>
          {shown.map((column) => {
            const sortKey = SORT_KEYS[column.id] ?? null;
            return (
              <th
                key={column.id}
                className={column.numeric ? "num" : undefined}
                aria-sort={ariaSortFor(sort, sortKey)}
                aria-label={sortKey ? undefined : column.label}
              >
                {sortKey ? (
                  <SortableHeader
                    label={column.label}
                    sortKey={sortKey}
                    sort={sort}
                    onSort={onSort}
                    numeric={column.numeric}
                  />
                ) : (
                  <span className="header-label">{column.label}</span>
                )}
                <ResizeHandle
                  column={column}
                  width={widthOf(column)}
                  onColumnsChange={(change) => changeColumns(change, column.id)}
                />
              </th>
            );
          })}
          <th className="actions-col">
            <span className="visually-hidden">Actions</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => {
          const { session, matched } = row;
          const label = session.title ?? session.id;
          const classes = [
            selected.has(session.uid) && "selected",
            !matched && "context-row",
          ]
            .filter(Boolean)
            .join(" ");
          return (
            <tr key={session.uid} className={classes || undefined}>
              <td className="check">
                <input
                  type="checkbox"
                  aria-label={`Select ${label}`}
                  checked={selected.has(session.uid)}
                  disabled={!isSelectable(session)}
                  onChange={(event) => {
                    const native = event.nativeEvent;
                    onToggle(
                      session.uid,
                      "shiftKey" in native && Boolean(native.shiftKey),
                    );
                  }}
                />
              </td>
              {shown.map((column) => cell(column, row))}
              <td className="actions-col">
                <button
                  type="button"
                  className="link"
                  aria-label={`Show files for ${label}`}
                  onClick={() => onShowFiles(session)}
                >
                  Files
                </button>
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
