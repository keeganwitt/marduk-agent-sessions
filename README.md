# Marduk

<img src="assets/marduk.svg" alt="Marduk holding a spade" width="192" height="192">

In the Babylonian creation epic, the _Enūma Eliš_, Marduk defeats Tiamat, the primordial sea, and creates the world from her body: half becomes the sky and half the earth. He gives the stars their stations and the year its months, bringing order out of chaos. His symbol is the _marru_, a spade, which is what he's holding above. (His name is pronounced MAR-dook, with the vowel of "book", not "duck".)

Your coding agents are less monstrous than Tiamat, but left alone they make their own kind of chaos: thousands of sessions spread across Claude Code, Codex, OpenCode and the rest, each stored in its own format and folder, most of which nobody will open again. Marduk is a cross-platform desktop app that puts them in order. It finds every agent's sessions, shows them in one list, and helps you clean up the ones you no longer need.

It reads the transcripts that coding agents keep on disk. You can filter by agent, age, size or project, then delete the sessions you don't need, or archive them when the agent supports archiving.

![Marduk listing Claude Code, Codex, OpenCode, GitHub Copilot and Pi sessions, with labels, models, a running session, expanded subagents, two selected sessions whose delete includes a Guardian review, and the + Filter menu open](docs/screenshot.png)

## Supported agents

| Agent                        | Location                                                                                                                                   | Delete                                                                                                                                                                                                  | Archive                                                          |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Claude Code                  | `~/.claude/projects` (or `$CLAUDE_CONFIG_DIR/projects`)                                                                                    | Moves the transcript, its `<id>.*` sidecar files and its `<id>/` folder (subagents, tool results) to the Trash                                                                                          | —                                                                |
| Codex                        | `~/.codex/sessions` and `~/.codex/archived_sessions` (or `$CODEX_HOME`)                                                                    | **Permanent**, through `codex delete --force <id>`, which also deletes the subagent sessions it spawned (not its Guardian reviews); needs the Codex CLI                                                 | `codex archive` / `codex unarchive`                              |
| GitHub Copilot (CLI and app) | `~/.copilot/session-state/<id>/` (or `$COPILOT_HOME/session-state`)                                                                        | Moves the session folder to the Trash; Copilot's own delete is only in its interactive UI. Run `/chronicle reindex` in Copilot afterwards to update its search index. A copy synced to GitHub.com stays | — (the app's archive is UI-only; the CLI has none)               |
| Copilot Chat (VS Code)       | `<VS Code user folder>/workspaceStorage/<id>/chatSessions/` and `globalStorage/emptyWindowChatSessions/` (also VS Code Insiders, VSCodium) | Moves the chat's `.json`/`.jsonl` file and its edit checkpoints to the Trash. VS Code has no delete command, and its chat history may still list the chat until you use Delete on it in VS Code         | — (VS Code has no chat archive)                                  |
| OpenCode                     | `~/.local/share/opencode/opencode*.db` (or `$XDG_DATA_HOME/opencode`, `$OPENCODE_DB`)                                                      | **Permanent**, through `opencode session delete <id>`, which also deletes the session's child sessions; needs the OpenCode CLI                                                                          | Shows OpenCode's archived state; OpenCode has no archive command |
| Kilo                         | `~/.local/share/kilo/kilo*.db` (or `$XDG_DATA_HOME/kilo`, `$KILO_DB`)                                                                      | **Permanent**, through `kilo session delete <id>`, which also deletes child sessions; needs the Kilo CLI                                                                                                | Shows Kilo's archived state; Kilo has no archive command         |
| Pi                           | `~/.pi/agent/sessions` (or `$PI_CODING_AGENT_DIR/sessions`)                                                                                | Moves the transcript to the Trash                                                                                                                                                                       | —                                                                |

OpenCode and Kilo (an OpenCode fork) keep every session in one SQLite database. Marduk opens it read-only, reads both OpenCode's current (`session_v2`) and older (`session`) tables, and never writes to it. If a session's messages are stored in a shape Marduk doesn't recognize (a newer OpenCode, say), the transcript is read through the agent's export command (`opencode session export`, `kilo export`) instead.

When an agent has its own delete or archive command, Marduk uses it, so the agent's own indexes stay consistent. Otherwise sessions go to the system Trash. Claude Code and Pi have no per-session delete command; Pi's own session picker also uses the Trash.

## Subagent sessions

Sessions an agent started on its own (Codex Guardian reviews and spawned subagents, OpenCode and Kilo child sessions) are nested under the session that started them. Expand a parent to see them. They go with their parent: select the parent to delete it and them. Codex leaves a session's Guardian reviews behind when it deletes the session, so Marduk deletes them too. A subagent whose parent was deleted is shown at the top level with a _Parent deleted_ badge, and can be selected on its own. Use **+ Filter → Parent deleted** to find these sessions. Claude Code keeps its subagents inside the parent session's folder (`<session>/subagents/`), so they appear under their session titled from their description and agent type, and can be read but not deleted, archived or labeled on their own: deleting the parent moves them to the Trash with it.

## Labels

Select sessions and use **Labels ▾** to add or remove labels, or create a new one by typing its name. Filter by label from the _Label_ dropdown, and rename, recolor or delete labels in **Settings**. Labels are stored by Marduk (in `labels.db` in its app data folder), keyed by agent and session id, so they survive an agent archiving or moving a session. Deleting a session in Marduk removes its labels; labels of sessions deleted some other way are removed after they've been missing from complete scans for 7 days.

## Agent memory

The **Memory** tab shows what agents remember between sessions.

![The Memory tab showing a project's Claude Code memory files, a warning about a memory missing from MEMORY.md, and the session each memory came from](docs/memory.png)

- **Claude Code** keeps memory per project, in `~/.claude/projects/<project>/memory/`: a `MEMORY.md` index that it loads into every session, plus one file per memory. Marduk lists each project's memory files with their type, description and the session that wrote them, and warns when the index is longer than the 200 lines Claude Code loads, when a memory isn't listed in the index, when the index links to a missing file, or when the project folder no longer exists. You can read the files, show the folder, or move a project's memory to the Trash.
- **Codex** keeps one memory store for all projects, in `~/.codex/memories/` (or `memories_v2/`), when its memories feature is on. Codex regenerates these files itself, so Marduk only shows them, and links each session summary to its session. Clear them with Codex's own **Settings › Personalization › Delete Codex memories**.

Deleting sessions doesn't make Claude Code's memory useless: it's per project, and Claude Code uses it again the next time you work there. When a delete removes a project's last session, the confirmation offers to move that project's memory to the Trash too, unchecked by default.

## Reading transcripts

![A Claude Code transcript in the side panel, with collapsed tool calls, a code block and a table](docs/transcript.png)

Click a session's title to read it in a side panel. Messages are rendered as Markdown, tool calls are collapsed with their output, and injected context (system prompts, AGENTS.md) is hidden unless you turn it on. Very long sessions show their first and last 500 messages. For Pi, only the active branch is shown, not abandoned rewinds.

## Safety rules

The app explains these too: **Help** has a tab per topic, and an **Agents** tab with how each agent deletes and archives and which folders Marduk reads on your machine.

**Help → Check for updates** checks GitHub for the latest Marduk release and offers a download link when a newer version is available. Checks run only when clicked; downloads open in your browser.

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
- **Linux:** WebKitGTK 4.1 and related development packages; see [Linux prerequisites](#linux-prerequisites) below for commands.

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

### Linux prerequisites

Install Rust stable, Node.js 22 and pnpm 10, then install the system packages for your distribution.

On Ubuntu 22.04 or later, or Debian 12 or later:

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config curl wget file \
  libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

On Fedora:

```sh
sudo dnf install -y gcc gcc-c++ make pkgconf-pkg-config curl wget file \
  webkit2gtk4.1-devel openssl-devel libappindicator-gtk3-devel \
  librsvg2-devel libxdo-devel
```

For other distributions, see the [Tauri Linux prerequisites](https://tauri.app/start/prerequisites/#linux).

### Building installers

Run these commands from the repository root on the OS you are building for. First install the frontend dependencies:

```sh
pnpm install --frozen-lockfile
```

| Build host | Command                              | Installer output                                                        |
| ---------- | ------------------------------------ | ----------------------------------------------------------------------- |
| Linux      | `pnpm tauri build --bundles deb,rpm` | `target/release/bundle/deb/*.deb` and `target/release/bundle/rpm/*.rpm` |
| macOS      | `pnpm tauri build --bundles dmg`     | `target/release/bundle/dmg/*.dmg`                                       |
| Windows    | `pnpm tauri build --bundles nsis`    | `target/release/bundle/nsis/*-setup.exe`                                |

Tauri builds the frontend and Rust application before packaging. The packages use the build host's architecture. On macOS, add `--target aarch64-apple-darwin` for Apple Silicon or `--target x86_64-apple-darwin` for Intel, after installing that target with `rustup target add <target>`. With an explicit target, the output is under `target/<target>/release/bundle/`.

Linux builds depend on the build host's system libraries. Build release packages on Ubuntu 22.04 to avoid requiring the newer glibc versions of newer build hosts; destination systems still need compatible GTK and WebKitGTK 4.1 libraries. See [Tauri's Linux compatibility guidance](https://v2.tauri.app/distribute/debian/#limitations).

Pushing a `v*` tag runs the checks in [GitHub Actions](.github/workflows/ci.yml), then the [release workflow](.github/workflows/release.yml) builds Linux x86_64 `.deb` and `.rpm` packages, Apple Silicon and Intel `.dmg` files, and a Windows x86_64 `.exe` installer. The installers are uploaded to a draft GitHub release for review before publication.

To release from a clean `main` branch, run:

```powershell
pnpm release 0.1.0
```

The command accepts a stable version (`X.Y.Z`, optionally prefixed with `v`). It checks for existing local and remote tags and requires local `main` to contain the latest `origin/main` commit. If remote changes need integrating, it stops before changing files or tagging; fetch and integrate those changes, then rerun it.

It updates `package.json`, `src-tauri/tauri.conf.json`, the workspace version in `Cargo.toml`, and both workspace packages in `Cargo.lock`, creates a release commit when versions change, and creates an annotated `v<version>` tag. The commit and tag are pushed together atomically to `origin`. Versions cannot go backwards.

The first `0.1.0` release can use this command too: when the files already have the requested version, it tags the current commit without creating a redundant commit. Commit the release script itself before running it. Once GitHub Actions finishes, review the draft release's installers and notes, then publish it on GitHub.

If the push fails, the local commit and tag remain available and the command prints the Git push command to retry after resolving the failure.

## Installing builds without a verified publisher

macOS builds use an [ad hoc code signature](https://v2.tauri.app/distribute/sign/macos/#ad-hoc-signing), which is needed for downloaded Apple Silicon apps. They are not signed with an Apple Developer ID or notarized, so macOS still requires approval. Windows builds are unsigned.

- **macOS:** Download the `aarch64` DMG for Apple Silicon or the `x64` DMG for Intel, then drag Marduk into Applications. Try opening it, then go to **System Settings → Privacy & Security → Open Anyway** and confirm. See [Apple's instructions](https://support.apple.com/en-us/102445).
- **Windows:** in the SmartScreen prompt, choose **More info**, then **Run anyway**.

For an older macOS build that reports “Marduk is damaged and can't be opened,” first confirm it came from this repository's GitHub release. With Marduk copied to Applications, remove its download quarantine attribute and try opening it again:

```sh
xattr -dr com.apple.quarantine /Applications/Marduk.app
```

This bypasses the download quarantine check for Marduk; it does not verify the app's publisher or repair a broken code signature. If it still fails, collect the signature check output with `codesign --verify --deep --strict --verbose=2 /Applications/Marduk.app`.

## License

[MIT License](LICENSE)
