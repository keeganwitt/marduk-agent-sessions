use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use crate::adapter::{AgentAdapter, DeletePlan};
use crate::delete::{self, ArchiveOutcome, DeleteBackend, DeleteOutcome, PlannedDelete};
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
    pub result: Result<PlannedDelete, String>,
}

/// Holds the adapters and the most recent scan. Deletes are resolved by uid against that
/// scan, so callers never supply file paths.
pub struct Catalog {
    adapters: Vec<Box<dyn AgentAdapter>>,
    running_window: Duration,
    snapshot: Mutex<HashMap<String, (usize, Session)>>,
}

/// Unique per stored copy of a session, unlike `Session::key`, which two copies share.
/// Includes the id because database-backed agents keep every session in one file.
pub fn uid(session: &Session) -> String {
    format!(
        "{}:{}#{}",
        session.agent,
        session.primary_file.to_string_lossy(),
        session.id
    )
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

    fn adapter_for(&self, session: &Session) -> Option<&dyn AgentAdapter> {
        self.adapters
            .iter()
            .find(|adapter| adapter.id() == session.agent)
            .map(|adapter| adapter.as_ref())
    }

    pub fn delete_plan(&self, session: &Session) -> Option<DeletePlan> {
        self.adapter_for(session).map(|adapter| adapter.delete_plan(session))
    }

    pub fn can_archive(&self, session: &Session) -> bool {
        self.adapter_for(session)
            .is_some_and(|adapter| adapter.archive_command(session).is_some())
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

    pub fn delete(
        &self,
        uids: &[String],
        backend: &dyn DeleteBackend,
        now: SystemTime,
    ) -> Vec<(String, DeleteOutcome)> {
        let mut snapshot = self.snapshot.lock().unwrap();
        uids.iter()
            .map(|uid| {
                let outcome = match snapshot.get(uid) {
                    None => DeleteOutcome::Refused { reason: not_found() },
                    Some((index, session)) => delete::execute(
                        self.adapters[*index].as_ref(),
                        session,
                        backend,
                        now,
                        self.running_window,
                    ),
                };
                if matches!(
                    outcome,
                    DeleteOutcome::Trashed { .. } | DeleteOutcome::DeletedByAgent { .. }
                ) {
                    snapshot.remove(uid);
                }
                (uid.clone(), outcome)
            })
            .collect()
    }

    /// The project folder a scanned session was run in, if it still exists.
    pub fn project_dir(&self, uid: &str) -> Result<std::path::PathBuf, String> {
        let snapshot = self.snapshot.lock().unwrap();
        let (_, session) = snapshot.get(uid).ok_or_else(not_found)?;
        let dir = session
            .project_dir
            .as_ref()
            .map(std::path::PathBuf::from)
            .ok_or_else(|| "This session doesn't record its project folder.".to_string())?;
        if dir.is_dir() {
            Ok(dir)
        } else {
            Err(format!("The project folder {} no longer exists.", dir.display()))
        }
    }

    pub fn transcript(&self, uid: &str) -> Result<crate::transcript::Transcript, String> {
        let (index, session) = self.snapshot.lock().unwrap().get(uid).cloned().ok_or_else(not_found)?;
        self.adapters[index]
            .transcript(&session)
            .map_err(|error| format!("Couldn't read {}: {error}", session.primary_file.display()))
    }

    /// True when `path` is one of the files of a session in the current scan. Lets the UI
    /// reveal session files without being able to point at arbitrary paths.
    pub fn owns_path(&self, path: &std::path::Path) -> bool {
        self.snapshot
            .lock()
            .unwrap()
            .values()
            .any(|(_, session)| session.primary_file == path || session.files.iter().any(|file| file == path))
    }

    /// Archives or unarchives each session with its agent's own command. Moved sessions
    /// leave the snapshot, so callers should rescan to see them at their new location.
    pub fn toggle_archive(
        &self,
        uids: &[String],
        backend: &dyn DeleteBackend,
        now: SystemTime,
    ) -> Vec<(String, ArchiveOutcome)> {
        let mut snapshot = self.snapshot.lock().unwrap();
        uids.iter()
            .map(|uid| {
                let outcome = match snapshot.get(uid) {
                    None => ArchiveOutcome::Refused { reason: not_found() },
                    Some((index, session)) => delete::toggle_archive(
                        self.adapters[*index].as_ref(),
                        session,
                        backend,
                        now,
                        self.running_window,
                    ),
                };
                if matches!(outcome, ArchiveOutcome::Done { .. }) {
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
    impl DeleteBackend for NoopTrash {
        fn trash(&self, _paths: &[std::path::PathBuf]) -> Result<(), String> {
            Ok(())
        }
        fn run(&self, _command: &crate::adapter::AgentCommand) -> Result<(), String> {
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
    fn owns_only_scanned_session_files() {
        let (root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let file = scans[0].sessions[0].primary_file.clone();
        assert!(catalog.owns_path(&file));
        assert!(!catalog.owns_path(root.path()));
        assert!(!catalog.owns_path(std::path::Path::new("/etc/passwd")));
    }

    #[test]
    fn project_dir_resolves_only_existing_folders_of_scanned_sessions() {
        let root = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let dir = root.path().join("--p--");
        fs::create_dir(&dir).unwrap();
        for (name, cwd) in [
            ("t_a.jsonl", project.path().to_str().unwrap()),
            ("t_b.jsonl", "/does/not/exist"),
        ] {
            fs::write(
                dir.join(name),
                format!(r#"{{"type":"session","id":"{name}","cwd":"{cwd}"}}"#),
            )
            .unwrap();
        }
        let catalog = Catalog::new(
            vec![Box::new(PiAdapter::new(root.path().to_path_buf()))],
            Duration::from_secs(60),
        );
        let scans = catalog.scan();
        let uid_of = |name: &str| uid(scans[0].sessions.iter().find(|s| s.id == name).unwrap());

        assert_eq!(catalog.project_dir(&uid_of("t_a.jsonl")).unwrap(), project.path());
        assert!(
            catalog
                .project_dir(&uid_of("t_b.jsonl"))
                .unwrap_err()
                .contains("no longer exists")
        );
        assert!(
            catalog
                .project_dir("pi:/nope")
                .unwrap_err()
                .contains("not in the current scan")
        );
    }

    #[test]
    fn pi_sessions_cannot_be_archived() {
        let (_root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let session = &scans[0].sessions[0];
        assert!(!catalog.can_archive(session));
        let outcome = catalog.toggle_archive(&[uid(session)], &NoopTrash, SystemTime::now());
        assert!(matches!(&outcome[0].1, ArchiveOutcome::Refused { reason } if reason.contains("can't archive")));
    }

    #[test]
    fn preview_lists_files_for_known_uid() {
        let (_root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let session = &scans[0].sessions[0];
        let preview = catalog.preview(&[uid(session)], SystemTime::now());
        assert_eq!(&preview[0].result.as_ref().unwrap().files, &session.files);
    }
}
