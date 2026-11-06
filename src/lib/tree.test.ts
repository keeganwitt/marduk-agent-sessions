import { describe, expect, it } from "vitest";
import { buildTree } from "./tree";
import { makeSession } from "../test/factories";
import type { SessionRow } from "./types";

function session(id: string, overrides: Partial<SessionRow> = {}): SessionRow {
  return makeSession({
    uid: `uid-${id}`,
    id,
    agent: "codex",
    title: id,
    ...overrides,
  });
}

const all = () => true;
const shape = (rows: ReturnType<typeof buildTree>) =>
  rows.map(
    (row) =>
      `${"  ".repeat(row.depth)}${row.session.id}${row.childCount ? ` (${row.childCount})` : ""}`,
  );

describe("buildTree", () => {
  const parent = session("parent", { lastActivity: "2026-11-05T00:00:00Z" });
  const child = session("child", {
    parentId: "parent",
    lastActivity: "2026-11-04T00:00:00Z",
  });
  const grandchild = session("grandchild", { parentId: "child" });
  const other = session("other", { lastActivity: "2026-11-06T00:00:00Z" });
  const sessions = [other, parent, child, grandchild];

  it("collapses children under their parent by default", () => {
    expect(shape(buildTree(sessions, all, null, new Set()))).toEqual([
      "other",
      "parent (1)",
    ]);
  });

  it("shows expanded children indented, recursively", () => {
    const rows = buildTree(
      sessions,
      all,
      null,
      new Set(["uid-parent", "uid-child"]),
    );
    expect(shape(rows)).toEqual([
      "other",
      "parent (1)",
      "  child (1)",
      "    grandchild",
    ]);
    expect(rows.find((row) => row.session.id === "parent")!.expanded).toBe(
      true,
    );
  });

  it("opens and dims ancestors of a matching descendant", () => {
    const rows = buildTree(
      sessions,
      (s) => s.id === "grandchild",
      null,
      new Set(),
    );
    expect(shape(rows)).toEqual([
      "parent (1)",
      "  child (1)",
      "    grandchild",
    ]);
    expect(rows.map((row) => row.matched)).toEqual([false, false, true]);
  });

  it("treats subagents whose parent is missing as top-level", () => {
    const orphan = session("orphan", { parentId: "gone", parentMissing: true });
    const stray = session("stray", { parentId: "never-scanned" });
    expect(shape(buildTree([orphan, stray], all, null, new Set()))).toEqual([
      "orphan",
      "stray",
    ]);
  });

  it("only links children to parents of the same agent", () => {
    const foreign = session("foreign", {
      agent: "opencode",
      parentId: "parent",
    });
    expect(shape(buildTree([parent, foreign], all, null, new Set()))).toEqual([
      "parent",
      "foreign",
    ]);
  });

  it("sorts siblings with the chosen sort", () => {
    const a = session("a", { parentId: "parent", sizeBytes: 1 });
    const b = session("b", { parentId: "parent", sizeBytes: 9 });
    const rows = buildTree(
      [parent, a, b],
      all,
      { key: "size", direction: "desc" },
      new Set(["uid-parent"]),
    );
    expect(shape(rows)).toEqual(["parent (2)", "  b", "  a"]);
  });

  it("survives a parent cycle without looping", () => {
    const x = session("x", { parentId: "y" });
    const y = session("y", { parentId: "x" });
    expect(
      shape(buildTree([x, y], all, null, new Set(["uid-x", "uid-y"]))),
    ).toEqual(["x", "y"]);
  });
});
