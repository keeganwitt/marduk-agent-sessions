use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{entries, idea_dirs, invalid_data, latest_copies, read_only, system_dir};
use crate::adapter::{AgentAdapter, DeletePlan, Mechanics, ScanProgress, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl::{Span, to_title};
use crate::session::{Fingerprint, Session, note_model};
use crate::transcript::{Role, Transcript, TranscriptBuilder};

const ID: &str = "jetbrains-air";
const DELETE: &str = "Air manages local session history in IntelliJ's Agent Sessions tool window.";

pub struct AirAdapter {
    system: PathBuf,
    cache: ParseCache<Fingerprint, EventMetadata>,
}

impl AirAdapter {
    pub fn new(system: PathBuf) -> Self {
        Self {
            system,
            cache: ParseCache::default(),
        }
    }

    pub fn from_env() -> Self {
        let system = super::super::env_dir("MARDUK_JETBRAINS_SYSTEM_DIR")
            .unwrap_or_else(|| system_dir(&super::super::home_dir()));
        Self::new(system)
    }

    fn locations(&self, result: &mut ScanResult) -> Vec<Location> {
        let mut locations = Vec::new();
        for installation in idea_dirs(&self.system, result) {
            for project in entries(&installation.join("acp/projects"), result, true) {
                for thread in entries(&project.join("threads"), result, true) {
                    let journal = thread.join("events.jsonl");
                    match fs_util::fingerprint(&journal) {
                        Ok(fingerprint) => locations.push(Location {
                            journal,
                            fingerprint,
                        }),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => {
                            result.record_error(format!("{}: {error}", journal.display()))
                        }
                    }
                }
            }
        }
        locations
    }

    fn load(
        &self,
        location: Location,
        progress: &dyn ScanProgress,
        result: &mut ScanResult,
    ) -> io::Result<Option<Session>> {
        let Location {
            journal,
            fingerprint,
        } = location;
        progress.advance(fingerprint.len);
        let metadata = self.cache.get_or_parse(&journal, fingerprint, || {
            let mut metadata = EventMetadata::default();
            for_event(&journal, |event| metadata.observe(event))?;
            Ok::<_, io::Error>(metadata)
        })?;

        let thread = journal.parent().expect("Air thread directory");
        let summary_path = thread.join("summary.json");
        let summary = optional_json::<Summary>(&summary_path, result);
        let Some(id) = metadata
            .id
            .or_else(|| summary.as_ref().map(|summary| summary.thread_id.clone()))
        else {
            return Ok(None);
        };
        let project = thread
            .parent()
            .and_then(Path::parent)
            .expect("Air project directory");
        let manifest = optional_json::<Manifest>(&project.join("store.json"), result);

        let mut size_bytes = fingerprint.len;
        let mut files = vec![journal.clone()];
        if let Some(summary_size) = fs::metadata(&summary_path).ok().map(|meta| meta.len()) {
            size_bytes += summary_size;
            files.push(summary_path);
        }
        let summary = summary.unwrap_or_default();
        let title = [
            summary.display_title,
            summary.user_title,
            metadata.title,
            summary.automatic_title,
        ]
        .into_iter()
        .flatten()
        .find_map(|title| to_title(&title))
        .or(metadata.first_prompt);
        let last_activity = [
            metadata.span.last,
            summary
                .provider_updated_at
                .and_then(DateTime::from_timestamp_millis),
            summary
                .last_user_prompt_at
                .and_then(DateTime::from_timestamp_millis),
            summary
                .agent_output_completed_at
                .and_then(DateTime::from_timestamp_millis),
        ]
        .into_iter()
        .flatten()
        .max()
        .unwrap_or_else(|| fingerprint.modified.into());
        let session = Session {
            agent: ID,
            id,
            project_dir: manifest
                .and_then(|manifest| absolute_path(manifest.project_directory))
                .or_else(|| metadata.cwd.and_then(absolute_path)),
            title,
            created: metadata.span.first.or_else(|| fs_util::created(&journal)),
            last_activity,
            size_bytes,
            archived: metadata.archived.or(summary.archived).unwrap_or(false),
            pinned: None,
            primary_file: journal,
            files,
            fingerprint,
            parent_id: None,
            deleted_with_parent: false,
            part_of_parent: false,
            projectless: false,
            models: metadata.models,
            agent_type: summary
                .runtime
                .and_then(|runtime| runtime.agent_id)
                .or(metadata.agent),
            empty: !metadata.has_messages,
        };
        Ok(Some(session))
    }
}

struct Location {
    journal: PathBuf,
    fingerprint: Fingerprint,
}

impl AgentAdapter for AirAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "JetBrains Air"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.system.clone()]
    }

    fn estimate_scan_bytes(&self) -> u64 {
        self.locations(&mut ScanResult::empty_complete())
            .iter()
            .map(|location| location.fingerprint.len)
            .sum()
    }

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let mut seen = HashSet::new();
        for location in self.locations(&mut result) {
            let path = location.journal.clone();
            match self.load(location, progress, &mut result) {
                Ok(Some(session)) => result.sessions.push(session),
                Ok(None) => {}
                Err(error) => result.record_error(format!("{}: {error}", path.display())),
            }
            seen.insert(path);
        }
        self.cache.retain(&seen);
        result.sessions = latest_copies(result.sessions);
        result
    }

    fn delete_plan(&self, _session: &Session) -> DeletePlan {
        read_only(DELETE)
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: "Air sessions are read-only in Marduk.".into(),
            archive: "Use Mark as Done in IntelliJ's Agent Sessions tool window.".into(),
        }
    }

    fn transcript(&self, session: &Session) -> io::Result<Transcript> {
        let mut replay = Replay::default();
        for_event(&session.primary_file, |event| replay.observe(event))?;
        replay.flush();
        Ok(replay.builder.finish())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    project_directory: String,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    thread_id: String,
    display_title: Option<String>,
    user_title: Option<String>,
    automatic_title: Option<String>,
    provider_updated_at: Option<i64>,
    last_user_prompt_at: Option<i64>,
    agent_output_completed_at: Option<i64>,
    archived: Option<bool>,
    runtime: Option<Runtime>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    agent_id: Option<String>,
}

fn optional_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    result: &mut ScanResult,
) -> Option<T> {
    let value =
        fs::read(path).and_then(|bytes| serde_json::from_slice(&bytes).map_err(invalid_data));
    match value {
        Ok(value) => Some(value),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => {
            result.record_error(format!("{}: {error}", path.display()));
            None
        }
    }
}

fn absolute_path(path: String) -> Option<String> {
    Path::new(&path).is_absolute().then_some(path)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Event {
    thread_id: String,
    turn_id: Option<String>,
    timestamp: i64,
    #[serde(rename = "type")]
    kind: String,
    payload: Value,
}

impl Event {
    fn timestamp(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp_millis(self.timestamp)
    }

    fn message(&self) -> Option<(Role, &str)> {
        if !matches!(self.kind.as_str(), "MessageChunk" | "MessageDelta") {
            return None;
        }
        let role = match self.payload["role"].as_str()? {
            "User" => Role::User,
            "Agent" if self.payload["messagePhase"].as_str() != Some("Working") => Role::Assistant,
            "System" => Role::Context,
            _ => return None,
        };
        Some((role, self.payload["contentDelta"].as_str()?))
    }

    fn message_id(&self) -> Option<&str> {
        self.payload["messageId"]
            .as_str()
            .or(self.payload["protocolMessageId"].as_str())
    }
}

fn for_event(path: &Path, mut observe: impl FnMut(&Event)) -> io::Result<()> {
    for (index, line) in BufReader::new(File::open(path)?).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let event: Event = serde_json::from_str(&line)
            .map_err(|error| invalid_data(format!("Air journal line {}: {error}", index + 1)))?;
        observe(&event);
    }
    Ok(())
}

#[derive(Clone, Default)]
struct EventMetadata {
    id: Option<String>,
    span: Span,
    cwd: Option<String>,
    title: Option<String>,
    first_prompt: Option<String>,
    agent: Option<String>,
    models: Vec<String>,
    archived: Option<bool>,
    has_messages: bool,
}

impl EventMetadata {
    fn observe(&mut self, event: &Event) {
        if self.id.is_none() {
            self.id = Some(event.thread_id.clone());
        }
        self.span.observe(event.timestamp());
        let payload = &event.payload;
        match event.kind.as_str() {
            "ThreadCreated" | "SessionCreated" | "RuntimeSessionBound" => {
                if let Some(cwd) = payload["cwd"].as_str() {
                    self.cwd = Some(cwd.into());
                }
                if let Some(agent) = payload["agentId"].as_str() {
                    self.agent = Some(agent.into());
                }
            }
            "ThreadUpdated" | "SessionUpdated" => {
                if let Some(title) = payload["title"].as_str() {
                    self.title = to_title(title);
                }
            }
            "ThreadArchived" | "SessionArchived" => self.archived = Some(true),
            "ThreadUnarchived" | "SessionUnarchived" => self.archived = Some(false),
            "SessionArchiveStateChanged" => self.archived = payload["archived"].as_bool(),
            "ConfigOptionUpdated" => {
                for option in payload["configOptions"].as_array().into_iter().flatten() {
                    if option["category"].as_str() == Some("model")
                        && let Some(model) = option["value"].as_str()
                    {
                        note_model(&mut self.models, model);
                    }
                }
            }
            _ => {}
        }
        if let Some((role, text)) = event.message() {
            if role != Role::Context && !text.trim().is_empty() {
                self.has_messages = true;
            }
            if role == Role::User && self.first_prompt.is_none() {
                self.first_prompt = to_title(text);
            }
        }
    }
}

#[derive(Default)]
struct Replay {
    builder: TranscriptBuilder,
    pending: Option<PendingMessage>,
}

struct PendingMessage {
    role: Role,
    id: Option<String>,
    turn: Option<String>,
    timestamp: Option<DateTime<Utc>>,
    text: String,
}

impl Replay {
    fn flush(&mut self) {
        if let Some(message) = self.pending.take() {
            self.builder
                .text(message.role, &message.text, message.timestamp);
        }
    }

    fn observe(&mut self, event: &Event) {
        if let Some((role, text)) = event.message() {
            if let Some(message) = self.pending.as_mut()
                && message.role == role
                && message.id.as_deref() == event.message_id()
                && message.turn == event.turn_id
            {
                message.text.push_str(text);
            } else {
                self.flush();
                self.pending = Some(PendingMessage {
                    role,
                    id: event.message_id().map(str::to_owned),
                    turn: event.turn_id.clone(),
                    timestamp: event.timestamp(),
                    text: text.to_owned(),
                });
            }
            return;
        }

        let payload = &event.payload;
        let timestamp = event.timestamp();
        let call_id = payload["toolCallId"].as_str();
        match event.kind.as_str() {
            "MessageContentReplaced" => {
                if let Some(message) = self.pending.as_mut()
                    && message.id.is_some()
                    && message.id.as_deref() == event.message_id()
                    && let Some(content) = payload["content"].as_str()
                {
                    content.clone_into(&mut message.text);
                }
            }
            "MessageCompleted" | "TurnEnded" => self.flush(),
            "ToolCallStarted" => {
                self.flush();
                let attrs = &payload["attributes"];
                let name = attrs["title"]
                    .as_str()
                    .or(attrs["kind"].as_str())
                    .unwrap_or("Tool");
                let input = attrs["command"]
                    .as_str()
                    .or(attrs["path"].as_str())
                    .or(attrs["url"].as_str())
                    .unwrap_or_default();
                self.builder.tool_call(call_id, name, input, timestamp);
            }
            "ToolCallFinished" => {
                if let Some(output) = payload["summary"].as_str().or(payload["status"].as_str()) {
                    self.builder.tool_result(call_id, output, timestamp);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn journal(root: &Path) -> PathBuf {
        let thread = root.join("IntelliJIdea2026.3/acp/projects/project/threads/acp%3Atest");
        fs::create_dir_all(&thread).unwrap();
        thread.join("events.jsonl")
    }

    #[test]
    fn reads_air_metadata_and_reassembles_streamed_replies() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let journal = journal(root);
        let thread = journal.parent().unwrap();
        let project = root.join("project");
        fs::write(
            thread.join("../../store.json"),
            json!({"schemaVersion": 1, "projectDirectory": project}).to_string(),
        )
        .unwrap();
        let summary = thread.join("summary.json");
        fs::write(
            &summary,
            json!({
                "schemaVersion": 10, "threadId": "acp:test", "displayTitle": "Renamed in Air",
                "archived": true, "runtime": {"agentId": "codex"},
            })
            .to_string(),
        )
        .unwrap();
        let event = |time, kind: &str, payload: Value| {
            json!({
                "id": format!("e{time}"), "threadId": "acp:test", "seq": time,
                "timestamp": time, "turnId": "turn", "type": kind, "payload": payload, "version": 1,
            })
        };
        let events = [
            event(1000, "ThreadCreated", json!({"cwd": root.join("worktree")})),
            event(
                1010,
                "MessageDelta",
                json!({"role": "User", "messageId": "prompt", "contentDelta": "Find it"}),
            ),
            event(
                1020,
                "ConfigOptionUpdated",
                json!({"configOptions": [{"category": "model", "value": "sample-model"}]}),
            ),
            event(
                1030,
                "MessageChunk",
                json!({"role": "Agent", "messagePhase": "Working", "contentDelta": "thinking"}),
            ),
            event(
                1040,
                "MessageChunk",
                json!({"role": "Agent", "messagePhase": "Final", "protocolMessageId": "answer", "contentDelta": "Here"}),
            ),
            event(
                1050,
                "MessageChunk",
                json!({"role": "Agent", "messagePhase": "Final", "protocolMessageId": "answer", "contentDelta": " it is"}),
            ),
            event(1060, "TurnEnded", json!({})),
        ];
        fs::write(
            &journal,
            events
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();

        let adapter = AirAdapter::new(root.into());
        let scan = adapter.scan();
        assert!(scan.complete, "{:?}", scan.errors);
        let [session] = scan.sessions.as_slice() else {
            panic!("expected one Air session");
        };
        assert_eq!(session.id, "acp:test");
        assert_eq!(session.project_dir.as_deref(), project.to_str());
        assert_eq!(session.title.as_deref(), Some("Renamed in Air"));
        assert_eq!(session.created.unwrap().timestamp_millis(), 1000);
        assert_eq!(session.last_activity.timestamp_millis(), 1060);
        assert_eq!(session.models, ["sample-model"]);
        assert_eq!(session.agent_type.as_deref(), Some("codex"));
        assert_eq!(session.files, [journal, summary]);
        assert!(session.archived);
        assert!(!session.empty);
        assert_eq!(adapter.delete_plan(session), read_only(DELETE));
        assert!(adapter.archive_command(session).is_none());
        let transcript = adapter.transcript(session).unwrap();
        assert_eq!(
            transcript
                .head
                .iter()
                .map(|m| (m.role, m.text.as_str()))
                .collect::<Vec<_>>(),
            [(Role::User, "Find it"), (Role::Assistant, "Here it is")]
        );
    }

    #[test]
    fn reports_an_unreadable_journal_as_an_incomplete_scan() {
        let temp = tempfile::tempdir().unwrap();
        let adapter = AirAdapter::new(temp.path().into());
        assert!(adapter.scan().complete);
        fs::write(journal(temp.path()), "{invalid journal}\n").unwrap();
        let scan = adapter.scan();
        assert!(!scan.complete);
        assert!(scan.errors[0].contains("Air journal line 1"));
    }
}
