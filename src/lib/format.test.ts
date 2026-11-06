import { describe, expect, it } from "vitest";
import { ageInDays, formatAge, formatBytes, shortenPath } from "./format";

describe("formatBytes", () => {
  it.each([
    [0, "0 B"],
    [1023, "1023 B"],
    [1536, "1.5 KB"],
    [10 * 1024 * 1024, "10 MB"],
    [3.25 * 1024 ** 3, "3.3 GB"],
  ])("formats %d as %s", (bytes, expected) => {
    expect(formatBytes(bytes)).toBe(expected);
  });
});

describe("formatAge", () => {
  const now = new Date("2026-11-06T12:00:00Z");

  it.each([
    ["2026-11-06T11:59:30Z", "just now"],
    ["2026-11-06T11:48:00Z", "12m ago"],
    ["2026-11-06T07:00:00Z", "5h ago"],
    ["2026-10-27T12:00:00Z", "10d ago"],
    ["2026-05-06T12:00:00Z", "6mo ago"],
    ["2023-11-06T12:00:00Z", "3y ago"],
  ])("formats %s as %s", (timestamp, expected) => {
    expect(formatAge(timestamp, now)).toBe(expected);
  });

  it("never reports a negative age for future timestamps", () => {
    expect(ageInDays("2026-11-07T12:00:00Z", now)).toBe(0);
  });
});

describe("shortenPath", () => {
  it.each([
    ["/Users/me/work/app", "/Users/me", "~/work/app"],
    ["/Users/me", "/Users/me", "~"],
    ["/Users/me/a/b/c", "/Users/me", "~/a/b/c"],
    ["/Users/me/Documents/Projects/sec-eng/service-account-manager-rest", "/Users/me", "~/…/sec-eng/service-account-manager-rest"],
    ["/Users/me/Documents/Codex/2026-11-06/did-x20", "/Users/me/", "~/…/2026-11-06/did-x20"],
    ["/Users/meow/x", "/Users/me", "/Users/meow/x"],
    ["/opt/a/b/c/d", "/Users/me", "/…/c/d"],
    ["/opt/a/b", "/Users/me", "/opt/a/b"],
    ["C:\\Users\\me\\src\\team\\app", "C:\\Users\\other", "C:\\…\\team\\app"],
    ["C:\\Users\\me\\src\\team\\app", "C:\\Users\\me", "~\\src\\team\\app"],
  ])("shortens %s (home %s) to %s", (path, home, expected) => {
    expect(shortenPath(path, home)).toBe(expected);
  });
});
