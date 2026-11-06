use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::value::RawValue;

use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, DeletePlan, ScanResult};
use crate::fs_util;
use crate::jsonl;
use crate::session::Session;

pub struct ClaudeAdapter {
    projects_dir: PathBuf,
}

impl ClaudeAdapter {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            projects_dir: config_dir.join("projects"),
        }
    }

    pub fn from_env() -> Self {
        Self::new(env_dir("CLAUDE_CONFIG_DIR").unwrap_or_else(|| home_dir().join(".claude")))
    }
}

impl AgentAdapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.projects_dir.clone()]
    }

    fn scan(&self) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let projects = match fs::read_dir(&self.projects_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return result,
            Err(error) => {
                result.record_error(format!("{}: {error}", self.projects_dir.display()));
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record<'a> {
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    timestamp: Option<Cow<'a, str>>,
    cwd: Option<String>,
    ai_title: Option<String>,
    custom_title: Option<String>,
    summary: Option<String>,
    is_meta: Option<bool>,
    #[serde(borrow)]
    message: Option<&'a RawValue>,
}

#[derive(Default)]
struct Parsed {
    cwd: Option<String>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    summary: Option<String>,
    first_prompt: Option<String>,
    last_timestamp: Option<DateTime<Utc>>,
}

impl Parsed {
    fn title(self) -> Option<String> {
        [self.custom_title, self.ai_title, self.summary, self.first_prompt]
            .into_iter()
            .flatten()
            .find_map(|candidate| jsonl::to_title(&candidate))
    }
}

fn parse_line(parsed: &mut Parsed, line: &str) {
    let Ok(record) = serde_json::from_str::<Record>(line) else {
        return;
    };
    if let Some(timestamp) = record.timestamp.as_deref() {
        parsed.last_timestamp = jsonl::later(parsed.last_timestamp, jsonl::parse_timestamp(timestamp));
    }
    if parsed.cwd.is_none() {
        parsed.cwd = record.cwd;
    }
    match record.kind.as_deref() {
        Some("custom-title") => parsed.custom_title = record.custom_title.or(parsed.custom_title.take()),
        Some("ai-title") => parsed.ai_title = record.ai_title.or(parsed.ai_title.take()),
        Some("summary") => parsed.summary = record.summary.or(parsed.summary.take()),
        Some("user") if parsed.first_prompt.is_none() && record.is_meta != Some(true) => {
            parsed.first_prompt = record.message.and_then(user_prompt_text);
        }
        _ => {}
    }
}

/// Text the user typed, ignoring tool results and harness-injected wrappers such as
/// `<command-name>` or `<local-command-caveat>`.
fn user_prompt_text(message: &RawValue) -> Option<String> {
    let message: serde_json::Value = serde_json::from_str(message.get()).ok()?;
    let text = jsonl::content_text(&message["content"])?;
    if text.trim_start().starts_with('<') {
        None
    } else {
        Some(text)
    }
}

fn read_session(path: &Path) -> std::io::Result<Session> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(path, |line| parse_line(&mut parsed, line))?;

    let fingerprint = fs_util::fingerprint(path)?;
    let id = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let files = session_files(path, &id);
    let last_activity = parsed.last_timestamp.unwrap_or_else(|| fingerprint.modified.into());
    Ok(Session {
        agent: "claude",
        size_bytes: files.iter().map(|file| fs_util::path_size(file)).sum(),
        project_dir: parsed.cwd.clone(),
        title: parsed.title(),
        id,
        last_activity,
        archived: false,
        primary_file: path.to_path_buf(),
        files,
        fingerprint,
    })
}

/// The transcript plus its sidecars: `<id>.*` files and the `<id>/` folder holding
/// subagent transcripts and tool results. The folder is only claimed for UUID ids so a
/// stray `memory.jsonl` can never pull in the project's `memory/` folder.
fn session_files(transcript: &Path, id: &str) -> Vec<PathBuf> {
    let mut files = vec![transcript.to_path_buf()];
    let Some(project_dir) = transcript.parent() else {
        return files;
    };
    let sidecar_prefix = format!("{id}.");
    if let Ok(entries) = fs::read_dir(project_dir) {
        let mut sidecars: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path != transcript && path.is_file())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&sidecar_prefix))
            })
            .collect();
        sidecars.sort();
        files.extend(sidecars);
    }
    let folder = project_dir.join(id);
    if fs_util::looks_like_uuid(id) && folder.is_dir() {
        files.push(folder);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "a9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f";

    fn write_project(config: &Path, project: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = config.join("projects").join(project);
        fs::create_dir_all(&dir).unwrap();
        for (name, content) in files {
            fs::write(dir.join(name), content).unwrap();
        }
        dir
    }

    fn scan_one(config: &Path) -> Session {
        let result = ClaudeAdapter::new(config.to_path_buf()).scan();
        assert!(result.complete, "{:?}", result.errors);
        assert_eq!(result.sessions.len(), 1);
        result.sessions.into_iter().next().unwrap()
    }

    #[test]
    fn missing_projects_dir_is_complete_and_empty() {
        let config = tempfile::tempdir().unwrap();
        let result = ClaudeAdapter::new(config.path().to_path_buf()).scan();
        assert!(result.complete);
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn parses_metadata_and_prefers_ai_title() {
        let config = tempfile::tempdir().unwrap();
        let transcript = [
            r#"{"type":"permission-mode","permissionMode":"auto","sessionId":"x"}"#,
            r#"{"type":"user","isMeta":true,"cwd":"/work/app","timestamp":"2026-11-01T10:00:00Z","message":{"role":"user","content":"<local-command-caveat>ignored</local-command-caveat>"}}"#,
            r#"{"type":"user","cwd":"/work/app","timestamp":"2026-11-01T10:01:00Z","message":{"role":"user","content":[{"type":"text","text":"Fix the   login bug"}]}}"#,
            r#"{"type":"ai-title","aiTitle":"Old title","sessionId":"x"}"#,
            r#"{"type":"assistant","timestamp":"2026-11-02T09:30:00Z","message":{"role":"assistant","content":[]}}"#,
            r#"{"type":"ai-title","aiTitle":"Login bug fix","sessionId":"x"}"#,
            "not json at all",
        ]
        .join("\n");
        write_project(config.path(), "-work-app", &[(&format!("{ID}.jsonl"), &transcript)]);

        let session = scan_one(config.path());

        assert_eq!(session.key(), format!("claude:{ID}"));
        assert_eq!(session.project_dir.as_deref(), Some("/work/app"));
        assert_eq!(session.title.as_deref(), Some("Login bug fix"));
        assert_eq!(session.last_activity.to_rfc3339(), "2026-11-02T09:30:00+00:00");
    }

    #[test]
    fn custom_title_beats_ai_title() {
        let config = tempfile::tempdir().unwrap();
        let transcript = [
            r#"{"type":"ai-title","aiTitle":"AI"}"#,
            r#"{"type":"custom-title","customTitle":"Mine"}"#,
        ]
        .join("\n");
        write_project(config.path(), "p", &[(&format!("{ID}.jsonl"), &transcript)]);
        assert_eq!(scan_one(config.path()).title.as_deref(), Some("Mine"));
    }

    #[test]
    fn falls_back_to_first_real_prompt_and_mtime() {
        let config = tempfile::tempdir().unwrap();
        let transcript = [
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"x"}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":"<command-name>/clear</command-name>"}}"#,
            r#"{"type":"user","message":{"role":"user","content":"real question"}}"#,
        ]
        .join("\n");
        let dir = write_project(config.path(), "p", &[(&format!("{ID}.jsonl"), &transcript)]);

        let session = scan_one(config.path());

        assert_eq!(session.title.as_deref(), Some("real question"));
        let mtime: DateTime<Utc> = fs::metadata(dir.join(format!("{ID}.jsonl")))
            .unwrap()
            .modified()
            .unwrap()
            .into();
        assert_eq!(session.last_activity, mtime);
    }

    #[test]
    fn includes_sidecars_and_session_folder_but_not_memory() {
        let config = tempfile::tempdir().unwrap();
        let dir = write_project(
            config.path(),
            "p",
            &[
                (&format!("{ID}.jsonl"), "{}"),
                (&format!("{ID}.precompact.json"), "{}"),
                ("other.jsonl", "{}"),
            ],
        );
        fs::create_dir_all(dir.join(ID).join("subagents")).unwrap();
        fs::write(dir.join(ID).join("subagents").join("agent.jsonl"), "12345").unwrap();
        fs::create_dir(dir.join("memory")).unwrap();
        fs::write(dir.join("memory").join("MEMORY.md"), "keep").unwrap();

        let result = ClaudeAdapter::new(config.path().to_path_buf()).scan();
        let session = result.sessions.iter().find(|s| s.id == ID).unwrap();

        assert_eq!(
            session.files,
            vec![
                dir.join(format!("{ID}.jsonl")),
                dir.join(format!("{ID}.precompact.json")),
                dir.join(ID)
            ]
        );
        assert_eq!(session.size_bytes, 2 + 2 + 5);
        assert!(result.sessions.iter().all(|s| !s.files.contains(&dir.join("memory"))));
    }

    #[test]
    fn non_uuid_transcript_never_claims_a_folder() {
        let config = tempfile::tempdir().unwrap();
        let dir = write_project(config.path(), "p", &[("memory.jsonl", "{}")]);
        fs::create_dir(dir.join("memory")).unwrap();

        let session = scan_one(config.path());

        assert_eq!(session.files, vec![dir.join("memory.jsonl")]);
    }

    #[test]
    fn delete_plan_trashes_all_session_files() {
        let config = tempfile::tempdir().unwrap();
        write_project(config.path(), "p", &[(&format!("{ID}.jsonl"), "{}")]);
        let adapter = ClaudeAdapter::new(config.path().to_path_buf());
        let session = adapter.scan().sessions.remove(0);
        assert_eq!(
            adapter.delete_plan(&session),
            DeletePlan::Trash {
                files: session.files.clone()
            }
        );
    }
}
