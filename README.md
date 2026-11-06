# Marduk

<img src="assets/marduk.svg" alt="Marduk holding a spade" width="192" height="192">

Marduk is a cross-platform desktop app for finding and cleaning up old AI coding-agent sessions.

It scans the transcripts that coding agents keep on disk and shows them in one list. You can filter by agent, age, size or project, then delete the sessions you don't need, or archive them when the agent supports archiving.

![Marduk listing Claude Code, Codex, OpenCode, GitHub Copilot CLI and Pi sessions, with labels, a running session and expanded subagents](docs/screenshot.png)

## Supported agents

| Agent              | Location                                                                              | Delete                                                                                                                                                                     | Archive                                                          |
| ------------------ | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Claude Code        | `~/.claude/projects` (or `$CLAUDE_CONFIG_DIR/projects`)                               | Moves the transcript, its `<id>.*` sidecar files and its `<id>/` folder (subagents, tool results) to the Trash                                                             | —                                                                |
| Codex              | `~/.codex/sessions` and `~/.codex/archived_sessions` (or `$CODEX_HOME`)               | **Permanent**, through `codex delete --force <id>`, which also deletes the subagent sessions it spawned (not its Guardian reviews); needs the Codex CLI                    | `codex archive` / `codex unarchive`                              |
| GitHub Copilot CLI | `~/.copilot/session-state/<id>/` (or `$COPILOT_HOME/session-state`)                   | Moves the session folder to the Trash; Copilot's own delete is only an in-session slash command. Run `/chronicle reindex` in Copilot afterwards to update its search index | — (Copilot CLI has no archive)                                   |
| OpenCode           | `~/.local/share/opencode/opencode*.db` (or `$XDG_DATA_HOME/opencode`, `$OPENCODE_DB`) | **Permanent**, through `opencode session delete <id>`, which also deletes the session's child sessions; needs the OpenCode CLI                                             | Shows OpenCode's archived state; OpenCode has no archive command |
| Kilo               | `~/.local/share/kilo/kilo*.db` (or `$XDG_DATA_HOME/kilo`, `$KILO_DB`)                 | **Permanent**, through `kilo session delete <id>`, which also deletes child sessions; needs the Kilo CLI                                                                   | Shows Kilo's archived state; Kilo has no archive command         |
| Pi                 | `~/.pi/agent/sessions` (or `$PI_CODING_AGENT_DIR/sessions`)                           | Moves the transcript to the Trash                                                                                                                                          | —                                                                |

OpenCode and Kilo (an OpenCode fork) keep every session in one SQLite database. Marduk opens it read-only, reads both OpenCode's current (`session_v2`) and older (`session`) tables, and never writes to it. If a session's messages are stored in a shape Marduk doesn't recognize (a newer OpenCode, say), the transcript is read through the agent's export command (`opencode session export`, `kilo export`) instead.

When an agent has its own delete or archive command, Marduk uses it, so the agent's own indexes stay consistent. Otherwise sessions go to the system Trash. Claude Code and Pi have no per-session delete command; Pi's own session picker also uses the Trash.

## Subagent sessions

Sessions an agent started on its own (Codex Guardian reviews and spawned subagents, OpenCode and Kilo child sessions) are nested under the session that started them. Expand a parent to see them. A subagent whose parent was deleted is shown at the top level with a _Parent deleted_ badge. Claude Code keeps its subagents inside the parent session's folder (`<session>/subagents/`), so they appear under their session titled from their description and agent type, and can be read but not deleted, archived or labeled on their own: deleting the parent moves them to the Trash with it.

## Labels

Select sessions and use **Labels ▾** to add or remove labels, or create a new one by typing its name. Filter by label from the _Label_ dropdown, and rename, recolor or delete labels in **Settings**. Labels are stored by Marduk (in `labels.db` in its app data folder), keyed by agent and session id, so they survive an agent archiving or moving a session. Deleting a session in Marduk removes its labels; labels of sessions deleted some other way are removed after they've been missing from complete scans for 7 days.

## Reading transcripts

Click a session's title to read it in a side panel. Messages are rendered as Markdown, tool calls are collapsed with their output, and injected context (system prompts, AGENTS.md) is hidden unless you turn it on. Very long sessions show their first and last 500 messages. For Pi, only the active branch is shown, not abandoned rewinds.

## Safety rules

- **Trash, unless the agent deletes natively.** Sessions without a native delete command go to the system Trash (Recycle Bin on Windows). If that fails, Marduk reports the error rather than deleting permanently. Native deletes that can't be undone (Codex) are labeled _Permanent_ in the confirmation dialog, which also shows the exact command.
- **Running sessions are never changed.** A session whose transcript changed in the last 5 minutes is marked _Running_ and can't be deleted or archived.
- **Changed since scan is refused.** If a transcript's size or modification time changed after the scan, Marduk refuses to delete it and asks you to rescan.
- **Confined to agent folders.** Every path is resolved (following `..` and symlinks) and must sit inside the agent's session folder.
- **Agent databases are never written to.** For agents that keep sessions in a database, Marduk will only delete through the agent's own CLI.
- **No invented archives.** Marduk only archives through an agent's own archive command, and doesn't create archive folders that agents don't know about.
- **Agent CLIs are found outside your shell too.** Apps opened from the Dock don't get your shell's `PATH`, so Marduk also looks in `~/.local/bin`, Homebrew and other common install folders. If yours is somewhere else, choose it in **Settings → Agent CLIs**, which also shows the version Marduk will run. Marduk never installs or updates agent CLIs itself, because a different version from the agent you use could migrate its data out from under it.

## Development

Requirements: Rust (stable), Node.js 22 and pnpm 10, plus the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS:

- **macOS:** Xcode Command Line Tools (`xcode-select --install`).
- **Windows:** Microsoft C++ Build Tools ("Desktop development with C++") and the WebView2 runtime (already installed on Windows 10 and 11). Use the `x86_64-pc-windows-msvc` Rust toolchain, which is the default from rustup.
- **Linux:** the WebKitGTK and related packages listed on that page.

The repo uses LF line endings on every OS (`.gitattributes`), so Windows checkouts stay byte-for-byte the same as macOS ones.

```sh
pnpm install
pnpm tauri dev                          # run the app
pnpm demo                               # run the app on made-up sessions (docs/demo.md)
pnpm test                               # frontend tests (Vitest)
cargo test --workspace                  # Rust tests
cargo run -p marduk-core --example scan # read-only scan of this machine, for checking adapters
cargo run --release -p marduk-core --example transcript # parse each agent's largest transcript
```

`pnpm demo` needs none of the agents installed, and it never touches your real sessions or labels. See [docs/demo.md](docs/demo.md) for what's in it and how to retake the screenshot.

The repo layout:

- `crates/marduk-core`: agent adapters, scanning and delete safety checks. It has no Tauri dependency, so you can test it with plain `cargo test`.
- `src-tauri`: the Tauri app. It contains thin commands over `marduk-core::catalog`.
- `src`: the React + TypeScript UI.

## Installing unsigned builds

Release builds are not code-signed yet, so the OS warns the first time you open them:

- **macOS:** Control-click the app, choose **Open**, then confirm. If macOS says the app is damaged, run `xattr -dr com.apple.quarantine /Applications/Marduk.app`.
- **Windows:** in the SmartScreen prompt, choose **More info**, then **Run anyway**.

## License

[Apache License 2.0](LICENSE)
