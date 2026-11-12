import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchLatestRelease, isNewer, LATEST_RELEASE_API } from "./updates";

const releaseUrl =
  "https://github.com/keeganwitt/marduk-agent-sessions/releases/tag/v1.3.0";
const respond = (status: number, body: unknown) =>
  vi
    .fn<typeof fetch>()
    .mockResolvedValue(new Response(JSON.stringify(body), { status }));

afterEach(() => vi.useRealTimers());

describe("isNewer", () => {
  it.each([
    ["1.10.0", "1.9.9", true],
    ["v2.0.0", "1.9.9", true],
    ["1.2.3", "1.2.3", false],
    ["1.2.2", "1.2.3", false],
    ["1.2.3+new", "1.2.3+old", false],
    ["1.2.3", "1.2.3-rc.1", true],
    ["1.2.3-rc.1", "1.2.3", false],
    ["1.2.3-rc.10", "1.2.3-rc.2", true],
    ["1.2.3-beta", "1.2.3-alpha", true],
    ["1.2.3-alpha.1", "1.2.3-alpha", true],
    ["1.2.3-alpha", "1.2.3-1", true],
  ])("compares %s with %s", (candidate, current, expected) => {
    expect(isNewer(candidate, current)).toBe(expected);
  });

  it.each(["unknown", "", "1.2", "1.2.3.4", "01.2.3", "1.2.3-01"])(
    "rejects invalid version %s",
    (version) => {
      expect(() => isNewer(version, "1.0.0")).toThrow();
      expect(() => isNewer("1.0.0", version)).toThrow();
    },
  );
});

describe("fetchLatestRelease", () => {
  it("requests the latest release and returns its version and page", async () => {
    const fetcher = respond(200, { tag_name: "v1.3.0", html_url: releaseUrl });
    await expect(fetchLatestRelease(fetcher)).resolves.toEqual({
      version: "1.3.0",
      url: releaseUrl,
    });
    expect(fetcher).toHaveBeenCalledWith(
      LATEST_RELEASE_API,
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it.each([
    [404, "updates.noReleases"],
    [403, "updates.httpError"],
    [500, "updates.httpError"],
  ])("reports HTTP %s", async (status, key) => {
    await expect(
      fetchLatestRelease(respond(Number(status), {})),
    ).rejects.toMatchObject({ key });
  });

  it.each([
    null,
    {},
    { tag_name: 12, html_url: releaseUrl },
    { tag_name: "bad", html_url: releaseUrl },
    { tag_name: "1.3.0", html_url: "javascript:alert(1)" },
    {
      tag_name: "1.3.0",
      html_url: "https://github.com/other/repo/releases/tag/v1",
    },
    {
      tag_name: "1.3.0",
      html_url:
        "https://github.com@evil.example/keeganwitt/marduk-agent-sessions/releases/tag/v1",
    },
  ])("rejects malformed or unsafe release metadata %j", async (body) => {
    await expect(fetchLatestRelease(respond(200, body))).rejects.toMatchObject({
      key: "updates.invalidResponse",
    });
  });

  it("reports network failure", async () => {
    await expect(
      fetchLatestRelease(vi.fn().mockRejectedValue(new TypeError("offline"))),
    ).rejects.toMatchObject({ key: "updates.networkError" });
  });

  it("rejects a download page for a different tag", async () => {
    await expect(
      fetchLatestRelease(
        respond(200, {
          tag_name: "v1.3.0",
          html_url:
            "https://github.com/keeganwitt/marduk-agent-sessions/releases/tag/v1.0.0",
        }),
      ),
    ).rejects.toMatchObject({ key: "updates.invalidResponse" });
  });

  it("accepts URL-encoded release tags", async () => {
    const url =
      "https://github.com/keeganwitt/marduk-agent-sessions/releases/tag/v1.3.0%2Bbuild.1";
    await expect(
      fetchLatestRelease(
        respond(200, { tag_name: "v1.3.0+build.1", html_url: url }),
      ),
    ).resolves.toEqual({ version: "1.3.0+build.1", url });
  });

  it("times out stalled requests", async () => {
    vi.useFakeTimers();
    const fetcher = vi.fn<typeof fetch>().mockImplementation(
      (_url, init) =>
        new Promise((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () =>
            reject(init.signal?.reason),
          );
        }),
    );
    const result = expect(fetchLatestRelease(fetcher)).rejects.toMatchObject({
      key: "updates.timeout",
    });
    await vi.advanceTimersByTimeAsync(15000);
    await result;
  });
});
