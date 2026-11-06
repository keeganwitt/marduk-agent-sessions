import { formatAge, formatBytes, shortenPath } from "../lib/format";
import type { SortKey, SortState } from "../lib/sort";
import type { TreeRow } from "../lib/tree";
import { isSelectable, type SessionRow } from "../lib/types";

interface Props {
  rows: TreeRow[];
  onToggleExpand: (uid: string) => void;
  homeDir: string;
  selected: ReadonlySet<string>;
  now: Date;
  sort: SortState | null;
  onSort: (key: SortKey) => void;
  onToggle: (uid: string) => void;
  onSetMany: (uids: string[], selected: boolean) => void;
  onOpenProject: (session: SessionRow) => void;
  onShowFiles: (session: SessionRow) => void;
  onOpenTranscript: (session: SessionRow) => void;
}

function StatusBadges({ session }: { session: SessionRow }) {
  return (
    <>
      {session.running && (
        <span className="badge running" title="Written to in the last few minutes, so it can't be changed.">
          Running
        </span>
      )}
      {session.parentId && session.parentMissing && (
        <span className="badge warn" title={`Started by session ${session.parentId}, which no longer exists`}>
          Parent deleted
        </span>
      )}
      {session.parentId && !session.parentMissing && (
        <span className="badge muted" title={`Started by session ${session.parentId}`}>
          Subagent
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
  const { rows, onToggleExpand, homeDir, selected, now, sort, onSort, onToggle, onSetMany, onOpenProject, onShowFiles, onOpenTranscript } =
    props;
  const selectable = rows.map((row) => row.session).filter(isSelectable).map((session) => session.uid);
  const allSelected = selectable.length > 0 && selectable.every((uid) => selected.has(uid));

  if (rows.length === 0) {
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
        {rows.map(({ session, depth, childCount, expanded, matched }) => {
          const label = session.title ?? session.id;
          const classes = [selected.has(session.uid) && "selected", !matched && "context-row"].filter(Boolean).join(" ");
          return (
            <tr key={session.uid} className={classes || undefined}>
              <td className="check">
                <input
                  type="checkbox"
                  aria-label={`Select ${label}`}
                  checked={selected.has(session.uid)}
                  disabled={!isSelectable(session)}
                  onChange={() => onToggle(session.uid)}
                />
              </td>
              <td className="title" style={{ paddingLeft: 10 + depth * 18 }}>
                {childCount > 0 ? (
                  <button
                    type="button"
                    className="expander"
                    aria-expanded={expanded}
                    aria-label={`${expanded ? "Hide" : "Show"} ${childCount} subagent ${childCount === 1 ? "session" : "sessions"} of ${label}`}
                    onClick={() => onToggleExpand(session.uid)}
                  >
                    {expanded ? "▾" : "▸"}
                  </button>
                ) : (
                  <span className="expander-spacer" aria-hidden="true" />
                )}
                <button
                  type="button"
                  className="link title-link"
                  title={`Read transcript (${session.id})`}
                  onClick={() => onOpenTranscript(session)}
                >
                  {label}
                </button>{" "}
                <StatusBadges session={session} />
                {childCount > 0 && !expanded && (
                  <span className="badge muted">
                    {childCount} {childCount === 1 ? "subagent" : "subagents"}
                  </span>
                )}
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
                    {shortenPath(session.projectDir, homeDir)}
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
