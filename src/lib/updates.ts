export const LATEST_RELEASE_API =
  "https://api.github.com/repos/keeganwitt/marduk-agent-sessions/releases/latest";

type ErrorKey =
  | "updates.noReleases"
  | "updates.httpError"
  | "updates.invalidResponse"
  | "updates.networkError"
  | "updates.timeout"
  | "updates.versionError";

export class UpdateError extends Error {
  constructor(
    public readonly key: ErrorKey,
    public readonly status = 0,
  ) {
    super(key);
  }
}

function parseVersion(version: string) {
  const match =
    /^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([\da-zA-Z-]+(?:\.[\da-zA-Z-]+)*))?(?:\+[\da-zA-Z-]+(?:\.[\da-zA-Z-]+)*)?$/.exec(
      version,
    );
  if (!match) throw new UpdateError("updates.versionError");
  const prerelease = match[4]?.split(".") ?? [];
  if (prerelease.some((part) => /^0\d+$/.test(part))) {
    throw new UpdateError("updates.versionError");
  }
  return { core: match.slice(1, 4).map(BigInt), prerelease };
}

export function isNewer(candidate: string, current: string): boolean {
  const a = parseVersion(candidate);
  const b = parseVersion(current);
  for (let i = 0; i < 3; i++) {
    if (a.core[i] !== b.core[i]) return a.core[i] > b.core[i];
  }
  if (!a.prerelease.length || !b.prerelease.length) {
    return !a.prerelease.length && !!b.prerelease.length;
  }
  for (let i = 0; i < Math.max(a.prerelease.length, b.prerelease.length); i++) {
    const left = a.prerelease[i];
    const right = b.prerelease[i];
    if (left === undefined || right === undefined) return right === undefined;
    if (left === right) continue;
    const leftNumeric = /^\d+$/.test(left);
    const rightNumeric = /^\d+$/.test(right);
    if (leftNumeric && rightNumeric) return BigInt(left) > BigInt(right);
    if (leftNumeric !== rightNumeric) return rightNumeric;
    return left > right;
  }
  return false;
}

export interface LatestRelease {
  version: string;
  url: string;
}

export async function fetchLatestRelease(
  fetcher: typeof fetch = fetch,
): Promise<LatestRelease> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15000);
  try {
    const response = await fetcher(LATEST_RELEASE_API, {
      headers: { Accept: "application/vnd.github+json" },
      signal: controller.signal,
      credentials: "omit",
    });
    if (response.status === 404) throw new UpdateError("updates.noReleases");
    if (!response.ok)
      throw new UpdateError("updates.httpError", response.status);
    let body: unknown;
    try {
      body = await response.json();
    } catch {
      if (controller.signal.aborted) throw new UpdateError("updates.timeout");
      throw new UpdateError("updates.invalidResponse");
    }
    if (
      !body ||
      typeof body !== "object" ||
      !("tag_name" in body) ||
      !("html_url" in body) ||
      typeof body.tag_name !== "string" ||
      typeof body.html_url !== "string"
    ) {
      throw new UpdateError("updates.invalidResponse");
    }
    try {
      parseVersion(body.tag_name);
      const url = new URL(body.html_url);
      if (
        url.origin !== "https://github.com" ||
        url.username ||
        url.password ||
        decodeURIComponent(url.pathname) !==
          `/keeganwitt/marduk-agent-sessions/releases/tag/${body.tag_name}`
      ) {
        throw new Error("Invalid release URL");
      }
      return { version: body.tag_name.replace(/^v/, ""), url: url.href };
    } catch {
      throw new UpdateError("updates.invalidResponse");
    }
  } catch (error) {
    if (error instanceof UpdateError) throw error;
    throw new UpdateError(
      controller.signal.aborted ? "updates.timeout" : "updates.networkError",
    );
  } finally {
    clearTimeout(timeout);
  }
}
