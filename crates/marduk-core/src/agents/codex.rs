use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

use super::cli::{Cli, CliOverrides};
use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, AgentCommand, DeletePlan, ScanResult};
use crate::fs_util;
use crate::jsonl;
use crate::session::Session;
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// Codex keeps rollouts at `sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl` and moves them to
/// `archived_sessions/` with `codex archive`. Thread names live in `session_index.jsonl`.
pub struct CodexAdapter {
    home: PathBuf,
    cli: Cli,
}

const PROGRAM: &str = "codex";
const INSTALL_URL: &str = "https://github.com/openai/codex#installing-and-running-codex-cli";

impl CodexAdapter {
    pub fn new(home: PathBuf, cli: Option<PathBuf>) -> Self {
        Self {
            home,
            cli: Cli::fixed(PROGRAM, INSTALL_URL, cli),
        }
    }

    pub fn from_env(overrides: std::sync::Arc<CliOverrides>) -> Self {
        Self {
            home: env_dir("CODEX_HOME").unwrap_or_else(|| home_dir().join(".codex")),
            cli: Cli::detect("codex", PROGRAM, INSTALL_URL, overrides),
        }
    }

    fn sessions_dir(&self) -> PathBuf {
        self.home.join("sessions")
    }

    fn archive_dir(&self) -> PathBuf {
        self.home.join("archived_sessions")
    }

    fn command(&self, session: &Session, args: &[&str]) -> Result<AgentCommand, String> {
        if !fs_util::looks_like_uuid(&session.id) {
            return Err("Codex can only manage sessions that have a UUID id.".to_string());
        }
        let program = self
            .cli
            .path()
            .ok_or_else(|| self.cli.missing_reason("Codex", "delete and archive Codex sessions"))?;
        let mut args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        args.push(session.id.clone());
        Ok(AgentCommand { program, args })
    }
}

impl AgentAdapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.sessions_dir(), self.archive_dir()]
    }

    fn scan(&self) -> ScanResult {
        self.cli.refresh();
        let mut result = ScanResult::empty_complete();
        let titles = read_thread_names(&self.home.join("session_index.jsonl"));
        let mut compressed = 0usize;
        for (root, archived) in [(self.sessions_dir(), false), (self.archive_dir(), true)] {
            let mut files = Vec::new();
            collect_rollouts(&root, &mut files, &mut compressed, &mut result);
            for path in files {
                match read_session(&path, archived, &titles) {
                    Ok(session) => result.sessions.push(session),
                    Err(error) => result.record_error(format!("{}: {error}", path.display())),
                }
            }
        }
        if compressed > 0 {
            result.record_error(format!(
                "{compressed} compressed rollouts (.jsonl.zst) were skipped; Marduk can't read them yet."
            ));
        }
        result
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        match self.command(session, &["delete", "--force"]) {
            Ok(command) => DeletePlan::AgentCommand {
                command,
                files: session.files.clone(),
                warning: None,
            },
            Err(reason) => DeletePlan::ReadOnly { reason },
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        read_transcript(&session.primary_file)
    }

    fn cli(&self) -> Option<&Cli> {
        Some(&self.cli)
    }

    fn archive_command(&self, session: &Session) -> Option<AgentCommand> {
        let verb = if session.archived { "unarchive" } else { "archive" };
        self.command(session, &[verb]).ok()
    }
}

fn collect_rollouts(dir: &Path, files: &mut Vec<PathBuf>, compressed: &mut usize, result: &mut ScanResult) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            result.record_error(format!("{}: {error}", dir.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if file_type.is_dir() {
            collect_rollouts(&path, files, compressed, result);
        } else if name.starts_with("rollout-") && name.ends_with(".jsonl") {
            files.push(path);
        } else if name.starts_with("rollout-") && name.ends_with(".jsonl.zst") {
            *compressed += 1;
        }
    }
}

fn read_thread_names(index: &Path) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let _ = jsonl::for_each_line(index, |line| {
        if let Ok(entry) = serde_json::from_str::<Value>(line)
            && let (Some(id), Some(name)) = (entry["id"].as_str(), entry["thread_name"].as_str())
        {
            names.insert(id.to_string(), name.to_string());
        }
    });
    names
}

#[derive(Deserialize)]
struct Record<'a> {
    #[serde(borrow)]
    timestamp: Option<Cow<'a, str>>,
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    payload: Option<&'a RawValue>,
}

#[derive(Default)]
struct Parsed {
    id: Option<String>,
    cwd: Option<String>,
    first_prompt: Option<String>,
    span: jsonl::Span,
}

/// Codex injects `<environment_context>` blocks and AGENTS.md contents as user messages.
fn is_injected(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('<') || text.starts_with("# AGENTS.md")
}

fn parse_line(parsed: &mut Parsed, line: &str) {
    let Ok(record) = serde_json::from_str::<Record>(line) else {
        return;
    };
    if let Some(timestamp) = record.timestamp.as_deref() {
        parsed.span.observe(jsonl::parse_timestamp(timestamp));
    }
    let Some(payload) = record.payload else {
        return;
    };
    match record.kind.as_deref() {
        Some("session_meta") => {
            if let Ok(meta) = serde_json::from_str::<Value>(payload.get()) {
                parsed.id = meta["id"].as_str().map(str::to_string).or(parsed.id.take());
                parsed.cwd = meta["cwd"].as_str().map(str::to_string).or(parsed.cwd.take());
                parsed
                    .span
                    .observe(meta["timestamp"].as_str().and_then(jsonl::parse_timestamp));
            }
        }
        Some("event_msg") if parsed.first_prompt.is_none() && payload.get().contains("\"user_message\"") => {
            if let Ok(event) = serde_json::from_str::<Value>(payload.get())
                && event["type"] == "user_message"
                && let Some(text) = event["message"].as_str().filter(|text| !is_injected(text))
            {
                parsed.first_prompt = Some(text.to_string());
            }
        }
        Some("response_item") if parsed.first_prompt.is_none() && payload.get().contains("\"user\"") => {
            if let Ok(item) = serde_json::from_str::<Value>(payload.get())
                && item["type"] == "message"
                && item["role"] == "user"
            {
                parsed.first_prompt = jsonl::content_text(&item["content"]).filter(|text| !is_injected(text));
            }
        }
        _ => {}
    }
}

/// Uses `response_item` records only; `event_msg` repeats the same conversation.
fn read_transcript(path: &Path) -> std::io::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    jsonl::for_each_line(path, |line| {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return;
        };
        if record["type"] != "response_item" {
            return;
        }
        let timestamp = record["timestamp"].as_str().and_then(jsonl::parse_timestamp);
        let item = &record["payload"];
        match item["type"].as_str() {
            Some("message") => {
                let text = jsonl::content_text(&item["content"]).unwrap_or_default();
                let role = match item["role"].as_str() {
                    Some("assistant") => Role::Assistant,
                    Some("user") if !is_injected(&text) => Role::User,
                    _ => Role::Context,
                };
                builder.text(role, &text, timestamp);
            }
            Some("function_call") => builder.tool_call(
                item["call_id"].as_str(),
                item["name"].as_str().unwrap_or("tool"),
                &pretty_arguments(&item["arguments"]),
                timestamp,
            ),
            Some("custom_tool_call") => builder.tool_call(
                item["call_id"].as_str(),
                item["name"].as_str().unwrap_or("tool"),
                &transcript::tool_input_text(&item["input"]),
                timestamp,
            ),
            Some("function_call_output" | "custom_tool_call_output") => builder.tool_result(
                item["call_id"].as_str(),
                &transcript::tool_output_text(&item["output"]),
                timestamp,
            ),
            _ => {}
        }
    })?;
    Ok(builder.finish())
}

/// `function_call.arguments` is a JSON document encoded as a string.
fn pretty_arguments(arguments: &Value) -> String {
    match arguments.as_str().map(serde_json::from_str::<Value>) {
        Some(Ok(parsed)) => transcript::tool_input_text(&parsed),
        _ => transcript::tool_input_text(arguments),
    }
}

fn id_from_file_name(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let tail = stem.len().saturating_sub(36);
    match stem.get(tail..) {
        Some(id) if fs_util::looks_like_uuid(id) => id.to_string(),
        _ => stem.into_owned(),
    }
}

fn read_session(path: &Path, archived: bool, titles: &HashMap<String, String>) -> std::io::Result<Session> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(path, |line| parse_line(&mut parsed, line))?;
    let fingerprint = fs_util::fingerprint(path)?;
    let id = parsed.id.unwrap_or_else(|| id_from_file_name(path));
    let indexed_name = titles.get(&id).filter(|name| !is_injected(name)).cloned();
    let title = [indexed_name, parsed.first_prompt]
        .into_iter()
        .flatten()
        .find_map(|text| jsonl::to_title(&text));
    Ok(Session {
        agent: "codex",
        id,
        project_dir: parsed.cwd,
        title,
        created: parsed.span.first.or_else(|| fs_util::created(path)),
        last_activity: parsed.span.last.unwrap_or_else(|| fingerprint.modified.into()),
        size_bytes: fingerprint.len,
        archived,
        primary_file: path.to_path_buf(),
        files: vec![path.to_path_buf()],
        fingerprint,
        parent_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01a1b209-2d68-7e51-bd0c-2954bbab717e";

    fn rollout(id: &str) -> String {
        [
            format!(
                r#"{{"timestamp":"2026-11-06T18:05:32.095Z","type":"session_meta","payload":{{"id":"{id}","timestamp":"2026-11-06T18:05:08Z","cwd":"/work/api","originator":"codex_cli_rs"}}}}"#
            ),
            r##"{"timestamp":"2026-11-06T18:05:33.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /work/api"}]}}"##.to_string(),
            r#"{"timestamp":"2026-11-06T18:05:34.000Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>cwd</environment_context>"}]}}"#.to_string(),
            r#"{"timestamp":"2026-11-06T18:05:35.000Z","type":"event_msg","payload":{"type":"user_message","message":"Add pagination to the users endpoint"}}"#.to_string(),
            r#"{"timestamp":"2026-11-06T19:00:00.000Z","type":"event_msg","payload":{"type":"task_complete"}}"#.to_string(),
        ]
        .join("\n")
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn adapter(home: &Path, cli: Option<&str>) -> CodexAdapter {
        CodexAdapter::new(home.to_path_buf(), cli.map(PathBuf::from))
    }

    #[test]
    fn scans_active_and_archived_rollouts() {
        let home = tempfile::tempdir().unwrap();
        let active = home
            .path()
            .join(format!("sessions/2026/11/06/rollout-2026-11-06T18-05-08-{ID}.jsonl"));
        write(&active, &rollout(ID));
        let archived_id = "019e98e4-15db-7de3-af03-a2b011ccb8d5";
        write(
            &home.path().join(format!(
                "archived_sessions/rollout-2026-06-05T17-45-50-{archived_id}.jsonl"
            )),
            &rollout(archived_id),
        );

        let result = adapter(home.path(), None).scan();

        assert!(result.complete, "{:?}", result.errors);
        let mut sessions = result.sessions;
        sessions.sort_by_key(|session| session.archived);
        assert_eq!(sessions.len(), 2);
        let session = &sessions[0];
        assert_eq!(session.key(), format!("codex:{ID}"));
        assert!(!session.archived);
        assert_eq!(session.project_dir.as_deref(), Some("/work/api"));
        assert_eq!(session.title.as_deref(), Some("Add pagination to the users endpoint"));
        assert_eq!(session.last_activity.to_rfc3339(), "2026-11-06T19:00:00+00:00");
        assert_eq!(session.created.unwrap().to_rfc3339(), "2026-11-06T18:05:08+00:00");
        assert_eq!(session.files, vec![active]);
        assert!(sessions[1].archived);
    }

    #[test]
    fn thread_name_from_index_wins_unless_it_is_injected() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home.path().join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &rollout(ID),
        );
        let other = "019e98e4-29fd-7f63-8cae-b345db610f8c";
        write(
            &home.path().join(format!("sessions/2026/11/06/rollout-y-{other}.jsonl")),
            &rollout(other),
        );
        write(
            &home.path().join("session_index.jsonl"),
            &[
                format!(r#"{{"id":"{ID}","thread_name":"old-name"}}"#),
                format!(r#"{{"id":"{ID}","thread_name":"pagination-work"}}"#),
                format!(r#"{{"id":"{other}","thread_name":"<command-name>/clear</command-name>"}}"#),
            ]
            .join("\n"),
        );

        let sessions = adapter(home.path(), None).scan().sessions;
        let title_of = |id: &str| sessions.iter().find(|s| s.id == id).unwrap().title.clone();

        assert_eq!(title_of(ID).as_deref(), Some("pagination-work"));
        assert_eq!(title_of(other).as_deref(), Some("Add pagination to the users endpoint"));
    }

    #[test]
    fn compressed_rollouts_make_the_scan_incomplete() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/01/01/rollout-x-{ID}.jsonl.zst")),
            "",
        );

        let result = adapter(home.path(), None).scan();

        assert!(!result.complete);
        assert!(result.errors[0].contains("compressed"));
    }

    #[test]
    fn id_falls_back_to_uuid_suffix_of_file_name() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/01/01/rollout-2026-01-01T00-00-00-{ID}.jsonl")),
            "",
        );
        assert_eq!(adapter(home.path(), None).scan().sessions[0].id, ID);
    }

    #[test]
    fn transcript_uses_response_items_and_pairs_tool_output() {
        let home = tempfile::tempdir().unwrap();
        let content = [
            r#"{"timestamp":"2026-11-06T18:05:32Z","type":"session_meta","payload":{"id":"x"}}"#,
            r#"{"timestamp":"2026-11-06T18:05:33Z","type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<app-context>"}]}}"#,
            r#"{"timestamp":"2026-11-06T18:05:34Z","type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Check CI"}]}}"#,
            r#"{"timestamp":"2026-11-06T18:05:34Z","type":"event_msg","payload":{"type":"user_message","message":"Check CI"}}"#,
            r#"{"timestamp":"2026-11-06T18:05:35Z","type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}","call_id":"c1"}}"#,
            r#"{"timestamp":"2026-11-06T18:05:36Z","type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"ci.yml"}}"#,
            r#"{"timestamp":"2026-11-06T18:05:37Z","type":"response_item","payload":{"type":"reasoning","summary":[]}}"#,
            r#"{"timestamp":"2026-11-06T18:05:38Z","type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"CI looks fine."}]}}"#,
        ]
        .join("\n");
        write(
            &home.path().join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &content,
        );
        let codex = adapter(home.path(), None);
        let session = codex.scan().sessions.remove(0);

        let messages = codex.transcript(&session).unwrap().head;

        let roles: Vec<Role> = messages.iter().map(|message| message.role).collect();
        assert_eq!(roles, vec![Role::Context, Role::User, Role::Tool, Role::Assistant]);
        let tool = messages[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "exec_command");
        assert_eq!(tool.output.as_deref(), Some("ci.yml"));
        assert_eq!(messages[2].text, "{\n  \"cmd\": \"ls\"\n}");
    }

    #[test]
    fn delete_and_archive_use_the_codex_cli() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home.path().join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &rollout(ID),
        );
        let codex = adapter(home.path(), Some("/bin/codex"));
        let mut session = codex.scan().sessions.remove(0);

        assert_eq!(
            codex.delete_plan(&session),
            DeletePlan::AgentCommand {
                command: AgentCommand {
                    program: PathBuf::from("/bin/codex"),
                    args: vec!["delete".into(), "--force".into(), ID.into()]
                },
                files: session.files.clone(),
                warning: None,
            }
        );
        assert_eq!(codex.archive_command(&session).unwrap().args, vec!["archive", ID]);
        session.archived = true;
        assert_eq!(codex.archive_command(&session).unwrap().args, vec!["unarchive", ID]);
    }

    #[test]
    fn read_only_without_cli_or_uuid() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home.path().join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &rollout(ID),
        );
        let without_cli = adapter(home.path(), None);
        let mut session = without_cli.scan().sessions.remove(0);

        assert!(
            matches!(without_cli.delete_plan(&session), DeletePlan::ReadOnly { reason } if reason.contains("Install the Codex CLI"))
        );
        assert!(without_cli.archive_command(&session).is_none());

        session.id = "not-a-uuid".into();
        let with_cli = adapter(home.path(), Some("/bin/codex"));
        assert!(matches!(with_cli.delete_plan(&session), DeletePlan::ReadOnly { reason } if reason.contains("UUID")));
    }
}
