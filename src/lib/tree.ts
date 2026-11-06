import { sortSessions, type SortState } from "./sort";
import type { SessionRow } from "./types";

export interface TreeRow {
  session: SessionRow;
  depth: number;
  /** Direct children that are shown when this row is expanded. */
  childCount: number;
  expanded: boolean;
  /** False for a parent shown only because one of its descendants matches the filters. */
  matched: boolean;
}

function nodeKey(agent: string, id: string): string {
  return `${agent}:${id}`;
}

/**
 * Nests subagent sessions under their parent. Sessions without a parent, and subagents whose
 * parent is gone, are top-level. A parent that doesn't match `matches` itself is still shown
 * (and opened) when one of its descendants does.
 */
export function buildTree(
  sessions: SessionRow[],
  matches: (session: SessionRow) => boolean,
  sort: SortState | null,
  expanded: ReadonlySet<string>,
): TreeRow[] {
  const byKey = new Map<string, SessionRow>();
  for (const session of sessions) {
    const key = nodeKey(session.agent, session.id);
    if (!byKey.has(key)) byKey.set(key, session);
  }
  const parentOf = (session: SessionRow): SessionRow | undefined =>
    session.parentId === null || session.parentMissing
      ? undefined
      : byKey.get(nodeKey(session.agent, session.parentId));
  // A parent chain that loops back on itself would otherwise hide every session in the loop.
  const inCycle = (session: SessionRow): boolean => {
    const seen = new Set<SessionRow>([session]);
    for (
      let current = parentOf(session);
      current;
      current = parentOf(current)
    ) {
      if (seen.has(current)) return true;
      seen.add(current);
    }
    return false;
  };
  const children = new Map<string, SessionRow[]>();
  const roots: SessionRow[] = [];
  for (const session of sessions) {
    const parentKey =
      session.parentId === null
        ? null
        : nodeKey(session.agent, session.parentId);
    const parent = parentOf(session);
    if (!parent || parent === session || inCycle(session)) {
      roots.push(session);
    } else {
      const siblings = children.get(parentKey!) ?? [];
      siblings.push(session);
      children.set(parentKey!, siblings);
    }
  }

  const matchCache = new Map<SessionRow, boolean>();
  const subtreeMatches = (
    session: SessionRow,
    path: Set<SessionRow>,
  ): boolean => {
    const cached = matchCache.get(session);
    if (cached !== undefined) return cached;
    if (path.has(session)) return false;
    path.add(session);
    const result =
      matches(session) ||
      (children.get(nodeKey(session.agent, session.id)) ?? []).some((child) =>
        subtreeMatches(child, path),
      );
    path.delete(session);
    matchCache.set(session, result);
    return result;
  };

  const rows: TreeRow[] = [];
  const visit = (
    siblings: SessionRow[],
    depth: number,
    path: Set<SessionRow>,
  ) => {
    const shown = sortSessions(
      siblings.filter(
        (session) => !path.has(session) && subtreeMatches(session, new Set()),
      ),
      sort,
    );
    for (const session of shown) {
      const own = matches(session);
      const kids = (
        children.get(nodeKey(session.agent, session.id)) ?? []
      ).filter((child) => !path.has(child) && subtreeMatches(child, new Set()));
      const open = kids.length > 0 && (expanded.has(session.uid) || !own);
      rows.push({
        session,
        depth,
        childCount: kids.length,
        expanded: open,
        matched: own,
      });
      if (open) {
        path.add(session);
        visit(kids, depth + 1, path);
        path.delete(session);
      }
    }
  };
  visit(roots, 0, new Set());
  return rows;
}
