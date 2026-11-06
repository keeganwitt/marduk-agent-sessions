import type { SessionRow } from "../lib/types";

export function makeSession(overrides: Partial<SessionRow> = {}): SessionRow {
  const uid = overrides.uid ?? "claude:/p/s.jsonl";
  return {
    uid,
    key: `claude:${uid}`,
    agent: "claude",
    agentName: "Claude Code",
    id: uid,
    title: "A session",
    projectDir: "/work/app",
    created: "2026-08-30T00:00:00Z",
    lastActivity: "2026-09-01T00:00:00Z",
    sizeBytes: 2048,
    files: [`/sessions/${uid}.jsonl`],
    archived: false,
    running: false,
    readOnlyReason: null,
    permanentDelete: false,
    canArchive: false,
    ...overrides,
  };
}
