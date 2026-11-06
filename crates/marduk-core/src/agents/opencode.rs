use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use super::cli::{Cli, CliOverrides};
use super::{env_dir, home_dir};
use crate::adapter::{AgentAdapter, AgentCommand, DeletePlan, ScanProgress, ScanResult};
use crate::jsonl;
use crate::session::{Fingerprint, Session};
use crate::transcript::{self, Role, Transcript, TranscriptBuilder};

/// What differs between OpenCode and its forks (Kilo); storage and CLI are otherwise shared.
pub struct Flavor {
    pub id: &'static str,
    pub name: &'static str,
    /// Folder under `$XDG_DATA_HOME`.
    data_dir: &'static str,
    /// Database file name prefixes: `<prefix>.db` and `<prefix>-<channel>.db`.
    db_prefixes: &'static [&'static str],
    db_env: &'static str,
    program: &'static str,
    install_url: &'static str,
    export_args: &'static [&'static str],
}

pub const OPENCODE: Flavor = Flavor {
    id: "opencode",
    name: "OpenCode",
    data_dir: "opencode",
    db_prefixes: &["opencode"],
    db_env: "OPENCODE_DB",
    program: "opencode",
    install_url: "https://opencode.ai/docs/",
    export_args: &["session", "export"],
};

/// Kilo dev channels fall back to an older `opencode-<channel>.db` in Kilo's own folder.
pub const KILO: Flavor = Flavor {
    id: "kilo",
    name: "Kilo",
    data_dir: "kilo",
    db_prefixes: &["kilo", "opencode"],
    db_env: "KILO_DB",
    program: "kilo",
    install_url: "https://kilo.ai/cli/install",
    export_args: &["export"],
};

/// OpenCode-family agents keep every session in one SQLite database. Sessions are rows of
/// `session_v2` (newer OpenCode) or `session` (older OpenCode, Kilo); messages are in
/// `session_message`/`session_part` or the older `message`/`part`, chosen per session by
/// where its messages actually are. Marduk only ever opens the database read-only.
pub struct OpenCodeAdapter {
    flavor: &'static Flavor,
    data_dir: PathBuf,
    explicit_db: Option<PathBuf>,
    cli: Cli,
}

const SESSION_TABLES: [&str; 2] = ["session_v2", "session"];

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Store {
    /// `session_message` + `session_part`.
    V2,
    /// `message` + `part`.
    V1,
}

impl Store {
    fn tables(self) -> (&'static str, &'static str) {
        match self {
            Store::V2 => ("session_message", "session_part"),
            Store::V1 => ("message", "part"),
        }
    }
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

/// OpenCode-family ids look like `ses_01J…`; anything else is refused before it reaches the CLI.
fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && !id.starts_with('-')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn plural(count: i64, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

impl OpenCodeAdapter {
    pub fn new(data_dir: PathBuf, explicit_db: Option<PathBuf>, cli: Option<PathBuf>) -> Self {
        Self::with_flavor(&OPENCODE, data_dir, explicit_db, cli)
    }

    pub fn with_flavor(
        flavor: &'static Flavor,
        data_dir: PathBuf,
        explicit_db: Option<PathBuf>,
        cli: Option<PathBuf>,
    ) -> Self {
        Self {
            flavor,
            data_dir,
            explicit_db,
            cli: Cli::fixed(flavor.program, flavor.install_url, cli),
        }
    }

    pub fn from_env(flavor: &'static Flavor, overrides: std::sync::Arc<CliOverrides>) -> Self {
        let data_dir = env_dir("XDG_DATA_HOME")
            .unwrap_or_else(|| home_dir().join(".local").join("share"))
            .join(flavor.data_dir);
        let explicit_db = env_dir(flavor.db_env).map(|db| {
            if db.is_absolute() {
                db
            } else {
                data_dir.join(db)
            }
        });
        Self {
            flavor,
            cli: Cli::detect(flavor.id, flavor.program, flavor.install_url, overrides),
            data_dir,
            explicit_db,
        }
    }

    fn databases(&self) -> Vec<PathBuf> {
        let mut databases: Vec<PathBuf> = std::fs::read_dir(&self.data_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        name.ends_with(".db")
                            && self.flavor.db_prefixes.iter().any(|prefix| {
                                name == format!("{prefix}.db")
                                    || name.starts_with(&format!("{prefix}-"))
                            })
                    })
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
        if let Err(error) = connection.query_row("select count(*) from sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        }) {
            result.record_error(format!("{}: {error}", db.display()));
            return;
        }
        let stores: Vec<Store> = [Store::V2, Store::V1]
            .into_iter()
            .filter(|store| has_table(&connection, store.tables().0))
            .collect();
        let tables: Vec<&str> = SESSION_TABLES
            .into_iter()
            .filter(|table| has_table(&connection, table))
            .collect();
        if stores.is_empty() || tables.is_empty() {
            result.record_error(format!(
                "{}: no {} session tables Marduk recognizes; this {} version may be newer than Marduk supports.",
                db.display(),
                self.flavor.name,
                self.flavor.name
            ));
            return;
        }
        let totals = match all_totals(&connection, &stores) {
            Ok(totals) => totals,
            Err(error) => {
                result.record_error(format!("{}: {error}", db.display()));
                return;
            }
        };
        let mut seen = HashSet::new();
        for table in tables {
            match read_rows(&connection, table) {
                Ok(rows) => {
                    for row in rows {
                        if !seen.insert(row.id.clone()) {
                            continue;
                        }
                        let (store, totals) = pick_store(&totals, &row.id);
                        let first_prompt = if row
                            .title
                            .as_deref()
                            .is_none_or(|title| title.trim().is_empty())
                        {
                            first_user_text(&connection, store, &row.id)
                        } else {
                            None
                        };
                        result
                            .sessions
                            .push(self.session_from_row(db, row, totals, first_prompt));
                    }
                }
                Err(error) => result.record_error(format!("{}: {error}", db.display())),
            }
        }
    }

    fn session_from_row(
        &self,
        db: &Path,
        row: Row,
        totals: Totals,
        first_prompt: Option<String>,
    ) -> Session {
        let fingerprint = fingerprint_of(row.updated_ms, totals);
        let title = [row.title, first_prompt]
            .into_iter()
            .flatten()
            .find_map(|text| jsonl::to_title(&text));
        Session {
            agent: self.flavor.id,
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
            deleted_with_parent: row.parent_id.is_some(),
            part_of_parent: false,
            parent_id: row.parent_id,
        }
    }

    fn unsafe_id_reason(&self) -> String {
        format!(
            "This session id can't be passed to the {} CLI safely.",
            self.flavor.name
        )
    }

    fn delete_command(&self, session: &Session) -> Result<AgentCommand, String> {
        if !is_safe_id(&session.id) {
            return Err(self.unsafe_id_reason());
        }
        let program = self.cli.path().ok_or_else(|| {
            self.cli.missing_reason(
                self.flavor.name,
                &format!("delete {} sessions", self.flavor.name),
            )
        })?;
        Ok(AgentCommand {
            program,
            args: vec!["session".into(), "delete".into(), session.id.clone()],
        })
    }

    /// `<cli> session delete` also deletes the session's child (subagent) sessions.
    fn child_warning(&self, db: &Path, session_id: &str) -> Option<String> {
        let connection = open(db).ok()?;
        let children: i64 = SESSION_TABLES
            .into_iter()
            .filter(|table| has_table(&connection, table))
            .map(|table| {
                connection
                    .query_row(
                        &format!("select count(*) from {table} where parent_id = ?1"),
                        [session_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .unwrap_or(0)
            })
            .sum();
        (children > 0).then(|| {
            format!(
                "{} also deletes this session's {}.",
                self.flavor.name,
                plural(children, "child session")
            )
        })
    }

    /// Reads a session through the CLI's export, which returns the agent's own decoded
    /// messages and so keeps working when its tables change shape.
    fn export_transcript(&self, cli: &Path, session_id: &str) -> Result<Transcript, String> {
        let mut args: Vec<&str> = self.flavor.export_args.to_vec();
        args.push(session_id);
        let output = super::cli::run_capture(cli, &args, EXPORT_TIMEOUT)?;
        let export: Value = serde_json::from_str(&output)
            .map_err(|error| format!("Unexpected export output: {error}"))?;
        let messages = export["messages"]
            .as_array()
            .ok_or("The export had no messages list.")?;
        let mut builder = TranscriptBuilder::default();
        for message in messages {
            if message.get("parts").is_some() {
                append_v1_message(
                    &mut builder,
                    &message["info"],
                    message["parts"].as_array().unwrap_or(&Vec::new()),
                );
            } else {
                append_v2_message(
                    &mut builder,
                    message["id"].as_str().unwrap_or_default(),
                    message["type"].as_str().unwrap_or_default(),
                    message["time"]["created"].as_i64().unwrap_or_default(),
                    message,
                    Vec::new(),
                );
            }
        }
        let mut transcript = builder.finish();
        transcript.note = Some(format!(
            "Read through `{} {}`, because Marduk didn't recognize how this session is stored.",
            self.flavor.program,
            self.flavor.export_args.join(" ")
        ));
        Ok(transcript)
    }
}

fn read_rows(connection: &Connection, table: &str) -> rusqlite::Result<Vec<Row>> {
    let archived = if has_column(connection, table, "time_archived") {
        "time_archived"
    } else {
        "null"
    };
    let mut statement = connection.prepare(&format!(
        "select id, parent_id, directory, title, time_created, time_updated, {archived} from {table}"
    ))?;
    statement
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
        .collect()
}

type StoreTotals = HashMap<Store, HashMap<String, Totals>>;

fn all_totals(connection: &Connection, stores: &[Store]) -> rusqlite::Result<StoreTotals> {
    stores
        .iter()
        .map(|store| Ok((*store, totals(connection, *store)?)))
        .collect()
}

/// Sessions OpenCode has migrated forward can have messages in both stores; the newer wins.
fn pick_store(totals: &StoreTotals, session_id: &str) -> (Store, Totals) {
    for store in [Store::V2, Store::V1] {
        if let Some(found) = totals
            .get(&store)
            .and_then(|by_session| by_session.get(session_id))
            && found.messages > 0
        {
            return (store, *found);
        }
    }
    let fallback = if totals.contains_key(&Store::V2) {
        Store::V2
    } else {
        Store::V1
    };
    (fallback, Totals::default())
}

fn totals(connection: &Connection, store: Store) -> rusqlite::Result<HashMap<String, Totals>> {
    let (messages, parts) = store.tables();
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
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
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

fn first_user_text(connection: &Connection, store: Store, session_id: &str) -> Option<String> {
    let sql = match store {
        Store::V2 => {
            "select data from session_message where session_id = ?1 and type = 'user' \
             order by seq, time_created, id limit 1"
        }
        Store::V1 => {
            "select p.data from part p join message m on m.id = p.message_id \
             where m.session_id = ?1 and json_extract(m.data, '$.role') = 'user' \
             and json_extract(p.data, '$.type') = 'text' order by m.time_created, m.id, p.id limit 1"
        }
    };
    let data: String = connection
        .query_row(sql, [session_id], |row| row.get(0))
        .ok()?;
    let value: Value = serde_json::from_str(&data).ok()?;
    value["text"].as_str().map(str::to_string)
}

/// The session row's update time and its messages' totals, or `None` when the row is gone.
fn current_state(
    connection: &Connection,
    session_id: &str,
) -> rusqlite::Result<Option<(Store, i64, Totals)>> {
    let mut updated = None;
    for table in SESSION_TABLES {
        if has_table(connection, table) {
            updated = connection
                .query_row(
                    &format!("select time_updated from {table} where id = ?1"),
                    [session_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if updated.is_some() {
                break;
            }
        }
    }
    let Some(updated) = updated else {
        return Ok(None);
    };
    for store in [Store::V2, Store::V1] {
        let (messages, parts) = store.tables();
        if !has_table(connection, messages) {
            continue;
        }
        let (count, message_updated): (i64, i64) = connection.query_row(
            &format!("select count(*), coalesce(max(time_updated), 0) from {messages} where session_id = ?1"),
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if count == 0 {
            continue;
        }
        let part_updated: i64 = if has_table(connection, parts) {
            connection.query_row(
                &format!(
                    "select coalesce(max(time_updated), 0) from {parts} where session_id = ?1"
                ),
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
        return Ok(Some((store, updated, totals)));
    }
    let store = if has_table(connection, "session_message") {
        Store::V2
    } else {
        Store::V1
    };
    Ok(Some((store, updated, Totals::default())))
}

impl AgentAdapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        self.flavor.id
    }

    fn display_name(&self) -> &'static str {
        self.flavor.name
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.data_dir.clone()]
    }

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        self.cli.refresh();
        let mut result = ScanResult::empty_complete();
        for db in self.databases() {
            if db.is_file() {
                self.scan_database(&db, &mut result);
                progress.advance(db.metadata().map(|meta| meta.len()).unwrap_or(0));
            }
        }
        result
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let databases: Vec<PathBuf> = self
            .databases()
            .into_iter()
            .filter(|db| db.is_file())
            .collect();
        super::total_size(&databases)
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        match self.delete_command(session) {
            Ok(command) => DeletePlan::AgentCommand {
                command,
                files: Vec::new(),
                warning: self.child_warning(&session.primary_file, &session.id),
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
            Some(cli) if is_safe_id(&session.id) => self.export_transcript(&cli, &session.id),
            Some(_) => Err(self.unsafe_id_reason()),
            None => Err(self
                .cli
                .missing_reason(self.flavor.name, "read sessions it can't read directly")),
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
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "session row is gone",
            )),
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

fn read_transcript(connection: &Connection, session_id: &str) -> rusqlite::Result<Transcript> {
    let mut builder = TranscriptBuilder::default();
    match current_state(connection, session_id)?.map(|(store, _, _)| store) {
        Some(Store::V2) => read_v2(connection, session_id, &mut builder)?,
        Some(Store::V1) => read_v1(connection, session_id, &mut builder)?,
        None => {}
    }
    Ok(builder.finish())
}

fn parse(data: &str) -> Value {
    serde_json::from_str(data).unwrap_or(Value::Null)
}

fn read_v2(
    connection: &Connection,
    session_id: &str,
    builder: &mut TranscriptBuilder,
) -> rusqlite::Result<()> {
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
    let mut statement = connection.prepare(
        "select id, type, time_created, data from session_message where session_id = ?1 \
         order by seq, time_created, id",
    )?;
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

/// One `session_message` row, or one message of a v2 export (same shape). `parts` are the
/// assistant's `session_part` rows, which come before any inline content.
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
        "compaction" => builder.text(
            Role::Context,
            data["summary"].as_str().unwrap_or_default(),
            timestamp,
        ),
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

fn append_v2_block(
    builder: &mut TranscriptBuilder,
    block: &Value,
    timestamp: Option<DateTime<Utc>>,
) {
    match block["type"].as_str() {
        Some("text") => builder.text(
            Role::Assistant,
            block["text"].as_str().unwrap_or_default(),
            timestamp,
        ),
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
                Some("completed") => builder.tool_result(
                    id,
                    &transcript::tool_output_text(&state["content"]),
                    timestamp,
                ),
                Some("error") => builder.tool_result(
                    id,
                    state["error"]["message"]
                        .as_str()
                        .unwrap_or("The tool failed."),
                    timestamp,
                ),
                _ => {}
            }
        }
        _ => {}
    }
}

fn read_v1(
    connection: &Connection,
    session_id: &str,
    builder: &mut TranscriptBuilder,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "select m.id, m.data, m.time_created from message m where m.session_id = ?1 order by m.time_created, m.id",
    )?;
    let messages = statement
        .query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut parts_statement = connection
        .prepare("select data, time_created from part where message_id = ?1 order by id")?;
    for (id, data, created) in messages {
        let mut info = parse(&data);
        if info["time"]["created"].is_null()
            && let Value::Object(fields) = &mut info
        {
            fields.insert("time".into(), serde_json::json!({ "created": created }));
        }
        let parts = parts_statement
            .query_map([&id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(data, part_created)| {
                let mut part = parse(&data);
                if let Value::Object(fields) = &mut part {
                    fields
                        .entry("time_created")
                        .or_insert(Value::from(part_created));
                }
                part
            })
            .collect::<Vec<_>>();
        append_v1_message(builder, &info, &parts);
    }
    Ok(())
}

/// One older-format message: `info` has the role, `parts` hold the text and tool calls.
/// Kilo's export uses this shape (`{ info, parts }`).
fn append_v1_message(builder: &mut TranscriptBuilder, info: &Value, parts: &[Value]) {
    let user = info["role"] == "user";
    let message_created = info["time"]["created"].as_i64();
    for part in parts {
        let created = part["time"]["start"]
            .as_i64()
            .or_else(|| part["time_created"].as_i64())
            .or(message_created)
            .unwrap_or_default();
        let timestamp = millis_to_datetime(created);
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

    fn insert_session(
        connection: &Connection,
        id: &str,
        parent: Option<&str>,
        title: Option<&str>,
        archived: bool,
    ) {
        connection
            .execute(
                "insert into session_v2 values (?1, ?2, '/work/app', ?3, 1762400000000, 1762400100000, ?4)",
                rusqlite::params![id, parent, title, archived.then_some(1762400200000i64)],
            )
            .unwrap();
    }

    fn insert_message(
        connection: &Connection,
        session: &str,
        id: &str,
        seq: i64,
        kind: &str,
        data: &str,
    ) {
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
        insert_message(
            &connection,
            "ses_b",
            "msg_1",
            1,
            "user",
            r#"{"text":"Why is CI red?"}"#,
        );
        insert_message(
            &connection,
            "ses_b",
            "msg_2",
            2,
            "assistant",
            r#"{"content":[]}"#,
        );

        let result = adapter(dir.path(), None).scan();

        assert!(result.complete, "{:?}", result.errors);
        let by_id = |id: &str| {
            result
                .sessions
                .iter()
                .find(|session| session.id == id)
                .unwrap()
        };
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
        insert_message(
            &connection,
            "ses_a",
            "msg_1",
            1,
            "system",
            r#"{"text":"env info"}"#,
        );
        insert_message(
            &connection,
            "ses_a",
            "msg_2",
            2,
            "user",
            r#"{"text":"List files"}"#,
        );
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
        assert_eq!(
            roles,
            vec![Role::Context, Role::User, Role::Tool, Role::Assistant]
        );
        assert_eq!(
            messages[2].tool.as_ref().unwrap().output.as_deref(),
            Some("a.txt")
        );
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
            sessions
                .iter()
                .find(|s| s.id == "ses_dup")
                .unwrap()
                .title
                .as_deref(),
            Some("New copy")
        );
        let old = sessions.iter().find(|s| s.id == "ses_old").unwrap();
        assert!(!old.archived);
        let messages = opencode.transcript(old).unwrap().head;
        assert_eq!(messages[0].text, "hello");
        assert_eq!(
            messages[1].tool.as_ref().unwrap().output.as_deref(),
            Some("file body")
        );
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
        insert_message(
            &connection,
            "ses_new",
            "msg_1",
            1,
            "from-the-future",
            r#"{"blob":1}"#,
        );
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
        insert_message(
            &connection,
            "ses_new",
            "msg_1",
            1,
            "from-the-future",
            r#"{"blob":1}"#,
        );
        let opencode = adapter(dir.path(), None);
        let session = opencode.scan().sessions.remove(0);

        let transcript = opencode.transcript(&session).unwrap();

        assert!(transcript.head.is_empty());
        assert!(
            transcript
                .note
                .unwrap()
                .contains("Install the OpenCode CLI")
        );
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

    /// Kilo: sessions in `session`, messages in either `message`/`part` or `session_message`.
    fn kilo_db(dir: &Path, name: &str) -> Connection {
        let connection = Connection::open(dir.join(name)).unwrap();
        connection
            .execute_batch(
                "create table session (id text primary key, parent_id text, directory text not null, title text not null,
                   time_created integer not null, time_updated integer not null, time_archived integer);
                 create table message (id text primary key, session_id text not null, time_created integer not null,
                   time_updated integer not null, data text not null);
                 create table part (id text primary key, message_id text not null, session_id text not null,
                   time_created integer not null, time_updated integer not null, data text not null);
                 create table session_message (id text primary key, session_id text not null, type text not null,
                   seq integer, time_created integer not null, time_updated integer not null, data text not null);",
            )
            .unwrap();
        connection
    }

    fn kilo(dir: &Path, cli: Option<PathBuf>) -> OpenCodeAdapter {
        OpenCodeAdapter::with_flavor(&KILO, dir.to_path_buf(), None, cli)
    }

    #[test]
    fn kilo_reads_each_session_from_the_store_its_messages_are_in() {
        let dir = tempfile::tempdir().unwrap();
        let connection = kilo_db(dir.path(), "kilo.db");
        connection
            .execute_batch(
                r#"insert into session values ('ses_v1', null, '/w', '', 1, 2, null);
                   insert into session values ('ses_v2', 'ses_v1', '/w', 'Newer', 1, 2, 5);
                   insert into message values ('m1', 'ses_v1', 10, 10, '{"role":"user"}');
                   insert into part values ('p1', 'm1', 'ses_v1', 10, 10, '{"type":"text","text":"old style prompt"}');
                   insert into session_message values ('msg_1', 'ses_v2', 'user', null, 20, 20, '{"text":"new style prompt"}');"#,
            )
            .unwrap();
        let adapter = kilo(dir.path(), Some(PathBuf::from("/bin/kilo")));

        let result = adapter.scan();

        assert!(result.complete, "{:?}", result.errors);
        let by_id = |id: &str| {
            result
                .sessions
                .iter()
                .find(|session| session.id == id)
                .unwrap()
        };
        let old = by_id("ses_v1");
        assert_eq!(old.agent, "kilo");
        assert_eq!(old.key(), "kilo:ses_v1");
        assert_eq!(old.title.as_deref(), Some("old style prompt"));
        let new = by_id("ses_v2");
        assert!(new.archived);
        assert_eq!(new.parent_id.as_deref(), Some("ses_v1"));
        assert_eq!(
            adapter.transcript(old).unwrap().head[0].text,
            "old style prompt"
        );
        assert_eq!(
            adapter.transcript(new).unwrap().head[0].text,
            "new style prompt"
        );
        assert_eq!(adapter.fingerprint(new).unwrap(), new.fingerprint);

        let DeletePlan::AgentCommand {
            command, warning, ..
        } = adapter.delete_plan(old)
        else {
            panic!("expected an agent command");
        };
        assert_eq!(command.args, vec!["session", "delete", "ses_v1"]);
        assert_eq!(
            warning.as_deref(),
            Some("Kilo also deletes this session's 1 child session.")
        );
    }

    #[test]
    fn kilo_finds_channel_databases_including_the_opencode_named_fallback() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["kilo.db", "kilo-dev.db", "opencode-beta.db"] {
            kilo_db(dir.path(), name)
                .execute(
                    "insert into session values (?1, null, '/w', 't', 1, 2, null)",
                    [name],
                )
                .unwrap();
        }
        std::fs::write(dir.path().join("kilo.db-wal"), "").unwrap();
        std::fs::write(dir.path().join("other.db"), "").unwrap();

        let ids: HashSet<String> = kilo(dir.path(), None)
            .scan()
            .sessions
            .into_iter()
            .map(|s| s.id)
            .collect();

        assert_eq!(
            ids,
            HashSet::from([
                "kilo.db".into(),
                "kilo-dev.db".into(),
                "opencode-beta.db".into()
            ])
        );
    }

    #[test]
    fn kilo_without_cli_names_kilo_in_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        kilo_db(dir.path(), "kilo.db")
            .execute(
                "insert into session values ('ses_a', null, '/w', 't', 1, 2, null)",
                [],
            )
            .unwrap();
        let adapter = kilo(dir.path(), None);
        let session = adapter.scan().sessions.remove(0);
        let DeletePlan::ReadOnly { reason } = adapter.delete_plan(&session) else {
            panic!("expected read-only");
        };
        assert!(reason.contains("Kilo CLI (`kilo`)"), "{reason}");
    }

    #[cfg(unix)]
    #[test]
    fn kilo_export_fallback_reads_the_older_message_shape() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        kilo_db(dir.path(), "kilo.db")
            .execute_batch(
                "insert into session values ('ses_x', null, '/w', 't', 1, 2, null);
                 insert into session_message values ('msg_1', 'ses_x', 'from-the-future', null, 1, 1, '{}');",
            )
            .unwrap();
        let export = dir.path().join("export.json");
        std::fs::write(
            &export,
            r#"{"info":{"id":"ses_x"},"messages":[
                {"info":{"role":"user","time":{"created":1}},"parts":[{"type":"text","text":"from export"}]},
                {"info":{"role":"assistant","time":{"created":2}},"parts":[
                  {"type":"tool","tool":"read","callID":"c","state":{"status":"completed","input":{},"output":"body"}},
                  {"type":"text","text":"Done"}]}]}"#,
        )
        .unwrap();
        let script = dir.path().join("kilo");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n[ \"$1 $2\" = \"export ses_x\" ] || exit 9\ncat '{}'\n",
                export.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let adapter = kilo(dir.path(), Some(script));
        let session = adapter.scan().sessions.remove(0);

        let transcript = adapter.transcript(&session).unwrap();

        assert_eq!(transcript.head[0].text, "from export");
        assert_eq!(
            transcript.head[1].tool.as_ref().unwrap().output.as_deref(),
            Some("body")
        );
        assert_eq!(transcript.head[2].text, "Done");
        assert!(transcript.note.unwrap().contains("`kilo export`"));
    }

    #[test]
    fn delete_uses_the_cli_warns_about_children_and_tracks_the_row() {
        let dir = tempfile::tempdir().unwrap();
        let (_, connection) = v2_db(dir.path());
        insert_session(&connection, "ses_parent", None, Some("p"), false);
        insert_session(
            &connection,
            "ses_child",
            Some("ses_parent"),
            Some("c"),
            false,
        );
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

        insert_message(
            &connection,
            "ses_a",
            "msg_9",
            9,
            "user",
            r#"{"text":"more"}"#,
        );

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
        assert!(matches!(
            with_cli.delete_plan(&session),
            DeletePlan::ReadOnly { .. }
        ));
        assert!(with_cli.archive_command(&session).is_none());
    }
}
