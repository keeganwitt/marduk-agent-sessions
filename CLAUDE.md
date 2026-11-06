# Marduk (repo: marduk-agent-sessions)

Marduk is a Tauri v2 desktop app (Rust backend, React + TypeScript + Vite frontend) for cleaning up old AI coding-agent sessions. Tags are planned for later.

## Commits

Author is `Keegan Witt <keeganwitt@gmail.com>`, set in this repo's local git config. Don't commit with any other identity.

## Layout

- `crates/marduk-core`: the `AgentAdapter` trait (`adapter.rs`), per-agent adapters (`agents/`), delete safety checks (`delete.rs`) and the scan snapshot (`catalog.rs`). No Tauri dependency.
- `src-tauri`: Tauri commands `scan_sessions`, `preview_delete`, `delete_sessions`, `toggle_archive`, `reveal_session_file`, `open_project_dir` and `load_transcript` (`lib.rs`), plus serializable DTOs (`dto.rs`).
- `src`: the UI. Backend calls go through `src/lib/ipc.ts`.

## Design rules (don't break these)

- If an agent has its own per-session delete or archive command, use it (`DeletePlan::AgentCommand`, `AgentAdapter::archive_command`). Today that's `codex delete --force` (permanent) and `codex archive`/`unarchive`. Without a native command, delete to the system Trash (`trash` crate; the macOS NSFileManager method avoids a permission prompt to automate Finder). Never fall back to unlinking.
- Permanent deletes must say so in the UI: `PreviewItem.permanent` and the exact `command` are shown in the dialog.
- After an agent command reports success, check that the transcript actually moved or disappeared, and treat it as a failure if it didn't.
- Agent CLIs are found with `agents::find_program`, which also searches common install folders because Dock-launched apps get a minimal `PATH`.
- The UI sends session `uid`s only. The backend resolves them against the last scan, so the UI never sends file paths. The one exception is `reveal_session_file`, which only accepts paths that `Catalog::owns_path` recognizes.
- `delete::preflight` and `delete::toggle_archive` refuse running sessions (`RUNNING_WINDOW`), sessions changed since the scan, and paths outside the adapter's roots.
- Never write to an agent's database. Delete through the agent's CLI (e.g. `codex delete`, `opencode session delete`), or report `DeletePlan::ReadOnly`.
- Don't invent archive folders. Only offer archiving through the agent's own command, and only show its native archive (e.g. Codex `archived_sessions`).
- `ScanResult.complete` must be false whenever part of an agent's storage couldn't be read. Future tag pruning depends on it.
- `Session::key()` (`agent:id`) is the path-independent identity reserved for tags.
- Links: `installExternalLinkHandler` (`src/lib/externalLinks.ts`) sends link clicks to `tauri-plugin-opener` (http, https and mailto only, scoped in `capabilities/default.json`). Tauri's webview ignores `target="_blank"`.

## Adding an adapter

Implement `AgentAdapter` (including `transcript`, built with `transcript::TranscriptBuilder` so long sessions keep their first and last messages and tool results attach to their calls) in `crates/marduk-core/src/agents/<agent>.rs`, register it in `default_adapters()`, and add tempdir-based tests next to it (see `claude.rs`). Before an agent's sessions are deletable, check the agent's CLI for native per-session delete and archive commands (and whether they're permanent or prompt for confirmation), and use them if they exist. Adapters ported from yuluod/AllSessions (Apache-2.0) need attribution in `THIRD_PARTY_NOTICES.md`.

## Releases

CI and releases run on GitHub Actions (`.github/workflows`). Binaries are unsigned for now, so don't add signing secrets or notarization steps unless asked.

## Checks before committing

`pnpm typecheck && pnpm test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
