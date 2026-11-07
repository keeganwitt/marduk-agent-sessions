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

### ChatGPT desktop app

Not feasible. Chats live on OpenAI's servers, and the Mac app keeps no local copy (only an `app_pairing_extensions` folder). There's no public API for listing or deleting consumer chats. The only option would be importing a ChatGPT data export as read-only, which isn't worth building.

## UI

### Filter by source (CLI, desktop app, VS Code)

Some agents' sessions don't say which surface created them. Claude Code and Codex write the same files from the CLI, the desktop app and the IDE extensions, so for those the filter would be guessing.
