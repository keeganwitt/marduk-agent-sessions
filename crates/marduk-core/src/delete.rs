use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::adapter::{AgentAdapter, AgentCommand, DeletePlan};
use crate::fs_util;
use crate::session::Session;

const AGENT_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

pub trait DeleteBackend {
    fn trash(&self, paths: &[PathBuf]) -> Result<(), String>;
    fn run(&self, command: &AgentCommand) -> Result<(), String>;
}

pub struct SystemBackend;

impl DeleteBackend for SystemBackend {
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

    fn run(&self, command: &AgentCommand) -> Result<(), String> {
        let args: Vec<&str> = command.args.iter().map(String::as_str).collect();
        crate::agents::cli::run_capture(&command.program, &args, AGENT_COMMAND_TIMEOUT).map(|_| ())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Method {
    Trash,
    AgentCommand(AgentCommand),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedDelete {
    pub files: Vec<PathBuf>,
    pub method: Method,
    pub warning: Option<String>,
}

impl PlannedDelete {
    pub fn is_permanent(&self) -> bool {
        matches!(self.method, Method::AgentCommand { .. })
    }

    pub fn command_line(&self) -> Option<String> {
        match &self.method {
            Method::Trash => None,
            Method::AgentCommand(command) => Some(command.command_line()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Trashed { files: Vec<PathBuf> },
    DeletedByAgent { files: Vec<PathBuf>, command: String },
    Refused { reason: String },
    Failed { error: String },
}

/// Checks everything that must hold before a session may be deleted and returns how it
/// would be deleted.
pub fn preflight(
    adapter: &dyn AgentAdapter,
    session: &Session,
    now: SystemTime,
    running_window: Duration,
) -> Result<PlannedDelete, String> {
    let (files, method, warning) = match adapter.delete_plan(session) {
        DeletePlan::ReadOnly { reason } => return Err(reason),
        DeletePlan::Trash { files, warning } => (files, Method::Trash, warning),
        DeletePlan::AgentCommand {
            command,
            files,
            warning,
        } => (files, Method::AgentCommand(command), warning),
    };

    check_unchanged_and_idle(adapter, session, now, running_window)?;

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
    Ok(PlannedDelete {
        files: existing,
        method,
        warning,
    })
}

fn check_unchanged_and_idle(
    adapter: &dyn AgentAdapter,
    session: &Session,
    now: SystemTime,
    running_window: Duration,
) -> Result<(), String> {
    let current = adapter
        .fingerprint(session)
        .map_err(|_| "The session no longer exists. Rescan to refresh the list.".to_string())?;
    if current != session.fingerprint {
        return Err("The session changed since it was scanned. Rescan and try again.".to_string());
    }
    if current.is_recent(now, running_window) {
        return Err("The session looks like it is still running.".to_string());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveOutcome {
    Done { command: String, archived: bool },
    Refused { reason: String },
    Failed { error: String },
}

/// Archives an active session, or unarchives an archived one, with the agent's own command.
pub fn toggle_archive(
    adapter: &dyn AgentAdapter,
    session: &Session,
    backend: &dyn DeleteBackend,
    now: SystemTime,
    running_window: Duration,
) -> ArchiveOutcome {
    let Some(command) = adapter.archive_command(session) else {
        return ArchiveOutcome::Refused {
            reason: format!("{} can't archive this session.", adapter.display_name()),
        };
    };
    if let Err(reason) = check_unchanged_and_idle(adapter, session, now, running_window) {
        return ArchiveOutcome::Refused { reason };
    }
    match backend.run(&command) {
        Err(error) => ArchiveOutcome::Failed { error },
        Ok(()) if adapter.still_present(session) => ArchiveOutcome::Failed {
            error: format!(
                "{} reported success but {} did not move.",
                adapter.display_name(),
                session.primary_file.display()
            ),
        },
        Ok(()) => ArchiveOutcome::Done {
            command: command.command_line(),
            archived: !session.archived,
        },
    }
}

pub fn execute(
    adapter: &dyn AgentAdapter,
    session: &Session,
    backend: &dyn DeleteBackend,
    now: SystemTime,
    running_window: Duration,
) -> DeleteOutcome {
    let plan = match preflight(adapter, session, now, running_window) {
        Err(reason) => return DeleteOutcome::Refused { reason },
        Ok(plan) => plan,
    };
    let command = plan.command_line();
    match plan.method {
        Method::Trash => match backend.trash(&plan.files) {
            Ok(()) => DeleteOutcome::Trashed { files: plan.files },
            Err(error) => DeleteOutcome::Failed { error },
        },
        Method::AgentCommand(agent_command) => match backend.run(&agent_command) {
            Err(error) => DeleteOutcome::Failed { error },
            Ok(()) if adapter.still_present(session) => DeleteOutcome::Failed {
                error: format!(
                    "{} reported success but {} still exists.",
                    adapter.display_name(),
                    session.primary_file.display()
                ),
            },
            Ok(()) => DeleteOutcome::DeletedByAgent {
                files: plan.files,
                command: command.unwrap_or_default(),
            },
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
        archive: Option<AgentCommand>,
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
        fn archive_command(&self, _session: &Session) -> Option<AgentCommand> {
            self.archive.clone()
        }
        fn transcript(&self, _session: &Session) -> std::io::Result<crate::transcript::Transcript> {
            Ok(Default::default())
        }
    }

    #[derive(Default)]
    struct FakeBackend {
        trashed: RefCell<Vec<Vec<PathBuf>>>,
        ran: RefCell<Vec<AgentCommand>>,
        fail: bool,
        command_removes: Option<PathBuf>,
    }

    impl DeleteBackend for FakeBackend {
        fn trash(&self, paths: &[PathBuf]) -> Result<(), String> {
            self.trashed.borrow_mut().push(paths.to_vec());
            if self.fail { Err("no trash here".into()) } else { Ok(()) }
        }

        fn run(&self, command: &AgentCommand) -> Result<(), String> {
            self.ran.borrow_mut().push(command.clone());
            if self.fail {
                return Err("exit 1".into());
            }
            if let Some(path) = &self.command_removes {
                fs::remove_file(path).unwrap();
            }
            Ok(())
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
            created: None,
            last_activity: Utc::now(),
            size_bytes: 1,
            archived: false,
            primary_file: path.to_path_buf(),
            files: vec![path.to_path_buf()],
            fingerprint: fs_util::fingerprint(path).unwrap(),
            parent_id: None,
            deleted_with_parent: false,
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
            warning: None,
        }
    }

    fn command_plan(file: &Path) -> DeletePlan {
        DeletePlan::AgentCommand {
            command: agent_command("delete"),
            files: vec![file.to_path_buf()],
            warning: None,
        }
    }

    fn agent_command(verb: &str) -> AgentCommand {
        AgentCommand {
            program: PathBuf::from("/bin/agent"),
            args: vec![verb.into(), "s1".into()],
        }
    }

    #[cfg(unix)]
    fn sh(script: &str) -> AgentCommand {
        AgentCommand {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".into(), script.into()],
        }
    }

    fn now() -> SystemTime {
        SystemTime::now()
    }

    const WINDOW: Duration = Duration::from_secs(60);

    #[test]
    fn trashes_existing_files() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file, &root.join("gone")]),
            root,
            archive: None,
        };
        let backend = FakeBackend::default();

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(
            outcome,
            DeleteOutcome::Trashed {
                files: vec![file.clone()]
            }
        );
        assert_eq!(*backend.trashed.borrow(), vec![vec![file]]);
    }

    #[test]
    fn refuses_read_only_plan() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            root,
            plan: DeletePlan::ReadOnly { reason: "nope".into() },
            archive: None,
        };
        let backend = FakeBackend::default();

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(outcome, DeleteOutcome::Refused { reason: "nope".into() });
        assert!(backend.trashed.borrow().is_empty());
    }

    #[test]
    fn refuses_when_file_changed_since_scan() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        fs::write(&file, b"{}\n{}\n").unwrap();
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: None,
        };

        let result = preflight(&adapter, &session, now(), WINDOW);

        assert!(result.unwrap_err().contains("changed since it was scanned"));
    }

    #[test]
    fn refuses_running_session() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: None,
        };

        let result = preflight(&adapter, &session, now(), Duration::from_secs(7200));

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
            archive: None,
        };

        let result = preflight(&adapter, &session, now(), WINDOW);

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
            archive: None,
        };
        let backend = FakeBackend::default();

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert!(matches!(outcome, DeleteOutcome::Refused { reason } if reason.contains("outside")));
        assert!(backend.trashed.borrow().is_empty());
    }

    #[test]
    fn reports_trash_failure_without_fallback() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: None,
        };
        let backend = FakeBackend {
            fail: true,
            ..Default::default()
        };

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(
            outcome,
            DeleteOutcome::Failed {
                error: "no trash here".into()
            }
        );
        assert!(file.exists());
    }

    #[test]
    fn agent_command_plan_is_permanent_and_runs_command() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: command_plan(&file),
            root,
            archive: None,
        };
        let backend = FakeBackend {
            command_removes: Some(file.clone()),
            ..Default::default()
        };

        let plan = preflight(&adapter, &session, now(), WINDOW).unwrap();
        assert!(plan.is_permanent());
        assert_eq!(plan.command_line().as_deref(), Some("/bin/agent delete s1"));

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(
            outcome,
            DeleteOutcome::DeletedByAgent {
                files: vec![file],
                command: "/bin/agent delete s1".into()
            }
        );
        assert!(backend.trashed.borrow().is_empty());
        assert_eq!(backend.ran.borrow().len(), 1);
    }

    #[test]
    fn agent_command_that_leaves_file_behind_is_a_failure() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: command_plan(&file),
            root,
            archive: None,
        };

        let outcome = execute(&adapter, &session, &FakeBackend::default(), now(), WINDOW);

        assert!(matches!(outcome, DeleteOutcome::Failed { error } if error.contains("still exists")));
    }

    #[test]
    fn agent_command_failure_is_reported() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: command_plan(&file),
            root,
            archive: None,
        };
        let backend = FakeBackend {
            fail: true,
            ..Default::default()
        };

        let outcome = execute(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(outcome, DeleteOutcome::Failed { error: "exit 1".into() });
    }

    #[test]
    fn archive_runs_agent_command_and_verifies_the_move() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: Some(agent_command("archive")),
        };
        let backend = FakeBackend {
            command_removes: Some(file.clone()),
            ..Default::default()
        };

        let outcome = toggle_archive(&adapter, &session, &backend, now(), WINDOW);

        assert_eq!(
            outcome,
            ArchiveOutcome::Done {
                command: "/bin/agent archive s1".into(),
                archived: true
            }
        );
    }

    #[test]
    fn archive_refused_without_native_support_or_while_running() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let unsupported = FakeAdapter {
            plan: trash_plan(&[&file]),
            root: root.clone(),
            archive: None,
        };
        assert!(matches!(
            toggle_archive(&unsupported, &session, &FakeBackend::default(), now(), WINDOW),
            ArchiveOutcome::Refused { reason } if reason.contains("can't archive")
        ));

        let supported = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: Some(agent_command("archive")),
        };
        let backend = FakeBackend::default();
        let outcome = toggle_archive(&supported, &session, &backend, now(), Duration::from_secs(7200));
        assert!(matches!(outcome, ArchiveOutcome::Refused { reason } if reason.contains("still running")));
        assert!(backend.ran.borrow().is_empty());
    }

    #[test]
    fn archive_that_leaves_file_in_place_is_a_failure() {
        let (_dir, root, file) = setup();
        let session = old_session(&file);
        let adapter = FakeAdapter {
            plan: trash_plan(&[&file]),
            root,
            archive: Some(agent_command("archive")),
        };

        let outcome = toggle_archive(&adapter, &session, &FakeBackend::default(), now(), WINDOW);

        assert!(matches!(outcome, ArchiveOutcome::Failed { error } if error.contains("did not move")));
    }

    #[cfg(unix)]
    #[test]
    fn system_backend_reports_nonzero_exit_with_stderr() {
        let error = SystemBackend.run(&sh("echo bad thing >&2; exit 3")).unwrap_err();
        assert!(error.contains("bad thing"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn system_backend_runs_successful_command() {
        SystemBackend.run(&sh("exit 0")).unwrap();
    }

    #[test]
    #[ignore = "moves a temp file to the real system Trash"]
    fn system_trash_moves_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("marduk-trash-test.txt");
        fs::write(&file, b"x").unwrap();
        SystemBackend.trash(std::slice::from_ref(&file)).unwrap();
        assert!(!file.exists());
    }
}
