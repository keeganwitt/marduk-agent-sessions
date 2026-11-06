import { describe, expect, it } from "vitest";
import { rangeBetween } from "./selection";

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
