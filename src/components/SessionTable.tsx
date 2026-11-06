import { formatAge, formatBytes } from "../lib/format";
import { isDeletable, type SessionRow } from "../lib/types";

interface Props {
  sessions: SessionRow[];
  selected: ReadonlySet<string>;
  now: Date;
  onToggle: (uid: string) => void;
  onSetMany: (uids: string[], selected: boolean) => void;
}

function StatusBadges({ session }: { session: SessionRow }) {
  return (
    <>
      {session.running && (
        <span className="badge running" title="Written to in the last few minutes, so it can't be deleted.">
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

export function SessionTable({ sessions, selected, now, onToggle, onSetMany }: Props) {
  const deletable = sessions.filter(isDeletable).map((session) => session.uid);
  const allSelected = deletable.length > 0 && deletable.every((uid) => selected.has(uid));

  if (sessions.length === 0) {
    return <p className="empty">No sessions match these filters.</p>;
  }

  return (
    <table className="sessions">
      <thead>
        <tr>
          <th className="check">
            <input
              type="checkbox"
              aria-label="Select all deletable sessions shown"
              checked={allSelected}
              disabled={deletable.length === 0}
              onChange={() => onSetMany(deletable, !allSelected)}
            />
          </th>
          <th>Session</th>
          <th>Agent</th>
          <th>Project</th>
          <th className="num">Last activity</th>
          <th className="num">Size</th>
        </tr>
      </thead>
      <tbody>
        {sessions.map((session) => {
          const canDelete = isDeletable(session);
          const label = session.title ?? session.id;
          return (
            <tr key={session.uid} className={selected.has(session.uid) ? "selected" : undefined}>
              <td className="check">
                <input
                  type="checkbox"
                  aria-label={`Select ${label}`}
                  checked={selected.has(session.uid)}
                  disabled={!canDelete}
                  onChange={() => onToggle(session.uid)}
                />
              </td>
              <td className="title">
                <span title={session.id}>{label}</span> <StatusBadges session={session} />
              </td>
              <td>{session.agentName}</td>
              <td className="project" title={session.projectDir ?? undefined}>
                {session.projectDir ?? "—"}
              </td>
              <td className="num" title={new Date(session.lastActivity).toLocaleString()}>
                {formatAge(session.lastActivity, now)}
              </td>
              <td className="num">{formatBytes(session.sizeBytes)}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
