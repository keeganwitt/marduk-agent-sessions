# Demo home

`pnpm demo` starts Marduk on a folder of made-up sessions instead of your real ones. We use it for screenshots and marketing material, and you can also use it to try a change against every agent's storage format, including agents you don't have installed (or don't pay for).

```sh
pnpm demo                                    # tauri dev on a fresh demo home
pnpm demo --dir ~/tmp/marduk-demo            # somewhere other than the system temp folder
pnpm demo --app /Applications/Marduk.app/Contents/MacOS/Marduk  # a built app instead of tauri dev
pnpm demo --no-padding                       # small files (see "Sizes" below)
```

Each run rewrites the demo home, so ages are relative to now: one Claude Code session shows as _Running_ for about five minutes, then turns into an ordinary session. Rerun the command to reset it, along with any labels or deletes you made. Quit the demo's Marduk first: rewriting the demo home replaces `labels.db`, and a running Marduk keeps writing to the file it opened, which then fails with "attempt to write a readonly database".

## What's in it

The sessions come from `crates/marduk-core/src/demo.rs`, which writes each agent's files the way that agent does:

- **Claude Code:** JSONL transcripts in `.claude/projects/<folder>/`, one of them running with four subagents in `<id>/subagents/`.
- **Codex:** rollouts in `.codex/sessions/YYYY/MM/DD/` and `.codex/archived_sessions/`, with spawned subagents and one subagent whose parent is gone (shown with _Parent deleted_).
- **GitHub Copilot CLI:** `events.jsonl` and `workspace.yaml` folders in `.copilot/session-state/`.
- **OpenCode:** a SQLite database with the current `session_v2` tables, including a child session and an archived one.
- **Kilo:** a SQLite database with OpenCode's older `session`/`message`/`part` tables.
- **Pi:** JSONL session trees in `.pi/agent/sessions/`, each with a reply the user rewound past, which Marduk should hide.
- **Labels:** five labels applied to eight sessions, in `marduk/labels.db`.

Every session has a short transcript with a prompt, a tool call and its output, and a markdown reply with a link, so the transcript panel has something to show.

The tests in `demo.rs` scan the demo home with every adapter and read every transcript. If an adapter change stops reading its demo sessions, `cargo test` fails, so update the demo along with the adapter. Note that the demo is only as accurate as our understanding of each agent's format, since both the adapters and the demo come from that understanding. It catches regressions, not drift in the agents themselves; checking that still needs real sessions.

## How it stays away from your real data

The demo sets these variables for Marduk (and for any agent CLI that Marduk runs):

| Variable              | Points at                           |
| --------------------- | ----------------------------------- |
| `MARDUK_HOME`         | the demo home, as Marduk's `~`      |
| `MARDUK_DATA_DIR`     | `marduk/` (labels and settings)     |
| `CLAUDE_CONFIG_DIR`   | `.claude`                           |
| `CODEX_HOME`          | `.codex`                            |
| `COPILOT_HOME`        | `.copilot`                          |
| `PI_CODING_AGENT_DIR` | `.pi/agent`                         |
| `XDG_DATA_HOME`       | `.local/share` (OpenCode and Kilo)  |
| `OPENCODE_DB`         | `.local/share/opencode/opencode.db` |
| `KILO_DB`             | `.local/share/kilo/kilo.db`         |

Every agent's own variable is set, rather than only `MARDUK_HOME`, so that one already exported in your shell can't point the demo at real sessions. We don't override `HOME`, because `tauri dev` would pass it on to Cargo and pnpm, which would then look for their caches in the demo home.

Some things are still shared with your normal copy of Marduk:

- **Agent CLIs:** Marduk still finds `codex`, `opencode` and `kilo` on your `PATH`. They get the variables above, so a delete or archive in the demo acts on the demo's files, but they're the real CLIs. Without them, Codex, OpenCode and Kilo sessions are read-only, as they would be on any machine without those agents.
- **Webview storage:** the theme and column layout are kept in the webview's local storage, which the demo doesn't move.
- **`--app`:** only builds that understand `MARDUK_HOME` and `MARDUK_DATA_DIR` are safe to run this way. An older build would show the demo sessions but save labels and settings with your real ones.

To use the demo home without the launcher (from another tool, say), write it and print the variables yourself:

```sh
cargo run -p marduk-core --example demo-home -- /tmp/marduk-demo          # VAR=value lines
cargo run -p marduk-core --example demo-home -- /tmp/marduk-demo --json   # a JSON object
```

The example refuses to write into a non-empty folder unless an earlier demo created it (it leaves a `.marduk-demo` marker), so a mistyped path can't wipe real files.

## Sizes

By default, transcripts are padded to the sizes listed in `demo.rs`, so the Size column and the header total look realistic. The padding is a hole in the file, which takes no disk space on file systems with sparse files (APFS, ext4). NTFS only makes a file sparse when asked to, which the demo doesn't do, so on Windows the padding is real and adds up to about 150 MB; use `--no-padding` if that matters. OpenCode and Kilo sessions are rows in a database, so they're padded with a snapshot part instead, which transcripts don't show. That padding takes real disk space, so those sessions are kept under 400 KB.

## Updating the README screenshot

1. Run `pnpm demo`.
2. Resize the window to about 1330 × 860 points, so that every default column fits without scrolling sideways.
3. Expand _Add dark mode to the settings page_ to show its subagents.
4. On macOS, capture the window with `screencapture -o -l <window id> docs/screenshot.png` (Screen Recording permission is required), or press ⇧⌘4, then Space, then ⌥-click the window to leave out the shadow.

Take the screenshot within five minutes of starting the demo, while the _Running_ badge is still showing.
