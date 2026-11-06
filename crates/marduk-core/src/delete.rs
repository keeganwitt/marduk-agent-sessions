use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::adapter::{AgentAdapter, DeletePlan};
use crate::fs_util;
use crate::session::Session;

pub trait Trasher {
    fn trash(&self, paths: &[PathBuf]) -> Result<(), String>;
}

pub struct SystemTrash;

impl Trasher for SystemTrash {
    fn trash(&self, paths: &[PathBuf]) -> Result<(), String> {
        #[allow(unused_mut)]
        let mut context = trash::TrashContext::default();
        // The default Finder method makes macOS prompt for permission to automate Finder.
        #[cfg(target_os = "macos")]
        {
            use trash::macos::{DeleteMethod, TrashContextExtMacos};
            context.set_delete_method(DeleteMethod::NsFileManager);
        }
        context
            .delete_all(paths)
            .map_err(|error| format!("Could not move to Trash: {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Trashed { files: Vec<PathBuf> },
    Refused { reason: String },
    Failed { error: String },
}

/// Checks everything that must hold before a session may be deleted and returns the
/// files that would be moved to the Trash.
pub fn preflight(
    adapter: &dyn AgentAdapter,
    session: &Session,
    now: SystemTime,
    running_window: Duration,
) -> Result<Vec<PathBuf>, String> {
    let files = match adapter.delete_plan(session) {
        DeletePlan::ReadOnly { reason } => return Err(reason),
        DeletePlan::Trash { files } => files,
    };

    let current = fs_util::fingerprint(&session.primary_file)
        .map_err(|_| "The session no longer exists. Rescan to refresh the list.".to_string())?;
    if current != session.fingerprint {
        return Err("The session changed since it was scanned. Rescan and try again.".to_string());
    }
    if current.is_recent(now, running_window) {
        return Err("The session looks like it is still running.".to_string());
    }

    let roots = adapter.roots();
    let existing: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| path == &session.primary_file || path.exists())
        .collect();
    if let Some(escaping) = existing.iter().find(|path| !fs_util::is_strictly_within(path, &roots)) {
        return Err(format!(
            "Refusing to delete {} because it is outside {}'s session folders.",
            escaping.display(),
            adapter.display_name()
        ));
    }
    Ok(existing)
}

pub fn execute(
    adapter: &dyn AgentAdapter,
    session: &Session,
    trasher: &dyn Trasher,
    now: SystemTime,
    running_window: Duration,
) -> DeleteOutcome {
    match preflight(adapter, session, now, running_window) {
        Err(reason) => DeleteOutcome::Refused { reason },
        Ok(files) => match trasher.trash(&files) {
            Ok(()) => DeleteOutcome::Trashed { files },
            Err(error) => DeleteOutcome::Failed { error },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::Path;

    use chrono::Utc;

    use super::*;
    use crate::adapter::ScanResult;

    struct FakeAdapter {
        root: PathBuf,
        plan: DeletePlan,
    }

    impl AgentAdapter for FakeAdapter {
        fn id(&self) -> &'static str {
            "fake"
        }
        fn display_name(&self) -> &'static str {
            "Fake"
        }
        fn roots(&self) -> Vec<PathBuf> {
            vec![self.root.clone()]
        }
        fn scan(&self) -> ScanResult {
            ScanResult::empty_complete()
        }
        fn delete_plan(&self, _session: &Session) -> DeletePlan {
            self.plan.clone()
        }
    }

    #[derive(Default)]
    struct RecordingTrash {
        calls: RefCell<Vec<Vec<PathBuf>>>,
        fail: bool,
    }

    impl Trasher for RecordingTrash {
        fn trash(&self, paths: &[PathBuf]) -> Result<(), String> {
            self.calls.borrow_mut().push(paths.to_vec());
            if self.fail { Err("no trash here".into()) } else { Ok(()) }
        }
    }

    fn old_session(path: &Path) -> Session {
        let hour_ago = filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        filetime::set_file_mtime(path, hour_ago).unwrap();
        Session {
            agent: "fake",
            id: "s1".into(),
            project_dir: None,
            title: None,
            last_activity: Utc::now(),
            size_bytes: 1,
            archived: false,
            primary_file: path.to_path_buf(),
            files: vec![path.to_path_buf()],
            fingerprint: fs_util::fingerprint(path).unwrap(),
        }
    }

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let file = root.join("s1.jsonl");
        fs::write(&file, b"{}\n").unwrap();
        (dir, root, file)
    }

    fn trash_plan(files: &[&Path]) -> DeletePlan {
        DeletePlan::Trash {
            files: files.iter().map(|path| path.to_path_buf()).collect(),
        }
    }

    #[test]
    fn trashes_existing_files() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file, &root.join("gone")]),
            root,
        };
        let trash = RecordingTrash::default();

        let outcome = execute(&adapter, &session, &trash, SystemTime::now(), Duration::from_secs(60));

        assert_eq!(
            outcome,
            DeleteOutcome::Trashed {
                files: vec![file.clone()]
            }
        );
        assert_eq!(*trash.calls.borrow(), vec![vec![file]]);
    }

    #[test]
    fn refuses_read_only_plan() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            root,
            plan: DeletePlan::ReadOnly { reason: "nope".into() },
        };
        let trash = RecordingTrash::default();

        let outcome = execute(&adapter, &session, &trash, SystemTime::now(), Duration::from_secs(60));

        assert_eq!(outcome, DeleteOutcome::Refused { reason: "nope".into() });
        assert!(trash.calls.borrow().is_empty());
    }

    #[test]
    fn refuses_when_file_changed_since_scan() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        fs::write(&file, b"{}\n{}\n").unwrap();
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
        };

        let result = preflight(&adapter, &session, SystemTime::now(), Duration::from_secs(60));

        assert!(result.unwrap_err().contains("changed since it was scanned"));
    }

    #[test]
    fn refuses_running_session() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
        };

        let result = preflight(&adapter, &session, SystemTime::now(), Duration::from_secs(7200));

        assert!(result.unwrap_err().contains("still running"));
    }

    #[test]
    fn refuses_missing_session() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        fs::remove_file(&file).unwrap();
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
        };

        let result = preflight(&adapter, &session, SystemTime::now(), Duration::from_secs(60));

        assert!(result.unwrap_err().contains("no longer exists"));
    }

    #[test]
    fn refuses_files_outside_roots() {
        let (dir, root, file) = setup();
        let outside = dir.path().join("precious.txt");
        fs::write(&outside, b"keep me").unwrap();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file, &outside]),
            root,
        };
        let trash = RecordingTrash::default();

        let outcome = execute(&adapter, &session, &trash, SystemTime::now(), Duration::from_secs(60));

        assert!(matches!(outcome, DeleteOutcome::Refused { reason } if reason.contains("outside")));
        assert!(trash.calls.borrow().is_empty());
    }

    #[test]
    fn reports_trash_failure_without_fallback() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
        };
        let trash = RecordingTrash {
            fail: true,
            ..Default::default()
        };

        let outcome = execute(&adapter, &session, &trash, SystemTime::now(), Duration::from_secs(60));

        assert_eq!(
            outcome,
            DeleteOutcome::Failed {
                error: "no trash here".into()
            }
        );
        assert!(file.exists());
    }

    #[test]
    #[ignore = "moves a temp file to the real system Trash"]
    fn system_trash_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("marduk-trash-test.txt");
        fs::write(&file, b"x").unwrap();
        SystemTrash.trash(std::slice::from_ref(&file)).unwrap();
        assert!(!file.exists());
    }
}
