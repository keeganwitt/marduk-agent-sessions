import { describe, expect, it } from "vitest";
import {
  EMPTY_FILTER,
  filterSessions,
  inProject,
  agentTypeOptions,
  isFiltering,
  matchProjects,
  modelOptions,
  NO_PROJECT,
  projectOptions,
  shownOptionalFilters,
} from "./filter";
import { makeSession } from "../test/factories";

const now = new Date("2026-11-06T12:00:00Z");

describe("filterSessions", () => {
  const old = makeSession({
    uid: "old",
    agent: "claude",
    lastActivity: "2026-09-01T00:00:00Z",
    sizeBytes: 5 * 1024 * 1024,
  });
  const recent = makeSession({
    uid: "recent",
    agent: "pi",
    lastActivity: "2026-11-05T00:00:00Z",
    sizeBytes: 100,
    title: "Fix Login",
  });
  const sessions = [old, recent];
  const uids = (filter = EMPTY_FILTER) =>
    filterSessions(sessions, filter, now).map((s) => s.uid);

  it("returns everything with the empty filter", () => {
    expect(uids()).toEqual(["old", "recent"]);
  });

  it("filters by agent", () => {
    expect(uids({ ...EMPTY_FILTER, agent: "pi" })).toEqual(["recent"]);
  });

  it("filters by archive state", () => {
    const archived = makeSession({ uid: "arch", archived: true });
    const archivedUids = (archive: "active" | "archived") =>
      filterSessions([old, archived], { ...EMPTY_FILTER, archive }, now).map(
        (s) => s.uid,
      );
    expect(archivedUids("archived")).toEqual(["arch"]);
    expect(archivedUids("active")).toEqual(["old"]);
  });

  it("filters by label, or by having no labels", () => {
    const labeled = makeSession({ uid: "labeled", labelIds: [7] });
    const uidsFor = (label: "none" | number) =>
      filterSessions([old, labeled], { ...EMPTY_FILTER, label }, now).map(
        (s) => s.uid,
      );
    expect(uidsFor(7)).toEqual(["labeled"]);
    expect(uidsFor("none")).toEqual(["old"]);
    expect(uidsFor(8)).toEqual([]);
  });

  it("filters by minimum age in days", () => {
    expect(uids({ ...EMPTY_FILTER, olderThanDays: 30 })).toEqual(["old"]);
  });

  it("filters by minimum size", () => {
    expect(uids({ ...EMPTY_FILTER, minSizeMb: 1 })).toEqual(["old"]);
  });

  it("matches title or project case-insensitively", () => {
    expect(uids({ ...EMPTY_FILTER, query: "login" })).toEqual(["recent"]);
    expect(uids({ ...EMPTY_FILTER, query: "/WORK" })).toEqual([
      "old",
      "recent",
    ]);
  });
});

describe("project filter", () => {
  it("matches the project and its subfolders, not folders that share a prefix", () => {
    expect(inProject("/work/app", "/work/app")).toBe(true);
    expect(inProject("/work/app/web", "/work/app/")).toBe(true);
    expect(inProject("/work/application", "/work/app")).toBe(false);
    expect(inProject("C:\\work\\app\\web", "C:\\work\\app")).toBe(true);
    expect(inProject(null, "/work/app")).toBe(false);
  });

  it("filters sessions to a project", () => {
    const sessions = [
      makeSession({ uid: "a", projectDir: "/work/app" }),
      makeSession({ uid: "b", projectDir: "/work/application" }),
      makeSession({ uid: "c", projectDir: null }),
    ];

    const shown = filterSessions(
      sessions,
      { ...EMPTY_FILTER, project: "/work/app" },
      now,
    );

    expect(shown.map((s) => s.uid)).toEqual(["a"]);
  });

  it("lists projects by session count, then by path, with chats as one entry", () => {
    const sessions = [
      makeSession({ uid: "1", projectDir: "/b" }),
      makeSession({ uid: "2", projectDir: "/a" }),
      makeSession({ uid: "3", projectDir: "/c" }),
      makeSession({ uid: "4", projectDir: "/c" }),
      makeSession({
        uid: "5",
        projectDir: "/chats/2026-11-06/tel",
        projectless: true,
      }),
      makeSession({
        uid: "6",
        projectDir: "/chats/2026-11-05/x20",
        projectless: true,
      }),
      makeSession({ uid: "7", projectDir: null }),
    ];

    expect(projectOptions(sessions)).toEqual([
      { dir: NO_PROJECT, count: 3 },
      { dir: "/c", count: 2 },
      { dir: "/a", count: 1 },
      { dir: "/b", count: 1 },
    ]);
  });

  it("filters to sessions without a project", () => {
    const sessions = [
      makeSession({
        uid: "chat",
        projectDir: "/chats/2026-11-06/tel",
        projectless: true,
      }),
      makeSession({ uid: "none", projectDir: null }),
      makeSession({ uid: "repo", projectDir: "/chats" }),
    ];
    const shown = (project: string) =>
      filterSessions(sessions, { ...EMPTY_FILTER, project }, now).map(
        (s) => s.uid,
      );

    expect(shown(NO_PROJECT)).toEqual(["chat", "none"]);
    expect(shown("/chats")).toEqual(["repo"]);
  });

  it("suggests projects by full path or by the path shown with ~", () => {
    const options = [
      { dir: "/Users/me/Projects/api", count: 3 },
      { dir: "/srv/web", count: 1 },
    ];

    expect(matchProjects(options, "~/proj", "/Users/me")).toEqual([options[0]]);
    expect(matchProjects(options, "SRV", "/Users/me")).toEqual([options[1]]);
    expect(matchProjects(options, "  ", "/Users/me")).toEqual(options);
    const withChats = [{ dir: NO_PROJECT, count: 9 }, ...options];
    expect(matchProjects(withChats, "no proj", "/Users/me")).toEqual([
      withChats[0],
    ]);
    expect(matchProjects(withChats, "<", "/Users/me")).toEqual([]);
  });
});

describe("model filter", () => {
  const sessions = [
    makeSession({ uid: "a", models: ["claude-opus-5", "claude-opus-5-5"] }),
    makeSession({ uid: "b", models: ["claude-opus-5-5"] }),
    makeSession({ uid: "c", models: [] }),
  ];

  it("matches a model the session used at any point", () => {
    const shown = (model: string) =>
      filterSessions(sessions, { ...EMPTY_FILTER, model }, now).map(
        (s) => s.uid,
      );

    expect(shown("claude-opus-5")).toEqual(["a"]);
    expect(shown("claude-opus-5-5")).toEqual(["a", "b"]);
  });

  it("lists models by how many sessions used them", () => {
    expect(modelOptions(sessions)).toEqual([
      { model: "claude-opus-5-5", count: 2 },
      { model: "claude-opus-5", count: 1 },
    ]);
  });
});

describe("agent type filter", () => {
  const sessions = [
    makeSession({ uid: "a", agentType: "Explore" }),
    makeSession({ uid: "b", agentType: "Explore" }),
    makeSession({ uid: "c", agentType: "architect" }),
    makeSession({ uid: "d", agentType: null }),
  ];

  it("matches the agent type a session ran as", () => {
    const shown = filterSessions(
      sessions,
      { ...EMPTY_FILTER, agentType: "architect" },
      now,
    );
    expect(shown.map((s) => s.uid)).toEqual(["c"]);
  });

  it("lists agent types by how many sessions ran as them", () => {
    expect(agentTypeOptions(sessions)).toEqual([
      { agentType: "Explore", count: 2 },
      { agentType: "architect", count: 1 },
    ]);
  });
});

describe("optional filters", () => {
  it("shows those added and any narrowing the list, in a fixed order", () => {
    expect(shownOptionalFilters([], EMPTY_FILTER)).toEqual([]);
    expect(shownOptionalFilters(["model", "archive"], EMPTY_FILTER)).toEqual([
      "archive",
      "model",
    ]);
    expect(
      shownOptionalFilters([], { ...EMPTY_FILTER, agentType: "Explore" }),
    ).toEqual(["agentType"]);
  });

  it("counts filters but not the search box as filtering", () => {
    expect(isFiltering(EMPTY_FILTER)).toBe(false);
    expect(isFiltering({ ...EMPTY_FILTER, query: "x" })).toBe(false);
    expect(isFiltering({ ...EMPTY_FILTER, minSizeMb: 10 })).toBe(true);
  });
});
