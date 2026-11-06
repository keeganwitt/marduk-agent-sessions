import { describe, expect, it } from "vitest";
import { DEFAULT_SORT, nextSort, sortSessions } from "./sort";
import { makeSession } from "../test/factories";

describe("nextSort", () => {
  it("cycles text columns ascending, descending, then back to the default order", () => {
    const first = nextSort(DEFAULT_SORT, "title");
    expect(first).toEqual({ key: "title", direction: "asc" });
    const second = nextSort(first, "title");
    expect(second).toEqual({ key: "title", direction: "desc" });
    expect(nextSort(second, "title")).toEqual(DEFAULT_SORT);
  });

  it("starts date and size columns newest or largest first", () => {
    const first = nextSort(DEFAULT_SORT, "size");
    expect(first).toEqual({ key: "size", direction: "desc" });
    expect(nextSort(first, "size")).toEqual({ key: "size", direction: "asc" });
    expect(nextSort(nextSort(first, "size"), "size")).toEqual(DEFAULT_SORT);
  });

  it("the default column switches between newest and oldest first", () => {
    const oldest = nextSort(DEFAULT_SORT, "lastActivity");
    expect(oldest).toEqual({ key: "lastActivity", direction: "asc" });
    expect(nextSort(oldest, "lastActivity")).toEqual(DEFAULT_SORT);
  });

  it("switching columns starts the new column fresh", () => {
    expect(nextSort({ key: "title", direction: "desc" }, "agent")).toEqual({
      key: "agent",
      direction: "asc",
    });
  });
});

describe("sortSessions", () => {
  const a = makeSession({
    uid: "a",
    title: "item 10",
    sizeBytes: 5,
    created: "2026-01-02T00:00:00Z",
  });
  const b = makeSession({
    uid: "b",
    title: "Item 9",
    sizeBytes: 50,
    created: null,
  });
  const c = makeSession({
    uid: "c",
    title: null,
    id: "zzz",
    sizeBytes: 1,
    created: "2026-01-01T00:00:00Z",
  });
  const uids = (sort: Parameters<typeof sortSessions>[1]) =>
    sortSessions([a, b, c], sort).map((s) => s.uid);

  it("leaves the original order when unsorted", () => {
    expect(uids(null)).toEqual(["a", "b", "c"]);
  });

  it("sorts titles naturally and case-insensitively, falling back to the id", () => {
    expect(uids({ key: "title", direction: "asc" })).toEqual(["b", "a", "c"]);
    expect(uids({ key: "title", direction: "desc" })).toEqual(["c", "a", "b"]);
  });

  it("sorts numbers numerically", () => {
    expect(uids({ key: "size", direction: "desc" })).toEqual(["b", "a", "c"]);
  });

  it("puts missing values last in both directions", () => {
    expect(uids({ key: "created", direction: "asc" })).toEqual(["c", "a", "b"]);
    expect(uids({ key: "created", direction: "desc" })).toEqual([
      "a",
      "c",
      "b",
    ]);
  });
});
