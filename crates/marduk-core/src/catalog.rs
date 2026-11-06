use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

use crate::adapter::{AgentAdapter, DeletePlan, ScanProgress};
use crate::delete::{self, ArchiveOutcome, DeleteBackend, DeleteOutcome, PlannedDelete};
use crate::session::Session;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanStatus {
    pub done: u64,
    pub total: u64,
    /// The agent being scanned, empty once the scan has finished.
    pub agent: &'static str,
}

const REPORT_INTERVAL: Duration = Duration::from_millis(50);

/// One agent's share of the overall progress, capped at its estimate.
struct ProgressTracker<'a> {
    base: u64,
    cap: u64,
    total: u64,
    agent: &'static str,
    done: AtomicU64,
    last_report: Mutex<Instant>,
    report: &'a (dyn Fn(ScanStatus) + Sync),
}

impl ScanProgress for ProgressTracker<'_> {
    fn advance(&self, bytes: u64) {
        let done = self.done.fetch_add(bytes, Ordering::Relaxed).saturating_add(bytes);
        let mut last = self.last_report.lock().unwrap();
        if last.elapsed() >= REPORT_INTERVAL {
            *last = Instant::now();
            (self.report)(ScanStatus {
                done: self.base + done.min(self.cap),
                total: self.total,
                agent: self.agent,
            });
        }
    }
}

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
        self.scan_with_progress(&|_| {})
    }

    /// Scans every agent, reporting progress against a total measured before any parsing,
    /// so it only moves forward and reaches `total` exactly when the scan finishes.
    pub fn scan_with_progress(&self, report: &(dyn Fn(ScanStatus) + Sync)) -> Vec<AgentScan> {
        let estimates: Vec<u64> = self
            .adapters
            .iter()
            .map(|adapter| adapter.estimate_scan_bytes())
            .collect();
        let total = estimates.iter().sum();
        let mut base = 0;
        let mut snapshot = HashMap::new();
        let mut scans = Vec::new();
        for (index, adapter) in self.adapters.iter().enumerate() {
            report(ScanStatus {
                done: base,
                total,
                agent: adapter.display_name(),
            });
            let tracker = ProgressTracker {
                base,
                cap: estimates[index],
                total,
                agent: adapter.display_name(),
                done: AtomicU64::new(0),
                last_report: Mutex::new(Instant::now()),
                report,
            };
            let result = adapter.scan_with(&tracker);
            base += estimates[index];
            for session in &result.sessions {
                snapshot.insert(uid(session), (index, session.clone()));
            }
            scans.push(AgentScan {
                agent: adapter.id(),
                display_name: adapter.display_name(),
                complete: result.complete,
                errors: result.errors,
                sessions: result.sessions,
            });
        }
        report(ScanStatus {
            done: total,
            total,
            agent: "",
        });
        *self.snapshot.lock().unwrap() = snapshot;
        scans
    }

    /// Each CLI-dependent agent's CLI, re-resolved so a changed setting shows immediately.
    pub fn cli_status(&self) -> Vec<(&'static str, &'static str, crate::agents::cli::CliInfo)> {
        self.adapters
            .iter()
            .filter_map(|adapter| {
                let cli = adapter.cli()?;
                cli.refresh();
                Some((adapter.id(), adapter.display_name(), cli.info()))
            })
            .collect()
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
        let selected: std::collections::HashSet<(&str, &str)> = uids
            .iter()
            .filter_map(|uid| snapshot.get(uid))
            .map(|(_, session)| (session.agent, session.id.as_str()))
            .collect();
        uids.iter()
            .map(|uid| match snapshot.get(uid) {
                Some((_, session)) if deleted_with_selected_ancestor(&snapshot, session, &selected) => Preview {
                    uid: uid.clone(),
                    session: Some(session.clone()),
                    result: Err("Deleted along with its parent session, which is also selected.".to_string()),
                },
                None => Preview {
                    uid: uid.clone(),
                    session: None,
                    result: Err(not_found()),
                },
                Some((index, session)) => {
                    let adapter = self.adapters[*index].as_ref();
                    let result = delete::preflight(adapter, session, now, self.running_window).map(|mut plan| {
                        if plan.is_permanent() && plan.warning.is_none() {
                            plan.warning = descendants_warning(&snapshot, session, adapter.display_name());
                        }
                        plan
                    });
                    Preview {
                        uid: uid.clone(),
                        session: Some(session.clone()),
                        result,
                    }
                }
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

    /// The path-independent `Session::key` of a scanned session.
    pub fn key_of(&self, uid: &str) -> Option<String> {
        self.snapshot.lock().unwrap().get(uid).map(|(_, session)| session.key())
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

/// True when `session` will go with a selected ancestor: each link up to that ancestor is
/// one the agent deletes along with its parent.
fn deleted_with_selected_ancestor(
    snapshot: &HashMap<String, (usize, Session)>,
    session: &Session,
    selected: &std::collections::HashSet<(&str, &str)>,
) -> bool {
    let by_id = |id: &str| {
        snapshot
            .values()
            .map(|(_, candidate)| candidate)
            .find(|candidate| candidate.agent == session.agent && candidate.id == id)
    };
    let mut current = session;
    let mut seen = std::collections::HashSet::new();
    while current.deleted_with_parent
        && let Some(parent_id) = current.parent_id.as_deref()
        && seen.insert(parent_id.to_string())
    {
        if selected.contains(&(session.agent, parent_id)) {
            return true;
        }
        match by_id(parent_id) {
            Some(parent) => current = parent,
            None => return false,
        }
    }
    false
}

/// Sessions the agent's delete of `session` will also remove, counted from the scan
/// (children, grandchildren, …) for agents whose adapter doesn't say so itself.
fn descendants_warning(
    snapshot: &HashMap<String, (usize, Session)>,
    session: &Session,
    agent_name: &str,
) -> Option<String> {
    let mut pending = vec![session.id.clone()];
    let mut seen = std::collections::HashSet::from([session.id.clone()]);
    let mut count = 0usize;
    while let Some(parent) = pending.pop() {
        for (_, child) in snapshot.values() {
            if child.agent == session.agent
                && child.deleted_with_parent
                && child.parent_id.as_deref() == Some(parent.as_str())
                && seen.insert(child.id.clone())
            {
                count += 1;
                pending.push(child.id.clone());
            }
        }
    }
    (count > 0).then(|| {
        format!(
            "{agent_name} also deletes {count} subagent {} that this session started.",
            if count == 1 { "session" } else { "sessions" }
        )
    })
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
    fn codex_delete_preview_counts_spawned_descendants_but_not_guardians() {
        use crate::agents::codex::CodexAdapter;
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/11/06");
        fs::create_dir_all(&day).unwrap();
        let uuid = |n: u8| format!("01a1b21a-26bc-7700-ae31-919e0387ce{n:02}");
        let write = |n: u8, extra: &str| {
            let path = day.join(format!("rollout-x-{}.jsonl", uuid(n)));
            fs::write(
                &path,
                format!(r#"{{"type":"session_meta","payload":{{"id":"{}"{extra}}}}}"#, uuid(n)),
            )
            .unwrap();
            let old = filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
            filetime::set_file_mtime(&path, old).unwrap();
        };
        let spawn = |parent: u8| {
            format!(
                r#","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{}"}}}}}}"#,
                uuid(parent)
            )
        };
        write(1, "");
        write(2, &spawn(1));
        write(3, &spawn(2));
        write(
            4,
            &format!(r#","parent_thread_id":"{}","thread_source":"guardian_review""#, uuid(1)),
        );
        let catalog = Catalog::new(
            vec![Box::new(CodexAdapter::new(
                home.path().to_path_buf(),
                Some("/bin/codex".into()),
            ))],
            Duration::from_secs(60),
        );
        let scans = catalog.scan();
        let parent = scans[0].sessions.iter().find(|s| s.id == uuid(1)).unwrap();

        let preview = catalog.preview(&[uid(parent)], SystemTime::now());

        let plan = preview[0].result.as_ref().unwrap();
        assert_eq!(
            plan.warning.as_deref(),
            Some("Codex also deletes 2 subagent sessions that this session started.")
        );
    }

    #[test]
    fn selected_children_deleted_with_a_selected_parent_are_not_deleted_separately() {
        use crate::agents::codex::CodexAdapter;
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/11/06");
        fs::create_dir_all(&day).unwrap();
        let uuid = |n: u8| format!("01a1b21a-26bc-7700-ae31-919e0387ce{n:02}");
        let write = |n: u8, extra: String| {
            let path = day.join(format!("rollout-x-{}.jsonl", uuid(n)));
            fs::write(
                &path,
                format!(r#"{{"type":"session_meta","payload":{{"id":"{}"{extra}}}}}"#, uuid(n)),
            )
            .unwrap();
            let old = filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
            filetime::set_file_mtime(&path, old).unwrap();
        };
        let spawn = |parent: u8| {
            format!(
                r#","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"{}"}}}}}}"#,
                uuid(parent)
            )
        };
        write(1, String::new());
        write(2, spawn(1));
        write(3, spawn(2));
        write(4, format!(r#","parent_thread_id":"{}""#, uuid(1)));
        let catalog = Catalog::new(
            vec![Box::new(CodexAdapter::new(
                home.path().to_path_buf(),
                Some("/bin/codex".into()),
            ))],
            Duration::from_secs(60),
        );
        let scans = catalog.scan();
        let uid_of = |n: u8| uid(scans[0].sessions.iter().find(|s| s.id == uuid(n)).unwrap());

        let preview = catalog.preview(&[uid_of(1), uid_of(3), uid_of(4)], SystemTime::now());

        assert!(preview[0].result.is_ok());
        assert!(
            preview[1]
                .result
                .as_ref()
                .unwrap_err()
                .contains("Deleted along with its parent")
        );
        assert!(
            preview[2].result.is_ok(),
            "guardian reviews aren't deleted with their parent"
        );
    }

    #[test]
    fn progress_only_moves_forward_and_ends_at_the_total() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("--p--");
        fs::create_dir(&dir).unwrap();
        for index in 0..5 {
            fs::write(dir.join(format!("t_{index}.jsonl")), "x".repeat(1000 * (index + 1))).unwrap();
        }
        let catalog = Catalog::new(
            vec![Box::new(PiAdapter::new(root.path().to_path_buf()))],
            Duration::from_secs(60),
        );
        let reports = Mutex::new(Vec::new());

        catalog.scan_with_progress(&|status| reports.lock().unwrap().push(status));

        let reports = reports.into_inner().unwrap();
        assert!(reports.windows(2).all(|pair| pair[0].done <= pair[1].done));
        assert!(
            reports
                .iter()
                .all(|status| status.total == 15_000 && status.done <= status.total)
        );
        assert_eq!(reports.last().unwrap().done, 15_000);
        assert_eq!(reports[0].agent, "Pi");
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
