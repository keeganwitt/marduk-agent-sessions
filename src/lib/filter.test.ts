import { describe, expect, it } from "vitest";
import {
  EMPTY_FILTER,
  filterSessions,
  inProject,
  matchProjects,
  projectOptions,
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

  it("lists projects by session count, then by path", () => {
    const sessions = ["/b", "/a", "/c", "/c", null].map((projectDir, index) =>
      makeSession({ uid: String(index), projectDir }),
    );

    expect(projectOptions(sessions)).toEqual([
      { dir: "/c", count: 2 },
      { dir: "/a", count: 1 },
      { dir: "/b", count: 1 },
    ]);
  });

  it("suggests projects by full path or by the path shown with ~", () => {
    const options = [
      { dir: "/Users/me/Projects/api", count: 3 },
      { dir: "/srv/web", count: 1 },
    ];

    expect(matchProjects(options, "~/proj", "/Users/me")).toEqual([options[0]]);
    expect(matchProjects(options, "SRV", "/Users/me")).toEqual([options[1]]);
    expect(matchProjects(options, "  ", "/Users/me")).toEqual(options);
  });
});
