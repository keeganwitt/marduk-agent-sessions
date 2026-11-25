use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::cli::{Cli, CliOverrides};
use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, DeletePlan, Mechanics, ScanProgress, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl::{self, Span, to_title};
use crate::resume::{ResumeMode, ResumeTarget};
use crate::session::{Fingerprint, Session, note_model};
use crate::transcript::{Role, Transcript, TranscriptBuilder, tool_input_text};

const ID: &str = "antigravity";
const PROGRAM: &str = "agy";
const INSTALL_URL: &str = "https://antigravity.google/docs/cli/reference";
const DELETE_REASON: &str = "Antigravity keeps session indexes in conversation_summaries.db and \
    agyhub_summaries_proto.pb, and `agy` has no non-interactive delete command. Delete sessions \
    inside Antigravity instead.";
const FTS_TRANSCRIPT_NOTE: &str =
    "Read from Antigravity's search index, because older .pb conversation files are encrypted.";
const ENCRYPTED_PB_NOTE: &str =
    "Older Antigravity .pb conversation files are encrypted and had no indexed text.";

const DATA_DIRS: [&str; 3] = ["antigravity", "antigravity-cli", "antigravity-ide"];

pub struct AntigravityAdapter {
    data_dirs: Vec<PathBuf>,
    cli: Cli,
    cache: ParseCache<Fingerprint, TranscriptMeta>,
}

impl AntigravityAdapter {
    pub fn new(data_dirs: Vec<PathBuf>) -> Self {
        Self {
            data_dirs,
            cli: Cli::fixed(PROGRAM, INSTALL_URL, None).optional(),
            cache: ParseCache::default(),
        }
    }

    pub fn from_env(overrides: Arc<CliOverrides>) -> Self {
        let data_dirs = if let Some(dir) = env_dir("MARDUK_ANTIGRAVITY_DIR") {
            vec![dir]
        } else {
            let gemini = home_dir().join(".gemini");
            DATA_DIRS.iter().map(|name| gemini.join(name)).collect()
        };
        Self {
            data_dirs,
            cli: Cli::detect(ID, PROGRAM, INSTALL_URL, overrides).optional(),
            cache: ParseCache::default(),
        }
    }

    pub fn with_cli(self, cli: Option<PathBuf>) -> Self {
        Self {
            cli: Cli::fixed(PROGRAM, INSTALL_URL, cli).optional(),
            ..self
        }
    }

    fn scan_dir(
        &self,
        data_dir: &Path,
        progress: &dyn ScanProgress,
        seen_transcripts: &mut HashSet<PathBuf>,
        result: &mut ScanResult,
    ) -> Vec<Session> {
        let conv_dir = data_dir.join("conversations");
        let entries = match fs::read_dir(&conv_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => {
                result.record_error(format!("{}: {error}", conv_dir.display()));
                return Vec::new();
            }
        };

        let mut migrated = HashSet::new();
        let mut by_id: BTreeMap<String, PathBuf> = BTreeMap::new();
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    result.record_error(format!("{}: {error}", conv_dir.display()));
                    continue;
                }
            };
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(id) = name.strip_prefix('.').and_then(|s| s.strip_suffix(".done")) {
                migrated.insert(id.to_string());
                continue;
            }
            let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                continue;
            };
            if ext != "db" && ext != "pb" {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if !fs_util::looks_like_uuid(stem) {
                continue;
            }
            let replace = match by_id.get(stem) {
                None => true,
                Some(existing) => {
                    existing.extension().and_then(|e| e.to_str()) != Some("db") && ext == "db"
                }
            };
            if replace {
                by_id.insert(stem.to_string(), path);
            }
        }

        let (summaries, fts_archived) =
            read_summaries(&data_dir.join("conversation_summaries.db"), result);

        let mut sessions = Vec::new();
        for (id, primary_file) in by_id {
            let is_pb = primary_file.extension().and_then(|e| e.to_str()) == Some("pb");
            if is_pb && migrated.contains(&id) {
                continue;
            }

            let mut fingerprint = match fs_util::fingerprint(&primary_file) {
                Ok(fp) => fp,
                Err(error) => {
                    result.record_error(format!("{}: {error}", primary_file.display()));
                    continue;
                }
            };

            let mut files = vec![primary_file.clone()];
            if !is_pb {
                for suffix in ["-wal", "-shm"] {
                    let sidecar = conv_dir.join(format!("{id}.db{suffix}"));
                    if let Ok(meta) = fs::metadata(&sidecar) {
                        if suffix == "-wal"
                            && let Ok(modified) = meta.modified()
                        {
                            fingerprint.modified = fingerprint.modified.max(modified);
                            fingerprint.len = fingerprint.len.saturating_add(meta.len());
                        }
                        files.push(sidecar);
                    }
                }
            }

            let brain_dir = data_dir.join("brain").join(&id);
            let transcript_path = brain_dir
                .join(".system_generated")
                .join("logs")
                .join("transcript.jsonl");
            if brain_dir.is_dir() {
                files.push(brain_dir);
            }

            let annotation_path = data_dir.join("annotations").join(format!("{id}.pbtxt"));
            let annotation = if annotation_path.is_file() {
                files.push(annotation_path.clone());
                fs::read_to_string(&annotation_path)
                    .ok()
                    .map(|text| parse_annotation(&text))
                    .unwrap_or_default()
            } else {
                Annotation::default()
            };

            for extra in [
                data_dir.join("browser_recordings").join(&id),
                data_dir.join("voice_chats").join(format!("{id}.json")),
                data_dir.join("presence").join(format!("{id}.lock")),
            ] {
                if extra.symlink_metadata().is_ok() {
                    files.push(extra);
                }
            }

            let transcript_meta = match fs_util::fingerprint(&transcript_path) {
                Ok(tfp) => {
                    progress.advance(tfp.len);
                    seen_transcripts.insert(transcript_path.clone());
                    fingerprint.modified = fingerprint.modified.max(tfp.modified);
                    match self.cache.get_or_parse(&transcript_path, tfp, || {
                        parse_transcript_meta(&transcript_path)
                    }) {
                        Ok(meta) => Some(meta),
                        Err(error) => {
                            result.record_error(format!("{}: {error}", transcript_path.display()));
                            None
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => {
                    result.record_error(format!("{}: {error}", transcript_path.display()));
                    None
                }
            };

            let summary = summaries.get(&id);
            let summary_modified = summary.and_then(|s| s.last_modified);
            if let Some(modified) = summary_modified {
                let sys: SystemTime = modified.into();
                fingerprint.modified = fingerprint.modified.max(sys);
            }

            let title = [
                annotation.title,
                summary.and_then(|s| s.title.clone()),
                summary.and_then(|s| s.preview.clone()),
                transcript_meta
                    .as_ref()
                    .and_then(|meta| meta.first_prompt.clone()),
            ]
            .into_iter()
            .flatten()
            .find_map(|text| to_title(&text));

            let project_dir = summary.and_then(|s| s.project_dir.clone());
            let projectless = project_dir.is_none()
                && summary.is_some_and(|s| s.project_id.as_deref() == Some("outside-of-project"));

            let last_activity = [
                transcript_meta.as_ref().and_then(|meta| meta.span.last),
                summary_modified,
                summary.and_then(|s| s.last_user_input),
                Some(fingerprint.modified.into()),
            ]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or_else(|| fingerprint.modified.into());

            let created = transcript_meta
                .as_ref()
                .and_then(|meta| meta.span.first)
                .or_else(|| fs_util::created(&primary_file));

            let archived = annotation
                .archived
                .or_else(|| fts_archived.get(&id).copied())
                .unwrap_or(false);

            let empty = match &transcript_meta {
                Some(meta) => !meta.has_messages,
                None => summary.is_some_and(|s| s.step_count == 0),
            };

            let size_bytes = files.iter().map(|path| fs_util::path_size(path)).sum();
            sessions.push(Session {
                agent: ID,
                id,
                project_dir,
                title,
                created,
                last_activity,
                size_bytes,
                archived,
                pinned: annotation.pinned,
                primary_file,
                files,
                fingerprint,
                parent_id: summary.and_then(|s| s.parent_id.clone()),
                deleted_with_parent: false,
                part_of_parent: false,
                projectless,
                models: transcript_meta.map(|meta| meta.models).unwrap_or_default(),
                agent_type: summary.and_then(|s| s.agent_name.clone()),
                empty,
            });
        }
        sessions
    }
}

impl AgentAdapter for AntigravityAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "Antigravity"
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.data_dirs.clone()
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let mut total = 0u64;
        for data_dir in &self.data_dirs {
            let Ok(entries) = fs::read_dir(data_dir.join("brain")) else {
                continue;
            };
            for entry in entries.flatten() {
                let transcript = entry
                    .path()
                    .join(".system_generated")
                    .join("logs")
                    .join("transcript.jsonl");
                if let Ok(meta) = fs::metadata(&transcript) {
                    total = total.saturating_add(meta.len());
                }
            }
        }
        total
    }

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        self.cli.refresh();
        let mut result = ScanResult::empty_complete();
        let mut seen_transcripts = HashSet::new();
        let mut all = Vec::new();
        for data_dir in &self.data_dirs {
            all.extend(self.scan_dir(data_dir, progress, &mut seen_transcripts, &mut result));
        }
        self.cache.retain(&seen_transcripts);
        result.sessions = latest_copies(all);
        result
    }

    fn delete_plan(&self, _session: &Session) -> DeletePlan {
        DeletePlan::ReadOnly {
            reason: DELETE_REASON.into(),
        }
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: "Antigravity sessions are read-only in Marduk. Delete them in Antigravity or \
                     with `agy`'s interactive session picker."
                .into(),
            archive: "Shows Antigravity's saved archive state. Archive or unarchive sessions in \
                      Antigravity."
                .into(),
        }
    }

    fn cli(&self) -> Option<&Cli> {
        Some(&self.cli)
    }

    fn resume_target(&self, session: &Session, mode: ResumeMode) -> Option<ResumeTarget> {
        (mode == ResumeMode::Terminal && fs_util::looks_like_uuid(&session.id))
            .then(|| ResumeTarget::cli(["--conversation", session.id.as_str()]))
    }

    fn transcript(&self, session: &Session) -> io::Result<Transcript> {
        let Some(data_dir) = session.primary_file.parent().and_then(Path::parent) else {
            return Ok(Transcript::default());
        };
        let transcript_path = data_dir
            .join("brain")
            .join(&session.id)
            .join(".system_generated")
            .join("logs")
            .join("transcript.jsonl");
        if transcript_path.is_file() {
            return read_jsonl_transcript(&transcript_path);
        }
        Ok(read_fts_transcript(
            &data_dir.join("conversation_summaries.db"),
            &session.id,
        ))
    }
}

fn latest_copies(sessions: Vec<Session>) -> Vec<Session> {
    let mut by_id: BTreeMap<String, Session> = BTreeMap::new();
    for session in sessions {
        let id = session.id.clone();
        if let Some(known) = by_id.get(&id) {
            let known_is_db = known.primary_file.extension().and_then(|e| e.to_str()) == Some("db");
            let next_is_db =
                session.primary_file.extension().and_then(|e| e.to_str()) == Some("db");
            if (known_is_db, known.last_activity, &known.primary_file)
                >= (next_is_db, session.last_activity, &session.primary_file)
            {
                continue;
            }
        }
        by_id.insert(id, session);
    }
    by_id.into_values().collect()
}

#[derive(Default)]
struct SummaryRow {
    title: Option<String>,
    preview: Option<String>,
    step_count: i64,
    last_modified: Option<DateTime<Utc>>,
    last_user_input: Option<DateTime<Utc>>,
    project_dir: Option<String>,
    project_id: Option<String>,
    parent_id: Option<String>,
    agent_name: Option<String>,
}

fn open_ro(db: &Path) -> rusqlite::Result<Connection> {
    let connection = Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(2))?;
    Ok(connection)
}

fn has_table(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "select 1 from sqlite_master where type = 'table' and name = ?1",
            [name],
            |_| Ok(()),
        )
        .optional()
        .ok()
        .flatten()
        .is_some()
}

fn has_column(connection: &Connection, table: &str, column: &str) -> bool {
    let Ok(mut statement) = connection.prepare(&format!("pragma table_info({table})")) else {
        return false;
    };
    statement
        .query_map([], |row| row.get::<_, String>(1))
        .map(|names| names.flatten().any(|name| name == column))
        .unwrap_or(false)
}

fn read_summaries(
    db: &Path,
    result: &mut ScanResult,
) -> (HashMap<String, SummaryRow>, HashMap<String, bool>) {
    let mut summaries = HashMap::new();
    let mut fts_archived = HashMap::new();
    if !db.exists() {
        return (summaries, fts_archived);
    }
    let connection = match open_ro(db) {
        Ok(connection) => connection,
        Err(error) => {
            result.record_error(format!("{}: {error}", db.display()));
            return (summaries, fts_archived);
        }
    };

    if has_table(&connection, "conversation_summaries") {
        let col = |name: &'static str, fallback: &'static str| -> &'static str {
            if has_column(&connection, "conversation_summaries", name) {
                name
            } else {
                fallback
            }
        };
        let query = format!(
            "select conversation_id, {}, {}, {}, {}, {}, {}, {}, {}, {} from conversation_summaries",
            col("title", "''"),
            col("preview", "''"),
            col("step_count", "0"),
            col("last_modified_time", "''"),
            col("last_user_input_time", "''"),
            col("workspace_uris", "''"),
            col("project_id", "''"),
            col("parent_conversation_id", "''"),
            col("agent_name", "''"),
        );
        match connection.prepare(&query) {
            Ok(mut stmt) => {
                let rows = stmt.query_map([], |row| {
                    let id: String = row.get(0)?;
                    let title: String = row.get::<_, Option<String>>(1)?.unwrap_or_default();
                    let preview: String = row.get::<_, Option<String>>(2)?.unwrap_or_default();
                    let step_count: i64 = row.get::<_, Option<i64>>(3)?.unwrap_or(0);
                    let last_modified: String =
                        row.get::<_, Option<String>>(4)?.unwrap_or_default();
                    let last_user_input: String =
                        row.get::<_, Option<String>>(5)?.unwrap_or_default();
                    let workspace_uris: String =
                        row.get::<_, Option<String>>(6)?.unwrap_or_default();
                    let project_id: String = row.get::<_, Option<String>>(7)?.unwrap_or_default();
                    let parent_id: String = row.get::<_, Option<String>>(8)?.unwrap_or_default();
                    let agent_name: String = row.get::<_, Option<String>>(9)?.unwrap_or_default();
                    Ok((
                        id,
                        SummaryRow {
                            title: non_empty(title),
                            preview: non_empty(preview),
                            step_count,
                            last_modified: parse_timestamp(&last_modified),
                            last_user_input: parse_timestamp(&last_user_input),
                            project_dir: first_workspace_path(&workspace_uris),
                            project_id: non_empty(project_id),
                            parent_id: non_empty(parent_id),
                            agent_name: non_empty(agent_name),
                        },
                    ))
                });
                match rows {
                    Ok(rows) => {
                        for row in rows {
                            match row {
                                Ok((id, summary)) => {
                                    summaries.insert(id, summary);
                                }
                                Err(error) => {
                                    result.record_error(format!("{}: {error}", db.display()))
                                }
                            }
                        }
                    }
                    Err(error) => result.record_error(format!("{}: {error}", db.display())),
                }
            }
            Err(error) => result.record_error(format!("{}: {error}", db.display())),
        }
    }

    if has_table(&connection, "conversations_fts")
        && has_column(&connection, "conversations_fts", "archived")
        && let Ok(mut stmt) = connection.prepare(
            "select conversation_id, max(archived) from conversations_fts group by conversation_id",
        )
        && let Ok(rows) = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?))
        })
    {
        for (id, archived) in rows.flatten() {
            if let Some(archived) = archived {
                fts_archived.insert(id, archived != 0);
            }
        }
    }

    (summaries, fts_archived)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Parses RFC 3339 or Go-style `YYYY-MM-DD HH:MM:SS(.frac)+00:00`, ignoring Go's zero time.
fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.starts_with("0001-01-01") {
        return None;
    }
    if let Some(parsed) = jsonl::parse_timestamp(trimmed) {
        return Some(parsed);
    }
    let normalized = trimmed.replacen(' ', "T", 1);
    jsonl::parse_timestamp(&normalized)
}

fn first_workspace_path(workspace_uris: &str) -> Option<String> {
    let trimmed = workspace_uris.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(trimmed) {
        return items
            .iter()
            .filter_map(Value::as_str)
            .find_map(file_uri_path);
    }
    file_uri_path(trimmed)
}

fn file_uri_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);
    let windows_drive = decoded.len() > 2
        && decoded.starts_with('/')
        && decoded.as_bytes()[2] == b':'
        && decoded.as_bytes()[1].is_ascii_alphabetic();
    Some(if windows_drive {
        let mut path = decoded[1..].to_string();
        if let Some(drive) = path.get_mut(0..1) {
            drive.make_ascii_uppercase();
        }
        path
    } else {
        decoded
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16)
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Annotation {
    title: Option<String>,
    archived: Option<bool>,
    pinned: Option<bool>,
}

fn parse_annotation(text: &str) -> Annotation {
    Annotation {
        title: pbtxt_string(text, "title"),
        archived: pbtxt_bool(text, "archived"),
        pinned: pbtxt_bool(text, "pinned"),
    }
}

fn pbtxt_field_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut rest = text;
    while let Some(pos) = rest.find(key) {
        let before_ok = pos == 0
            || !rest.as_bytes()[pos - 1].is_ascii_alphanumeric()
                && rest.as_bytes()[pos - 1] != b'_';
        let after = &rest[pos + key.len()..];
        let trimmed = after.trim_start();
        if before_ok && let Some(value) = trimmed.strip_prefix(':') {
            return Some(value.trim_start());
        }
        rest = after;
    }
    None
}

fn pbtxt_bool(text: &str, key: &str) -> Option<bool> {
    let value = pbtxt_field_value(text, key)?;
    let token = value
        .split(|c: char| c.is_whitespace() || c == '}' || c == ',')
        .next()?;
    match token {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn pbtxt_string(text: &str, key: &str) -> Option<String> {
    let value = pbtxt_field_value(text, key)?;
    let mut chars = value.chars();
    if chars.next() != Some('"') {
        return None;
    }
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return (!out.trim().is_empty()).then_some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                '\\' => out.push('\\'),
                '"' => out.push('"'),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    None
}

#[derive(Clone, Default)]
struct TranscriptMeta {
    span: Span,
    models: Vec<String>,
    first_prompt: Option<String>,
    has_messages: bool,
}

fn parse_transcript_meta(path: &Path) -> io::Result<TranscriptMeta> {
    let mut meta = TranscriptMeta::default();
    jsonl::for_each_line(path, |line| {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            return;
        };
        meta.span
            .observe(entry["created_at"].as_str().and_then(parse_timestamp));
        if let Some(model) = entry["model"].as_str() {
            note_model(&mut meta.models, model);
        }
        let source = entry["source"].as_str().unwrap_or_default();
        let kind = entry["type"].as_str().unwrap_or_default();
        match (source, kind) {
            ("USER_EXPLICIT", "USER_INPUT") => {
                if let Some(content) = entry["content"].as_str() {
                    let prompt = extract_user_request(content);
                    if !prompt.trim().is_empty() {
                        meta.has_messages = true;
                        if meta.first_prompt.is_none() {
                            meta.first_prompt = Some(prompt.to_string());
                        }
                    }
                }
            }
            ("MODEL", "PLANNER_RESPONSE")
                if entry["content"]
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty()) =>
            {
                meta.has_messages = true;
            }
            _ => {}
        }
    })?;
    Ok(meta)
}

fn extract_user_request(content: &str) -> &str {
    if let Some(start) = content.find("<USER_REQUEST>") {
        let after = &content[start + "<USER_REQUEST>".len()..];
        if let Some(end) = after.find("</USER_REQUEST>") {
            return after[..end].trim();
        }
    }
    content.trim()
}

fn strip_step_timestamps(content: &str) -> &str {
    let mut rest = content.trim_start();
    loop {
        if let Some(next) = rest
            .strip_prefix("Created At:")
            .or_else(|| rest.strip_prefix("Completed At:"))
        {
            match next.find('\n') {
                Some(index) => rest = &next[index + 1..],
                None => return "",
            }
        } else {
            return rest.trim();
        }
    }
}

/// Unwraps JSON-encoded string values inside tool_calls args (Antigravity logs args as JSON strings).
fn clean_tool_args(args: &Value) -> Value {
    match args {
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    let cleaned = value
                        .as_str()
                        .and_then(|s| serde_json::from_str::<Value>(s).ok())
                        .unwrap_or_else(|| value.clone());
                    (key.clone(), cleaned)
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

fn read_jsonl_transcript(path: &Path) -> io::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    let mut pending_calls: VecDeque<String> = VecDeque::new();
    let mut next_call = 0usize;

    jsonl::for_each_line(path, |line| {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let ts = entry["created_at"].as_str().and_then(parse_timestamp);
        let source = entry["source"].as_str().unwrap_or_default();
        let kind = entry["type"].as_str().unwrap_or_default();

        match (source, kind) {
            ("USER_EXPLICIT", "USER_INPUT") => {
                pending_calls.clear();
                if let Some(content) = entry["content"].as_str() {
                    builder.text(Role::User, extract_user_request(content), ts);
                }
            }
            ("MODEL", "PLANNER_RESPONSE") => {
                if let Some(content) = entry["content"].as_str() {
                    builder.text(Role::Assistant, content, ts);
                }
                if let Some(calls) = entry["tool_calls"].as_array() {
                    for call in calls {
                        let name = call["name"].as_str().unwrap_or("tool");
                        let args = clean_tool_args(&call["args"]);
                        let input = tool_input_text(&args);
                        let id = format!("c{next_call}");
                        next_call += 1;
                        builder.tool_call(Some(&id), name, &input, ts);
                        pending_calls.push_back(id);
                    }
                }
            }
            ("MODEL", _) => {
                let raw = entry["content"]
                    .as_str()
                    .or_else(|| entry["error"].as_str())
                    .unwrap_or_default();
                let output = strip_step_timestamps(raw);
                let id = pending_calls.pop_front();
                if !output.is_empty() {
                    builder.tool_result(id.as_deref(), output, ts);
                }
            }
            ("SYSTEM", _) => {
                if let Some(text) = entry["content"]
                    .as_str()
                    .or_else(|| entry["error"].as_str())
                {
                    builder.text(Role::Context, text, ts);
                }
            }
            _ => {}
        }
    })?;

    Ok(builder.finish())
}

fn read_fts_transcript(db: &Path, id: &str) -> Transcript {
    let mut builder = TranscriptBuilder::default();
    let mut found = false;
    if let Ok(connection) = open_ro(db)
        && has_table(&connection, "conversations_fts")
        && let Ok(mut stmt) = connection.prepare(
            "select user_body, agent_body from conversations_fts \
             where conversation_id = ?1 and chunk_index >= 0 order by chunk_index",
        )
        && let Ok(rows) = stmt.query_map([id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            ))
        })
    {
        for (user_body, agent_body) in rows.flatten() {
            if !user_body.trim().is_empty() {
                builder.text(Role::User, user_body.trim(), None);
                found = true;
            }
            if !agent_body.trim().is_empty() {
                builder.text(Role::Assistant, agent_body.trim(), None);
                found = true;
            }
        }
    }
    let mut transcript = builder.finish();
    transcript.note = Some(if found {
        FTS_TRANSCRIPT_NOTE.into()
    } else {
        ENCRYPTED_PB_NOTE.into()
    });
    transcript
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use serde_json::json;

    use super::*;

    const SESSION_A: &str = "11111111-1111-4111-8111-111111111111";
    const SESSION_B: &str = "22222222-2222-4222-8222-222222222222";
    const SESSION_C: &str = "33333333-3333-4333-8333-333333333333";

    fn create_summaries_db(dir: &Path) -> Connection {
        let connection = Connection::open(dir.join("conversation_summaries.db")).unwrap();
        connection
            .execute_batch(
                "create table conversation_summaries (
                    conversation_id text primary key,
                    title text not null default '',
                    preview text not null default '',
                    step_count integer not null default 0,
                    last_modified_time datetime not null,
                    workspace_uris text not null,
                    status text not null default '',
                    project_id text not null default '',
                    agent_name text not null default '',
                    parent_conversation_id text not null default '',
                    last_user_input_time datetime not null default ''
                );
                create virtual table conversations_fts using fts5(
                    conversation_id unindexed,
                    chunk_index unindexed,
                    archived unindexed,
                    title,
                    user_body,
                    agent_body
                );",
            )
            .unwrap();
        connection
    }

    fn write_transcript(dir: &Path, id: &str, lines: &[Value]) {
        let logs = dir
            .join("brain")
            .join(id)
            .join(".system_generated")
            .join("logs");
        fs::create_dir_all(&logs).unwrap();
        let body = lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(logs.join("transcript.jsonl"), body).unwrap();
    }

    #[test]
    fn scans_db_and_pb_sessions_with_annotations_and_transcripts() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("antigravity");
        fs::create_dir_all(root.join("conversations")).unwrap();
        fs::create_dir_all(root.join("annotations")).unwrap();
        fs::write(
            root.join("conversations").join(format!("{SESSION_A}.db")),
            b"db",
        )
        .unwrap();
        fs::write(
            root.join("conversations").join(format!("{SESSION_B}.pb")),
            b"enc",
        )
        .unwrap();

        fs::write(
            root.join("annotations").join(format!("{SESSION_A}.pbtxt")),
            "title:\"Custom Title\" archived:true pinned:true last_user_view_time:{seconds:1}",
        )
        .unwrap();

        let db = create_summaries_db(&root);
        db.execute(
            "insert into conversation_summaries values (?1, '', 'Fallback preview', 4, \
             '2026-11-25 14:00:00.5+00:00', '[\"file:///Users/me/My%20App\"]', \
             'CASCADE_RUN_STATUS_IDLE', 'proj-1', '', '', '2026-11-25 13:59:00+00:00')",
            params![SESSION_A],
        )
        .unwrap();
        db.execute(
            "insert into conversation_summaries values (?1, '', 'Old pb chat', 2, \
             '2026-05-01 10:00:00+00:00', '', 'CASCADE_RUN_STATUS_IDLE', \
             'outside-of-project', '', '', '0001-01-01 00:00:00+00:00')",
            params![SESSION_B],
        )
        .unwrap();
        db.execute(
            "insert into conversations_fts values (?1, 0, 1, 'Old pb chat', 'How does X work?', 'It works like Y.')",
            params![SESSION_B],
        )
        .unwrap();

        write_transcript(
            &root,
            SESSION_A,
            &[
                json!({
                    "step_index": 0,
                    "source": "USER_EXPLICIT",
                    "type": "USER_INPUT",
                    "created_at": "2026-11-25T13:50:00Z",
                    "content": "<USER_REQUEST>\nFix the bug\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nnow\n</ADDITIONAL_METADATA>"
                }),
                json!({
                    "step_index": 1,
                    "source": "MODEL",
                    "type": "PLANNER_RESPONSE",
                    "created_at": "2026-11-25T13:51:00Z",
                    "model": "gemini-3-pro",
                    "content": "Checking the file.",
                    "tool_calls": [{
                        "name": "run_command",
                        "args": {"CommandLine": "\"cargo test\""}
                    }]
                }),
                json!({
                    "step_index": 2,
                    "source": "MODEL",
                    "type": "RUN_COMMAND",
                    "created_at": "2026-11-25T13:52:00Z",
                    "content": "Created At: 2026-11-25T13:52:00Z\nCompleted At: 2026-11-25T13:52:01Z\ntest result: ok."
                }),
                json!({
                    "step_index": 3,
                    "source": "SYSTEM",
                    "type": "SYSTEM_MESSAGE",
                    "created_at": "2026-11-25T13:52:02Z",
                    "content": "Reminder"
                }),
            ],
        );

        let adapter = AntigravityAdapter::new(vec![root]);
        let scan = adapter.scan();
        assert!(scan.complete && scan.errors.is_empty(), "{:?}", scan.errors);
        assert_eq!(scan.sessions.len(), 2);

        let a = scan.sessions.iter().find(|s| s.id == SESSION_A).unwrap();
        assert_eq!(a.title.as_deref(), Some("Custom Title"));
        assert_eq!(a.project_dir.as_deref(), Some("/Users/me/My App"));
        assert!(!a.projectless);
        assert!(a.archived);
        assert_eq!(a.pinned, Some(true));
        assert_eq!(a.models, vec!["gemini-3-pro"]);
        assert!(!a.empty);
        assert!(matches!(
            adapter.delete_plan(a),
            DeletePlan::ReadOnly { .. }
        ));
        assert_eq!(
            adapter
                .resume_target(a, ResumeMode::Terminal)
                .map(|target| match target {
                    ResumeTarget::Cli { args, .. } => args,
                    ResumeTarget::Url(_) => unreachable!(),
                }),
            Some(vec!["--conversation".into(), SESSION_A.into()])
        );
        assert!(adapter.resume_target(a, ResumeMode::Desktop).is_none());

        let transcript_a = adapter.transcript(a).unwrap();
        assert_eq!(transcript_a.note, None);
        assert_eq!(transcript_a.head.len(), 4);
        assert_eq!(transcript_a.head[0].role, Role::User);
        assert_eq!(transcript_a.head[0].text, "Fix the bug");
        assert_eq!(transcript_a.head[1].role, Role::Assistant);
        assert_eq!(transcript_a.head[1].text, "Checking the file.");
        assert_eq!(transcript_a.head[2].role, Role::Tool);
        let tool = transcript_a.head[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "run_command");
        assert!(transcript_a.head[2].text.contains("cargo test"));
        assert_eq!(tool.output.as_deref(), Some("test result: ok."));
        assert_eq!(transcript_a.head[3].role, Role::Context);

        let b = scan.sessions.iter().find(|s| s.id == SESSION_B).unwrap();
        assert_eq!(b.title.as_deref(), Some("Old pb chat"));
        assert_eq!(b.project_dir, None);
        assert!(b.projectless);
        assert!(b.archived);
        let transcript_b = adapter.transcript(b).unwrap();
        assert_eq!(transcript_b.note.as_deref(), Some(FTS_TRANSCRIPT_NOTE));
        assert_eq!(transcript_b.head.len(), 2);
        assert_eq!(transcript_b.head[0].text, "How does X work?");
        assert_eq!(transcript_b.head[1].text, "It works like Y.");
    }

    #[test]
    fn skips_migrated_ide_pb_files_and_deduplicates_across_dirs() {
        let temp = tempfile::tempdir().unwrap();
        let desktop = temp.path().join("antigravity");
        let ide = temp.path().join("antigravity-ide");
        fs::create_dir_all(desktop.join("conversations")).unwrap();
        fs::create_dir_all(ide.join("conversations")).unwrap();

        fs::write(
            desktop
                .join("conversations")
                .join(format!("{SESSION_A}.db")),
            b"db",
        )
        .unwrap();
        fs::write(
            ide.join("conversations").join(format!("{SESSION_A}.pb")),
            b"pb",
        )
        .unwrap();
        fs::write(
            ide.join("conversations").join(format!(".{SESSION_A}.done")),
            b"",
        )
        .unwrap();
        fs::write(
            ide.join("conversations").join(format!("{SESSION_C}.pb")),
            b"pb",
        )
        .unwrap();
        fs::write(
            ide.join("conversations").join(format!(".{SESSION_C}.done")),
            b"",
        )
        .unwrap();

        let adapter = AntigravityAdapter::new(vec![desktop, ide]);
        let scan = adapter.scan();
        assert!(scan.complete);
        let ids: Vec<&str> = scan.sessions.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec![SESSION_A]);
    }

    #[test]
    fn parses_windows_and_posix_workspace_uris() {
        assert_eq!(
            first_workspace_path(r#"["file:///c%3A/Users/me/My%20Repo"]"#).as_deref(),
            Some("C:/Users/me/My Repo")
        );
        assert_eq!(
            first_workspace_path(r#"["file:///C:/Users/me/Repo"]"#).as_deref(),
            Some("C:/Users/me/Repo")
        );
        assert_eq!(
            first_workspace_path(r#"["file:///Users/me/Repo"]"#).as_deref(),
            Some("/Users/me/Repo")
        );
        assert_eq!(first_workspace_path(""), None);
    }

    #[test]
    fn parses_pbtxt_annotations_with_escapes() {
        let ann = parse_annotation(
            r#"title:"Fix \"quoted\" \\ path" archived:true pinned:false last_user_view_time:{seconds:1}"#,
        );
        assert_eq!(
            ann,
            Annotation {
                title: Some(r#"Fix "quoted" \ path"#.into()),
                archived: Some(true),
                pinned: Some(false),
            }
        );
    }

    #[test]
    fn missing_directories_produce_empty_complete_scan() {
        let temp = tempfile::tempdir().unwrap();
        let adapter = AntigravityAdapter::new(vec![temp.path().join("missing")]);
        let scan = adapter.scan();
        assert!(scan.complete && scan.errors.is_empty() && scan.sessions.is_empty());
    }
}
