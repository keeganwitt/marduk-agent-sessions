import { describe, expect, it } from "vitest";
import { EMPTY_FILTER, filterSessions } from "./filter";
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
