# Marduk

Marduk is a cross-platform desktop app for finding and cleaning up old AI coding-agent sessions.

It scans the transcripts that coding agents keep on disk and shows them in one list. You can filter by agent, age, size or project, then move the sessions you don't need to the system Trash.

## Supported agents

| Agent | Location | Delete |
|---|---|---|
| Claude Code | `~/.claude/projects` (or `$CLAUDE_CONFIG_DIR/projects`) | Moves the transcript, its `<id>.*` sidecar files and its `<id>/` folder (subagents, tool results) to the Trash |
| Pi | `~/.pi/agent/sessions` (or `$PI_CODING_AGENT_DIR/sessions`) | Moves the transcript to the Trash |

Codex and OpenCode are next.

## Safety rules

- **Trash only.** Sessions go to the system Trash (Recycle Bin on Windows). If that fails, Marduk reports the error and does not delete the session permanently instead.
- **Running sessions are never deleted.** A session whose transcript changed in the last 5 minutes is marked *Running* and refused.
- **Changed since scan is refused.** If a transcript's size or modification time changed after the scan, Marduk refuses to delete it and asks you to rescan.
- **Confined to agent folders.** Every path is resolved (following `..` and symlinks) and must sit inside the agent's session folder.
- **Agent databases are never written to.** For agents that keep sessions in a database, Marduk will only delete through the agent's own CLI.
- **No invented archives.** Marduk doesn't create archive folders that agents don't know about.

## Development

Requirements: Rust (stable), Node.js 22 and pnpm 10. On Linux you also need the [Tauri system dependencies](https://tauri.app/start/prerequisites/).

```sh
pnpm install
pnpm tauri dev                          # run the app
pnpm test                               # frontend tests (Vitest)
cargo test --workspace                  # Rust tests
cargo run -p marduk-core --example scan # read-only scan of this machine, for checking adapters
```

The repo layout:

- `crates/marduk-core`: agent adapters, scanning and delete safety checks. It has no Tauri dependency, so you can test it with plain `cargo test`.
- `src-tauri`: the Tauri app. It contains thin commands over `marduk-core::catalog`.
- `src`: the React + TypeScript UI.

## License

[Apache License 2.0](LICENSE)
