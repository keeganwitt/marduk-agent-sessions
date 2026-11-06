# Marduk (repo: marduk-agent-sessions)

Marduk is a Tauri v2 desktop app (Rust backend, React + TypeScript + Vite frontend) for cleaning up old AI coding-agent sessions. Tags are planned for later.

## Commits

Author is `Keegan Witt <keeganwitt@gmail.com>`, set in this repo's local git config. Don't commit with any other identity.

## Layout

- `crates/marduk-core`: the `AgentAdapter` trait (`adapter.rs`), per-agent adapters (`agents/`), delete safety checks (`delete.rs`) and the scan snapshot (`catalog.rs`). No Tauri dependency.
- `src-tauri`: Tauri commands `scan_sessions`, `preview_delete`, `delete_sessions`, `toggle_archive`, `reveal_session_file`, `open_project_dir` and `load_transcript` (`lib.rs`), plus serializable DTOs (`dto.rs`).
- `src`: the UI. Backend calls go through `src/lib/ipc.ts`.

## Design rules (don't break these)

- If an agent has its own per-session delete or archive command, use it (`DeletePlan::AgentCommand`, `AgentAdapter::archive_command`). Today that's `codex delete --force` (permanent), `codex archive`/`unarchive` `opencode session delete` and `kilo session delete` (permanent, cascades to child sessions; surfaced via `DeletePlan::AgentCommand.warning`). Without a native command, delete to the system Trash (`trash` crate; the macOS NSFileManager method avoids a permission prompt to automate Finder). Never fall back to unlinking.
- Native delete commands that only exist interactively (Copilot's `/session delete`, Antigravity's TUI) don't count; use the Trash and explain side effects with `DeletePlan::Trash.warning`.
- Permanent deletes must say so in the UI: `PreviewItem.permanent` and the exact `command` are shown in the dialog.
- After an agent command reports success, check that the transcript actually moved or disappeared, and treat it as a failure if it didn't.
- Agent CLIs go through `agents::cli::Cli`: a user-chosen path from Settings (`CliOverrides`, persisted by `src-tauri/src/settings.rs` in the app config folder) wins, otherwise `agents::find_program`, which also searches common install folders because Dock-launched apps get a minimal `PATH`. CLIs are re-resolved on every scan. Don't download or bundle agent CLIs: a version that differs from the user's agent could migrate its data.
- The UI sends session `uid`s only. The backend resolves them against the last scan, so the UI never sends file paths. The one exception is `reveal_session_file`, which only accepts paths that `Catalog::owns_path` recognizes.
- `delete::preflight` and `delete::toggle_archive` refuse running sessions (`RUNNING_WINDOW`), sessions changed since the scan, and paths outside the adapter's roots.
- Never write to an agent's database. Delete through the agent's CLI (e.g. `codex delete`, `opencode session delete`), or report `DeletePlan::ReadOnly`.
- Don't invent archive folders. Only offer archiving through the agent's own command, and only show its native archive (e.g. Codex `archived_sessions`).
- OpenCode and Kilo share `agents/opencode.rs`; their differences (folder, database names, env var, CLI, export command) live in `opencode::Flavor`. Messages are read from whichever store (`session_message` or `message`) a session's messages are actually in.
- Database-backed agents (OpenCode, Kilo) open their database with `SQLITE_OPEN_READ_ONLY` and override `AgentAdapter::fingerprint`/`still_present` to use the session's row; the database file's own mtime is meaningless per session. Their sessions have no `files`, and `uid` includes the session id because they share `primary_file`. When OpenCode's stored messages aren't recognized, `transcript` falls back to `opencode session export` and sets `Transcript.note`.
- Run agent CLIs through `agents::cli::run_capture` (or `SystemBackend::run`), which reads output while the process runs; reading after exit deadlocks on large output.
- Agent support is built from each agent's source and docs, with fixtures in tests, even when nobody on the project uses that agent.
- `ScanResult.complete` must be false whenever part of an agent's storage couldn't be read. Future tag pruning depends on it.
- `Session::key()` (`agent:id`) is the path-independent identity labels are keyed by (`marduk_core::labels::LabelStore`). Label pruning (`reconcile`) only judges agents whose scan was complete, after `PRUNE_GRACE`.
- Labels are solid, squared chips (`.label-chip`); status badges are tinted, round pills (`.badge`). Keep them visually distinct.
- Links: `installExternalLinkHandler` (`src/lib/externalLinks.ts`) sends link clicks to `tauri-plugin-opener` (http, https and mailto only, scoped in `capabilities/default.json`). Tauri's webview ignores `target="_blank"`.

## Adding an adapter

Implement `AgentAdapter` (including `transcript`, built with `transcript::TranscriptBuilder` so long sessions keep their first and last messages and tool results attach to their calls) in `crates/marduk-core/src/agents/<agent>.rs`, register it in `default_adapters()`, and add tempdir-based tests next to it (see `claude.rs`). Before an agent's sessions are deletable, check the agent's CLI for native per-session delete and archive commands (and whether they're permanent or prompt for confirmation), and use them if they exist. Adapters ported from yuluod/AllSessions (Apache-2.0) need attribution in `THIRD_PARTY_NOTICES.md`.

## Agents deliberately not supported yet

- **Gemini CLI** (decided 2026-11-06): skipped for now; most users are moving to Antigravity. If revisited: storage is `~/.gemini/tmp/<project-slug>/chats/session-*.jsonl` (JSONL with `$set`/`$rewindTo`/`$patch` replay records, slug mapping in `~/.gemini/projects.json`), and `gemini --delete-session <uuid>` is a non-interactive, permanent delete that also removes subagent chats, `logs/session-<id>.jsonl`, `tool-outputs/session-<id>/` and the `<slug>/<id>/` plans folder.
- **Antigravity** (`agy`, desktop app, IDE): skipped because its storage is undocumented. Community reports say `~/.gemini/antigravity-cli/conversations/<uuid>.db` plus `brain/<uuid>/`, with shared indexes (`conversation_summaries.db`, `agyhub_summaries_proto.pb`, `last_conversations.json`), and older `.pb` conversations are encrypted. Its only delete is in the TUI, and we can't tell which files a conversation owns or how its indexes react to removal.

## Releases

CI and releases run on GitHub Actions (`.github/workflows`). Binaries are unsigned for now, so don't add signing secrets or notarization steps unless asked.

## Checks before committing

Formatting uses each tool's defaults (rustfmt, Prettier); `pnpm format` fixes TypeScript and CSS formatting.

`pnpm typecheck && pnpm lint && pnpm format:check && pnpm test && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`
