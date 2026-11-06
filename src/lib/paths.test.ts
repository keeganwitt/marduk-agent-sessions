import { describe, expect, it } from "vitest";
import { sharedFolder, splitPath } from "./paths";

describe("paths", () => {
  it("splits on either separator", () => {
    expect(splitPath("/a/b/c.jsonl")).toEqual({ dir: "/a/b", name: "c.jsonl" });
    expect(splitPath("C:\\a\\b.db")).toEqual({ dir: "C:\\a", name: "b.db" });
    expect(splitPath("plain")).toEqual({ dir: "", name: "plain" });
  });

  it("finds the folder all paths share, if any", () => {
    expect(sharedFolder(["/p/a.jsonl", "/p/a"])).toBe("/p");
    expect(sharedFolder(["/p/a", "/q/b"])).toBeNull();
    expect(sharedFolder([])).toBeNull();
  });
});
