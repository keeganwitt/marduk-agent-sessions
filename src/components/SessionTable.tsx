import { formatAge, formatBytes } from "../lib/format";
import type { SortKey, SortState } from "../lib/sort";
import { isSelectable, type SessionRow } from "../lib/types";

interface Props {
  sessions: SessionRow[];
  selected: ReadonlySet<string>;
  now: Date;
  sort: SortState | null;
  onSort: (key: SortKey) => void;
  onToggle: (uid: string) => void;
  onSetMany: (uids: string[], selected: boolean) => void;
  onOpenProject: (session: SessionRow) => void;
  onShowFiles: (session: SessionRow) => void;
}

function StatusBadges({ session }: { session: SessionRow }) {
  return (
    <>
      {session.running && (
        <span className="badge running" title="Written to in the last few minutes, so it can't be changed.">
          Running
        </span>
      )}
      {session.archived && <span className="badge">Archived</span>}
      {session.readOnlyReason && (
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
  const ariaSort = direction === "asc" ? "ascending" : direction === "desc" ? "descending" : "none";
  return (
    <th className={numeric ? "num" : undefined} aria-sort={ariaSort}>
      <button type="button" className="sort" onClick={() => onSort(sortKey)}>
        {label}
        <span className="sort-indicator" aria-hidden="true">
          {direction === "asc" ? "▲" : direction === "desc" ? "▼" : ""}
        </span>
      </button>
    </th>
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

export function SessionTable(props: Props) {
  const { sessions, selected, now, sort, onSort, onToggle, onSetMany, onOpenProject, onShowFiles } = props;
  const selectable = sessions.filter(isSelectable).map((session) => session.uid);
  const allSelected = selectable.length > 0 && selectable.every((uid) => selected.has(uid));

  if (sessions.length === 0) {
    return <p className="empty">No sessions match these filters.</p>;
  }

  const header = (label: string, sortKey: SortKey, numeric = false) => (
    <SortableHeader label={label} sortKey={sortKey} sort={sort} onSort={onSort} numeric={numeric} />
  );

  return (
    <table className="sessions">
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
          {header("Session", "title")}
          {header("Agent", "agent")}
          {header("Project", "project")}
          {header("Created", "created", true)}
          {header("Last activity", "lastActivity", true)}
          {header("Size", "size", true)}
          <th className="actions-col">
            <span className="visually-hidden">Actions</span>
          </th>
        </tr>
      </thead>
      <tbody>
        {sessions.map((session) => {
          const label = session.title ?? session.id;
          return (
            <tr key={session.uid} className={selected.has(session.uid) ? "selected" : undefined}>
              <td className="check">
                <input
                  type="checkbox"
                  aria-label={`Select ${label}`}
                  checked={selected.has(session.uid)}
                  disabled={!isSelectable(session)}
                  onChange={() => onToggle(session.uid)}
                />
              </td>
              <td className="title">
                <span title={session.id}>{label}</span> <StatusBadges session={session} />
              </td>
              <td>{session.agentName}</td>
              <td className="project">
                {session.projectDir ? (
                  <button
                    type="button"
                    className="link"
                    title={`Open ${session.projectDir}`}
                    aria-label={`Open project folder ${session.projectDir}`}
                    onClick={() => onOpenProject(session)}
                  >
                    {session.projectDir}
                  </button>
                ) : (
                  <span className="muted">—</span>
                )}
              </td>
              <DateCell value={session.created} now={now} />
              <DateCell value={session.lastActivity} now={now} />
              <td className="num">{formatBytes(session.sizeBytes)}</td>
              <td className="actions-col">
                <button type="button" className="link" aria-label={`Show files for ${label}`} onClick={() => onShowFiles(session)}>
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
