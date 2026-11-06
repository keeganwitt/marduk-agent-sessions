use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, DeletePlan, ScanResult};
use crate::fs_util;
use crate::jsonl::{self, Span};
use crate::session::Session;
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// Copilot's delete is an in-session slash command (`/session delete`), so Marduk moves the
/// folder to the Trash, which leaves Copilot's search index pointing at it.
const INDEX_WARNING: &str =
    "Copilot's session search index keeps an entry for this session until you run `/chronicle reindex` in Copilot.";

/// GitHub Copilot CLI keeps one folder per session at `session-state/<id>/`, holding an
/// `events.jsonl` event log and a flat `workspace.yaml` with the name, summary and folders.
pub struct CopilotAdapter {
    sessions_dir: PathBuf,
}

impl CopilotAdapter {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    pub fn from_env() -> Self {
        let home = env_dir("COPILOT_HOME").unwrap_or_else(|| home_dir().join(".copilot"));
        Self::new(home.join("session-state"))
    }
}

impl AgentAdapter for CopilotAdapter {
    fn id(&self) -> &'static str {
        "copilot"
    }

    fn display_name(&self) -> &'static str {
        "GitHub Copilot CLI"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.sessions_dir.clone()]
    }

    fn scan(&self) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let entries = match fs::read_dir(&self.sessions_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return result,
            Err(error) => {
                result.record_error(format!("{}: {error}", self.sessions_dir.display()));
                return result;
            }
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let events = dir.join("events.jsonl");
            if !dir.is_dir() || !events.is_file() {
                continue;
            }
            match read_session(&dir, &events) {
                Ok(session) => result.sessions.push(session),
                Err(error) => result.record_error(format!("{}: {error}", events.display())),
            }
        }
        result
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        DeletePlan::Trash {
            files: session.files.clone(),
            warning: Some(INDEX_WARNING.to_string()),
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        read_transcript(&session.primary_file)
    }
}

/// The keys Marduk uses from `workspace.yaml`, which is a flat `key: value` list.
#[derive(Default, Debug, PartialEq)]
struct Workspace {
    id: Option<String>,
    cwd: Option<String>,
    git_root: Option<String>,
    name: Option<String>,
    summary: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            let inner = &value[1..value.len() - 1];
            return if quote == '"' {
                inner.replace("\\\"", "\"").replace("\\n", "\n").replace("\\\\", "\\")
            } else {
                inner.replace("''", "'")
            };
        }
    }
    value.to_string()
}

/// Reads top-level scalars, including `|` and `>` block scalars. Nested mappings are skipped.
fn parse_workspace(text: &str) -> Workspace {
    let mut workspace = Workspace::default();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') || line.starts_with([' ', '\t']) {
            continue;
        }
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let rest = rest.trim();
        let value = if let Some(style) = rest.chars().next().filter(|c| matches!(c, '|' | '>')) {
            let mut block = Vec::new();
            while let Some(next) = lines.peek() {
                if !next.trim().is_empty() && !next.starts_with([' ', '\t']) {
                    break;
                }
                block.push(next.trim());
                lines.next();
            }
            while block.last().is_some_and(|line| line.is_empty()) {
                block.pop();
            }
            block.join(if style == '>' { " " } else { "\n" })
        } else {
            unquote(rest)
        };
        let value = Some(value).filter(|value| !value.is_empty() && value != "null" && value != "~");
        match key.trim() {
            "id" => workspace.id = value,
            "cwd" => workspace.cwd = value,
            "git_root" => workspace.git_root = value,
            "name" => workspace.name = value,
            "summary" => workspace.summary = value,
            "created_at" => workspace.created_at = value,
            "updated_at" => workspace.updated_at = value,
            _ => {}
        }
    }
    workspace
}

#[derive(Default)]
struct Parsed {
    session_id: Option<String>,
    start_cwd: Option<String>,
    first_prompt: Option<String>,
    span: Span,
}

fn parse_line(parsed: &mut Parsed, line: &str) {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return;
    };
    parsed
        .span
        .observe(record["timestamp"].as_str().and_then(jsonl::parse_timestamp));
    let data = &record["data"];
    match record["type"].as_str() {
        Some("session.start") => {
            parsed.session_id = data["sessionId"]
                .as_str()
                .filter(|id| !id.is_empty())
                .map(str::to_string);
            parsed.start_cwd = ["cwd", "gitRoot", "git_root"]
                .iter()
                .find_map(|key| data["context"][key].as_str().or_else(|| data[key].as_str()))
                .filter(|dir| !dir.is_empty())
                .map(str::to_string);
        }
        Some("user.message") if parsed.first_prompt.is_none() => {
            parsed.first_prompt = data["content"].as_str().map(str::to_string);
        }
        _ => {}
    }
}

fn read_session(dir: &Path, events: &Path) -> std::io::Result<Session> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(events, |line| parse_line(&mut parsed, line))?;
    let workspace = fs::read_to_string(dir.join("workspace.yaml"))
        .map(|text| parse_workspace(&text))
        .unwrap_or_default();
    parsed
        .span
        .observe(workspace.updated_at.as_deref().and_then(jsonl::parse_timestamp));
    let fingerprint = fs_util::fingerprint(events)?;
    let id = parsed
        .session_id
        .or(workspace.id)
        .unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().into_owned());
    let title = [workspace.name, workspace.summary, parsed.first_prompt]
        .into_iter()
        .flatten()
        .find_map(|text| jsonl::to_title(&text));
    let created = workspace
        .created_at
        .as_deref()
        .and_then(jsonl::parse_timestamp)
        .or(parsed.span.first)
        .or_else(|| fs_util::created(events));
    Ok(Session {
        agent: "copilot",
        id,
        project_dir: workspace.cwd.or(workspace.git_root).or(parsed.start_cwd),
        title,
        created,
        last_activity: parsed.span.last.unwrap_or_else(|| fingerprint.modified.into()),
        size_bytes: fs_util::path_size(dir),
        archived: false,
        primary_file: events.to_path_buf(),
        files: vec![dir.to_path_buf()],
        fingerprint,
        parent_id: None,
    })
}

/// Tool results are a string, a `{ content, detailedContent }` object, or content blocks.
fn result_text(value: &Value) -> String {
    match value {
        Value::Object(fields) => fields
            .get("content")
            .or_else(|| fields.get("detailedContent"))
            .or_else(|| fields.get("message"))
            .map(transcript::tool_output_text)
            .unwrap_or_else(|| transcript::tool_input_text(value)),
        other => transcript::tool_output_text(other),
    }
}

/// `assistant.message.toolRequests` and `tool.execution_start` describe the same call, so
/// each tool call id is recorded once.
fn read_transcript(path: &Path) -> std::io::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    let mut seen_calls = HashSet::new();
    let mut names: HashMap<String, String> = HashMap::new();
    jsonl::for_each_line(path, |line| {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let timestamp = record["timestamp"].as_str().and_then(jsonl::parse_timestamp);
        let data = &record["data"];
        let mut call = |builder: &mut TranscriptBuilder, id: &str, name: &str, arguments: &Value| {
            if !id.is_empty() {
                names.insert(id.to_string(), name.to_string());
                if !seen_calls.insert(id.to_string()) {
                    return;
                }
            }
            let id = Some(id).filter(|id| !id.is_empty());
            builder.tool_call(id, name, &transcript::tool_input_text(arguments), timestamp);
        };
        match record["type"].as_str() {
            Some("user.message") => builder.text(Role::User, data["content"].as_str().unwrap_or_default(), timestamp),
            Some("assistant.message") => {
                builder.text(Role::Assistant, data["content"].as_str().unwrap_or_default(), timestamp);
                for request in data["toolRequests"].as_array().into_iter().flatten() {
                    call(
                        &mut builder,
                        request["toolCallId"].as_str().unwrap_or_default(),
                        request["name"].as_str().unwrap_or("tool"),
                        &request["arguments"],
                    );
                }
            }
            Some("tool.execution_start") => call(
                &mut builder,
                data["toolCallId"].as_str().unwrap_or_default(),
                data["toolName"].as_str().unwrap_or("tool"),
                &data["arguments"],
            ),
            Some("tool.execution_complete") => {
                let output = if data["result"].is_null() {
                    result_text(&data["error"])
                } else {
                    result_text(&data["result"])
                };
                builder.tool_result(
                    data["toolCallId"].as_str().filter(|id| !id.is_empty()),
                    &output,
                    timestamp,
                );
            }
            Some("session.compaction_complete") => builder.text(
                Role::Context,
                data["summaryContent"].as_str().unwrap_or_default(),
                timestamp,
            ),
            Some("session.task_complete") => {
                builder.text(Role::Context, data["summary"].as_str().unwrap_or_default(), timestamp)
            }
            _ => {}
        }
    })?;
    Ok(builder.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "459987cf-cdb8-4ba0-9065-f470cbe762ce";

    fn write_session(root: &Path, dir_name: &str, events: &[&str], workspace: Option<&str>) -> PathBuf {
        let dir = root.join(dir_name);
        fs::create_dir_all(dir.join("checkpoints")).unwrap();
        fs::write(dir.join("checkpoints").join("1.json"), "{}").unwrap();
        if let Some(workspace) = workspace {
            fs::write(dir.join("workspace.yaml"), workspace).unwrap();
        }
        fs::write(dir.join("events.jsonl"), events.join("\n")).unwrap();
        dir
    }

    fn events() -> Vec<&'static str> {
        vec![
            r#"{"type":"session.start","timestamp":"2026-04-25T15:42:52.327Z","data":{"sessionId":"459987cf-cdb8-4ba0-9065-f470cbe762ce","context":{"cwd":"/work/ax","branch":"main"}}}"#,
            r#"{"type":"user.message","timestamp":"2026-04-25T15:43:00Z","data":{"content":"Fix the login bug"}}"#,
            r#"{"type":"assistant.message","timestamp":"2026-04-25T15:43:05Z","data":{"content":"Looking at the code","reasoningText":"find entry point","toolRequests":[{"toolCallId":"call-1","name":"bash","arguments":{"command":"ls"}}]}}"#,
            r#"{"type":"tool.execution_start","timestamp":"2026-04-25T15:43:06Z","data":{"toolCallId":"call-1","toolName":"bash","arguments":{"command":"ls"}}}"#,
            r#"{"type":"tool.execution_complete","timestamp":"2026-04-25T15:43:07Z","data":{"toolCallId":"call-1","success":true,"result":{"content":"login.ts"}}}"#,
            r#"{"type":"session.task_complete","timestamp":"2026-04-25T15:50:00Z","data":{"summary":"Fixed the redirect"}}"#,
            "not json",
        ]
    }

    fn scan_one(root: &Path) -> Session {
        let result = CopilotAdapter::new(root.to_path_buf()).scan();
        assert!(result.complete, "{:?}", result.errors);
        assert_eq!(result.sessions.len(), 1);
        result.sessions.into_iter().next().unwrap()
    }

    #[test]
    fn scans_events_and_prefers_workspace_name() {
        let root = tempfile::tempdir().unwrap();
        let dir = write_session(
            root.path(),
            ID,
            &events(),
            Some(
                "id: other\ncwd: /work/ax-repo\nname: \"Login fix\"\nsummary: |\n  Fixed the\n  redirect\ncreated_at: 2026-04-25T15:42:00Z\nupdated_at: 2026-04-25T16:00:00Z\n",
            ),
        );

        let session = scan_one(root.path());

        assert_eq!(session.key(), format!("copilot:{ID}"));
        assert_eq!(session.title.as_deref(), Some("Login fix"));
        assert_eq!(session.project_dir.as_deref(), Some("/work/ax-repo"));
        assert_eq!(session.created.unwrap().to_rfc3339(), "2026-04-25T15:42:00+00:00");
        assert_eq!(session.last_activity.to_rfc3339(), "2026-04-25T16:00:00+00:00");
        assert_eq!(session.files, vec![dir.clone()]);
        assert_eq!(session.primary_file, dir.join("events.jsonl"));
        assert!(session.size_bytes > 0);
    }

    #[test]
    fn falls_back_to_summary_then_first_prompt_and_start_cwd() {
        let root = tempfile::tempdir().unwrap();
        write_session(
            root.path(),
            ID,
            &events(),
            Some("summary: >\n  Fixed the\n  redirect\n"),
        );
        assert_eq!(scan_one(root.path()).title.as_deref(), Some("Fixed the redirect"));

        let other = tempfile::tempdir().unwrap();
        write_session(other.path(), ID, &events(), None);
        let session = scan_one(other.path());
        assert_eq!(session.title.as_deref(), Some("Fix the login bug"));
        assert_eq!(session.project_dir.as_deref(), Some("/work/ax"));
    }

    #[test]
    fn folders_without_events_are_ignored() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("empty")).unwrap();
        fs::write(root.path().join("stray.txt"), "").unwrap();
        let result = CopilotAdapter::new(root.path().to_path_buf()).scan();
        assert!(result.complete);
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn transcript_deduplicates_tool_calls_and_attaches_results() {
        let root = tempfile::tempdir().unwrap();
        write_session(root.path(), ID, &events(), None);
        let adapter = CopilotAdapter::new(root.path().to_path_buf());
        let session = adapter.scan().sessions.remove(0);

        let messages = adapter.transcript(&session).unwrap().head;

        let roles: Vec<Role> = messages.iter().map(|message| message.role).collect();
        assert_eq!(roles, vec![Role::User, Role::Assistant, Role::Tool, Role::Context]);
        let tool = messages[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "bash");
        assert_eq!(tool.output.as_deref(), Some("login.ts"));
    }

    #[test]
    fn delete_trashes_the_whole_session_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = write_session(root.path(), ID, &events(), None);
        let adapter = CopilotAdapter::new(root.path().to_path_buf());
        let session = adapter.scan().sessions.remove(0);
        assert_eq!(
            adapter.delete_plan(&session),
            DeletePlan::Trash {
                files: vec![dir],
                warning: Some(INDEX_WARNING.to_string()),
            }
        );
    }

    #[test]
    fn workspace_parser_handles_quotes_blocks_and_nesting() {
        let workspace = parse_workspace(
            "# comment\nid: 'it''s'\nname: \"say \\\"hi\\\"\"\nnested:\n  name: ignored\nsummary: |\n  line one\n  line two\n\ncwd: null\n",
        );
        assert_eq!(workspace.id.as_deref(), Some("it's"));
        assert_eq!(workspace.name.as_deref(), Some("say \"hi\""));
        assert_eq!(workspace.summary.as_deref(), Some("line one\nline two"));
        assert_eq!(workspace.cwd, None);
    }
}
