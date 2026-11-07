import { ageInDays, tildePath } from "./format";
import type { SessionRow } from "./types";

export type ArchiveFilter = "all" | "active" | "archived";

/** "any", "none" (no labels), or a label id. */
export type LabelFilter = "any" | "none" | number;

export interface SessionFilter {
  agent: string;
  /** A model the session used at any point, or `ALL_MODELS`. */
  model: string;
  archive: ArchiveFilter;
  label: LabelFilter;
  /** A project folder (its subfolders match too), or `NO_PROJECT`. */
  project: string | null;
  olderThanDays: number | null;
  minSizeMb: number | null;
  query: string;
}

export const ALL_AGENTS = "all";
export const ALL_MODELS = "all";

/** Sessions started without a project, or with no folder at all. Never a real path. */
export const NO_PROJECT = "<no project>";

export const EMPTY_FILTER: SessionFilter = {
  agent: ALL_AGENTS,
  model: ALL_MODELS,
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
    (filter.model === ALL_MODELS || session.models.includes(filter.model)) &&
    (filter.archive === "all" ||
      session.archived === (filter.archive === "archived")) &&
    (filter.label === "any" ||
      (filter.label === "none"
        ? session.labelIds.length === 0
        : session.labelIds.includes(filter.label))) &&
    (filter.project === null || inProjectFilter(session, filter.project)) &&
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

function inProjectFilter(session: SessionRow, project: string): boolean {
  return project === NO_PROJECT
    ? hasNoProject(session)
    : !session.projectless && inProject(session.projectDir, project);
}

export function hasNoProject(session: SessionRow): boolean {
  return session.projectless || session.projectDir === null;
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

/**
 * Every project folder with its session count, the most used first. Sessions without a
 * project are one `NO_PROJECT` option, rather than one folder per chat.
 */
export function projectOptions(sessions: SessionRow[]): ProjectOption[] {
  const counts = new Map<string, number>();
  for (const session of sessions) {
    const dir = hasNoProject(session) ? NO_PROJECT : session.projectDir!;
    counts.set(dir, (counts.get(dir) ?? 0) + 1);
  }
  return [...counts]
    .map(([dir, count]) => ({ dir, count }))
    .sort((a, b) => b.count - a.count || a.dir.localeCompare(b.dir));
}

/** How a project option or filter reads in the UI. */
export function projectLabel(dir: string, homeDir: string): string {
  return dir === NO_PROJECT ? "No project" : tildePath(dir, homeDir);
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
      (dir !== NO_PROJECT && dir.toLowerCase().includes(needle)) ||
      projectLabel(dir, homeDir).toLowerCase().includes(needle),
  );
}

/** Every model sessions used, with how many sessions used it, the most used first. */
export function modelOptions(
  sessions: SessionRow[],
): { model: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const session of sessions) {
    for (const model of session.models) {
      counts.set(model, (counts.get(model) ?? 0) + 1);
    }
  }
  return [...counts]
    .map(([model, count]) => ({ model, count }))
    .sort((a, b) => b.count - a.count || a.model.localeCompare(b.model));
}
