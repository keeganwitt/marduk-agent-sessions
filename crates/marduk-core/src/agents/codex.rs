use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use serde_json::value::RawValue;

use super::cli::{Cli, CliOverrides};
use super::{env_dir, home_dir, total_size};
use crate::adapter::ScanProgress;
use crate::adapter::{AgentAdapter, AgentCommand, DeletePlan, Mechanics, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl;
use crate::memory::{self, MemoryFile, MemoryStore};
use crate::session::Fingerprint;
use crate::session::Session;
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// Codex keeps rollouts at `sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl` and moves them to
/// `archived_sessions/` with `codex archive`. Thread names live in `session_index.jsonl`.
pub struct CodexAdapter {
    home: PathBuf,
    /// Where Codex Desktop makes a folder for each chat started without a project.
    chats_dir: PathBuf,
    cli: Cli,
    cache: ParseCache<Fingerprint, Parsed>,
}

const PROGRAM: &str = "codex";
const INSTALL_URL: &str = "https://github.com/openai/codex#installing-and-running-codex-cli";

impl CodexAdapter {
    pub fn new(home: PathBuf, cli: Option<PathBuf>) -> Self {
        Self {
            home,
            chats_dir: default_chats_dir(),
            cli: Cli::fixed(PROGRAM, INSTALL_URL, cli),
            cache: ParseCache::default(),
        }
    }

    pub fn from_env(overrides: std::sync::Arc<CliOverrides>) -> Self {
        Self {
            home: env_dir("CODEX_HOME").unwrap_or_else(|| home_dir().join(".codex")),
            chats_dir: default_chats_dir(),
            cli: Cli::detect("codex", PROGRAM, INSTALL_URL, overrides),
            cache: ParseCache::default(),
        }
    }

    /// Every rollout with whether it's archived, plus how many compressed ones were skipped.
    fn rollouts(&self, result: &mut ScanResult) -> (Vec<(PathBuf, bool)>, usize) {
        let mut compressed = 0usize;
        let mut rollouts = Vec::new();
        for (root, archived) in [(self.sessions_dir(), false), (self.archive_dir(), true)] {
            let mut files = Vec::new();
            collect_rollouts(&root, &mut files, &mut compressed, result);
            rollouts.extend(files.into_iter().map(|path| (path, archived)));
        }
        (rollouts, compressed)
    }

    fn read_segment(&self, path: &Path, progress: &dyn ScanProgress) -> std::io::Result<Segment> {
        let fingerprint = fs_util::fingerprint(path)?;
        let parsed = self
            .cache
            .get_or_parse(path, fingerprint, || parse_file(path))?;
        progress.advance(fingerprint.len);
        Ok(Segment {
            path: path.to_path_buf(),
            fingerprint,
            parsed,
        })
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
        let program = self.cli.path().ok_or_else(|| {
            self.cli
                .missing_reason("Codex", "delete and archive Codex sessions")
        })?;
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

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        self.cli.refresh();
        let mut result = ScanResult::empty_complete();
        let titles = read_thread_names(&self.home.join("session_index.jsonl"));
        let (rollouts, compressed) = self.rollouts(&mut result);
        let mut threads: HashMap<(String, bool), Vec<Segment>> = HashMap::new();
        for (path, archived) in &rollouts {
            match self.read_segment(path, progress) {
                Ok(segment) => {
                    let id = segment
                        .parsed
                        .id
                        .clone()
                        .unwrap_or_else(|| id_from_file_name(path));
                    threads.entry((id, *archived)).or_default().push(segment);
                }
                Err(error) => result.record_error(format!("{}: {error}", path.display())),
            }
        }
        for ((id, archived), segments) in threads {
            result.sessions.push(session_from(
                id,
                archived,
                &titles,
                &self.chats_dir,
                segments,
            ));
        }
        self.cache
            .retain(&rollouts.into_iter().map(|(path, _)| path).collect());
        if compressed > 0 {
            result.record_error(format!(
                "{compressed} compressed rollouts (.jsonl.zst) were skipped; Marduk can't read them yet."
            ));
        }
        result
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let (rollouts, _) = self.rollouts(&mut ScanResult::default());
        total_size(
            &rollouts
                .into_iter()
                .map(|(path, _)| path)
                .collect::<Vec<_>>(),
        )
    }

    fn memory(&self, sessions: &[Session]) -> Vec<MemoryStore> {
        let ids: HashSet<&str> = sessions.iter().map(|session| session.id.as_str()).collect();
        let enabled = memories_enabled(&self.home.join("config.toml"));
        MEMORY_FOLDERS
            .iter()
            .map(|name| self.home.join(name))
            .filter(|folder| folder.is_dir())
            .map(|folder| memory_store(&folder, &ids, enabled))
            .collect()
    }

    fn owns_memory_file(&self, path: &Path) -> bool {
        let Ok(path) = path.canonicalize() else {
            return false;
        };
        path.is_file()
            && !path.components().any(|part| part.as_os_str() == ".git")
            && MEMORY_FOLDERS.iter().any(|name| {
                self.home
                    .join(name)
                    .canonicalize()
                    .is_ok_and(|folder| path.starts_with(folder))
            })
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: "Runs `codex delete --force <id>`, which deletes permanently, along with the \
                     subagent sessions it spawned (but not its Guardian reviews). Needs the Codex \
                     CLI; without it, Codex sessions are read-only."
                .to_string(),
            archive: "Runs `codex archive <id>` or `codex unarchive <id>`, which move the \
                      session between Codex's sessions and archived_sessions folders."
                .to_string(),
        }
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
        read_transcript(&session.files)
    }

    /// A thread can span several files (paginated history), so all of them count.
    fn fingerprint(&self, session: &Session) -> std::io::Result<Fingerprint> {
        let fingerprints = session
            .files
            .iter()
            .map(|file| fs_util::fingerprint(file))
            .collect::<std::io::Result<Vec<_>>>()?;
        Ok(combined_fingerprint(&fingerprints))
    }

    fn still_present(&self, session: &Session) -> bool {
        session.files.iter().any(|file| file.exists())
    }

    fn cli(&self) -> Option<&Cli> {
        Some(&self.cli)
    }

    fn archive_command(&self, session: &Session) -> Option<AgentCommand> {
        let verb = if session.archived {
            "unarchive"
        } else {
            "archive"
        };
        self.command(session, &[verb]).ok()
    }
}

fn collect_rollouts(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    compressed: &mut usize,
    result: &mut ScanResult,
) {
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
    ordinal: Option<u64>,
    #[serde(rename = "type", borrow)]
    kind: Option<Cow<'a, str>>,
    #[serde(borrow)]
    payload: Option<&'a RawValue>,
}

#[derive(Default, Clone)]
struct Parsed {
    id: Option<String>,
    parent_id: Option<String>,
    /// Spawned subagents are deleted with their parent; guardian reviews are not.
    spawned: bool,
    seen_header: bool,
    cwd: Option<String>,
    first_prompt: Option<String>,
    span: jsonl::Span,
    /// Forked subagents start with a copy of their parent's history; records numbered
    /// below this ordinal are that copy.
    history_start: Option<u64>,
    /// Ordinal of the first record, which orders a paginated thread's files.
    first_ordinal: Option<u64>,
    agent_path: Option<String>,
    agent_nickname: Option<String>,
    agent_role: Option<String>,
    runtime_roots: Vec<String>,
    models: Vec<String>,
}

impl Parsed {
    fn inherited(&self, ordinal: Option<u64>) -> bool {
        matches!((self.history_start, ordinal), (Some(start), Some(ordinal)) if ordinal < start)
    }

    /// "review_consent · Volta" from a spawned subagent's task name and nickname; its role is
    /// the session's agent type.
    fn subagent_title(&self) -> Option<String> {
        let task = self
            .agent_path
            .as_deref()
            .and_then(|path| path.rsplit('/').find(|part| !part.is_empty()));
        let who = self.agent_nickname.clone();
        match (task, who) {
            (Some(task), Some(who)) => Some(format!("{task} · {who}")),
            (Some(task), None) => Some(task.to_string()),
            (None, who) => who,
        }
    }
}

/// Codex's memory store: `memories_v2` on fresh installs, `memories` on older ones.
const MEMORY_FOLDERS: [&str; 2] = ["memories_v2", "memories"];

/// Whether `[features] memories = true` is set; Codex memories are off by default.
fn memories_enabled(config: &Path) -> bool {
    let Ok(text) = fs::read_to_string(config) else {
        return false;
    };
    let mut table = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            table = name.trim().to_string();
        } else if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            if (table == "features" && key == "memories")
                || (table.is_empty() && key == "features.memories")
            {
                return value.split('#').next().unwrap_or_default().trim() == "true";
            }
        }
    }
    false
}

/// Codex generates every file here; Marduk only shows them.
fn memory_store(folder: &Path, session_ids: &HashSet<&str>, enabled: bool) -> MemoryStore {
    let mut files = Vec::new();
    collect_memory_files(folder, folder, &mut files);
    let rank = |file: &MemoryFile| match file.kind.as_deref() {
        Some("loaded") => 0,
        Some("index") => 1,
        Some("raw notes") => 2,
        Some("session summary") => 3,
        Some("skill") => 4,
        _ => 5,
    };
    files.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.path.cmp(&b.path)));
    // Session summaries are named after when they were written; newest first.
    let first = files.iter().position(|file| rank(file) == 3);
    let count = files.iter().filter(|file| rank(file) == 3).count();
    if let Some(first) = first {
        files[first..first + count].reverse();
    }
    let summaries: Vec<&MemoryFile> = files
        .iter()
        .filter(|file| file.kind.as_deref() == Some("session summary"))
        .collect();
    let live = summaries
        .iter()
        .filter(|file| {
            file.session_id
                .as_deref()
                .is_some_and(|id| session_ids.contains(id))
        })
        .count();
    let mut problems = Vec::new();
    if summaries.len() > live {
        let gone = summaries.len() - live;
        problems.push(format!(
            "{gone} session {} from sessions that no longer exist.",
            if gone == 1 {
                "summary is"
            } else {
                "summaries are"
            }
        ));
    }
    let reset = "Codex treats these files as generated state. To clear them, use Settings › \
                 Personalization › Delete Codex memories in Codex.";
    MemoryStore {
        agent: "codex",
        agent_name: "Codex",
        folder: folder.to_path_buf(),
        project_dir: None,
        project_exists: None,
        sessions: live,
        files,
        problems,
        note: Some(if enabled {
            reset.to_string()
        } else {
            format!(
                "Codex memories are turned off, so Codex neither uses nor updates these files. {reset}"
            )
        }),
        can_trash: false,
    }
}

fn collect_memory_files(root: &Path, dir: &Path, files: &mut Vec<MemoryFile>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        })
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_memory_files(root, &path, files);
        } else if path.is_file() {
            files.push(memory_file(root, path));
        }
    }
}

fn memory_file(root: &Path, path: PathBuf) -> MemoryFile {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(&path)
        .to_string_lossy()
        .replace('\\', "/");
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let (title, kind, description, session_id) = match relative.as_str() {
        "memory_summary.md" => (
            relative.clone(),
            Some("loaded"),
            Some("The summary Codex loads into new sessions."),
            None,
        ),
        "MEMORY.md" => (
            relative.clone(),
            Some("index"),
            Some("What Codex has learned, by task group."),
            None,
        ),
        "raw_memories.md" => (
            relative.clone(),
            Some("raw notes"),
            Some("Notes from individual sessions, before Codex combines them."),
            None,
        ),
        _ if relative.starts_with("rollout_summaries/") => {
            let header = fs::read_to_string(&path)
                .map(|text| memory::header_fields(&text))
                .unwrap_or_default();
            // `2026-08-18T20-42-25-eXxg-vault_backed_services`: a timestamp, a tag, then the topic.
            let topic = stem
                .splitn(7, '-')
                .nth(6)
                .unwrap_or(&stem)
                .replace('_', " ");
            (
                topic,
                Some("session summary"),
                None,
                header.get("thread_id").cloned(),
            )
        }
        _ if relative.starts_with("skills/") => (
            path.parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or(stem),
            Some("skill"),
            None,
            None,
        ),
        _ => (relative.clone(), None, None, None),
    };
    MemoryFile {
        title,
        description: description.map(str::to_string),
        kind: kind.map(str::to_string),
        session_id,
        modified: memory::modified(&path),
        size: memory::size(&path),
        path,
    }
}

fn default_chats_dir() -> PathBuf {
    home_dir().join("Documents").join("Codex")
}

/// Codex Desktop starts a chat without a project in a throwaway `<chats>/<YYYY-MM-DD>/<name>`
/// folder. Newer rollouts record `<chats>` as a workspace root; older ones record no roots.
fn is_projectless(cwd: &str, runtime_roots: &[String], chats_dir: &Path) -> bool {
    let cwd = Path::new(cwd);
    runtime_roots
        .iter()
        .map(Path::new)
        .chain(std::iter::once(chats_dir))
        .any(|root| {
            cwd.strip_prefix(root).is_ok_and(|rest| {
                let mut parts = rest.components();
                matches!(
                    (parts.next(), parts.next()),
                    (Some(Component::Normal(day)), Some(Component::Normal(_))) if is_date(day)
                )
            })
        })
}

fn is_date(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        name.len() == 10
            && name.bytes().enumerate().all(|(index, byte)| match index {
                4 | 7 => byte == b'-',
                _ => byte.is_ascii_digit(),
            })
    })
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
    if let Some(ordinal) = record.ordinal {
        parsed.first_ordinal = Some(
            parsed
                .first_ordinal
                .map_or(ordinal, |first| first.min(ordinal)),
        );
    }
    let inherited = parsed.inherited(record.ordinal);
    let Some(payload) = record.payload else {
        return;
    };
    match record.kind.as_deref() {
        // Forked subagents copy their parent's history, header included, after their own
        // header; only the first header describes this session.
        Some("session_meta") if !parsed.seen_header => {
            if let Ok(meta) = serde_json::from_str::<Value>(payload.get()) {
                parsed.seen_header = true;
                parsed.id = meta["id"].as_str().map(str::to_string);
                parsed.cwd = meta["cwd"].as_str().map(str::to_string);
                parsed.parent_id = parent_thread(&meta);
                parsed.spawned = meta["source"]["subagent"]["thread_spawn"].is_object();
                parsed.history_start = meta["subagent_history_start_ordinal"].as_u64();
                let spawn = &meta["source"]["subagent"]["thread_spawn"];
                let field = |name: &str| {
                    meta[name]
                        .as_str()
                        .or_else(|| spawn[name].as_str())
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                };
                parsed.agent_path = field("agent_path");
                parsed.agent_nickname = field("agent_nickname");
                parsed.agent_role = field("agent_role");
                parsed.runtime_roots = meta["runtime_workspace_roots"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|root| root.as_str().map(str::to_string))
                    .collect();
                parsed
                    .span
                    .observe(meta["timestamp"].as_str().and_then(jsonl::parse_timestamp));
            }
        }
        Some("turn_context") if !inherited => {
            if let Ok(context) = serde_json::from_str::<Value>(payload.get())
                && let Some(model) = context["model"].as_str()
            {
                crate::session::note_model(&mut parsed.models, model);
            }
        }
        Some("event_msg")
            if parsed.first_prompt.is_none()
                && !inherited
                && payload.get().contains("\"user_message\"") =>
        {
            if let Ok(event) = serde_json::from_str::<Value>(payload.get())
                && event["type"] == "user_message"
                && let Some(text) = event["message"].as_str().filter(|text| !is_injected(text))
            {
                parsed.first_prompt = Some(text.to_string());
            }
        }
        Some("response_item")
            if parsed.first_prompt.is_none()
                && !inherited
                && payload.get().contains("\"user\"") =>
        {
            if let Ok(item) = serde_json::from_str::<Value>(payload.get())
                && item["type"] == "message"
                && item["role"] == "user"
            {
                parsed.first_prompt =
                    jsonl::content_text(&item["content"]).filter(|text| !is_injected(text));
            }
        }
        _ => {}
    }
}

/// The thread that spawned this one: guardian reviews record `parent_thread_id` at the top
/// level, spawned subagents under `source.subagent.thread_spawn`.
fn parent_thread(meta: &Value) -> Option<String> {
    [
        &meta["parent_thread_id"],
        &meta["source"]["subagent"]["thread_spawn"]["parent_thread_id"],
    ]
    .into_iter()
    .find_map(|value| value.as_str().filter(|id| !id.is_empty()))
    .map(str::to_string)
}

const ENCRYPTED_ONLY_NOTE: &str = "This session only contains an encrypted context record from Codex, so there's nothing readable to show.";

const FORK_NOTE: &str = "This subagent started from a copy of its parent's conversation; those earlier messages are in the parent session.";

/// Uses `response_item` records only; `event_msg` repeats the same conversation. A forked
/// subagent's copied parent history is skipped.
fn read_transcript(files: &[PathBuf]) -> std::io::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    let mut encrypted_context = false;
    let mut header = Parsed::default();
    let mut skipped = false;
    for path in files {
        jsonl::for_each_line(path, |line| {
            let Ok(record) = serde_json::from_str::<Value>(line) else {
                return;
            };
            if record["type"] == "session_meta" {
                parse_line(&mut header, line);
                return;
            }
            if header.inherited(record["ordinal"].as_u64()) {
                skipped = true;
                return;
            }
            if record["type"] != "response_item" {
                return;
            }
            let timestamp = record["timestamp"]
                .as_str()
                .and_then(jsonl::parse_timestamp);
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
                Some("compaction") => {
                    encrypted_context |= item["encrypted_content"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty())
                }
                _ => {}
            }
        })?;
    }
    let mut transcript = builder.finish();
    let readable = transcript
        .head
        .iter()
        .chain(&transcript.tail)
        .any(|message| message.role != Role::Context);
    if encrypted_context && !readable {
        transcript.note = Some(ENCRYPTED_ONLY_NOTE.to_string());
    } else if skipped {
        transcript.note = Some(FORK_NOTE.to_string());
    }
    Ok(transcript)
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

fn parse_file(path: &Path) -> std::io::Result<Parsed> {
    let mut parsed = Parsed::default();
    jsonl::for_each_line(path, |line| parse_line(&mut parsed, line))?;
    Ok(parsed)
}

/// One rollout file of a thread.
struct Segment {
    path: PathBuf,
    fingerprint: Fingerprint,
    parsed: Parsed,
}

fn combined_fingerprint(fingerprints: &[Fingerprint]) -> Fingerprint {
    Fingerprint {
        len: fingerprints.iter().map(|fingerprint| fingerprint.len).sum(),
        modified: fingerprints
            .iter()
            .map(|fingerprint| fingerprint.modified)
            .max()
            .unwrap_or(std::time::UNIX_EPOCH),
    }
}

/// Builds one session from a thread's files, ordered by their first record ordinal (a
/// paginated thread continues in files named `rollout-…-<id>_<page>.jsonl`).
fn session_from(
    id: String,
    archived: bool,
    titles: &HashMap<String, String>,
    chats_dir: &Path,
    mut segments: Vec<Segment>,
) -> Session {
    segments.sort_by(|a, b| {
        (a.parsed.first_ordinal.unwrap_or(0), &a.path)
            .cmp(&(b.parsed.first_ordinal.unwrap_or(0), &b.path))
    });
    let first = &segments[0].parsed;
    let first_prompt = segments
        .iter()
        .find_map(|segment| segment.parsed.first_prompt.clone());
    let mut span = jsonl::Span::default();
    for segment in &segments {
        span.observe(segment.parsed.span.first);
        span.observe(segment.parsed.span.last);
    }
    let fingerprint = combined_fingerprint(
        &segments
            .iter()
            .map(|segment| segment.fingerprint)
            .collect::<Vec<_>>(),
    );
    let indexed_name = titles.get(&id).filter(|name| !is_injected(name)).cloned();
    let title = [indexed_name, first.subagent_title(), first_prompt]
        .into_iter()
        .flatten()
        .find_map(|text| jsonl::to_title(&text));
    let primary = segments[0].path.clone();
    Session {
        agent: "codex",
        id,
        project_dir: first.cwd.clone(),
        title,
        created: span.first.or_else(|| fs_util::created(&primary)),
        last_activity: span.last.unwrap_or_else(|| fingerprint.modified.into()),
        size_bytes: fingerprint.len,
        archived,
        files: segments
            .iter()
            .map(|segment| segment.path.clone())
            .collect(),
        primary_file: primary,
        fingerprint,
        parent_id: first.parent_id.clone(),
        deleted_with_parent: first.spawned,
        part_of_parent: false,
        projectless: first
            .cwd
            .as_deref()
            .is_some_and(|cwd| is_projectless(cwd, &first.runtime_roots, chats_dir)),
        models: segments.iter().fold(Vec::new(), |mut models, segment| {
            for model in &segment.parsed.models {
                crate::session::note_model(&mut models, model);
            }
            models
        }),
        agent_type: first.agent_role.clone(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

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
        let active = home.path().join(format!(
            "sessions/2026/11/06/rollout-2026-11-06T18-05-08-{ID}.jsonl"
        ));
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
        assert_eq!(
            session.title.as_deref(),
            Some("Add pagination to the users endpoint")
        );
        assert_eq!(
            session.last_activity.to_rfc3339(),
            "2026-11-06T19:00:00+00:00"
        );
        assert_eq!(
            session.created.unwrap().to_rfc3339(),
            "2026-11-06T18:05:08+00:00"
        );
        assert_eq!(session.files, vec![active]);
        assert!(sessions[1].archived);
    }

    #[test]
    fn recognizes_chats_started_without_a_project() {
        let home = tempfile::tempdir().unwrap();
        let rollout_with = |id: &str, meta: serde_json::Value| {
            let mut payload = serde_json::json!({ "id": id });
            payload
                .as_object_mut()
                .unwrap()
                .extend(meta.as_object().unwrap().clone());
            write(
                &home
                    .path()
                    .join(format!("sessions/2026/11/06/rollout-x-{id}.jsonl")),
                &serde_json::json!({ "type": "session_meta", "payload": payload }).to_string(),
            );
        };
        let ids = [
            "019e98e4-0000-7000-8000-000000000001",
            "019e98e4-0000-7000-8000-000000000002",
            "019e98e4-0000-7000-8000-000000000003",
            "019e98e4-0000-7000-8000-000000000004",
        ];
        rollout_with(
            ids[0],
            serde_json::json!({ "cwd": "/u/chats/2026-11-06/tel", "runtime_workspace_roots": ["/u/chats"] }),
        );
        rollout_with(
            ids[1],
            serde_json::json!({ "cwd": "/u/Documents/Codex/2026-08-25/x20" }),
        );
        rollout_with(
            ids[2],
            serde_json::json!({ "cwd": "/u/repo", "runtime_workspace_roots": ["/u/repo"] }),
        );
        rollout_with(
            ids[3],
            serde_json::json!({ "cwd": "/u/Documents/Codex/notes", "runtime_workspace_roots": ["/u/Documents/Codex/notes"] }),
        );
        let codex = CodexAdapter {
            chats_dir: PathBuf::from("/u/Documents/Codex"),
            ..adapter(home.path(), None)
        };

        let sessions = codex.scan().sessions;

        let projectless = |id: &str| {
            sessions
                .iter()
                .find(|session| session.id == id)
                .unwrap()
                .projectless
        };
        assert!(projectless(ids[0]), "recorded chats root");
        assert!(
            projectless(ids[1]),
            "default chats folder, no roots recorded"
        );
        assert!(!projectless(ids[2]), "a project");
        assert!(!projectless(ids[3]), "not a dated chat folder");
    }

    #[test]
    fn lists_codex_memory_read_only_and_links_summaries_to_sessions() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &rollout(ID),
        );
        let memories = home.path().join("memories");
        write(&memories.join("memory_summary.md"), "summary");
        write(&memories.join("MEMORY.md"), "# Task Group");
        write(&memories.join("raw_memories.md"), "raw");
        write(
            &memories.join("rollout_summaries/2026-08-18T20-42-25-eXxg-vault_catalogue.md"),
            &format!("thread_id: {ID}\ncwd: /work\n\nOutcome: success"),
        );
        write(
            &memories.join("rollout_summaries/2026-08-19T10-00-00-AbCd-gone_session.md"),
            "thread_id: 019e98e4-dead-7000-8000-000000000000\n\nOutcome: success",
        );
        write(&memories.join("skills/verify-save/SKILL.md"), "skill");
        write(&memories.join(".git/HEAD"), "ref: refs/heads/main");
        let codex = adapter(home.path(), None);
        let sessions = codex.scan().sessions;

        let stores = codex.memory(&sessions);

        assert_eq!(stores.len(), 1);
        let store = &stores[0];
        assert!(!store.can_trash);
        assert!(
            store
                .note
                .as_deref()
                .unwrap()
                .starts_with("Codex memories are turned off")
        );
        let kinds: Vec<(&str, Option<&str>)> = store
            .files
            .iter()
            .map(|file| (file.title.as_str(), file.kind.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("memory_summary.md", Some("loaded")),
                ("MEMORY.md", Some("index")),
                ("raw_memories.md", Some("raw notes")),
                ("gone session", Some("session summary")),
                ("vault catalogue", Some("session summary")),
                ("verify-save", Some("skill")),
            ]
        );
        let vault = store
            .files
            .iter()
            .find(|file| file.title == "vault catalogue")
            .unwrap();
        assert_eq!(vault.session_id.as_deref(), Some(ID));
        assert_eq!(store.sessions, 1);
        assert_eq!(
            store.problems,
            ["1 session summary is from sessions that no longer exist."]
        );
        assert!(codex.owns_memory_file(&memories.join("MEMORY.md")));
        assert!(!codex.owns_memory_file(&memories.join(".git/HEAD")));
        assert!(!codex.owns_memory_file(&home.path().join("config.toml")));
    }

    #[test]
    fn reads_whether_codex_memories_are_on() {
        let home = tempfile::tempdir().unwrap();
        let config = home.path().join("config.toml");
        write(&config, "model = \"x\"\n[features]\nmemories = true # on\n");
        assert!(memories_enabled(&config));
        write(
            &config,
            "[features]\nmemories = false\n[memories]\nuse_memories = true\n",
        );
        assert!(!memories_enabled(&config));
        write(&config, "features.memories = true\n");
        assert!(memories_enabled(&config));
        assert!(!memories_enabled(&home.path().join("missing.toml")));
    }

    #[test]
    fn records_models_from_turn_context() {
        let home = tempfile::tempdir().unwrap();
        let context = |model: &str| {
            format!(r#"{{"type":"turn_context","payload":{{"model":"{model}","cwd":"/work"}}}}"#)
        };
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &[
                rollout(ID),
                context("gpt-6.1"),
                context("gpt-6.1-mini"),
                context("gpt-6.1"),
            ]
            .join("\n"),
        );

        let sessions = adapter(home.path(), None).scan().sessions;

        assert_eq!(sessions[0].models, ["gpt-6.1-mini", "gpt-6.1"]);
    }

    #[test]
    fn thread_name_from_index_wins_unless_it_is_injected() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &rollout(ID),
        );
        let other = "019e98e4-29fd-7f63-8cae-b345db610f8c";
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-y-{other}.jsonl")),
            &rollout(other),
        );
        write(
            &home.path().join("session_index.jsonl"),
            &[
                format!(r#"{{"id":"{ID}","thread_name":"old-name"}}"#),
                format!(r#"{{"id":"{ID}","thread_name":"pagination-work"}}"#),
                format!(
                    r#"{{"id":"{other}","thread_name":"<command-name>/clear</command-name>"}}"#
                ),
            ]
            .join("\n"),
        );

        let sessions = adapter(home.path(), None).scan().sessions;
        let title_of = |id: &str| sessions.iter().find(|s| s.id == id).unwrap().title.clone();

        assert_eq!(title_of(ID).as_deref(), Some("pagination-work"));
        assert_eq!(
            title_of(other).as_deref(),
            Some("Add pagination to the users endpoint")
        );
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
            &home.path().join(format!(
                "sessions/2026/01/01/rollout-2026-01-01T00-00-00-{ID}.jsonl"
            )),
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
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &content,
        );
        let codex = adapter(home.path(), None);
        let session = codex.scan().sessions.remove(0);

        let messages = codex.transcript(&session).unwrap().head;

        let roles: Vec<Role> = messages.iter().map(|message| message.role).collect();
        assert_eq!(
            roles,
            vec![Role::Context, Role::User, Role::Tool, Role::Assistant]
        );
        let tool = messages[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "exec_command");
        assert_eq!(tool.output.as_deref(), Some("ci.yml"));
        assert_eq!(messages[2].text, "{\n  \"cmd\": \"ls\"\n}");
    }

    #[test]
    fn records_the_parent_thread_of_guardian_reviews_and_spawned_subagents() {
        let home = tempfile::tempdir().unwrap();
        let guardian = "01a1b21a-26bc-7700-ae31-919e0387ce69";
        let spawned = "01a1b21a-26bc-7700-ae31-919e0387ce70";
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-a-{guardian}.jsonl")),
            &format!(
                r#"{{"type":"session_meta","payload":{{"id":"{guardian}","parent_thread_id":"parent-1","thread_source":"guardian_review","source":{{"subagent":{{"other":"guardian"}}}}}}}}"#
            ),
        );
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-b-{spawned}.jsonl")),
            &format!(
                r#"{{"type":"session_meta","payload":{{"id":"{spawned}","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"parent-2","depth":1}}}}}}}}}}"#
            ),
        );
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-c-{ID}.jsonl")),
            &rollout(ID),
        );

        let sessions = adapter(home.path(), None).scan().sessions;
        let parent_of = |id: &str| {
            sessions
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .parent_id
                .clone()
        };

        assert_eq!(parent_of(guardian).as_deref(), Some("parent-1"));
        assert_eq!(parent_of(spawned).as_deref(), Some("parent-2"));
        let cascades = |id: &str| {
            sessions
                .iter()
                .find(|s| s.id == id)
                .unwrap()
                .deleted_with_parent
        };
        assert!(
            !cascades(guardian),
            "codex delete leaves guardian reviews behind"
        );
        assert!(cascades(spawned));
        assert_eq!(parent_of(ID), None);
    }

    #[test]
    fn forked_subagents_use_their_own_header_not_the_copied_parent_one() {
        let home = tempfile::tempdir().unwrap();
        let parent = "01a1b209-2d68-7e51-bd0c-2954bbab717e";
        let child = "01a1b219-0bdd-7943-9859-dc596e56a148";
        let content = [
            format!(
                r#"{{"type":"session_meta","payload":{{"id":"{child}","forked_from_id":"{parent}","cwd":"/child","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{parent}"}}}}}}}}}}"#
            ),
            format!(r#"{{"type":"session_meta","payload":{{"id":"{parent}","cwd":"/parent","source":"vscode"}}}}"#),
        ]
        .join("\n");
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{child}.jsonl")),
            &content,
        );

        let session = adapter(home.path(), None).scan().sessions.remove(0);

        assert_eq!(session.id, child);
        assert_eq!(session.parent_id.as_deref(), Some(parent));
        assert_eq!(session.project_dir.as_deref(), Some("/child"));
        assert!(session.deleted_with_parent);
    }

    #[test]
    fn forked_subagents_are_titled_by_task_and_skip_the_copied_history() {
        let home = tempfile::tempdir().unwrap();
        let parent = "01a1b209-2d68-7e51-bd0c-2954bbab717e";
        let child = "01a1b219-0bdd-7943-9859-dc596e56a148";
        let content = [
            format!(
                r#"{{"ordinal":5,"type":"session_meta","payload":{{"id":"{child}","forked_from_id":"{parent}","subagent_history_start_ordinal":3,"agent_path":"/root/review_consent","agent_nickname":"Volta","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{parent}"}}}}}}}}}}"#
            ),
            format!(r#"{{"ordinal":0,"type":"session_meta","payload":{{"id":"{parent}"}}}}"#),
            r#"{"ordinal":1,"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"The parent's question"}]}}"#.to_string(),
            r#"{"ordinal":2,"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"The parent's answer"}]}}"#.to_string(),
            r#"{"ordinal":3,"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Review the consent flow"}]}}"#.to_string(),
            r#"{"ordinal":4,"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Reviewed."}]}}"#.to_string(),
        ]
        .join("\n");
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{child}.jsonl")),
            &content,
        );
        let codex = adapter(home.path(), None);
        let session = codex.scan().sessions.remove(0);

        assert_eq!(session.title.as_deref(), Some("review_consent · Volta"));
        let transcript = codex.transcript(&session).unwrap();
        let texts: Vec<&str> = transcript
            .head
            .iter()
            .map(|message| message.text.as_str())
            .collect();
        assert_eq!(texts, vec!["Review the consent flow", "Reviewed."]);
        assert_eq!(transcript.note.as_deref(), Some(FORK_NOTE));
    }

    #[test]
    fn subagents_without_a_task_name_use_their_nickname_or_their_own_prompt() {
        let parsed = Parsed {
            agent_nickname: Some("Kepler".into()),
            agent_role: Some("explorer".into()),
            ..Parsed::default()
        };
        assert_eq!(parsed.subagent_title().as_deref(), Some("Kepler"));
        assert_eq!(Parsed::default().subagent_title(), None);
    }

    #[test]
    fn paginated_threads_are_one_session_across_their_files() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/09/18");
        let header = |ordinal: u32| {
            format!(
                r#"{{"ordinal":{ordinal},"timestamp":"2026-09-18T0{}:00:00Z","type":"session_meta","payload":{{"id":"{ID}","cwd":"/w","history_mode":"paginated"}}}}"#,
                ordinal / 100
            )
        };
        let message = |ordinal: u32, text: &str| {
            format!(
                r#"{{"ordinal":{ordinal},"type":"response_item","payload":{{"type":"message","role":"user","content":[{{"type":"input_text","text":"{text}"}}]}}}}"#
            )
        };
        write(
            &day.join(format!(
                "rollout-2026-09-18T11-39-28-{ID}_01a0b52c-341a-7111-86d8-3ca6756e56fd.jsonl"
            )),
            &[header(600), message(601, "second page")].join("\n"),
        );
        write(
            &day.join(format!("rollout-2026-09-18T00-20-09-{ID}.jsonl")),
            &[header(100), message(101, "first page")].join("\n"),
        );
        let codex = adapter(home.path(), Some("/bin/codex"));

        let sessions = codex.scan().sessions;

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.files.len(), 2);
        assert!(
            session
                .primary_file
                .ends_with(format!("rollout-2026-09-18T00-20-09-{ID}.jsonl"))
        );
        assert_eq!(session.title.as_deref(), Some("first page"));
        assert_eq!(
            session.created.unwrap().to_rfc3339(),
            "2026-09-18T01:00:00+00:00"
        );
        assert_eq!(
            session.last_activity.to_rfc3339(),
            "2026-09-18T06:00:00+00:00"
        );
        let texts: Vec<String> = codex
            .transcript(session)
            .unwrap()
            .head
            .into_iter()
            .map(|m| m.text)
            .collect();
        assert_eq!(texts, vec!["first page", "second page"]);
        assert_eq!(codex.fingerprint(session).unwrap(), session.fingerprint);
        let DeletePlan::AgentCommand { files, .. } = codex.delete_plan(session) else {
            panic!("expected codex delete");
        };
        assert_eq!(files, session.files);

        std::fs::remove_file(&session.files[0]).unwrap();
        assert!(codex.still_present(session), "a page is still on disk");
    }

    #[test]
    fn encrypted_only_sessions_explain_why_nothing_is_shown() {
        let home = tempfile::tempdir().unwrap();
        let content = [
            format!(r#"{{"type":"session_meta","payload":{{"id":"{ID}"}}}}"#),
            r#"{"type":"response_item","payload":{"type":"compaction","encrypted_content":"gAAAAABpZ..."}}"#
                .to_string(),
            r#"{"type":"event_msg","payload":{"type":"thread_settings_applied"}}"#.to_string(),
        ]
        .join("\n");
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &content,
        );
        let codex = adapter(home.path(), None);
        let session = codex.scan().sessions.remove(0);

        let transcript = codex.transcript(&session).unwrap();

        assert!(transcript.head.is_empty());
        assert_eq!(transcript.note.as_deref(), Some(ENCRYPTED_ONLY_NOTE));
    }

    #[test]
    fn readable_sessions_with_compaction_get_no_note() {
        let home = tempfile::tempdir().unwrap();
        let content = format!(
            "{}\n{}\n{}",
            rollout(ID),
            r#"{"type":"response_item","payload":{"type":"compaction","encrypted_content":"gAAAA"}}"#,
            r#"{"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done"}]}}"#
        );
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
            &content,
        );
        let codex = adapter(home.path(), None);
        let session = codex.scan().sessions.remove(0);
        assert!(codex.transcript(&session).unwrap().note.is_none());
    }

    #[test]
    fn rescans_reuse_unchanged_files_and_reread_changed_ones() {
        let home = tempfile::tempdir().unwrap();
        let path = home
            .path()
            .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl"));
        let prompt = |text: &str| {
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{ID}\"}}}}\n{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"{text}\"}}}}"
            )
        };
        write(&path, &prompt("first prompt"));
        let stamp =
            filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        filetime::set_file_mtime(&path, stamp).unwrap();
        let codex = adapter(home.path(), None);
        let title = |codex: &CodexAdapter| codex.scan().sessions.remove(0).title;
        assert_eq!(title(&codex).as_deref(), Some("first prompt"));

        write(&path, &prompt("other prompt"));
        filetime::set_file_mtime(&path, stamp).unwrap();
        assert_eq!(
            title(&codex).as_deref(),
            Some("first prompt"),
            "same size and mtime: the cached parse is reused"
        );

        write(&path, &prompt("a longer, changed prompt"));
        assert_eq!(title(&codex).as_deref(), Some("a longer, changed prompt"));
    }

    #[test]
    fn delete_and_archive_use_the_codex_cli() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
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
        assert_eq!(
            codex.archive_command(&session).unwrap().args,
            vec!["archive", ID]
        );
        session.archived = true;
        assert_eq!(
            codex.archive_command(&session).unwrap().args,
            vec!["unarchive", ID]
        );
    }

    #[test]
    fn read_only_without_cli_or_uuid() {
        let home = tempfile::tempdir().unwrap();
        write(
            &home
                .path()
                .join(format!("sessions/2026/11/06/rollout-x-{ID}.jsonl")),
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
        assert!(
            matches!(with_cli.delete_plan(&session), DeletePlan::ReadOnly { reason } if reason.contains("UUID"))
        );
    }
}
