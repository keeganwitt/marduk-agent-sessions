# Marduk (repo: marduk-agent-sessions)

Marduk is a Tauri v2 desktop app (Rust backend, React + TypeScript + Vite frontend) for cleaning up old AI coding-agent sessions. Tags are planned for later.

## Commits

Author is `Keegan Witt <keeganwitt@gmail.com>`, set in this repo's local git config. Don't commit with any other identity.

## Layout

- `crates/marduk-core`: the `AgentAdapter` trait (`adapter.rs`), per-agent adapters (`agents/`), delete safety checks (`delete.rs`) and the scan snapshot (`catalog.rs`). No Tauri dependency.
- `src-tauri`: Tauri commands `scan_sessions`, `preview_delete` and `delete_sessions` (`lib.rs`), plus serializable DTOs (`dto.rs`).
- `src`: the UI. Backend calls go through `src/lib/ipc.ts`.

## Design rules (don't break these)

- Deletes go to the system Trash (`trash` crate; the macOS NSFileManager method avoids a permission prompt to automate Finder). Never fall back to unlinking.
- The UI sends session `uid`s only. The backend resolves them against the last scan, so the UI never sends file paths.
- `delete::preflight` refuses running sessions (`RUNNING_WINDOW`), sessions changed since the scan, and paths outside the adapter's roots.
- Never write to an agent's database. Delete through the agent's CLI (e.g. `codex delete`, `opencode session delete`), or report `DeletePlan::ReadOnly`.
- Don't invent archive folders. Only show an agent's native archive (e.g. Codex `archived_sessions`).
- `ScanResult.complete` must be false whenever part of an agent's storage couldn't be read. Future tag pruning depends on it.
- `Session::key()` (`agent:id`) is the path-independent identity reserved for tags.
- Links: `installExternalLinkHandler` (`src/lib/externalLinks.ts`) sends link clicks to `tauri-plugin-opener` (http, https and mailto only, scoped in `capabilities/default.json`). Tauri's webview ignores `target="_blank"`.

## Adding an adapter

Implement `AgentAdapter` in `crates/marduk-core/src/agents/<agent>.rs`, register it in `default_adapters()`, and add tempdir-based tests next to it (see `claude.rs`). Before an agent's sessions are deletable, confirm how that agent deletes and keeps sessions itself. Adapters ported from yuluod/AllSessions (Apache-2.0) need attribution in `THIRD_PARTY_NOTICES.md`.

## Checks before committing

`pnpm typecheck && pnpm test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
