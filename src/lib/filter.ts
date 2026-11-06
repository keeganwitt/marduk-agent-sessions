import { ageInDays } from "./format";
import type { SessionRow } from "./types";

export type ArchiveFilter = "all" | "active" | "archived";

/** "any", "none" (no labels), or a label id. */
export type LabelFilter = "any" | "none" | number;

export interface SessionFilter {
  agent: string;
  archive: ArchiveFilter;
  label: LabelFilter;
  olderThanDays: number | null;
  minSizeMb: number | null;
  query: string;
}

export const ALL_AGENTS = "all";

export const EMPTY_FILTER: SessionFilter = {
  agent: ALL_AGENTS,
  archive: "all",
  label: "any",
  olderThanDays: null,
  minSizeMb: null,
  query: "",
};

export function sessionMatcher(
  filter: SessionFilter,
  now: Date,
): (session: SessionRow) => boolean {
  const query = filter.query.trim().toLowerCase();
  const minBytes =
    filter.minSizeMb === null ? null : filter.minSizeMb * 1024 * 1024;
  return (session) =>
    (filter.agent === ALL_AGENTS || session.agent === filter.agent) &&
    (filter.archive === "all" ||
      session.archived === (filter.archive === "archived")) &&
    (filter.label === "any" ||
      (filter.label === "none"
        ? session.labelIds.length === 0
        : session.labelIds.includes(filter.label))) &&
    (filter.olderThanDays === null ||
      ageInDays(session.lastActivity, now) >= filter.olderThanDays) &&
    (minBytes === null || session.sizeBytes >= minBytes) &&
    (query === "" ||
      (session.title ?? "").toLowerCase().includes(query) ||
      (session.projectDir ?? "").toLowerCase().includes(query));
}

export function filterSessions(
  sessions: SessionRow[],
  filter: SessionFilter,
  now: Date,
): SessionRow[] {
  return sessions.filter(sessionMatcher(filter, now));
}
