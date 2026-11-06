import { ageInDays } from "./format";
import type { SessionRow } from "./types";

export type ArchiveFilter = "all" | "active" | "archived";

export interface SessionFilter {
  agent: string;
  archive: ArchiveFilter;
  showSubagents: boolean;
  olderThanDays: number | null;
  minSizeMb: number | null;
  query: string;
}

export const ALL_AGENTS = "all";

export const EMPTY_FILTER: SessionFilter = {
  agent: ALL_AGENTS,
  archive: "all",
  showSubagents: false,
  olderThanDays: null,
  minSizeMb: null,
  query: "",
};

export function filterSessions(sessions: SessionRow[], filter: SessionFilter, now: Date): SessionRow[] {
  const query = filter.query.trim().toLowerCase();
  const minBytes = filter.minSizeMb === null ? null : filter.minSizeMb * 1024 * 1024;
  return sessions.filter(
    (session) =>
      (filter.agent === ALL_AGENTS || session.agent === filter.agent) &&
      (filter.archive === "all" || session.archived === (filter.archive === "archived")) &&
      (filter.showSubagents || session.parentId === null) &&
      (filter.olderThanDays === null || ageInDays(session.lastActivity, now) >= filter.olderThanDays) &&
      (minBytes === null || session.sizeBytes >= minBytes) &&
      (query === "" ||
        (session.title ?? "").toLowerCase().includes(query) ||
        (session.projectDir ?? "").toLowerCase().includes(query)),
  );
}
