import { invoke } from "@tauri-apps/api/core";
import type { ArchiveResult, DeleteResult, PreviewItem, ScanResponse, Transcript } from "./types";

export function scanSessions(): Promise<ScanResponse> {
  return invoke("scan_sessions");
}

export function previewDelete(uids: string[]): Promise<PreviewItem[]> {
  return invoke("preview_delete", { uids });
}

export function deleteSessions(uids: string[]): Promise<DeleteResult[]> {
  return invoke("delete_sessions", { uids });
}

export function toggleArchive(uids: string[]): Promise<ArchiveResult[]> {
  return invoke("toggle_archive", { uids });
}

export function revealSessionFile(path: string): Promise<void> {
  return invoke("reveal_session_file", { path });
}

export function openProjectDir(uid: string): Promise<void> {
  return invoke("open_project_dir", { uid });
}

export function loadTranscript(uid: string): Promise<Transcript> {
  return invoke("load_transcript", { uid });
}
