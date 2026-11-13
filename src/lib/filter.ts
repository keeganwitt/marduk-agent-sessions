import { ageInDays, tildePath } from "./format";
import type { SessionRow } from "./types";

export type ArchiveFilter = "all" | "active" | "archived";

/** "any", "none" (no labels), or a label id. */
export type LabelFilter = "any" | "none" | number;

/** "without" means standalone: no subagents of its own, and not nested under a parent. */
export type SubagentFilter = "any" | "with" | "without";

export interface SessionFilter {
  agent: string;
  /** A model the session used at any point, or `ALL_MODELS`. */
  model: string;
  /** The agent definition the session ran as, or `ALL_AGENT_TYPES`. */
  agentType: string;
  archive: ArchiveFilter;
  subagents: SubagentFilter;
  parentMissing: boolean;
  empty: boolean;
  label: LabelFilter;
  /** A project folder (its subfolders match too), or `NO_PROJECT`. */
  project: string | null;
  olderThanDays: number | null;
  minSizeMb: number | null;
  query: string;
}

export const ALL_AGENTS = "all";
export const ALL_MODELS = "all";
export const ALL_AGENT_TYPES = "all";

/** Sessions started without a project, or with no folder at all. Never a real path. */
export const NO_PROJECT = "<no project>";

export const EMPTY_FILTER: SessionFilter = {
  agent: ALL_AGENTS,
  model: ALL_MODELS,
  agentType: ALL_AGENT_TYPES,
  archive: "all",
  subagents: "any",
  parentMissing: false,
  empty: false,
  label: "any",
  project: null,
  olderThanDays: null,
  minSizeMb: null,
  query: "",
};

export function sessionMatcher(
  filter: SessionFilter,
  now: Date,
  sessions: readonly SessionRow[],
): (session: SessionRow) => boolean {
  const query = filter.query.trim().toLowerCase();
  const subagentMatches = subagentMatcher(filter.subagents, sessions);
  const minBytes =
    filter.minSizeMb === null ? null : filter.minSizeMb * 1024 * 1024;
  return (session) =>
    (filter.agent === ALL_AGENTS || session.agent === filter.agent) &&
    (filter.model === ALL_MODELS || session.models.includes(filter.model)) &&
    (filter.agentType === ALL_AGENT_TYPES ||
      session.agentType === filter.agentType) &&
    (filter.archive === "all" ||
      session.archived === (filter.archive === "archived")) &&
    subagentMatches(session) &&
    (!filter.parentMissing ||
      Boolean(session.parentId && session.parentMissing)) &&
    (!filter.empty || session.empty) &&
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
  return sessions.filter(sessionMatcher(filter, now, sessions));
}

function sessionKey(agent: string, id: string): string {
  return `${agent}:${id}`;
}

/** Keys of sessions that started a subagent present in the scan. */
function parentKeys(sessions: readonly SessionRow[]): Set<string> {
  const present = new Set(sessions.map((s) => sessionKey(s.agent, s.id)));
  const parents = new Set<string>();
  for (const session of sessions) {
    if (session.parentId === null || session.parentId === session.id) continue;
    const key = sessionKey(session.agent, session.parentId);
    if (present.has(key)) parents.add(key);
  }
  return parents;
}

export function hasSubagents(sessions: readonly SessionRow[]): boolean {
  return parentKeys(sessions).size > 0;
}

function subagentMatcher(
  subagents: SubagentFilter,
  sessions: readonly SessionRow[],
): (session: SessionRow) => boolean {
  if (subagents === "any") return () => true;
  const parents = parentKeys(sessions);
  const isParent = (session: SessionRow) =>
    parents.has(sessionKey(session.agent, session.id));
  if (subagents === "with") return isParent;
  const nested = (session: SessionRow) =>
    session.parentId !== null &&
    session.parentId !== session.id &&
    !session.parentMissing &&
    parents.has(sessionKey(session.agent, session.parentId));
  return (session) => !isParent(session) && !nested(session);
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

/** Every agent type sessions ran as, with how many did, the most common first. */
export function agentTypeOptions(
  sessions: SessionRow[],
): { agentType: string; count: number }[] {
  const counts = new Map<string, number>();
  for (const { agentType } of sessions) {
    if (agentType) counts.set(agentType, (counts.get(agentType) ?? 0) + 1);
  }
  return [...counts]
    .map(([agentType, count]) => ({ agentType, count }))
    .sort(
      (a, b) => b.count - a.count || a.agentType.localeCompare(b.agentType),
    );
}

/** Every filter the bar can show, in the order it shows them. */
export const FILTERS = [
  "agent",
  "olderThanDays",
  "minSizeMb",
  "label",
  "project",
  "archive",
  "model",
  "agentType",
  "subagents",
  "parentMissing",
  "empty",
] as const;
export type FilterId = (typeof FILTERS)[number];

/** Shown until someone removes them; the rest are added with "+ Filter". */
export const DEFAULT_FILTERS: readonly FilterId[] = [
  "agent",
  "olderThanDays",
  "minSizeMb",
  "label",
  "project",
];

/** Whether a filter is narrowing the list. */
export function isFilterActive(filter: SessionFilter, id: FilterId): boolean {
  return filter[id] !== EMPTY_FILTER[id];
}

/** Filters to show: those chosen, plus any narrowing the list, so none hide. */
export function shownFilters(
  chosen: readonly FilterId[],
  filter: SessionFilter,
): FilterId[] {
  return FILTERS.filter(
    (id) => chosen.includes(id) || isFilterActive(filter, id),
  );
}

export function isDefaultFilterSet(chosen: readonly FilterId[]): boolean {
  return (
    chosen.length === DEFAULT_FILTERS.length &&
    DEFAULT_FILTERS.every((id) => chosen.includes(id))
  );
}

/**
 * The saved choice of filters. Before every filter was removable, only the optional
 * filters someone added were saved (`legacyAdded`); the defaults were always shown.
 */
export function restoreChosenFilters(
  saved: unknown,
  legacyAdded: unknown,
): FilterId[] {
  if (Array.isArray(saved)) return FILTERS.filter((id) => saved.includes(id));
  const added = Array.isArray(legacyAdded) ? legacyAdded : [];
  return FILTERS.filter(
    (id) => DEFAULT_FILTERS.includes(id) || added.includes(id),
  );
}

/** Whether any filter (not the search box, which clears on its own) is narrowing the list. */
export function isFiltering(filter: SessionFilter): boolean {
  return (Object.keys(EMPTY_FILTER) as (keyof SessionFilter)[]).some(
    (key) => key !== "query" && filter[key] !== EMPTY_FILTER[key],
  );
}
