import { invoke } from "@tauri-apps/api/core";
import type { DeleteResult, PreviewItem, ScanResponse } from "./types";

export function scanSessions(): Promise<ScanResponse> {
  return invoke("scan_sessions");
}

export function previewDelete(uids: string[]): Promise<PreviewItem[]> {
  return invoke("preview_delete", { uids });
}

export function deleteSessions(uids: string[]): Promise<DeleteResult[]> {
  return invoke("delete_sessions", { uids });
}
