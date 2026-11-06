use std::path::PathBuf;
use std::time::SystemTime;

use marduk_core::catalog::{self, AgentScan, Catalog, Preview};
use marduk_core::{ArchiveOutcome, DeleteOutcome, DeletePlan, Message, Role, Session, Transcript};
use serde::Serialize;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentSummary {
    pub id: &'static str,
    pub name: &'static str,
    pub complete: bool,
    pub errors: Vec<String>,
    pub session_count: usize,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub uid: String,
    pub key: String,
    pub agent: &'static str,
    pub agent_name: &'static str,
    pub id: String,
    pub title: Option<String>,
    pub project_dir: Option<String>,
    pub created: Option<String>,
    pub last_activity: String,
    pub size_bytes: u64,
    pub files: Vec<String>,
    pub archived: bool,
    pub running: bool,
    pub read_only_reason: Option<String>,
    pub permanent_delete: bool,
    pub can_archive: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub agents: Vec<AgentSummary>,
    pub sessions: Vec<SessionRow>,
    pub home_dir: String,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolDto {
    pub name: String,
    pub output: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    pub role: &'static str,
    pub text: String,
    pub timestamp: Option<String>,
    pub tool: Option<ToolDto>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptDto {
    pub head: Vec<MessageDto>,
    pub omitted: usize,
    pub tail: Vec<MessageDto>,
    pub shortened: bool,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub uid: String,
    pub title: Option<String>,
    pub agent_name: Option<&'static str>,
    pub files: Vec<String>,
    pub permanent: bool,
    pub command: Option<String>,
    pub refused_reason: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum DeleteResult {
    Trashed {
        uid: String,
        files: Vec<String>,
    },
    Deleted {
        uid: String,
        files: Vec<String>,
        command: String,
    },
    Refused {
        uid: String,
        message: String,
    },
    Failed {
        uid: String,
        message: String,
    },
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum ArchiveResult {
    Done {
        uid: String,
        archived: bool,
        command: String,
    },
    Refused {
        uid: String,
        message: String,
    },
    Failed {
        uid: String,
        message: String,
    },
}

fn paths(files: Vec<PathBuf>) -> Vec<String> {
    files
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn session_row(catalog: &Catalog, session: &Session, now: SystemTime) -> SessionRow {
    let plan = catalog.delete_plan(session);
    SessionRow {
        uid: catalog::uid(session),
        key: session.key(),
        agent: session.agent,
        agent_name: catalog.display_name(session.agent),
        id: session.id.clone(),
        title: session.title.clone(),
        project_dir: session.project_dir.clone(),
        created: session.created.map(|created| created.to_rfc3339()),
        last_activity: session.last_activity.to_rfc3339(),
        size_bytes: session.size_bytes,
        files: paths(session.files.clone()),
        archived: session.archived,
        running: session.is_running(now, catalog.running_window()),
        read_only_reason: match &plan {
            Some(DeletePlan::ReadOnly { reason }) => Some(reason.clone()),
            _ => None,
        },
        permanent_delete: matches!(plan, Some(DeletePlan::AgentCommand { .. })),
        can_archive: catalog.can_archive(session),
    }
}

pub fn scan_response(catalog: &Catalog, scans: Vec<AgentScan>, now: SystemTime) -> ScanResponse {
    let mut sessions: Vec<SessionRow> = scans
        .iter()
        .flat_map(|scan| scan.sessions.iter().map(|session| session_row(catalog, session, now)))
        .collect();
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    let agents = scans
        .into_iter()
        .map(|scan| AgentSummary {
            id: scan.agent,
            name: scan.display_name,
            complete: scan.complete,
            session_count: scan.sessions.len(),
            errors: scan.errors,
        })
        .collect();
    ScanResponse {
        agents,
        sessions,
        home_dir: marduk_core::home_dir().to_string_lossy().into_owned(),
    }
}

fn message_dto(message: Message) -> MessageDto {
    MessageDto {
        role: match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
            Role::Context => "context",
        },
        text: message.text,
        timestamp: message.timestamp.map(|timestamp| timestamp.to_rfc3339()),
        tool: message.tool.map(|tool| ToolDto {
            name: tool.name,
            output: tool.output,
        }),
    }
}

pub fn transcript_dto(transcript: Transcript) -> TranscriptDto {
    TranscriptDto {
        head: transcript.head.into_iter().map(message_dto).collect(),
        omitted: transcript.omitted,
        tail: transcript.tail.into_iter().map(message_dto).collect(),
        shortened: transcript.shortened,
    }
}

pub fn preview_item(catalog: &Catalog, preview: Preview) -> PreviewItem {
    let (files, permanent, command, refused_reason) = match preview.result {
        Ok(plan) => {
            let permanent = plan.is_permanent();
            let command = plan.command_line();
            (paths(plan.files), permanent, command, None)
        }
        Err(reason) => (Vec::new(), false, None, Some(reason)),
    };
    PreviewItem {
        uid: preview.uid,
        title: preview.session.as_ref().and_then(|session| session.title.clone()),
        agent_name: preview
            .session
            .as_ref()
            .map(|session| catalog.display_name(session.agent)),
        files,
        permanent,
        command,
        refused_reason,
    }
}

pub fn delete_result(uid: String, outcome: DeleteOutcome) -> DeleteResult {
    match outcome {
        DeleteOutcome::Trashed { files } => DeleteResult::Trashed {
            uid,
            files: paths(files),
        },
        DeleteOutcome::DeletedByAgent { files, command } => DeleteResult::Deleted {
            uid,
            files: paths(files),
            command,
        },
        DeleteOutcome::Refused { reason } => DeleteResult::Refused { uid, message: reason },
        DeleteOutcome::Failed { error } => DeleteResult::Failed { uid, message: error },
    }
}

pub fn archive_result(uid: String, outcome: ArchiveOutcome) -> ArchiveResult {
    match outcome {
        ArchiveOutcome::Done { command, archived } => ArchiveResult::Done { uid, archived, command },
        ArchiveOutcome::Refused { reason } => ArchiveResult::Refused { uid, message: reason },
        ArchiveOutcome::Failed { error } => ArchiveResult::Failed { uid, message: error },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn delete_result_serializes_with_status_tag() {
        let json =
            serde_json::to_value(delete_result("u".into(), DeleteOutcome::Refused { reason: "r".into() })).unwrap();
        assert_eq!(json, json!({"status": "refused", "uid": "u", "message": "r"}));
    }

    #[test]
    fn trashed_result_lists_files() {
        let outcome = DeleteOutcome::Trashed {
            files: vec![PathBuf::from("/a")],
        };
        let json = serde_json::to_value(delete_result("u".into(), outcome)).unwrap();
        assert_eq!(json, json!({"status": "trashed", "uid": "u", "files": ["/a"]}));
    }

    #[test]
    fn agent_delete_result_includes_command() {
        let outcome = DeleteOutcome::DeletedByAgent {
            files: vec![PathBuf::from("/a")],
            command: "codex delete --force x".into(),
        };
        let json = serde_json::to_value(delete_result("u".into(), outcome)).unwrap();
        assert_eq!(
            json,
            json!({"status": "deleted", "uid": "u", "files": ["/a"], "command": "codex delete --force x"})
        );
    }

    #[test]
    fn transcript_messages_serialize_roles_and_tools() {
        let transcript = Transcript {
            head: vec![Message {
                role: Role::Tool,
                text: "ls".into(),
                timestamp: None,
                tool: Some(marduk_core::ToolCall {
                    name: "bash".into(),
                    output: Some("a".into()),
                }),
            }],
            omitted: 3,
            tail: vec![],
            shortened: false,
        };
        let json = serde_json::to_value(transcript_dto(transcript)).unwrap();
        assert_eq!(
            json,
            json!({
                "head": [{"role": "tool", "text": "ls", "timestamp": null, "tool": {"name": "bash", "output": "a"}}],
                "omitted": 3,
                "tail": [],
                "shortened": false
            })
        );
    }

    #[test]
    fn archive_result_serializes_with_status_tag() {
        let outcome = ArchiveOutcome::Done {
            command: "codex archive x".into(),
            archived: true,
        };
        let json = serde_json::to_value(archive_result("u".into(), outcome)).unwrap();
        assert_eq!(
            json,
            json!({"status": "done", "uid": "u", "archived": true, "command": "codex archive x"})
        );
    }
}
