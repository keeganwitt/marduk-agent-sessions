import { ageInDays, tildePath } from "./format";
import type { SessionRow } from "./types";

export type ArchiveFilter = "all" | "active" | "archived";

/** "any", "none" (no labels), or a label id. */
export type LabelFilter = "any" | "none" | number;

export interface SessionFilter {
  agent: string;
  archive: ArchiveFilter;
  label: LabelFilter;
  /** A project folder; its subfolders match too. */
  project: string | null;
  olderThanDays: number | null;
  minSizeMb: number | null;
  query: string;
}

export const ALL_AGENTS = "all";

export const EMPTY_FILTER: SessionFilter = {
  agent: ALL_AGENTS,
  archive: "all",
  label: "any",
  project: null,
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
    (filter.project === null ||
      inProject(session.projectDir, filter.project)) &&
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

/** True when `dir` is `project` or one of its subfolders. */
export function inProject(dir: string | null, project: string): boolean {
  if (dir === null) return false;
  const base = project.replace(/[\\/]+$/, "");
  return (
    dir === base || dir.startsWith(`${base}/`) || dir.startsWith(`${base}\\`)
  );
}

export interface ProjectOption {
  dir: string;
  count: number;
}

/** Every project folder with its session count, the most used first. */
export function projectOptions(sessions: SessionRow[]): ProjectOption[] {
  const counts = new Map<string, number>();
  for (const session of sessions) {
    if (session.projectDir) {
      counts.set(session.projectDir, (counts.get(session.projectDir) ?? 0) + 1);
    }
  }
  return [...counts]
    .map(([dir, count]) => ({ dir, count }))
    .sort((a, b) => b.count - a.count || a.dir.localeCompare(b.dir));
}

/** Projects whose path, in full or with `~` for home, contains `text`. */
export function matchProjects(
  options: ProjectOption[],
  text: string,
  homeDir: string,
): ProjectOption[] {
  const needle = text.trim().toLowerCase();
  if (needle === "") return options;
  return options.filter(
    ({ dir }) =>
      dir.toLowerCase().includes(needle) ||
      tildePath(dir, homeDir).toLowerCase().includes(needle),
  );
}
