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
  created: string | null;
  lastActivity: string;
  sizeBytes: number;
  files: string[];
  database: string | null;
  parentId: string | null;
  archived: boolean;
  running: boolean;
  readOnlyReason: string | null;
  permanentDelete: boolean;
  canArchive: boolean;
}

export interface ScanResponse {
  agents: AgentSummary[];
  sessions: SessionRow[];
  homeDir: string;
}

export type MessageRole = "user" | "assistant" | "tool" | "context";

export interface TranscriptMessage {
  role: MessageRole;
  text: string;
  timestamp: string | null;
  tool: { name: string; output: string | null } | null;
}

export interface Transcript {
  head: TranscriptMessage[];
  omitted: number;
  tail: TranscriptMessage[];
  shortened: boolean;
}

export interface PreviewItem {
  uid: string;
  title: string | null;
  agentName: string | null;
  files: string[];
  permanent: boolean;
  command: string | null;
  warning: string | null;
  refusedReason: string | null;
}

export type DeleteResult =
  | { status: "trashed"; uid: string; files: string[] }
  | { status: "deleted"; uid: string; files: string[]; command: string }
  | { status: "refused"; uid: string; message: string }
  | { status: "failed"; uid: string; message: string };

export type ArchiveResult =
  | { status: "done"; uid: string; archived: boolean; command: string }
  | { status: "refused"; uid: string; message: string }
  | { status: "failed"; uid: string; message: string };

export function isDeletable(session: SessionRow): boolean {
  return !session.running && session.readOnlyReason === null;
}

export function isArchivable(session: SessionRow): boolean {
  return !session.running && session.canArchive;
}

export function isSelectable(session: SessionRow): boolean {
  return isDeletable(session) || isArchivable(session);
}
