use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::value::RawValue;

use super::{env_dir, home_dir, jsonl_two_levels, total_size};
use crate::adapter::ScanProgress;
use crate::adapter::{AgentAdapter, DeletePlan, Mechanics, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl;
use crate::memory::{self, MemoryFile, MemoryStore};
use crate::session::Fingerprint;
use crate::session::Session;
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

pub struct ClaudeAdapter {
    projects_dir: PathBuf,
    /// Claude Code's global config, which lists every project path it has opened.
    global_config: PathBuf,
    cache: ParseCache<Fingerprint, Parsed>,
}

const MEMORY_INDEX: &str = "MEMORY.md";
/// Claude Code loads only the start of `MEMORY.md` into a session.
const INDEX_LINE_LIMIT: usize = 200;
const INDEX_BYTE_LIMIT: u64 = 25 * 1024;

impl ClaudeAdapter {
    pub fn new(config_dir: PathBuf) -> Self {
        Self {
            projects_dir: config_dir.join("projects"),
            global_config: config_dir.join(".claude.json"),
            cache: ParseCache::default(),
        }
    }

    /// With `CLAUDE_CONFIG_DIR` set, `.claude.json` is inside it; otherwise it's `~/.claude.json`,
    /// next to (not inside) `~/.claude`.
    pub fn from_env() -> Self {
        match env_dir("CLAUDE_CONFIG_DIR") {
            Some(config_dir) => Self::new(config_dir),
            None => Self {
                global_config: home_dir().join(".claude.json"),
                ..Self::new(home_dir().join(".claude"))
            },
        }
    }

    /// Project paths Claude Code has opened, by their folder name under `projects/`.
    fn known_projects(&self) -> HashMap<String, String> {
        let config = fs::read_to_string(&self.global_config)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
        config
            .as_ref()
            .and_then(|config| config["projects"].as_object())
            .map(|projects| {
                projects
                    .keys()
                    .map(|path| (project_folder_name(path), path.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The memory in `<project folder>/memory`, with what's wrong with it. `sessions` are the
    /// project's top-level sessions.
    fn memory_store(
        &self,
        folder: &Path,
        sessions: &[&Session],
        known: &HashMap<String, String>,
    ) -> MemoryStore {
        let memory_dir = folder.join("memory");
        let name = folder.file_name().unwrap_or_default().to_string_lossy();
        let project_dir = sessions
            .iter()
            .find_map(|session| session.project_dir.clone())
            .or_else(|| known.get(name.as_ref()).cloned());
        let project_exists = project_dir.as_deref().map(|dir| Path::new(dir).is_dir());
        let mut paths: Vec<PathBuf> = fs::read_dir(&memory_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        paths.sort_by_key(|path| {
            (
                path.file_name() != Some(OsStr::new(MEMORY_INDEX)),
                path.clone(),
            )
        });
        let mut index: Option<String> = None;
        let files: Vec<MemoryFile> = paths
            .into_iter()
            .map(|path| {
                let text = fs::read_to_string(&path).unwrap_or_default();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                let stem = path.file_stem().unwrap_or_default().to_string_lossy();
                let fields = memory::frontmatter(&text);
                let is_index = file_name == MEMORY_INDEX;
                let file = MemoryFile {
                    title: if is_index {
                        MEMORY_INDEX.to_string()
                    } else {
                        fields
                            .get("name")
                            .cloned()
                            .unwrap_or_else(|| stem.into_owned())
                    },
                    description: if is_index {
                        Some(
                            "The index Claude Code loads into every session in this project."
                                .to_string(),
                        )
                    } else {
                        fields.get("description").cloned()
                    },
                    kind: if is_index {
                        Some("index".to_string())
                    } else {
                        fields.get("type").cloned()
                    },
                    session_id: fields.get("originSessionId").cloned(),
                    modified: fields
                        .get("modified")
                        .and_then(|value| jsonl::parse_timestamp(value))
                        .or_else(|| memory::modified(&path)),
                    size: memory::size(&path),
                    path,
                };
                if is_index {
                    index = Some(text);
                }
                file
            })
            .collect();
        let problems = memory_problems(
            project_dir.as_deref(),
            project_exists,
            index.as_deref(),
            &files,
        );
        MemoryStore {
            agent: "claude",
            agent_name: "Claude Code",
            folder: memory_dir,
            project_dir,
            project_exists,
            sessions: sessions.len(),
            files,
            problems,
            note: None,
            can_trash: true,
        }
    }

    fn top_level_by_folder(sessions: &[Session]) -> HashMap<PathBuf, Vec<&Session>> {
        let mut by_folder: HashMap<PathBuf, Vec<&Session>> = HashMap::new();
        for session in sessions.iter().filter(|session| !session.part_of_parent) {
            if let Some(folder) = session.primary_file.parent() {
                by_folder
                    .entry(folder.to_path_buf())
                    .or_default()
                    .push(session);
            }
        }
        by_folder
    }
}

/// Claude Code names a project's folder after its path, with every other character a dash.
fn project_folder_name(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn memory_problems(
    project_dir: Option<&str>,
    project_exists: Option<bool>,
    index: Option<&str>,
    files: &[MemoryFile],
) -> Vec<String> {
    let mut problems = Vec::new();
    if let (Some(dir), Some(false)) = (project_dir, project_exists) {
        problems.push(format!("The project folder {dir} no longer exists."));
    }
    let topics: Vec<&MemoryFile> = files
        .iter()
        .filter(|file| file.kind.as_deref() != Some("index"))
        .collect();
    if files.is_empty() {
        problems.push("The memory folder is empty.".to_string());
        return problems;
    }
    let Some(index) = index else {
        problems.push(format!(
            "There's no {MEMORY_INDEX}, the index Claude Code loads."
        ));
        return problems;
    };
    let lines = index.lines().count();
    if lines > INDEX_LINE_LIMIT {
        problems.push(format!(
            "{MEMORY_INDEX} has {lines} lines; Claude Code only loads the first {INDEX_LINE_LIMIT}."
        ));
    } else if index.len() as u64 > INDEX_BYTE_LIMIT {
        problems.push(format!(
            "{MEMORY_INDEX} is {} KB; Claude Code only loads the first {} KB.",
            index.len() / 1024,
            INDEX_BYTE_LIMIT / 1024
        ));
    }
    let links: HashSet<String> = memory::link_targets(index).into_iter().collect();
    let unlisted: Vec<String> = topics
        .iter()
        .filter_map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !links.contains(name))
        .collect();
    if !unlisted.is_empty() {
        problems.push(format!(
            "Not listed in {MEMORY_INDEX}, so Claude Code won't find {}: {}.",
            if unlisted.len() == 1 { "it" } else { "them" },
            unlisted.join(", ")
        ));
    }
    let present: HashSet<String> = files
        .iter()
        .filter_map(|file| {
            file.path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect();
    let mut missing: Vec<&String> = links
        .iter()
        .filter(|link| !present.contains(*link))
        .collect();
    missing.sort();
    if !missing.is_empty() {
        problems.push(format!(
            "{MEMORY_INDEX} links to files that don't exist: {}.",
            missing
                .iter()
                .map(|link| link.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    problems
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

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let files = jsonl_two_levels(&self.projects_dir, &mut result);
        let mut seen: Vec<PathBuf> = Vec::new();
        for path in &files {
            match self.read_session(path, progress) {
                Ok(session) => {
                    for subagent in subagent_transcripts(path) {
                        match self.read_subagent(&subagent, &session, progress) {
                            Ok(child) => result.sessions.push(child),
                            Err(error) => {
                                result.record_error(format!("{}: {error}", subagent.display()))
                            }
                        }
                        seen.push(subagent);
                    }
                    result.sessions.push(session);
                }
                Err(error) => result.record_error(format!("{}: {error}", path.display())),
            }
        }
        seen.extend(files);
        self.cache.retain(&seen.into_iter().collect());
        result
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let files = jsonl_two_levels(&self.projects_dir, &mut ScanResult::default());
        let subagents: Vec<PathBuf> = files
            .iter()
            .flat_map(|path| subagent_transcripts(path))
            .collect();
        total_size(&files) + total_size(&subagents)
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        if session.part_of_parent {
            return DeletePlan::ReadOnly {
                reason: "This subagent is part of its parent session; deleting the parent deletes it too.".to_string(),
            };
        }
        DeletePlan::Trash {
            files: session.files.clone(),
            warning: None,
        }
    }

    fn memory(&self, sessions: &[Session]) -> Vec<MemoryStore> {
        let known = self.known_projects();
        let by_folder = Self::top_level_by_folder(sessions);
        let mut folders: Vec<PathBuf> = fs::read_dir(&self.projects_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|folder| folder.join("memory").is_dir())
            .collect();
        folders.sort();
        folders
            .iter()
            .map(|folder| {
                let sessions = by_folder.get(folder).map(Vec::as_slice).unwrap_or_default();
                self.memory_store(folder, sessions, &known)
            })
            .collect()
    }

    fn owns_memory_file(&self, path: &Path) -> bool {
        path.is_file()
            && path.extension().is_some_and(|ext| ext == "md")
            && path
                .parent()
                .is_some_and(|folder| self.owns_memory_folder(folder))
    }

    /// Only `<projects>/<project>/memory` itself, after resolving symlinks and `..`.
    fn owns_memory_folder(&self, folder: &Path) -> bool {
        let (Ok(folder), Ok(root)) = (folder.canonicalize(), self.projects_dir.canonicalize())
        else {
            return false;
        };
        folder.file_name() == Some(OsStr::new("memory"))
            && folder.parent().and_then(Path::parent) == Some(root.as_path())
    }

    /// Memory is per project: it's used while the project has any transcript left.
    fn memory_in_use(&self, folder: &Path) -> bool {
        folder
            .parent()
            .and_then(|project| fs::read_dir(project).ok())
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
    }

    fn memory_left_behind(&self, deleting: &[&Session], all: &[Session]) -> Vec<MemoryStore> {
        let known = self.known_projects();
        let deleting_files: HashSet<&Path> = deleting
            .iter()
            .map(|session| session.primary_file.as_path())
            .collect();
        let folders: BTreeSet<&Path> = deleting
            .iter()
            .filter(|session| !session.part_of_parent)
            .filter_map(|session| session.primary_file.parent())
            .collect();
        folders
            .into_iter()
            .filter(|folder| folder.join("memory").is_dir())
            .filter(|folder| {
                !all.iter().any(|session| {
                    !session.part_of_parent
                        && session.primary_file.parent() == Some(*folder)
                        && !deleting_files.contains(session.primary_file.as_path())
                })
            })
            .map(|folder| {
                let in_folder: Vec<&Session> = deleting
                    .iter()
                    .copied()
                    .filter(|session| session.primary_file.parent() == Some(folder))
                    .collect();
                MemoryStore {
                    sessions: 0,
                    ..self.memory_store(folder, &in_folder, &known)
                }
            })
            .filter(|store| !store.files.is_empty())
            .collect()
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: "Moves the transcript, its sidecar files and its session folder (subagents, \
                     tool results) to the Trash. Claude Code has no delete command of its own. \
                     Subagents are only deleted along with their session."
                .to_string(),
            archive: "Claude Code has no archive.".to_string(),
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        read_transcript(&session.primary_file, session.part_of_parent)
    }
}

/// `<project>/<session-id>/subagents/agent-*.jsonl`, the transcripts of subagents the
/// session started. They live inside the parent's folder and belong to it.
fn subagent_transcripts(transcript: &Path) -> Vec<PathBuf> {
    let id = transcript.file_stem().unwrap_or_default().to_string_lossy();
    if !fs_util::looks_like_uuid(&id) {
        return Vec::new();
    }
    let Some(folder) = transcript
        .parent()
        .map(|dir| dir.join(id.as_ref()).join("subagents"))
    else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file())
        .collect();
    files.sort();
    files
}

/// "Check codex delete cascades to guardians · general-purpose" from a subagent's
/// `agent-<id>.meta.json`.
fn subagent_meta_title(meta: &Path) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(meta).ok()?).ok()?;
    let description = value["description"]
        .as_str()
        .filter(|text| !text.trim().is_empty());
    let kind = value["agentType"]
        .as_str()
        .filter(|text| !text.trim().is_empty());
    match (description, kind) {
        (Some(description), Some(kind)) => Some(format!("{description} · {kind}")),
        (Some(description), None) => Some(description.to_string()),
        (None, Some(kind)) => Some(kind.to_string()),
        (None, None) => None,
    }
}

fn user_role(is_meta: bool, text: &str) -> Role {
    if is_meta || text.trim_start().starts_with('<') {
        Role::Context
    } else {
        Role::User
    }
}

/// A parent's transcript skips sidechain records (its subagents' turns); a subagent's own
/// transcript is entirely sidechain records.
fn read_transcript(path: &Path, sidechain: bool) -> std::io::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    jsonl::for_each_line(path, |line| {
        let Ok(record) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        if !sidechain && record["isSidechain"] == true {
            return;
        }
        let timestamp = record["timestamp"]
            .as_str()
            .and_then(jsonl::parse_timestamp);
        let is_meta = record["isMeta"] == true;
        let content = &record["message"]["content"];
        match record["type"].as_str() {
            Some("user") => match content {
                serde_json::Value::String(text) => {
                    builder.text(user_role(is_meta, text), text, timestamp)
                }
                serde_json::Value::Array(blocks) => {
                    for block in blocks {
                        match block["type"].as_str() {
                            Some("text") => {
                                let text = block["text"].as_str().unwrap_or_default();
                                builder.text(user_role(is_meta, text), text, timestamp);
                            }
                            Some("tool_result") => builder.tool_result(
                                block["tool_use_id"].as_str(),
                                &transcript::tool_output_text(&block["content"]),
                                timestamp,
                            ),
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Some("assistant") => {
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => builder.text(
                            Role::Assistant,
                            block["text"].as_str().unwrap_or_default(),
                            timestamp,
                        ),
                        Some("tool_use") => builder.tool_call(
                            block["id"].as_str(),
                            block["name"].as_str().unwrap_or("tool"),
                            &transcript::tool_input_text(&block["input"]),
                            timestamp,
                        ),
                        _ => {}
                    }
                }
            }
            Some("summary") => builder.text(
                Role::Context,
                record["summary"].as_str().unwrap_or_default(),
                timestamp,
            ),
            _ => {}
        }
    })?;
    Ok(builder.finish())
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

#[derive(Default, Clone)]
struct Parsed {
    cwd: Option<String>,
    custom_title: Option<String>,
    ai_title: Option<String>,
    summary: Option<String>,
    first_prompt: Option<String>,
    span: jsonl::Span,
}

impl Parsed {
    fn title(self) -> Option<String> {
        [
            self.custom_title,
            self.ai_title,
            self.summary,
            self.first_prompt,
        ]
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
        parsed.span.observe(jsonl::parse_timestamp(timestamp));
    }
    if parsed.cwd.is_none() {
        parsed.cwd = record.cwd;
    }
    match record.kind.as_deref() {
        Some("custom-title") => {
            parsed.custom_title = record.custom_title.or(parsed.custom_title.take())
        }
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

fn parse_file(path: &Path) -> std::io::Result<Parsed> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(path, |line| parse_line(&mut parsed, line))?;
    Ok(parsed)
}

impl ClaudeAdapter {
    fn read_session(&self, path: &Path, progress: &dyn ScanProgress) -> std::io::Result<Session> {
        let fingerprint = fs_util::fingerprint(path)?;
        let parsed = self
            .cache
            .get_or_parse(path, fingerprint, || parse_file(path))?;
        progress.advance(fingerprint.len);
        Ok(session_from(path, parsed, fingerprint))
    }

    fn read_subagent(
        &self,
        path: &Path,
        parent: &Session,
        progress: &dyn ScanProgress,
    ) -> std::io::Result<Session> {
        let fingerprint = fs_util::fingerprint(path)?;
        let parsed = self
            .cache
            .get_or_parse(path, fingerprint, || parse_file(path))?;
        progress.advance(fingerprint.len);
        let stem = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let meta = path.with_file_name(format!("{stem}.meta.json"));
        let title = subagent_meta_title(&meta).or_else(|| parsed.clone().title());
        let mut files = vec![path.to_path_buf()];
        if meta.is_file() {
            files.push(meta);
        }
        Ok(Session {
            agent: "claude",
            id: format!("{}/{stem}", parent.id),
            project_dir: parsed.cwd.clone().or_else(|| parent.project_dir.clone()),
            title,
            created: parsed.span.first.or_else(|| fs_util::created(path)),
            last_activity: parsed
                .span
                .last
                .unwrap_or_else(|| fingerprint.modified.into()),
            size_bytes: files.iter().map(|file| fs_util::path_size(file)).sum(),
            archived: false,
            primary_file: path.to_path_buf(),
            files,
            fingerprint,
            parent_id: Some(parent.id.clone()),
            deleted_with_parent: true,
            part_of_parent: true,
            projectless: false,
        })
    }
}

fn session_from(path: &Path, parsed: Parsed, fingerprint: Fingerprint) -> Session {
    let id = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let files = session_files(path, &id);
    let last_activity = parsed
        .span
        .last
        .unwrap_or_else(|| fingerprint.modified.into());
    let created = parsed.span.first.or_else(|| fs_util::created(path));
    Session {
        agent: "claude",
        size_bytes: files.iter().map(|file| fs_util::path_size(file)).sum(),
        project_dir: parsed.cwd.clone(),
        title: parsed.title(),
        id,
        created,
        last_activity,
        archived: false,
        primary_file: path.to_path_buf(),
        files,
        fingerprint,
        parent_id: None,
        deleted_with_parent: false,
        part_of_parent: false,
        projectless: false,
    }
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
    use chrono::{DateTime, Utc};

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

    const OTHER_ID: &str = "b1e2c3d4-0000-4000-8000-000000000002";

    /// A project with two sessions and memory (an index, a listed memory, an unlisted one and
    /// a link to a missing file), and a session-less project known only from `.claude.json`.
    fn memory_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let app = root.path().join("work").join("app");
        fs::create_dir_all(&app).unwrap();
        let app_str = app.to_string_lossy().into_owned();
        let line = serde_json::json!({ "type": "user", "cwd": app_str, "message": { "role": "user", "content": "hi" } }).to_string();
        let folder = write_project(
            &config,
            &project_folder_name(&app_str),
            &[
                (&format!("{ID}.jsonl"), &line),
                (&format!("{OTHER_ID}.jsonl"), &line),
            ],
        );
        let memory_dir = folder.join("memory");
        fs::create_dir_all(&memory_dir).unwrap();
        fs::write(
            memory_dir.join("MEMORY.md"),
            "- [Git](git.md) — hook\n- [Gone](gone.md) — hook\n",
        )
        .unwrap();
        fs::write(
            memory_dir.join("git.md"),
            "---\nname: git-identity\ndescription: Use the personal email\nmetadata:\n  type: feedback\n  originSessionId: a9d8\n---\nBody\n",
        )
        .unwrap();
        fs::write(memory_dir.join("stray.md"), "no frontmatter").unwrap();
        let old = root.path().join("work").join("old");
        let old_str = old.to_string_lossy().into_owned();
        let old_memory = config
            .join("projects")
            .join(project_folder_name(&old_str))
            .join("memory");
        fs::create_dir_all(&old_memory).unwrap();
        fs::write(old_memory.join("MEMORY.md"), "").unwrap();
        fs::write(
            config.join(".claude.json"),
            serde_json::json!({ "projects": { app_str.clone(): {}, old_str: {} } }).to_string(),
        )
        .unwrap();
        (root, config, folder)
    }

    #[test]
    fn lists_project_memory_with_its_problems() {
        let (_root, config, folder) = memory_fixture();
        let adapter = ClaudeAdapter::new(config.clone());
        let sessions = adapter.scan().sessions;

        let stores = adapter.memory(&sessions);

        assert_eq!(stores.len(), 2);
        let app = stores
            .iter()
            .find(|store| store.folder == folder.join("memory"))
            .unwrap();
        assert_eq!(app.sessions, 2);
        assert_eq!(app.project_exists, Some(true));
        let titles: Vec<&str> = app.files.iter().map(|file| file.title.as_str()).collect();
        assert_eq!(titles, ["MEMORY.md", "git-identity", "stray"]);
        let git = &app.files[1];
        assert_eq!(git.kind.as_deref(), Some("feedback"));
        assert_eq!(git.session_id.as_deref(), Some("a9d8"));
        assert_eq!(git.description.as_deref(), Some("Use the personal email"));
        assert!(
            app.problems
                .iter()
                .any(|problem| problem.contains("won't find it: stray.md"))
        );
        assert!(
            app.problems
                .iter()
                .any(|problem| problem.contains("don't exist: gone.md"))
        );

        let old = stores
            .iter()
            .find(|store| store.folder != folder.join("memory"))
            .unwrap();
        assert_eq!(old.sessions, 0);
        assert!(old.project_dir.as_deref().unwrap().ends_with("old"));
        assert_eq!(old.project_exists, Some(false));
        assert!(old.problems[0].contains("no longer exists"));
    }

    #[test]
    fn warns_when_the_index_is_longer_than_claude_loads() {
        let files = vec![MemoryFile {
            path: PathBuf::from("MEMORY.md"),
            title: "MEMORY.md".into(),
            description: None,
            kind: Some("index".into()),
            session_id: None,
            modified: None,
            size: 0,
        }];

        let problems = memory_problems(None, None, Some(&"- line\n".repeat(201)), &files);

        assert_eq!(
            problems,
            ["MEMORY.md has 201 lines; Claude Code only loads the first 200."]
        );
    }

    #[test]
    fn memory_is_left_behind_only_by_deleting_a_projects_last_session() {
        let (_root, config, folder) = memory_fixture();
        let adapter = ClaudeAdapter::new(config);
        let sessions = adapter.scan().sessions;

        let one = adapter.memory_left_behind(&[&sessions[0]], &sessions);
        let both = adapter.memory_left_behind(&[&sessions[0], &sessions[1]], &sessions);

        assert!(one.is_empty());
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].folder, folder.join("memory"));
        assert_eq!(both[0].sessions, 0);
    }

    #[test]
    fn owns_only_memory_folders_and_their_markdown_files() {
        let (root, config, folder) = memory_fixture();
        let adapter = ClaudeAdapter::new(config);
        let memory_dir = folder.join("memory");

        assert!(adapter.owns_memory_folder(&memory_dir));
        assert!(adapter.owns_memory_file(&memory_dir.join("git.md")));
        assert!(!adapter.owns_memory_folder(&folder));
        assert!(!adapter.owns_memory_folder(&memory_dir.join("..").join("..").join("..")));
        assert!(!adapter.owns_memory_file(&folder.join(format!("{ID}.jsonl"))));
        assert!(!adapter.owns_memory_folder(root.path()));
        assert!(adapter.memory_in_use(&memory_dir));
        fs::remove_file(folder.join(format!("{ID}.jsonl"))).unwrap();
        fs::remove_file(folder.join(format!("{OTHER_ID}.jsonl"))).unwrap();
        assert!(!adapter.memory_in_use(&memory_dir));
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
        write_project(
            config.path(),
            "-work-app",
            &[(&format!("{ID}.jsonl"), &transcript)],
        );

        let session = scan_one(config.path());

        assert_eq!(session.key(), format!("claude:{ID}"));
        assert_eq!(session.project_dir.as_deref(), Some("/work/app"));
        assert_eq!(session.title.as_deref(), Some("Login bug fix"));
        assert_eq!(
            session.last_activity.to_rfc3339(),
            "2026-11-02T09:30:00+00:00"
        );
        assert_eq!(
            session.created.unwrap().to_rfc3339(),
            "2026-11-01T10:00:00+00:00"
        );
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
        assert!(
            result
                .sessions
                .iter()
                .all(|s| !s.files.contains(&dir.join("memory")))
        );
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
    fn transcript_pairs_tools_marks_context_and_skips_sidechains() {
        let config = tempfile::tempdir().unwrap();
        let transcript = [
            r#"{"type":"user","isMeta":true,"message":{"role":"user","content":"<local-command-caveat>x</local-command-caveat>"}}"#,
            r#"{"type":"user","timestamp":"2026-11-01T10:00:00Z","message":{"role":"user","content":"What's in **src**?"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"thinking","thinking":""}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls src"}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"main.rs"}]}}"#,
            r#"{"type":"assistant","isSidechain":true,"message":{"role":"assistant","content":[{"type":"text","text":"subagent noise"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Just `main.rs`."}]}}"#,
        ]
        .join("\n");
        write_project(config.path(), "p", &[(&format!("{ID}.jsonl"), &transcript)]);
        let adapter = ClaudeAdapter::new(config.path().to_path_buf());
        let session = adapter.scan().sessions.remove(0);

        let messages = adapter.transcript(&session).unwrap().head;

        let roles: Vec<Role> = messages.iter().map(|message| message.role).collect();
        assert_eq!(
            roles,
            vec![Role::Context, Role::User, Role::Tool, Role::Assistant]
        );
        assert_eq!(messages[1].text, "What's in **src**?");
        assert!(messages[1].timestamp.is_some());
        let tool = messages[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "Bash");
        assert_eq!(tool.output.as_deref(), Some("main.rs"));
        assert!(messages[2].text.contains("ls src"));
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
                files: session.files.clone(),
                warning: None,
            }
        );
    }
}
