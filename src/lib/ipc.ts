import { Channel, invoke } from "@tauri-apps/api/core";
import type { Theme } from "./theme";
import type { ArchiveResult, CliStatus, DeleteResult, FileDetail, Label, PreviewItem, ScanProgress, ScanResponse, Transcript } from "./types";

export function scanSessions(onProgress: (progress: ScanProgress) => void = () => {}): Promise<ScanResponse> {
  const channel = new Channel<ScanProgress>();
  channel.onmessage = onProgress;
  return invoke("scan_sessions", { onProgress: channel });
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

export function cliStatuses(): Promise<CliStatus[]> {
  return invoke("cli_statuses");
}

export function setCliPath(agent: string, path: string | null): Promise<CliStatus[]> {
  return invoke("set_cli_path", { agent, path });
}

export function getTheme(): Promise<Theme> {
  return invoke("get_theme");
}

export function saveTheme(theme: Theme): Promise<void> {
  return invoke("set_theme", { theme });
}

export function listLabels(): Promise<Label[]> {
  return invoke("list_labels");
}

export function createLabel(name: string, color: string): Promise<Label> {
  return invoke("create_label", { name, color });
}

export function updateLabel(id: number, name: string, color: string): Promise<void> {
  return invoke("update_label", { id, name, color });
}

export function deleteLabel(id: number): Promise<void> {
  return invoke("delete_label", { id });
}

export function applyLabels(uids: string[], add: number[], remove: number[]): Promise<void> {
  return invoke("apply_labels", { uids, add, remove });
}

export function fileDetails(paths: string[]): Promise<FileDetail[]> {
  return invoke("file_details", { paths });
}
