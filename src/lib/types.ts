export interface AgentSummary {
  id: string;
  name: string;
  complete: boolean;
  errors: string[];
  sessionCount: number;
}

export interface SessionRow {
  uid: string;
  key: string;
  agent: string;
  agentName: string;
  id: string;
  title: string | null;
  projectDir: string | null;
  lastActivity: string;
  sizeBytes: number;
  archived: boolean;
  running: boolean;
  readOnlyReason: string | null;
}

export interface ScanResponse {
  agents: AgentSummary[];
  sessions: SessionRow[];
}

export interface PreviewItem {
  uid: string;
  title: string | null;
  agentName: string | null;
  files: string[];
  refusedReason: string | null;
}

export type DeleteResult =
  | { status: "trashed"; uid: string; files: string[] }
  | { status: "refused"; uid: string; message: string }
  | { status: "failed"; uid: string; message: string };

export function isDeletable(session: SessionRow): boolean {
  return !session.running && session.readOnlyReason === null;
}
