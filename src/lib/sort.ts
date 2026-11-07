import type { SessionRow } from "./types";

export type SortKey =
  "title" | "agent" | "model" | "project" | "created" | "lastActivity" | "size";
export type SortDirection = "asc" | "desc";

export interface SortState {
  key: SortKey;
  direction: SortDirection;
}

const NEWEST_OR_LARGEST_FIRST: ReadonlySet<SortKey> = new Set([
  "created",
  "lastActivity",
  "size",
]);

function firstDirection(key: SortKey): SortDirection {
  return NEWEST_OR_LARGEST_FIRST.has(key) ? "desc" : "asc";
}

/** The order sessions are shown in until the user picks another column. */
export const DEFAULT_SORT: SortState = {
  key: "lastActivity",
  direction: "desc",
};

/**
 * Clicking a column sorts by it, clicking again reverses, and a third click returns to the
 * default order. The default column itself just switches direction.
 */
export function nextSort(current: SortState, key: SortKey): SortState {
  const first = firstDirection(key);
  if (current.key !== key) return { key, direction: first };
  if (current.direction === first)
    return { key, direction: first === "asc" ? "desc" : "asc" };
  return key === DEFAULT_SORT.key ? { key, direction: first } : DEFAULT_SORT;
}

function sortValue(session: SessionRow, key: SortKey): string | number | null {
  switch (key) {
    case "title":
      return session.title ?? session.id;
    case "agent":
      return session.agentName;
    case "model":
      return session.models[session.models.length - 1] ?? null;
    case "project":
      return session.projectDir;
    case "created":
      return session.created === null ? null : Date.parse(session.created);
    case "lastActivity":
      return Date.parse(session.lastActivity);
    case "size":
      return session.sizeBytes;
  }
}

const collator = new Intl.Collator(undefined, {
  sensitivity: "base",
  numeric: true,
});

/** Missing values always sort last, whichever direction is chosen. */
export function sortSessions(
  sessions: SessionRow[],
  sort: SortState | null,
): SessionRow[] {
  if (!sort) return sessions;
  const factor = sort.direction === "asc" ? 1 : -1;
  return [...sessions].sort((a, b) => {
    const left = sortValue(a, sort.key);
    const right = sortValue(b, sort.key);
    if (left === null || right === null)
      return left === right ? 0 : left === null ? 1 : -1;
    const order =
      typeof left === "number" && typeof right === "number"
        ? left - right
        : collator.compare(String(left), String(right));
    return order * factor;
  });
}
