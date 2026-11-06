use std::path::PathBuf;
use std::time::SystemTime;

use marduk_core::catalog::{self, AgentScan, Catalog, Preview};
use marduk_core::{DeleteOutcome, Session};
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
    pub last_activity: String,
    pub size_bytes: u64,
    pub archived: bool,
    pub running: bool,
    pub read_only_reason: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub agents: Vec<AgentSummary>,
    pub sessions: Vec<SessionRow>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub uid: String,
    pub title: Option<String>,
    pub agent_name: Option<&'static str>,
    pub files: Vec<String>,
    pub refused_reason: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum DeleteResult {
    Trashed { uid: String, files: Vec<String> },
    Refused { uid: String, message: String },
    Failed { uid: String, message: String },
}

fn paths(files: Vec<PathBuf>) -> Vec<String> {
    files
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn session_row(catalog: &Catalog, session: &Session, now: SystemTime) -> SessionRow {
    SessionRow {
        uid: catalog::uid(session),
        key: session.key(),
        agent: session.agent,
        agent_name: catalog.display_name(session.agent),
        id: session.id.clone(),
        title: session.title.clone(),
        project_dir: session.project_dir.clone(),
        last_activity: session.last_activity.to_rfc3339(),
        size_bytes: session.size_bytes,
        archived: session.archived,
        running: session.is_running(now, catalog.running_window()),
        read_only_reason: catalog.read_only_reason(session),
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
    ScanResponse { agents, sessions }
}

pub fn preview_item(catalog: &Catalog, preview: Preview) -> PreviewItem {
    let (files, refused_reason) = match preview.result {
        Ok(files) => (paths(files), None),
        Err(reason) => (Vec::new(), Some(reason)),
    };
    PreviewItem {
        uid: preview.uid,
        title: preview.session.as_ref().and_then(|session| session.title.clone()),
        agent_name: preview
            .session
            .as_ref()
            .map(|session| catalog.display_name(session.agent)),
        files,
        refused_reason,
    }
}

pub fn delete_result(uid: String, outcome: DeleteOutcome) -> DeleteResult {
    match outcome {
        DeleteOutcome::Trashed { files } => DeleteResult::Trashed {
            uid,
            files: paths(files),
        },
        DeleteOutcome::Refused { reason } => DeleteResult::Refused { uid, message: reason },
        DeleteOutcome::Failed { error } => DeleteResult::Failed { uid, message: error },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_result_serializes_with_status_tag() {
        let json =
            serde_json::to_value(delete_result("u".into(), DeleteOutcome::Refused { reason: "r".into() })).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"status": "refused", "uid": "u", "message": "r"})
        );
    }

    #[test]
    fn trashed_result_lists_files() {
        let json = serde_json::to_value(delete_result(
            "u".into(),
            DeleteOutcome::Trashed {
                files: vec![PathBuf::from("/a")],
            },
        ))
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"status": "trashed", "uid": "u", "files": ["/a"]})
        );
    }
}
