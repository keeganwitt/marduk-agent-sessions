import type { SessionRow } from "./types";

export type SortKey = "title" | "agent" | "project" | "created" | "lastActivity" | "size";
export type SortDirection = "asc" | "desc";

export interface SortState {
  key: SortKey;
  direction: SortDirection;
}

const NEWEST_OR_LARGEST_FIRST: ReadonlySet<SortKey> = new Set(["created", "lastActivity", "size"]);

function firstDirection(key: SortKey): SortDirection {
  return NEWEST_OR_LARGEST_FIRST.has(key) ? "desc" : "asc";
}

/** Clicking a column sorts by it, clicking again reverses, and a third click clears the sort. */
export function nextSort(current: SortState | null, key: SortKey): SortState | null {
  const first = firstDirection(key);
  if (current?.key !== key) return { key, direction: first };
  if (current.direction === first) return { key, direction: first === "asc" ? "desc" : "asc" };
  return null;
}

function sortValue(session: SessionRow, key: SortKey): string | number | null {
  switch (key) {
    case "title":
      return session.title ?? session.id;
    case "agent":
      return session.agentName;
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

const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

/** Missing values always sort last, whichever direction is chosen. */
export function sortSessions(sessions: SessionRow[], sort: SortState | null): SessionRow[] {
  if (!sort) return sessions;
  const factor = sort.direction === "asc" ? 1 : -1;
  return [...sessions].sort((a, b) => {
    const left = sortValue(a, sort.key);
    const right = sortValue(b, sort.key);
    if (left === null || right === null) return left === right ? 0 : left === null ? 1 : -1;
    const order = typeof left === "number" && typeof right === "number" ? left - right : collator.compare(String(left), String(right));
    return order * factor;
  });
}
