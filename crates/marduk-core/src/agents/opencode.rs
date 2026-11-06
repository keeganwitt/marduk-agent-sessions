use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::cli::{Cli, CliOverrides};
use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, AgentCommand, DeletePlan, ScanResult};
use crate::jsonl;
use crate::session::{Fingerprint, Session};
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// OpenCode keeps every session in one SQLite database. Newer versions use `session_v2` /
/// `session_message` / `session_part`; older ones `session` / `message` / `part`, which
/// OpenCode migrates forward. Marduk only ever opens the database read-only.
pub struct OpenCodeAdapter {
    data_dir: PathBuf,
    explicit_db: Option<PathBuf>,
    cli: Cli,
}

const PROGRAM: &str = "opencode";
const INSTALL_URL: &str = "https://opencode.ai/docs/";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layout {
    V2,
    V1,
}

struct Row {
    id: String,
    parent_id: Option<String>,
    directory: Option<String>,
    title: Option<String>,
    created_ms: i64,
    updated_ms: i64,
    archived: bool,
}

type LayoutRows = (Vec<Row>, HashMap<String, Totals>);

#[derive(Default, Clone, Copy)]
struct Totals {
    messages: u64,
    bytes: u64,
    updated_ms: i64,
}

fn millis_to_system_time(millis: i64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(millis.max(0) as u64)
}

fn millis_to_datetime(millis: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp_millis(millis)
}

fn open(db: &Path) -> rusqlite::Result<Connection> {
    let connection =
        Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
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

/// OpenCode ids look like `ses_01J…`; anything else is refused before it reaches the CLI.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty() && !id.starts_with('-') && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl OpenCodeAdapter {
    pub fn new(data_dir: PathBuf, explicit_db: Option<PathBuf>, cli: Option<PathBuf>) -> Self {
        Self {
            data_dir,
            explicit_db,
            cli: Cli::fixed(PROGRAM, INSTALL_URL, cli),
        }
    }

    pub fn from_env(overrides: std::sync::Arc<CliOverrides>) -> Self {
        let data_dir = env_dir("XDG_DATA_HOME")
            .unwrap_or_else(|| home_dir().join(".local").join("share"))
            .join("opencode");
        let explicit_db = env_dir("OPENCODE_DB").map(|db| if db.is_absolute() { db } else { data_dir.join(db) });
        Self {
            data_dir,
            explicit_db,
            cli: Cli::detect("opencode", PROGRAM, INSTALL_URL, overrides),
        }
    }

    /// `opencode.db`, plus `opencode-<channel>.db` for non-release channels.
    fn databases(&self) -> Vec<PathBuf> {
        let mut databases: Vec<PathBuf> = std::fs::read_dir(&self.data_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("opencode") && name.ends_with(".db"))
            })
            .collect();
        if let Some(db) = &self.explicit_db
            && !databases.contains(db)
        {
            databases.push(db.clone());
        }
        databases.sort();
        databases
    }

    fn scan_database(&self, db: &Path, result: &mut ScanResult) {
        let connection = match open(db) {
            Ok(connection) => connection,
            Err(error) => {
                result.record_error(format!("{}: {error}", db.display()));
                return;
            }
        };
        if let Err(error) = connection.query_row("select count(*) from sqlite_master", [], |row| row.get::<_, i64>(0)) {
            result.record_error(format!("{}: {error}", db.display()));
            return;
        }
        let mut seen = HashSet::new();
        let mut recognized = false;
        for layout in [Layout::V2, Layout::V1] {
            match read_layout(&connection, layout) {
                Ok(None) => {}
                Ok(Some((rows, totals))) => {
                    recognized = true;
                    for row in rows {
                        if !seen.insert(row.id.clone()) {
                            continue;
                        }
                        let totals = totals.get(&row.id).copied().unwrap_or_default();
                        let first_prompt = if row.title.as_deref().is_none_or(|title| title.trim().is_empty()) {
                            first_user_text(&connection, layout, &row.id)
                        } else {
                            None
                        };
                        result.sessions.push(session_from_row(db, row, totals, first_prompt));
                    }
                }
                Err(error) => result.record_error(format!("{}: {error}", db.display())),
            }
        }
        if !recognized {
            result.record_error(format!(
                "{}: no OpenCode session tables Marduk recognizes; this OpenCode version may be newer than Marduk supports.",
                db.display()
            ));
        }
    }

    fn delete_command(&self, session: &Session) -> Result<AgentCommand, String> {
        if !is_safe_id(&session.id) {
            return Err("This OpenCode session id can't be passed to the OpenCode CLI safely.".to_string());
        }
        let program = self
            .cli
            .path()
            .ok_or_else(|| self.cli.missing_reason("OpenCode", "delete OpenCode sessions"))?;
        Ok(AgentCommand {
            program,
            args: vec!["session".into(), "delete".into(), session.id.clone()],
        })
    }
}

fn read_layout(connection: &Connection, layout: Layout) -> rusqlite::Result<Option<LayoutRows>> {
    let (sessions, messages) = match layout {
        Layout::V2 => ("session_v2", "session_message"),
        Layout::V1 => ("session", "message"),
    };
    if !has_table(connection, sessions) || !has_table(connection, messages) {
        return Ok(None);
    }
    let archived = if has_column(connection, sessions, "time_archived") {
        "time_archived"
    } else {
        "null"
    };
    let mut statement = connection.prepare(&format!(
        "select id, parent_id, directory, title, time_created, time_updated, {archived} from {sessions}"
    ))?;
    let rows = statement
        .query_map([], |row| {
            Ok(Row {
                id: row.get(0)?,
                parent_id: row.get(1)?,
                directory: row.get(2)?,
                title: row.get(3)?,
                created_ms: row.get(4)?,
                updated_ms: row.get(5)?,
                archived: row.get::<_, Option<i64>>(6)?.is_some(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Some((rows, totals(connection, layout)?)))
}

fn totals(connection: &Connection, layout: Layout) -> rusqlite::Result<HashMap<String, Totals>> {
    let (messages, parts) = match layout {
        Layout::V2 => ("session_message", "session_part"),
        Layout::V1 => ("message", "part"),
    };
    let mut totals: HashMap<String, Totals> = HashMap::new();
    let mut statement = connection.prepare(&format!(
        "select session_id, count(*), coalesce(sum(length(data)), 0), coalesce(max(time_updated), 0) from {messages} group by session_id"
    ))?;
    for row in statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })? {
        let (id, count, bytes, updated) = row?;
        totals.insert(
            id,
            Totals {
                messages: count as u64,
                bytes: bytes as u64,
                updated_ms: updated,
            },
        );
    }
    if has_table(connection, parts) {
        let mut statement = connection.prepare(&format!(
            "select session_id, coalesce(sum(length(data)), 0), coalesce(max(time_updated), 0) from {parts} group by session_id"
        ))?;
        for row in statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
        })? {
            let (id, bytes, updated) = row?;
            let entry = totals.entry(id).or_default();
            entry.bytes += bytes as u64;
            entry.updated_ms = entry.updated_ms.max(updated);
        }
    }
    Ok(totals)
}

fn fingerprint_of(row_updated_ms: i64, totals: Totals) -> Fingerprint {
    Fingerprint {
        len: totals.messages,
        modified: millis_to_system_time(row_updated_ms.max(totals.updated_ms)),
    }
}

fn session_from_row(db: &Path, row: Row, totals: Totals, first_prompt: Option<String>) -> Session {
    let fingerprint = fingerprint_of(row.updated_ms, totals);
    let title = [row.title, first_prompt]
        .into_iter()
        .flatten()
        .find_map(|text| jsonl::to_title(&text));
    Session {
        agent: "opencode",
        id: row.id,
        project_dir: row.directory.filter(|dir| !dir.is_empty()),
        title,
        created: millis_to_datetime(row.created_ms),
        last_activity: fingerprint.modified.into(),
        size_bytes: totals.bytes,
        archived: row.archived,
        primary_file: db.to_path_buf(),
        files: Vec::new(),
        fingerprint,
        parent_id: row.parent_id,
    }
}

fn first_user_text(connection: &Connection, layout: Layout, session_id: &str) -> Option<String> {
    let sql = match layout {
        Layout::V2 => "select data from session_message where session_id = ?1 and type = 'user' order by seq limit 1",
        Layout::V1 => {
            "select p.data from part p join message m on m.id = p.message_id \
             where m.session_id = ?1 and json_extract(m.data, '$.role') = 'user' \
             and json_extract(p.data, '$.type') = 'text' order by m.time_created, m.id, p.id limit 1"
        }
    };
    let data: String = connection.query_row(sql, [session_id], |row| row.get(0)).ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value["text"].as_str().map(str::to_string)
}

fn current_state(connection: &Connection, session_id: &str) -> rusqlite::Result<Option<(Layout, i64, Totals)>> {
    for layout in [Layout::V2, Layout::V1] {
        let (sessions, messages, parts) = match layout {
            Layout::V2 => ("session_v2", "session_message", "session_part"),
            Layout::V1 => ("session", "message", "part"),
        };
        if !has_table(connection, sessions) {
            continue;
        }
        let updated: Option<i64> = connection
            .query_row(
                &format!("select time_updated from {sessions} where id = ?1"),
                [session_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(updated) = updated else {
            continue;
        };
        let (count, message_updated): (i64, i64) = connection.query_row(
            &format!("select count(*), coalesce(max(time_updated), 0) from {messages} where session_id = ?1"),
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let part_updated: i64 = if has_table(connection, parts) {
            connection.query_row(
                &format!("select coalesce(max(time_updated), 0) from {parts} where session_id = ?1"),
                [session_id],
                |row| row.get(0),
            )?
        } else {
            0
        };
        let totals = Totals {
            messages: count as u64,
            bytes: 0,
            updated_ms: message_updated.max(part_updated),
        };
        return Ok(Some((layout, updated, totals)));
    }
    Ok(None)
}

impl AgentAdapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn display_name(&self) -> &'static str {
        "OpenCode"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.data_dir.clone()]
    }

    fn scan(&self) -> ScanResult {
        self.cli.refresh();
        let mut result = ScanResult::empty_complete();
        for db in self.databases() {
            if db.is_file() {
                self.scan_database(&db, &mut result);
            }
        }
        result
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        match self.delete_command(session) {
            Ok(command) => DeletePlan::AgentCommand {
                command,
                files: Vec::new(),
                warning: child_warning(&session.primary_file, &session.id),
            },
            Err(reason) => DeletePlan::ReadOnly { reason },
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        let direct = open(&session.primary_file)
            .and_then(|connection| read_transcript(&connection, &session.id))
            .map_err(|error| error.to_string());
        let recognized = match &direct {
            Ok(transcript) => !transcript.head.is_empty() || session.fingerprint.len == 0,
            Err(_) => false,
        };
        if recognized {
            return direct.map_err(std::io::Error::other);
        }
        let fallback = match self.cli.path() {
            Some(cli) if is_safe_id(&session.id) => export_transcript(&cli, &session.id),
            Some(_) => Err("This session id can't be passed to the OpenCode CLI safely.".to_string()),
            None => Err(self
                .cli
                .missing_reason("OpenCode", "read sessions it can't read directly")),
        };
        match (direct, fallback) {
            (_, Ok(transcript)) => Ok(transcript),
            (Ok(mut transcript), Err(reason)) => {
                transcript.note = Some(format!(
                    "Marduk didn't recognize how this session's messages are stored. {reason}"
                ));
                Ok(transcript)
            }
            (Err(direct), Err(reason)) => Err(std::io::Error::other(format!("{direct}. {reason}"))),
        }
    }

    fn fingerprint(&self, session: &Session) -> std::io::Result<Fingerprint> {
        let connection = open(&session.primary_file).map_err(std::io::Error::other)?;
        match current_state(&connection, &session.id).map_err(std::io::Error::other)? {
            Some((_, updated, totals)) => Ok(fingerprint_of(updated, totals)),
            None => Err(std::io::Error::new(std::io::ErrorKind::NotFound, "session row is gone")),
        }
    }

    fn cli(&self) -> Option<&Cli> {
        Some(&self.cli)
    }

    fn still_present(&self, session: &Session) -> bool {
        // If the database can't be read, assume the session is still there so a delete
        // is never reported as done without proof.
        open(&session.primary_file)
            .and_then(|connection| current_state(&connection, &session.id))
            .map_or(true, |state| state.is_some())
    }
}

/// `opencode session delete` also deletes the session's child (subagent) sessions.
fn child_warning(db: &Path, session_id: &str) -> Option<String> {
    let connection = open(db).ok()?;
    let mut children = 0i64;
    for table in ["session_v2", "session"] {
        if has_table(&connection, table) {
            children += connection
                .query_row(
                    &format!("select count(*) from {table} where parent_id = ?1"),
                    [session_id],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap_or(0);
        }
    }
    (children > 0).then(|| {
        format!(
            "OpenCode also deletes this session's {children} child {}.",
            if children == 1 { "session" } else { "sessions" }
        )
    })
}

fn read_transcript(connection: &Connection, session_id: &str) -> rusqlite::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    match current_state(connection, session_id)?.map(|(layout, _, _)| layout) {
        Some(Layout::V2) => read_v2(connection, session_id, &mut builder)?,
        Some(Layout::V1) => read_v1(connection, session_id, &mut builder)?,
        None => {}
    }
    Ok(builder.finish())
}

fn parse(data: &str) -> Value {
    serde_json::from_str(data).unwrap_or(Value::Null)
}

fn read_v2(connection: &Connection, session_id: &str, builder: &mut TranscriptBuilder) -> rusqlite::Result<()> {
    let mut parts: HashMap<String, Vec<Value>> = HashMap::new();
    if has_table(connection, "session_part") {
        let mut statement = connection.prepare(
            "select message_id, type, data from session_part where session_id = ?1 order by message_id, ordinal",
        )?;
        for row in statement.query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })? {
            let (message_id, kind, data) = row?;
            let mut part = parse(&data);
            if let Value::Object(fields) = &mut part {
                fields.entry("type").or_insert(Value::String(kind));
            }
            parts.entry(message_id).or_default().push(part);
        }
    }
    let mut statement = connection
        .prepare("select id, type, time_created, data from session_message where session_id = ?1 order by seq")?;
    for row in statement.query_map([session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })? {
        let (id, kind, created, data) = row?;
        let content = parts.remove(&id).unwrap_or_default();
        append_v2_message(builder, &id, &kind, created, &parse(&data), content);
    }
    Ok(())
}

/// One `session_message` row, or one message of `opencode session export` (same shape).
/// `parts` are the assistant's `session_part` rows, which come before any inline content.
fn append_v2_message(
    builder: &mut TranscriptBuilder,
    id: &str,
    kind: &str,
    created_ms: i64,
    data: &Value,
    parts: Vec<Value>,
) {
    let timestamp = millis_to_datetime(created_ms);
    let text = data["text"].as_str().unwrap_or_default();
    match kind {
        "user" => builder.text(Role::User, text, timestamp),
        "synthetic" | "system" | "skill" => builder.text(Role::Context, text, timestamp),
        "shell" => {
            builder.tool_call(
                Some(id),
                "shell",
                data["command"].as_str().unwrap_or_default(),
                timestamp,
            );
            if let Some(output) = data["output"]["output"].as_str() {
                builder.tool_result(Some(id), output, timestamp);
            }
        }
        "compaction" => builder.text(Role::Context, data["summary"].as_str().unwrap_or_default(), timestamp),
        "assistant" => {
            let inline = data["content"].as_array().cloned().unwrap_or_default();
            for block in parts.into_iter().chain(inline) {
                append_v2_block(builder, &block, timestamp);
            }
        }
        _ => {}
    }
}

const EXPORT_TIMEOUT: Duration = Duration::from_secs(60);

/// Reads a session through `opencode session export`, which returns OpenCode's own decoded
/// `{ info, messages }` and so keeps working when its tables change shape.
fn export_transcript(cli: &Path, session_id: &str) -> Result<Transcript, String> {
    let output = super::cli::run_capture(cli, &["session", "export", session_id], EXPORT_TIMEOUT)?;
    let export: Value = serde_json::from_str(&output).map_err(|error| format!("Unexpected export output: {error}"))?;
    let messages = export["messages"]
        .as_array()
        .ok_or("The export had no messages list.")?;
    let mut builder = TranscriptBuilder::default();
    for message in messages {
        append_v2_message(
            &mut builder,
            message["id"].as_str().unwrap_or_default(),
            message["type"].as_str().unwrap_or_default(),
            message["time"]["created"].as_i64().unwrap_or_default(),
            message,
            Vec::new(),
        );
    }
    let mut transcript = builder.finish();
    transcript.note = Some(
        "Read through `opencode session export`, because Marduk didn't recognize how this session is stored."
            .to_string(),
    );
    Ok(transcript)
}

fn append_v2_block(builder: &mut TranscriptBuilder, block: &Value, timestamp: Option<DateTime<Utc>>) {
    match block["type"].as_str() {
        Some("text") => builder.text(Role::Assistant, block["text"].as_str().unwrap_or_default(), timestamp),
        Some("tool") => {
            let id = block["id"].as_str();
            let state = &block["state"];
            builder.tool_call(
                id,
                block["name"].as_str().unwrap_or("tool"),
                &transcript::tool_input_text(&state["input"]),
                timestamp,
            );
            match state["status"].as_str() {
                Some("completed") => {
                    builder.tool_result(id, &transcript::tool_output_text(&state["content"]), timestamp)
                }
                Some("error") => builder.tool_result(
                    id,
                    state["error"]["message"].as_str().unwrap_or("The tool failed."),
                    timestamp,
                ),
                _ => {}
            }
        }
        _ => {}
    }
}

fn read_v1(connection: &Connection, session_id: &str, builder: &mut TranscriptBuilder) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "select m.data, p.data, coalesce(p.time_created, m.time_created) from part p join message m on m.id = p.message_id \
         where m.session_id = ?1 order by m.time_created, m.id, p.id",
    )?;
    for row in statement.query_map([session_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (message, part, created) = row?;
        let (message, part) = (parse(&message), parse(&part));
        let timestamp = millis_to_datetime(created);
        let user = message["role"] == "user";
        match part["type"].as_str() {
            Some("text") if part["ignored"] == true => {}
            Some("text") => {
                let role = match (user, part["synthetic"] == true) {
                    (_, true) => Role::Context,
                    (true, false) => Role::User,
                    (false, false) => Role::Assistant,
                };
                builder.text(role, part["text"].as_str().unwrap_or_default(), timestamp);
            }
            Some("tool") => {
                let id = part["callID"].as_str();
                let state = &part["state"];
                builder.tool_call(
                    id,
                    part["tool"].as_str().unwrap_or("tool"),
                    &transcript::tool_input_text(&state["input"]),
                    timestamp,
                );
                let output = match state["status"].as_str() {
                    Some("error") => state["error"].clone(),
                    _ => state["output"].clone(),
                };
                if !output.is_null() {
                    builder.tool_result(id, &transcript::tool_output_text(&output), timestamp);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v2_db(dir: &Path) -> (PathBuf, Connection) {
        let path = dir.join("opencode.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "create table session_v2 (id text primary key, parent_id text, directory text not null, title text,
                   time_created integer not null, time_updated integer not null, time_archived integer);
                 create table session_message (id text primary key, session_id text not null, type text not null,
                   seq integer not null, time_created integer not null, time_updated integer not null, data text not null);
                 create table session_part (message_id text not null, session_id text not null, ordinal integer not null,
                   type text not null, call_id text, time_created integer not null, time_updated integer not null,
                   data text not null);",
            )
            .unwrap();
        (path, connection)
    }

    fn insert_session(connection: &Connection, id: &str, parent: Option<&str>, title: Option<&str>, archived: bool) {
        connection
            .execute(
                "insert into session_v2 values (?1, ?2, '/work/app', ?3, 1762400000000, 1762400100000, ?4)",
                rusqlite::params![id, parent, title, archived.then_some(1762400200000i64)],
            )
            .unwrap();
    }

    fn insert_message(connection: &Connection, session: &str, id: &str, seq: i64, kind: &str, data: &str) {
        connection
            .execute(
                "insert into session_message values (?1, ?2, ?3, ?4, 1762400000000 + ?4, 1762400000000 + ?4, ?5)",
                rusqlite::params![id, session, kind, seq, data],
            )
            .unwrap();
    }

    fn adapter(dir: &Path, cli: Option<&str>) -> OpenCodeAdapter {
        OpenCodeAdapter::new(dir.to_path_buf(), None, cli.map(PathBuf::from))
    }

    #[test]
    fn scans_v2_sessions_with_titles_archive_state_and_fingerprints() {
        let dir = tempfile::tempdir().unwrap();
        let (db, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_a", None, Some("Refactor auth"), false);
        insert_session(&connection, "ses_b", None, None, true);
        insert_message(&connection, "ses_b", "msg_1", 1, "user", r#"{"text":"Why is CI red?"}"#);
        insert_message(&connection, "ses_b", "msg_2", 2, "assistant", r#"{"content":[]}"#);

        let result = adapter(dir.path(), None).scan();

        assert!(result.complete, "{:?}", result.errors);
        let by_id = |id: &str| result.sessions.iter().find(|session| session.id == id).unwrap();
        assert_eq!(by_id("ses_a").title.as_deref(), Some("Refactor auth"));
        assert!(!by_id("ses_a").archived);
        let b = by_id("ses_b");
        assert_eq!(b.title.as_deref(), Some("Why is CI red?"));
        assert!(b.archived);
        assert_eq!(b.project_dir.as_deref(), Some("/work/app"));
        assert_eq!(b.primary_file, db);
        assert!(b.files.is_empty());
        assert_eq!(b.fingerprint.len, 2);
        assert_eq!(b.created.unwrap().timestamp_millis(), 1762400000000);
        assert!(b.size_bytes > 0);
    }

    #[test]
    fn missing_data_dir_is_complete_and_empty() {
        let dir = tempfile::tempdir().unwrap();
        let result = adapter(&dir.path().join("absent"), None).scan();
        assert!(result.complete);
        assert!(result.sessions.is_empty());
    }

    #[test]
    fn unreadable_database_makes_the_scan_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("opencode.db"), "not sqlite").unwrap();
        let result = adapter(dir.path(), None).scan();
        assert!(!result.complete);
    }

    #[test]
    fn unknown_schema_makes_the_scan_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        Connection::open(dir.path().join("opencode.db"))
            .unwrap()
            .execute_batch("create table something_else (id text)")
            .unwrap();
        let result = adapter(dir.path(), None).scan();
        assert!(!result.complete);
        assert!(result.errors[0].contains("no OpenCode session tables"));
    }

    #[test]
    fn v2_transcript_merges_parts_and_inline_content_with_tool_output() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_a", None, Some("t"), false);
        insert_message(&connection, "ses_a", "msg_1", 1, "system", r#"{"text":"env info"}"#);
        insert_message(&connection, "ses_a", "msg_2", 2, "user", r#"{"text":"List files"}"#);
        insert_message(
            &connection,
            "ses_a",
            "msg_3",
            3,
            "assistant",
            r#"{"content":[{"type":"text","text":"Done."}]}"#,
        );
        connection
            .execute(
                "insert into session_part values ('msg_3', 'ses_a', 0, 'tool', 'c1', 1, 1, ?1)",
                [r#"{"id":"c1","name":"bash","state":{"status":"completed","input":{"cmd":"ls"},"content":[{"type":"text","text":"a.txt"}]}}"#],
            )
            .unwrap();
        connection
            .execute(
                "insert into session_part values ('msg_3', 'ses_a', 1, 'reasoning', null, 1, 1, ?1)",
                [r#"{"text":"thinking"}"#],
            )
            .unwrap();
        let opencode = adapter(dir.path(), None);
        let session = opencode.scan().sessions.remove(0);

        let messages = opencode.transcript(&session).unwrap().head;

        let roles: Vec<Role> = messages.iter().map(|message| message.role).collect();
        assert_eq!(roles, vec![Role::Context, Role::User, Role::Tool, Role::Assistant]);
        assert_eq!(messages[2].tool.as_ref().unwrap().output.as_deref(), Some("a.txt"));
        assert_eq!(messages[3].text, "Done.");
    }

    #[test]
    fn v1_sessions_are_read_and_v2_wins_for_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        connection
            .execute_batch(
                "create table session (id text primary key, parent_id text, directory text not null, title text not null,
                   time_created integer not null, time_updated integer not null);
                 create table message (id text primary key, session_id text not null, time_created integer not null,
                   time_updated integer not null, data text not null);
                 create table part (id text primary key, message_id text not null, session_id text not null,
                   time_created integer not null, time_updated integer not null, data text not null);
                 insert into session values ('ses_old', null, '/legacy', 'Legacy work', 1, 2);
                 insert into session values ('ses_dup', null, '/legacy', 'Old copy', 1, 2);
                 insert into message values ('m1', 'ses_old', 1, 1, '{\"role\":\"user\"}');
                 insert into part values ('p1', 'm1', 'ses_old', 1, 1, '{\"type\":\"text\",\"text\":\"hello\"}');
                 insert into message values ('m2', 'ses_old', 2, 2, '{\"role\":\"assistant\"}');
                 insert into part values ('p2', 'm2', 'ses_old', 2, 2,
                   '{\"type\":\"tool\",\"tool\":\"read\",\"callID\":\"c\",\"state\":{\"status\":\"completed\",\"input\":{},\"output\":\"file body\"}}');",
            )
            .unwrap();
        insert_session(&connection, "ses_dup", None, Some("New copy"), false);
        let opencode = adapter(dir.path(), None);

        let sessions = opencode.scan().sessions;

        assert_eq!(sessions.len(), 2);
        assert_eq!(
            sessions.iter().find(|s| s.id == "ses_dup").unwrap().title.as_deref(),
            Some("New copy")
        );
        let old = sessions.iter().find(|s| s.id == "ses_old").unwrap();
        assert!(!old.archived);
        let messages = opencode.transcript(old).unwrap().head;
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[1].tool.as_ref().unwrap().output.as_deref(), Some("file body"));
    }

    #[cfg(unix)]
    fn fake_cli(dir: &Path, export: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let export_file = dir.join("export.json");
        std::fs::write(&export_file, export).unwrap();
        let script = dir.join("opencode");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n[ \"$1 $2 $3\" = \"session export ses_new\" ] || exit 9\ncat '{}'\n",
                export_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        script
    }

    #[cfg(unix)]
    #[test]
    fn falls_back_to_cli_export_when_messages_are_unrecognized() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_new", None, Some("t"), false);
        insert_message(&connection, "ses_new", "msg_1", 1, "from-the-future", r#"{"blob":1}"#);
        let cli = fake_cli(
            dir.path(),
            r#"{"info":{"id":"ses_new"},"messages":[
                {"id":"msg_1","type":"user","text":"hello from export","time":{"created":1762400000000}},
                {"id":"msg_2","type":"assistant","time":{"created":1762400001000},
                 "content":[{"type":"tool","id":"c1","name":"read","state":{"status":"completed","input":{},
                   "content":[{"type":"text","text":"file body"}]}},{"type":"text","text":"Done"}]}]}"#,
        );
        let opencode = OpenCodeAdapter::new(dir.path().to_path_buf(), None, Some(cli));
        let session = opencode.scan().sessions.remove(0);

        let transcript = opencode.transcript(&session).unwrap();

        assert_eq!(transcript.head[0].text, "hello from export");
        assert_eq!(
            transcript.head[1].tool.as_ref().unwrap().output.as_deref(),
            Some("file body")
        );
        assert_eq!(transcript.head[2].text, "Done");
        assert!(transcript.note.unwrap().contains("opencode session export"));
    }

    #[test]
    fn unrecognized_messages_without_cli_explain_why() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_new", None, Some("t"), false);
        insert_message(&connection, "ses_new", "msg_1", 1, "from-the-future", r#"{"blob":1}"#);
        let opencode = adapter(dir.path(), None);
        let session = opencode.scan().sessions.remove(0);

        let transcript = opencode.transcript(&session).unwrap();

        assert!(transcript.head.is_empty());
        assert!(transcript.note.unwrap().contains("Install the OpenCode CLI"));
    }

    #[test]
    fn empty_sessions_do_not_trigger_the_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_empty", None, Some("t"), false);
        let opencode = adapter(dir.path(), Some("/nonexistent/opencode"));
        let session = opencode.scan().sessions.remove(0);

        let transcript = opencode.transcript(&session).unwrap();

        assert!(transcript.note.is_none());
    }

    #[test]
    fn delete_uses_the_cli_warns_about_children_and_tracks_the_row() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_parent", None, Some("p"), false);
        insert_session(&connection, "ses_child", Some("ses_parent"), Some("c"), false);
        let opencode = adapter(dir.path(), Some("/bin/opencode"));
        let session = opencode
            .scan()
            .sessions
            .into_iter()
            .find(|s| s.id == "ses_parent")
            .unwrap();

        let DeletePlan::AgentCommand {
            command,
            files,
            warning,
        } = opencode.delete_plan(&session)
        else {
            panic!("expected an agent command");
        };
        assert_eq!(command.args, vec!["session", "delete", "ses_parent"]);
        assert!(files.is_empty());
        assert_eq!(
            warning.as_deref(),
            Some("OpenCode also deletes this session's 1 child session.")
        );
        assert_eq!(opencode.fingerprint(&session).unwrap(), session.fingerprint);
        assert!(opencode.still_present(&session));

        connection
            .execute("delete from session_v2 where id = 'ses_parent'", [])
            .unwrap();
        assert!(!opencode.still_present(&session));
        assert!(opencode.fingerprint(&session).is_err());
    }

    #[test]
    fn fingerprint_changes_when_a_message_is_added() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_a", None, Some("t"), false);
        let opencode = adapter(dir.path(), None);
        let session = opencode.scan().sessions.remove(0);

        insert_message(&connection, "ses_a", "msg_9", 9, "user", r#"{"text":"more"}"#);

        assert_ne!(opencode.fingerprint(&session).unwrap(), session.fingerprint);
    }

    #[test]
    fn read_only_without_cli_or_with_unsafe_id() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_a", None, Some("t"), false);
        let without_cli = adapter(dir.path(), None);
        let mut session = without_cli.scan().sessions.remove(0);
        assert!(
            matches!(without_cli.delete_plan(&session), DeletePlan::ReadOnly { reason } if reason.contains("Install"))
        );

        session.id = "--help".into();
        let with_cli = adapter(dir.path(), Some("/bin/opencode"));
        assert!(matches!(with_cli.delete_plan(&session), DeletePlan::ReadOnly { .. }));
        assert!(with_cli.archive_command(&session).is_none());
    }
}
