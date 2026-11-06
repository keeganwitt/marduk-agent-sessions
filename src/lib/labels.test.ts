import { describe, expect, it } from "vitest";
import { labelState, labelStyle, nextColor } from "./labels";

describe("labels", () => {
  it("reports whether all, some or none of the sessions have a label", () => {
    const sessions = [{ labelIds: [1, 2] }, { labelIds: [2] }];
    expect(labelState(2, sessions)).toBe("all");
    expect(labelState(1, sessions)).toBe("some");
    expect(labelState(3, sessions)).toBe("none");
  });

  it("falls back to gray for unknown colors and rotates colors for new labels", () => {
    expect(labelStyle("neon")).toEqual(labelStyle("gray"));
    expect(nextColor([])).not.toBe("gray");
  });
});
