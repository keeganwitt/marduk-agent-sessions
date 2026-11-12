use std::path::PathBuf;
use std::time::SystemTime;

use marduk_core::catalog::{self, AgentScan, Catalog, Preview, ScanStatus};
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
    /// Set for database-backed agents, whose sessions have no files of their own.
    pub database: Option<String>,
    pub parent_id: Option<String>,
    /// The parent isn't in the scan (it was deleted), so this subagent stands on its own.
    pub parent_missing: bool,
    /// Whether the agent deletes this session along with its parent.
    pub deleted_with_parent: bool,
    /// Stored inside its parent session: shown and readable, but not separately changed.
    pub part_of_parent: bool,
    /// Started without a project; `project_dir` is a throwaway folder the agent made.
    pub projectless: bool,
    /// Models the session used, the current one last.
    pub models: Vec<String>,
    /// The agent definition the session ran as (custom agent, subagent type or role).
    pub agent_type: Option<String>,
    pub archived: bool,
    pub running: bool,
    pub read_only_reason: Option<String>,
    pub permanent_delete: bool,
    pub can_archive: bool,
    pub label_ids: Vec<i64>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabelDto {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub count: u64,
}

impl From<marduk_core::labels::Label> for LabelDto {
    fn from(label: marduk_core::labels::Label) -> Self {
        Self {
            id: label.id,
            name: label.name,
            color: label.color,
            count: label.count,
        }
    }
}

/// Drops labels of sessions missing from complete scans for longer than the grace period,
/// then attaches each session's labels to its row.
pub fn attach_labels(
    response: &mut ScanResponse,
    labels: &marduk_core::labels::LabelStore,
    now: SystemTime,
) -> Result<(), String> {
    let seen: std::collections::HashSet<String> = response
        .sessions
        .iter()
        .map(|row| row.key.clone())
        .collect();
    let complete: std::collections::HashSet<&str> = response
        .agents
        .iter()
        .filter(|agent| agent.complete)
        .map(|agent| agent.id)
        .collect();
    labels.reconcile(&seen, &complete, now, marduk_core::labels::PRUNE_GRACE)?;
    let assignments = labels.assignments()?;
    for row in &mut response.sessions {
        if let Some(ids) = assignments.get(&row.key) {
            row.label_ids = ids.clone();
        }
    }
    Ok(())
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileDetail {
    pub path: String,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub exists: bool,
}

/// Size and kind of session files, for paths that belong to scanned sessions only.
pub fn file_details(catalog: &Catalog, paths: &[String]) -> Vec<FileDetail> {
    paths
        .iter()
        .filter(|path| catalog.owns_path(std::path::Path::new(path)))
        .map(|path| {
            let path_buf = std::path::Path::new(path);
            let meta = std::fs::symlink_metadata(path_buf).ok();
            FileDetail {
                path: path.clone(),
                size_bytes: marduk_core::fs_util::path_size(path_buf),
                is_dir: meta.as_ref().is_some_and(|meta| meta.is_dir()),
                exists: meta.is_some(),
            }
        })
        .collect()
}

#[derive(Serialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressDto {
    pub done: u64,
    pub total: u64,
    pub agent: &'static str,
}

pub fn scan_progress(status: ScanStatus) -> ScanProgressDto {
    ScanProgressDto {
        done: status.done,
        total: status.total,
        agent: status.agent,
    }
}

#[derive(Clone, Serialize, Debug, PartialEq)]
pub struct TaskProgressDto {
    pub done: usize,
    pub total: usize,
}

impl From<marduk_core::catalog::TaskProgress> for TaskProgressDto {
    fn from(progress: marduk_core::catalog::TaskProgress) -> Self {
        Self {
            done: progress.done,
            total: progress.total,
        }
    }
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
    pub note: Option<String>,
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
    pub warning: Option<String>,
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
        database: session
            .files
            .is_empty()
            .then(|| session.primary_file.to_string_lossy().into_owned()),
        parent_id: session.parent_id.clone(),
        parent_missing: false,
        deleted_with_parent: session.deleted_with_parent,
        part_of_parent: session.part_of_parent,
        projectless: session.projectless,
        models: session.models.clone(),
        agent_type: session.agent_type.clone(),
        archived: session.archived,
        running: session.is_running(now, catalog.running_window()),
        read_only_reason: match &plan {
            Some(DeletePlan::ReadOnly { reason }) => Some(reason.clone()),
            _ => None,
        },
        permanent_delete: matches!(plan, Some(DeletePlan::AgentCommand { .. })),
        label_ids: Vec::new(),
        can_archive: catalog.can_archive(session),
    }
}

pub fn scan_response(catalog: &Catalog, scans: Vec<AgentScan>, now: SystemTime) -> ScanResponse {
    let mut sessions: Vec<SessionRow> = scans
        .iter()
        .flat_map(|scan| {
            scan.sessions
                .iter()
                .map(|session| session_row(catalog, session, now))
        })
        .collect();
    sessions.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
    mark_missing_parents(&mut sessions, &scans);
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

/// Only judged for agents whose scan was complete: an unreadable part of an agent's storage
/// could hold the parent.
fn mark_missing_parents(sessions: &mut [SessionRow], scans: &[AgentScan]) {
    let complete: std::collections::HashSet<&str> = scans
        .iter()
        .filter(|scan| scan.complete)
        .map(|scan| scan.agent)
        .collect();
    let present: std::collections::HashSet<(&str, String)> = sessions
        .iter()
        .map(|session| (session.agent, session.id.clone()))
        .collect();
    for session in sessions.iter_mut() {
        if let Some(parent) = &session.parent_id {
            session.parent_missing = complete.contains(session.agent)
                && !present.contains(&(session.agent, parent.clone()));
        }
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
        note: transcript.note,
    }
}

pub fn preview_item(catalog: &Catalog, preview: Preview) -> PreviewItem {
    let (files, permanent, command, warning, refused_reason) = match preview.result {
        Ok(plan) => {
            let permanent = plan.is_permanent();
            let command = plan.command_line();
            (paths(plan.files), permanent, command, plan.warning, None)
        }
        Err(reason) => (Vec::new(), false, None, None, Some(reason)),
    };
    PreviewItem {
        uid: preview.uid,
        title: preview
            .session
            .as_ref()
            .and_then(|session| session.title.clone()),
        agent_name: preview
            .session
            .as_ref()
            .map(|session| catalog.display_name(session.agent)),
        files,
        permanent,
        command,
        warning,
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
        DeleteOutcome::Refused { reason } => DeleteResult::Refused {
            uid,
            message: reason,
        },
        DeleteOutcome::Failed { error } => DeleteResult::Failed {
            uid,
            message: error,
        },
    }
}

pub fn archive_result(uid: String, outcome: ArchiveOutcome) -> ArchiveResult {
    match outcome {
        ArchiveOutcome::Done { command, archived } => ArchiveResult::Done {
            uid,
            archived,
            command,
        },
        ArchiveOutcome::Refused { reason } => ArchiveResult::Refused {
            uid,
            message: reason,
        },
        ArchiveOutcome::Failed { error } => ArchiveResult::Failed {
            uid,
            message: error,
        },
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentHelpDto {
    pub id: &'static str,
    pub name: &'static str,
    pub roots: Vec<String>,
    pub delete: String,
    pub archive: String,
}

impl From<marduk_core::catalog::AgentHelp> for AgentHelpDto {
    fn from(help: marduk_core::catalog::AgentHelp) -> Self {
        Self {
            id: help.id,
            name: help.name,
            roots: paths(help.roots),
            delete: help.mechanics.delete,
            archive: help.mechanics.archive,
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFileDto {
    pub path: String,
    pub title: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub session_id: Option<String>,
    pub modified: Option<String>,
    pub size: u64,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPurgeDto {
    pub agent: &'static str,
    pub agent_name: &'static str,
    pub folder: String,
    pub project_dir: String,
    pub command: String,
}

pub fn project_purge(catalog: &Catalog, purge: marduk_core::ProjectPurge) -> ProjectPurgeDto {
    ProjectPurgeDto {
        agent: purge.agent,
        agent_name: catalog.display_name(purge.agent),
        folder: purge.folder.to_string_lossy().into_owned(),
        project_dir: purge.project_dir,
        command: purge.command.command_line(),
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStoreDto {
    pub agent: &'static str,
    pub agent_name: &'static str,
    pub folder: String,
    pub project_dir: Option<String>,
    pub project_exists: Option<bool>,
    pub sessions: usize,
    pub files: Vec<MemoryFileDto>,
    pub problems: Vec<String>,
    pub note: Option<String>,
    pub can_trash: bool,
}

impl From<marduk_core::memory::MemoryStore> for MemoryStoreDto {
    fn from(store: marduk_core::memory::MemoryStore) -> Self {
        Self {
            agent: store.agent,
            agent_name: store.agent_name,
            folder: store.folder.to_string_lossy().into_owned(),
            project_dir: store.project_dir,
            project_exists: store.project_exists,
            sessions: store.sessions,
            files: store
                .files
                .into_iter()
                .map(|file| MemoryFileDto {
                    path: file.path.to_string_lossy().into_owned(),
                    title: file.title,
                    description: file.description,
                    kind: file.kind,
                    session_id: file.session_id,
                    modified: file.modified.map(|modified| modified.to_rfc3339()),
                    size: file.size,
                })
                .collect(),
            problems: store.problems,
            note: store.note,
            can_trash: store.can_trash,
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn task_progress_serializes_done_and_total() {
        let dto = TaskProgressDto::from(marduk_core::catalog::TaskProgress { done: 1, total: 3 });
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            serde_json::json!({"done": 1, "total": 3})
        );
    }

    #[test]
    fn memory_store_carries_files_and_flags() {
        let dto = MemoryStoreDto::from(marduk_core::memory::MemoryStore {
            agent: "claude",
            agent_name: "Claude Code",
            folder: PathBuf::from("/c/projects/-w/memory"),
            project_dir: Some("/w".into()),
            project_exists: Some(false),
            sessions: 0,
            files: vec![marduk_core::memory::MemoryFile {
                path: PathBuf::from("/c/projects/-w/memory/git.md"),
                title: "git".into(),
                description: None,
                kind: Some("feedback".into()),
                session_id: Some("s1".into()),
                modified: None,
                size: 12,
            }],
            problems: vec!["gone".into()],
            note: None,
            can_trash: true,
        });

        assert_eq!(dto.folder, "/c/projects/-w/memory");
        assert_eq!(dto.files[0].path, "/c/projects/-w/memory/git.md");
        assert_eq!(dto.files[0].session_id.as_deref(), Some("s1"));
        assert!(dto.can_trash);
    }

    #[test]
    fn project_purge_names_its_agent_and_command() {
        let catalog = Catalog::new(
            vec![Box::new(marduk_core::agents::claude::ClaudeAdapter::new(
                PathBuf::from("/c"),
            ))],
            marduk_core::RUNNING_WINDOW,
        );

        let dto = project_purge(
            &catalog,
            marduk_core::ProjectPurge {
                agent: "claude",
                folder: PathBuf::from("/c/projects/-w"),
                project_dir: "/w".into(),
                command: marduk_core::AgentCommand {
                    program: PathBuf::from("/bin/claude"),
                    args: vec!["purge".into(), "-y".into(), "/w".into()],
                },
            },
        );

        assert_eq!(dto.agent_name, "Claude Code");
        assert_eq!(dto.folder, "/c/projects/-w");
        assert_eq!(dto.command, "/bin/claude purge -y /w");
    }

    #[test]
    fn agent_help_carries_folders_and_mechanics() {
        let dto = AgentHelpDto::from(marduk_core::catalog::AgentHelp {
            id: "pi",
            name: "Pi",
            roots: vec![PathBuf::from("/home/me/.pi/agent/sessions")],
            mechanics: marduk_core::Mechanics {
                delete: "Trash".into(),
                archive: "None".into(),
            },
        });

        assert_eq!(dto.roots, vec!["/home/me/.pi/agent/sessions".to_string()]);
        assert_eq!(
            (dto.delete.as_str(), dto.archive.as_str()),
            ("Trash", "None")
        );
    }

    use serde_json::json;

    use super::*;

    fn row(agent: &'static str, id: &str, parent: Option<&str>) -> SessionRow {
        SessionRow {
            uid: format!("{agent}:{id}"),
            key: format!("{agent}:{id}"),
            agent,
            agent_name: agent,
            id: id.into(),
            title: None,
            project_dir: None,
            created: None,
            last_activity: String::new(),
            size_bytes: 0,
            files: vec![],
            database: None,
            parent_id: parent.map(str::to_string),
            parent_missing: false,
            deleted_with_parent: false,
            part_of_parent: false,
            projectless: false,
            models: vec![],
            agent_type: None,
            archived: false,
            running: false,
            read_only_reason: None,
            permanent_delete: false,
            can_archive: false,
            label_ids: vec![],
        }
    }

    fn scan(agent: &'static str, complete: bool) -> AgentScan {
        AgentScan {
            agent,
            display_name: agent,
            complete,
            errors: vec![],
            sessions: vec![],
        }
    }

    #[test]
    fn file_details_cover_only_files_of_scanned_sessions() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("--p--");
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("t_a.jsonl");
        std::fs::write(&file, "12345").unwrap();
        let catalog = Catalog::new(
            vec![Box::new(marduk_core::agents::pi::PiAdapter::new(
                root.path().to_path_buf(),
            ))],
            marduk_core::RUNNING_WINDOW,
        );
        catalog.scan();
        let outside = root.path().join("secret.txt");
        std::fs::write(&outside, "x").unwrap();

        let details = file_details(
            &catalog,
            &[
                file.to_string_lossy().into_owned(),
                outside.to_string_lossy().into_owned(),
            ],
        );

        assert_eq!(
            details,
            vec![FileDetail {
                path: file.to_string_lossy().into_owned(),
                size_bytes: 5,
                is_dir: false,
                exists: true,
            }]
        );
    }

    #[test]
    fn attaches_labels_to_rows_by_session_key() {
        let labels = marduk_core::labels::LabelStore::in_memory().unwrap();
        let work = labels.create("Work", "blue").unwrap();
        let now = SystemTime::now();
        labels
            .apply(
                &["codex:a".to_string(), "codex:gone".to_string()],
                &[work.id],
                &[],
                now,
            )
            .unwrap();
        let mut response = ScanResponse {
            agents: vec![AgentSummary {
                id: "codex",
                name: "Codex",
                complete: true,
                errors: vec![],
                session_count: 2,
            }],
            sessions: vec![row("codex", "a", None), row("codex", "b", None)],
            home_dir: String::new(),
        };

        attach_labels(&mut response, &labels, now).unwrap();

        assert_eq!(response.sessions[0].label_ids, vec![work.id]);
        assert!(response.sessions[1].label_ids.is_empty());
        let later = now + marduk_core::labels::PRUNE_GRACE + std::time::Duration::from_secs(60);
        attach_labels(&mut response, &labels, later).unwrap();
        assert_eq!(
            labels.labels().unwrap()[0].count,
            1,
            "the missing session's label was pruned"
        );
    }

    #[test]
    fn marks_subagents_whose_parent_is_gone_only_for_complete_scans() {
        let mut sessions = vec![
            row("codex", "parent", None),
            row("codex", "child", Some("parent")),
            row("codex", "orphan", Some("deleted")),
            row("opencode", "unsure", Some("deleted")),
            row("opencode", "other-agent-parent", Some("parent")),
        ];

        mark_missing_parents(
            &mut sessions,
            &[scan("codex", true), scan("opencode", false)],
        );

        let missing: Vec<&str> = sessions
            .iter()
            .filter(|session| session.parent_missing)
            .map(|session| session.id.as_str())
            .collect();
        assert_eq!(missing, vec!["orphan"]);
    }

    #[test]
    fn delete_result_serializes_with_status_tag() {
        let json = serde_json::to_value(delete_result(
            "u".into(),
            DeleteOutcome::Refused { reason: "r".into() },
        ))
        .unwrap();
        assert_eq!(
            json,
            json!({"status": "refused", "uid": "u", "message": "r"})
        );
    }

    #[test]
    fn trashed_result_lists_files() {
        let outcome = DeleteOutcome::Trashed {
            files: vec![PathBuf::from("/a")],
        };
        let json = serde_json::to_value(delete_result("u".into(), outcome)).unwrap();
        assert_eq!(
            json,
            json!({"status": "trashed", "uid": "u", "files": ["/a"]})
        );
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
            note: None,
        };
        let json = serde_json::to_value(transcript_dto(transcript)).unwrap();
        assert_eq!(
            json,
            json!({
                "head": [{"role": "tool", "text": "ls", "timestamp": null, "tool": {"name": "bash", "output": "a"}}],
                "omitted": 3,
                "tail": [],
                "shortened": false,
                "note": null
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
