# Deferred ideas

Ideas we looked into and set aside, with what stopped them, so nobody has to redo the research. Before picking one up again, check whether the blocker still holds: agents change quickly.

## Agents

### Gemini CLI

Skipped because most of its users are moving to Antigravity, and Google stopped serving Gemini CLI to free, AI Pro and Ultra users on 2026-06-18. It's still maintained for Code Assist Standard/Enterprise and paid API or Vertex users.

Nothing technical blocks it. If it comes back:

- **Storage:** `~/.gemini/tmp/<project-slug>/chats/session-*.jsonl` (or under `$GEMINI_CLI_HOME`). The file is JSONL with `$set`, `$rewindTo` and `$patch` records that have to be replayed. `~/.gemini/projects.json` maps slugs to project folders. Subagent chats are in a subfolder named after the parent session.
- **Delete:** `gemini --delete-session <uuid>` doesn't prompt and is permanent. It also removes subagent chats, `logs/session-<id>.jsonl`, `tool-outputs/session-<id>/` and the `<slug>/<id>/` plans folder, and there's no central index to leave stale. It probably only sees the current project, so it would have to run from the session's project folder. How it identifies the project on Windows hasn't been checked.
- **Without the CLI:** move that same set of files to the Trash. That matters because people who switched to Antigravity may have uninstalled Gemini, and its 30-day cleanup only runs when the CLI starts.

### Antigravity (`agy` CLI, desktop app, IDE)

Skipped because its storage is undocumented, and we can't safely delete from it.

- **Unknown file set:** community reverse-engineering says a conversation is `~/.gemini/antigravity-cli/conversations/<uuid>.db` plus `brain/<uuid>/`. The desktop app uses `~/.gemini/antigravity/` and the IDE `~/.gemini/antigravity-ide/`. Older `.pb` conversations are encrypted.
- **Shared indexes:** `conversation_summaries.db`, `agyhub_summaries_proto.pb`, `last_conversations.json` and `annotations/*.pbtxt` would probably keep entries for a conversation moved to the Trash. Nothing documents whether Antigravity then shows a ghost conversation, shows an error or cleans up after itself.
- **No delete command:** `agy` deletes only inside its interactive picker (CLAUDE.md doesn't count interactive-only deletes), and archive is only in the desktop app. `agy` also deletes its own oldest conversations once there are more than 500.
- **ACP:** the docs say `agy acp` can list, load and resume sessions, which could give read-only listing. Not tested: none of Antigravity's products were installed. Whether the desktop app or IDE expose ACP is unknown.

### Cursor

Skipped because there wasn't a good way to integrate with the agent.

- **ACP lists too little:** `cursor-agent`'s ACP server returns a session's ID, folder, title and last-updated time (tested read-only). But it only lists sessions created through ACP clients like Zed, not chats started with the `cursor-agent` command itself. It has no delete or archive either. Only the no-folder and home-folder filters were tried.
- **CLI chats:** each is its own folder under `~/.cursor/chats/` with a small SQLite file, so the Trash would be a clean delete. But reading them would mean reverse-engineering an undocumented format.
- **Editor chats:** these are inside Cursor's main app database (310 MB here). At most that could be read-only, since Marduk never writes to an agent's database.

### Devin / Windsurf

Skipped for the same reasons as Antigravity, going by AllSessions' notes:

- **Desktop app:** one SQLite database per session under `acp-messages/`, with titles and folders in a shared `state.vscdb`. Moving a session's database to the Trash would leave stale entries there.
- **CLI:** keeps everything in one shared `sessions.db`. Removing a session means writing to it, which Marduk doesn't do.

Reading is possible, though: unlike Antigravity's, these aren't encrypted. Read-only support would work.

### Amp

Skipped because Amp keeps threads on its server. Marduk is built to read local files and never deals with accounts or authentication; supporting Amp would make every scan contact Amp's server through the user's logged-in CLI.

- **Local files are a stale cache:** `~/.local/share/amp/threads/T-*.json` held 29 threads, the newest from May 2026, and only 8 of them were still on the server, which listed 114.
- **The CLI has everything, non-interactively:**
  - `amp threads list --json` returns id, title, updated, `tree` (the project as a `file://` URI, or null) and message count. There's no archived flag: archived threads are the ones `--include-archived` adds.
  - `amp threads export <id>` returns the full thread: messages with `text`, `thinking`, `tool_use` and `tool_result` blocks, the model under each assistant message's `usage`, and `meta.visibility`.
  - `amp threads delete <id>` deletes permanently, on the server and locally. For a thread shared with a workspace, that deletes it for everyone.
  - `amp threads archive <id>` (and `--unarchive`).
  - `amp threads rename <id> <name>`, a native rename (see [Renaming sessions](#renaming-sessions)).
- **Sizes aren't available**, so size filters wouldn't apply, and the model is only known after an export.
- **If revisited:** it should be opt-in in Settings, report a logged-out CLI as an incomplete scan, and give the demo a fake `amp` script.

### ChatGPT desktop app

Not feasible. Chats live on OpenAI's servers, and the Mac app keeps no local copy (only an `app_pairing_extensions` folder). There's no public API for listing or deleting consumer chats. The only option would be importing a ChatGPT data export as read-only, which isn't worth building.

## UI

### Filter by source (CLI, desktop app, VS Code)

Some agents' sessions don't say which surface created them. Claude Code and Codex write the same files from the CLI, the desktop app and the IDE extensions, so for those the filter would be guessing.

### Renaming sessions

Wanted so a session's title is easier to find again, and to resume from, in the agent itself. That needs the agent's own name to change; a name stored only in Marduk doesn't show up in the agent's session list (Claude Code's `/resume`, Codex's picker). It would also leave two names for one session. A searchable note field was considered for the same reason and rejected.

Blocked because no agent has a non-interactive rename command, and Marduk doesn't write to agents' data:

- **Claude Code:** `/rename` works only inside a session. It appends a `custom-title` record to the transcript, and writing that from outside could collide with the agent writing the same file.
- **Codex:** thread names live in Codex's own `session_index`, which Marduk would have to edit.

Revisit if an agent adds a rename command Marduk could run, like `codex delete` for deletes.
