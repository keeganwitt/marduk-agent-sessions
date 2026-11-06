use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, DeletePlan, ScanResult};
use crate::fs_util;
use crate::jsonl;
use crate::session::Session;
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// Pi stores one JSONL file per session at `<sessions>/--<encoded cwd>--/<timestamp>_<id>.jsonl`.
pub struct PiAdapter {
    sessions_dir: PathBuf,
}

impl PiAdapter {
    pub fn new(sessions_dir: PathBuf) -> Self {
        Self { sessions_dir }
    }

    pub fn from_env() -> Self {
        let agent_dir = env_dir("PI_CODING_AGENT_DIR").unwrap_or_else(|| home_dir().join(".pi").join("agent"));
        Self::new(agent_dir.join("sessions"))
    }
}

impl AgentAdapter for PiAdapter {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn display_name(&self) -> &'static str {
        "Pi"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.sessions_dir.clone()]
    }

    fn scan(&self) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let projects = match fs::read_dir(&self.sessions_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return result,
            Err(error) => {
                result.record_error(format!("{}: {error}", self.sessions_dir.display()));
                return result;
            }
        };
        for project in projects.flatten() {
            let project_path = project.path();
            if !project_path.is_dir() {
                continue;
            }
            let entries = match fs::read_dir(&project_path) {
                Ok(entries) => entries,
                Err(error) => {
                    result.record_error(format!("{}: {error}", project_path.display()));
                    continue;
                }
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file() {
                    match read_session(&path) {
                        Ok(session) => result.sessions.push(session),
                        Err(error) => result.record_error(format!("{}: {error}", path.display())),
                    }
                }
            }
        }
        result
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        DeletePlan::Trash {
            files: session.files.clone(),
            warning: None,
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        read_transcript(&session.primary_file)
    }
}

/// Pi sessions are trees: rewinds and forks leave abandoned entries in the file. Only the
/// branch ending at the last entry is what the user last saw.
fn active_branch(records: &[Value]) -> std::collections::HashSet<String> {
    let parents: HashMap<&str, Option<&str>> = records
        .iter()
        .filter_map(|record| Some((record["id"].as_str()?, record["parentId"].as_str())))
        .collect();
    let mut active = std::collections::HashSet::new();
    let mut current = records.iter().rev().find_map(|record| record["id"].as_str());
    while let Some(id) = current {
        if !active.insert(id.to_string()) {
            break;
        }
        current = parents.get(id).copied().flatten();
    }
    active
}

fn read_transcript(path: &Path) -> std::io::Result<Transcript> {
    let mut records = Vec::new();
    jsonl::for_each_line(path, |line| {
        if let Ok(record) = serde_json::from_str::<Value>(line) {
            records.push(record);
        }
    })?;
    let active = active_branch(&records);
    let mut builder = TranscriptBuilder::default();
    for record in &records {
        if let Some(id) = record["id"].as_str()
            && !active.contains(id)
        {
            continue;
        }
        let timestamp = record["timestamp"].as_str().and_then(jsonl::parse_timestamp);
        match record["type"].as_str() {
            Some("message") => append_message(&mut builder, &record["message"], timestamp),
            Some("compaction" | "branch_summary") => {
                builder.text(Role::Context, record["summary"].as_str().unwrap_or_default(), timestamp)
            }
            _ => {}
        }
    }
    Ok(builder.finish())
}

fn append_message(builder: &mut TranscriptBuilder, message: &Value, timestamp: Option<chrono::DateTime<chrono::Utc>>) {
    let content = &message["content"];
    match message["role"].as_str() {
        Some("user") => builder.text(Role::User, &jsonl::content_text(content).unwrap_or_default(), timestamp),
        Some("assistant") => {
            let blocks = match content {
                Value::Array(blocks) => blocks.clone(),
                other => vec![other.clone()],
            };
            for block in &blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        builder.text(Role::Assistant, block["text"].as_str().unwrap_or_default(), timestamp)
                    }
                    Some("toolCall" | "tool_call" | "tool_use") => builder.tool_call(
                        block["id"].as_str(),
                        block["name"].as_str().unwrap_or("tool"),
                        &transcript::tool_input_text(&block["arguments"]),
                        timestamp,
                    ),
                    _ => {
                        if let Some(text) = block.as_str() {
                            builder.text(Role::Assistant, text, timestamp);
                        }
                    }
                }
            }
        }
        Some("toolResult" | "tool_result") => builder.tool_result(
            message["toolCallId"]
                .as_str()
                .or_else(|| message["tool_call_id"].as_str()),
            &transcript::tool_output_text(content),
            timestamp,
        ),
        Some("branchSummary" | "branch_summary" | "compactionSummary" | "compaction_summary") => builder.text(
            Role::Context,
            message["summary"].as_str().unwrap_or_default(),
            timestamp,
        ),
        _ => {}
    }
}

#[derive(Default)]
struct Parsed {
    id: Option<String>,
    cwd: Option<String>,
    name: Option<String>,
    first_prompt: Option<String>,
    span: jsonl::Span,
}

fn parse_line(parsed: &mut Parsed, line: &str) {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if let Some(timestamp) = record["timestamp"].as_str() {
        parsed.span.observe(jsonl::parse_timestamp(timestamp));
    }
    match record["type"].as_str() {
        Some("session") => {
            parsed.id = record["id"].as_str().map(str::to_string);
            parsed.cwd = record["cwd"].as_str().map(str::to_string);
        }
        Some("session_info") => {
            if let Some(name) = record["name"].as_str() {
                parsed.name = Some(name.to_string());
            }
        }
        Some("message") if parsed.first_prompt.is_none() && record["message"]["role"] == "user" => {
            parsed.first_prompt = jsonl::content_text(&record["message"]["content"]);
        }
        _ => {}
    }
}

fn id_from_file_name(path: &Path) -> String {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    match stem.split_once('_') {
        Some((_, id)) if !id.is_empty() => id.to_string(),
        _ => stem.into_owned(),
    }
}

fn read_session(path: &Path) -> std::io::Result<Session> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(path, |line| parse_line(&mut parsed, line))?;
    let fingerprint = fs_util::fingerprint(path)?;
    let title = [parsed.name, parsed.first_prompt]
        .into_iter()
        .flatten()
        .find_map(|text| jsonl::to_title(&text));
    Ok(Session {
        agent: "pi",
        id: parsed.id.unwrap_or_else(|| id_from_file_name(path)),
        project_dir: parsed.cwd,
        title,
        created: parsed.span.first.or_else(|| fs_util::created(path)),
        last_activity: parsed.span.last.unwrap_or_else(|| fingerprint.modified.into()),
        size_bytes: fingerprint.len,
        archived: false,
        primary_file: path.to_path_buf(),
        files: vec![path.to_path_buf()],
        fingerprint,
        parent_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "2026-11-05T21-42-56-603Z_01a1adaa-395a-7422-9391-5092469eda9a.jsonl";

    fn write_session(root: &Path, content: &str) -> PathBuf {
        let dir = root.join("--Users-me-work--");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE);
        fs::write(&path, content).unwrap();
        path
    }

    fn scan_one(root: &Path) -> Session {
        let result = PiAdapter::new(root.to_path_buf()).scan();
        assert!(result.complete, "{:?}", result.errors);
        assert_eq!(result.sessions.len(), 1);
        result.sessions.into_iter().next().unwrap()
    }

    #[test]
    fn parses_header_prompt_and_latest_timestamp() {
        let root = tempfile::tempdir().unwrap();
        let content = [
            r#"{"type":"session","version":3,"id":"01a1adaa-395a-7422-9391-5092469eda9a","timestamp":"2026-11-05T21:42:56.603Z","cwd":"/Users/me/work"}"#,
            r#"{"type":"model_change","id":"a","parentId":null,"timestamp":"2026-11-05T21:42:57.000Z","modelId":"m"}"#,
            r#"{"type":"message","id":"b","parentId":"a","timestamp":"2026-11-05T21:43:00.000Z","message":{"role":"user","content":[{"type":"text","text":"refactor the parser"}]}}"#,
            r#"{"type":"message","id":"c","parentId":"b","timestamp":"2026-11-05T21:45:00.000Z","message":{"role":"assistant","content":[{"type":"text","text":"done"}]}}"#,
        ]
        .join("\n");
        let path = write_session(root.path(), &content);

        let session = scan_one(root.path());

        assert_eq!(session.key(), "pi:01a1adaa-395a-7422-9391-5092469eda9a");
        assert_eq!(session.project_dir.as_deref(), Some("/Users/me/work"));
        assert_eq!(session.title.as_deref(), Some("refactor the parser"));
        assert_eq!(session.last_activity.to_rfc3339(), "2026-11-05T21:45:00+00:00");
        assert_eq!(session.created.unwrap().to_rfc3339(), "2026-11-05T21:42:56.603+00:00");
        assert_eq!(session.files, vec![path]);
    }

    #[test]
    fn session_info_name_is_preferred_title() {
        let root = tempfile::tempdir().unwrap();
        let content = [
            r#"{"type":"message","message":{"role":"user","content":"first prompt"}}"#,
            r#"{"type":"session_info","name":"Named session"}"#,
        ]
        .join("\n");
        write_session(root.path(), &content);
        assert_eq!(scan_one(root.path()).title.as_deref(), Some("Named session"));
    }

    #[test]
    fn missing_header_falls_back_to_file_name_id() {
        let root = tempfile::tempdir().unwrap();
        write_session(root.path(), "");
        assert_eq!(scan_one(root.path()).id, "01a1adaa-395a-7422-9391-5092469eda9a");
    }

    #[test]
    fn transcript_follows_the_active_branch_and_pairs_tools() {
        let root = tempfile::tempdir().unwrap();
        let content = [
            r#"{"type":"session","version":3,"id":"s","cwd":"/w"}"#,
            r#"{"type":"message","id":"u1","parentId":null,"message":{"role":"user","content":[{"type":"text","text":"first try"}]}}"#,
            r#"{"type":"message","id":"a1","parentId":"u1","message":{"role":"assistant","content":[{"type":"text","text":"abandoned answer"}]}}"#,
            r#"{"type":"message","id":"u2","parentId":null,"message":{"role":"user","content":[{"type":"text","text":"rewound prompt"}]}}"#,
            r#"{"type":"message","id":"a2","parentId":"u2","message":{"role":"assistant","content":[{"type":"toolCall","id":"c1","name":"bash","arguments":{"command":"ls"}}]}}"#,
            r#"{"type":"message","id":"r1","parentId":"a2","message":{"role":"toolResult","toolCallId":"c1","toolName":"bash","content":[{"type":"text","text":"a.txt"}]}}"#,
            r#"{"type":"message","id":"a3","parentId":"r1","message":{"role":"assistant","content":[{"type":"text","text":"Found a.txt"}]}}"#,
        ]
        .join("\n");
        write_session(root.path(), &content);
        let adapter = PiAdapter::new(root.path().to_path_buf());
        let session = adapter.scan().sessions.remove(0);

        let messages = adapter.transcript(&session).unwrap().head;

        let texts: Vec<&str> = messages.iter().map(|message| message.text.as_str()).collect();
        assert_eq!(texts[0], "rewound prompt");
        assert!(!texts.contains(&"abandoned answer"));
        assert_eq!(messages[1].tool.as_ref().unwrap().output.as_deref(), Some("a.txt"));
        assert_eq!(messages[2].text, "Found a.txt");
    }

    #[test]
    fn missing_sessions_dir_is_complete_and_empty() {
        let root = tempfile::tempdir().unwrap();
        let result = PiAdapter::new(root.path().join("absent")).scan();
        assert!(result.complete);
        assert!(result.sessions.is_empty());
    }
}
