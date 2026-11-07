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
  parentMissing: boolean;
  deletedWithParent: boolean;
  /** Stored inside its parent session (Claude Code subagents): readable, not separately changed. */
  partOfParent: boolean;
  /** Started without a project; `projectDir` is a throwaway folder the agent made. */
  projectless: boolean;
  /** Models the session used, the current one last. */
  models: string[];
  /** The agent definition the session ran as (custom agent, subagent type or role). */
  agentType: string | null;
  archived: boolean;
  running: boolean;
  readOnlyReason: string | null;
  permanentDelete: boolean;
  canArchive: boolean;
  labelIds: number[];
}

export interface Label {
  id: number;
  name: string;
  color: string;
  /** Sessions with this label. */
  count: number;
}

export interface ScanProgress {
  done: number;
  total: number;
  /** The agent being scanned, empty once the scan has finished. */
  agent: string;
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
  note: string | null;
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
  return (
    !session.partOfParent && (isDeletable(session) || isArchivable(session))
  );
}

export interface MemoryFile {
  path: string;
  title: string;
  description: string | null;
  /** A memory type (`feedback`, `project`, …) or a role (`index`, `loaded`, …). */
  kind: string | null;
  /** The session the memory came from, when the agent records it. */
  sessionId: string | null;
  modified: string | null;
  size: number;
}

/** Claude Code keeps memory per project; Codex keeps one global store. */
export interface MemoryStore {
  agent: string;
  agentName: string;
  folder: string;
  projectDir: string | null;
  projectExists: boolean | null;
  sessions: number;
  files: MemoryFile[];
  problems: string[];
  note: string | null;
  canTrash: boolean;
}

export interface AgentHelp {
  id: string;
  name: string;
  /** Folders the agent's sessions are read from. */
  roots: string[];
  delete: string;
  archive: string;
}

export interface CliStatus {
  agent: string;
  agentName: string;
  program: string;
  installUrl: string;
  path: string | null;
  overridden: boolean;
  version: string | null;
  problem: string | null;
}

export interface FileDetail {
  path: string;
  sizeBytes: number;
  isDir: boolean;
  exists: boolean;
}
