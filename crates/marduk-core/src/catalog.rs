use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::adapter::{AgentAdapter, DeletePlan};
use crate::delete::{self, DeleteOutcome, Trasher};
use crate::session::Session;

pub struct AgentScan {
    pub agent: &'static str,
    pub display_name: &'static str,
    pub complete: bool,
    pub errors: Vec<String>,
    pub sessions: Vec<Session>,
}

pub struct Preview {
    pub uid: String,
    pub session: Option<Session>,
    pub result: Result<Vec<PathBuf>, String>,
}

/// Holds the adapters and the most recent scan. Deletes are resolved by uid against that
/// scan, so callers never supply file paths.
pub struct Catalog {
    adapters: Vec<Box<dyn AgentAdapter>>,
    running_window: Duration,
    snapshot: Mutex<HashMap<String, (usize, Session)>>,
}

/// Unique per transcript file, unlike `Session::key`, which two copies of a session share.
pub fn uid(session: &Session) -> String {
    format!("{}:{}", session.agent, session.primary_file.to_string_lossy())
}

impl Catalog {
    pub fn new(adapters: Vec<Box<dyn AgentAdapter>>, running_window: Duration) -> Self {
        Self {
            adapters,
            running_window,
            snapshot: Mutex::new(HashMap::new()),
        }
    }

    pub fn running_window(&self) -> Duration {
        self.running_window
    }

    pub fn scan(&self) -> Vec<AgentScan> {
        let mut snapshot = HashMap::new();
        let scans = self
            .adapters
            .iter()
            .enumerate()
            .map(|(index, adapter)| {
                let result = adapter.scan();
                for session in &result.sessions {
                    snapshot.insert(uid(session), (index, session.clone()));
                }
                AgentScan {
                    agent: adapter.id(),
                    display_name: adapter.display_name(),
                    complete: result.complete,
                    errors: result.errors,
                    sessions: result.sessions,
                }
            })
            .collect();
        *self.snapshot.lock().unwrap() = snapshot;
        scans
    }

    pub fn display_name(&self, agent: &str) -> &'static str {
        self.adapters
            .iter()
            .find(|adapter| adapter.id() == agent)
            .map_or("Unknown", |adapter| adapter.display_name())
    }

    pub fn read_only_reason(&self, session: &Session) -> Option<String> {
        let adapter = self.adapters.iter().find(|adapter| adapter.id() == session.agent)?;
        match adapter.delete_plan(session) {
            DeletePlan::ReadOnly { reason } => Some(reason),
            DeletePlan::Trash { .. } => None,
        }
    }

    pub fn preview(&self, uids: &[String], now: SystemTime) -> Vec<Preview> {
        let snapshot = self.snapshot.lock().unwrap();
        uids.iter()
            .map(|uid| match snapshot.get(uid) {
                None => Preview {
                    uid: uid.clone(),
                    session: None,
                    result: Err(not_found()),
                },
                Some((index, session)) => Preview {
                    uid: uid.clone(),
                    session: Some(session.clone()),
                    result: delete::preflight(self.adapters[*index].as_ref(), session, now, self.running_window),
                },
            })
            .collect()
    }

    pub fn delete(&self, uids: &[String], trasher: &dyn Trasher, now: SystemTime) -> Vec<(String, DeleteOutcome)> {
        let mut snapshot = self.snapshot.lock().unwrap();
        uids.iter()
            .map(|uid| {
                let outcome = match snapshot.get(uid) {
                    None => DeleteOutcome::Refused { reason: not_found() },
                    Some((index, session)) => delete::execute(
                        self.adapters[*index].as_ref(),
                        session,
                        trasher,
                        now,
                        self.running_window,
                    ),
                };
                if matches!(outcome, DeleteOutcome::Trashed { .. }) {
                    snapshot.remove(uid);
                }
                (uid.clone(), outcome)
            })
            .collect()
    }
}

fn not_found() -> String {
    "This session is not in the current scan. Rescan and try again.".to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::agents::pi::PiAdapter;

    struct NoopTrash;
    impl Trasher for NoopTrash {
        fn trash(&self, _paths: &[PathBuf]) -> Result<(), String> {
            Ok(())
        }
    }

    fn catalog_with_old_pi_session() -> (tempfile::TempDir, Catalog) {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("--p--");
        fs::create_dir(&dir).unwrap();
        let file = dir.join("t_abc.jsonl");
        fs::write(&file, r#"{"type":"session","id":"abc"}"#).unwrap();
        let old = filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        filetime::set_file_mtime(&file, old).unwrap();
        let catalog = Catalog::new(
            vec![Box::new(PiAdapter::new(root.path().to_path_buf()))],
            Duration::from_secs(60),
        );
        (root, catalog)
    }

    #[test]
    fn scan_then_delete_by_uid_removes_from_snapshot() {
        let (_root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let id = uid(&scans[0].sessions[0]);

        let first = catalog.delete(std::slice::from_ref(&id), &NoopTrash, SystemTime::now());
        assert!(matches!(first[0].1, DeleteOutcome::Trashed { .. }));

        let second = catalog.delete(&[id], &NoopTrash, SystemTime::now());
        assert!(
            matches!(&second[0].1, DeleteOutcome::Refused { reason } if reason.contains("not in the current scan"))
        );
    }

    #[test]
    fn unknown_uid_is_refused_in_preview() {
        let (_root, catalog) = catalog_with_old_pi_session();
        catalog.scan();
        let preview = catalog.preview(&["pi:/etc/passwd".into()], SystemTime::now());
        assert!(preview[0].session.is_none());
        assert!(preview[0].result.is_err());
    }

    #[test]
    fn preview_lists_files_for_known_uid() {
        let (_root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let session = &scans[0].sessions[0];
        let preview = catalog.preview(&[uid(session)], SystemTime::now());
        assert_eq!(preview[0].result.as_ref().unwrap(), &session.files);
    }
}
