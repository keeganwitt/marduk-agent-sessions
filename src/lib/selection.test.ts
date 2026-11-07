import { describe, expect, it } from "vitest";
import { makeSession } from "../test/factories";
import { rangeBetween, withLeftoverSubagents } from "./selection";

describe("rangeBetween", () => {
  const order = ["a", "b", "c", "d", "e"];

  it("covers the rows between anchor and target, in either direction", () => {
    expect(rangeBetween(order, "b", "d")).toEqual(["b", "c", "d"]);
    expect(rangeBetween(order, "d", "b")).toEqual(["b", "c", "d"]);
  });

  it("is just the target without a usable anchor", () => {
    expect(rangeBetween(order, null, "c")).toEqual(["c"]);
    expect(rangeBetween(order, "gone", "c")).toEqual(["c"]);
  });

  it("handles anchor and target being the same row", () => {
    expect(rangeBetween(order, "c", "c")).toEqual(["c"]);
  });
});

describe("withLeftoverSubagents", () => {
  const parent = makeSession({ uid: "p", id: "p", agent: "codex" });
  const spawned = makeSession({
    uid: "s",
    id: "s",
    agent: "codex",
    parentId: "p",
    deletedWithParent: true,
  });
  const review = makeSession({
    uid: "r",
    id: "r",
    agent: "codex",
    parentId: "p",
  });
  const spawnedReview = makeSession({
    uid: "sr",
    id: "sr",
    agent: "codex",
    parentId: "s",
  });
  const otherAgent = makeSession({
    uid: "o",
    id: "o",
    agent: "opencode",
    parentId: "p",
  });
  const all = [parent, spawned, review, spawnedReview, otherAgent];

  it("adds subagents the agent's delete leaves behind, at any depth", () => {
    const uids = withLeftoverSubagents([parent], all).map((s) => s.uid);

    expect(uids).toEqual(["p", "r", "sr"]);
  });

  it("leaves out subagents that are part of their parent's files", () => {
    const claudeParent = makeSession({ uid: "c", id: "c" });
    const inside = makeSession({
      uid: "ci",
      id: "c/agent-1",
      parentId: "c",
      partOfParent: true,
    });

    expect(
      withLeftoverSubagents([claudeParent], [claudeParent, inside]),
    ).toEqual([claudeParent]);
  });
});
