use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, DeletePlan, ScanResult};
use crate::fs_util;
use crate::jsonl;
use crate::session::Session;

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
        }
    }
}

#[derive(Default)]
struct Parsed {
    id: Option<String>,
    cwd: Option<String>,
    name: Option<String>,
    first_prompt: Option<String>,
    last_timestamp: Option<DateTime<Utc>>,
}

fn parse_line(parsed: &mut Parsed, line: &str) {
    let Ok(record) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if let Some(timestamp) = record["timestamp"].as_str() {
        parsed.last_timestamp = jsonl::later(parsed.last_timestamp, jsonl::parse_timestamp(timestamp));
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
        last_activity: parsed.last_timestamp.unwrap_or_else(|| fingerprint.modified.into()),
        size_bytes: fingerprint.len,
        archived: false,
        primary_file: path.to_path_buf(),
        files: vec![path.to_path_buf()],
        fingerprint,
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
    fn missing_sessions_dir_is_complete_and_empty() {
        let root = tempfile::tempdir().unwrap();
        let result = PiAdapter::new(root.path().join("absent")).scan();
        assert!(result.complete);
        assert!(result.sessions.is_empty());
    }
}
