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
        let done = self
            .done
            .fetch_add(bytes, Ordering::Relaxed)
            .saturating_add(bytes);
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

pub struct AgentHelp {
    pub id: &'static str,
    pub name: &'static str,
    pub roots: Vec<std::path::PathBuf>,
    pub mechanics: crate::adapter::Mechanics,
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

    /// Every agent with the folders it's read from and how its sessions are deleted and
    /// archived, for the help dialog.
    pub fn agent_help(&self) -> Vec<AgentHelp> {
        self.adapters
            .iter()
            .map(|adapter| AgentHelp {
                id: adapter.id(),
                name: adapter.display_name(),
                roots: adapter.roots(),
                mechanics: adapter.mechanics(),
            })
            .collect()
    }

    /// Each adapter's sessions from the last scan.
    fn sessions_by_adapter(&self) -> Vec<Vec<Session>> {
        let mut sessions = vec![Vec::new(); self.adapters.len()];
        for (index, session) in self.snapshot.lock().unwrap().values() {
            sessions[*index].push(session.clone());
        }
        sessions
    }

    /// Every agent's memory, judged against the last scan.
    pub fn memory(&self) -> Vec<crate::memory::MemoryStore> {
        let sessions = self.sessions_by_adapter();
        self.adapters
            .iter()
            .zip(&sessions)
            .flat_map(|(adapter, sessions)| adapter.memory(sessions))
            .collect()
    }

    /// Memory no session would use once `uids` are deleted, offered for deletion with them.
    pub fn memory_left_behind(&self, uids: &[String]) -> Vec<crate::memory::MemoryStore> {
        let snapshot = self.snapshot.lock().unwrap().clone();
        let sessions = self.sessions_by_adapter();
        self.adapters
            .iter()
            .enumerate()
            .flat_map(|(index, adapter)| {
                let deleting: Vec<&Session> = uids
                    .iter()
                    .filter_map(|uid| snapshot.get(uid))
                    .filter(|(owner, _)| *owner == index)
                    .map(|(_, session)| session)
                    .collect();
                if deleting.is_empty() {
                    Vec::new()
                } else {
                    adapter.memory_left_behind(&deleting, &sessions[index])
                }
            })
            .collect()
    }

    /// The text of a memory file, if some agent owns it.
    pub fn read_memory_file(&self, path: &std::path::Path) -> Result<String, String> {
        const LIMIT: u64 = 2 * 1024 * 1024;
        if !self
            .adapters
            .iter()
            .any(|adapter| adapter.owns_memory_file(path))
        {
            return Err(format!("{} isn't an agent memory file.", path.display()));
        }
        if crate::memory::size(path) > LIMIT {
            return Err(format!("{} is too large to show.", path.display()));
        }
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Moves a whole memory folder to the Trash. With `only_if_unused`, refuses a folder
    /// whose sessions haven't all been deleted, as when it's offered alongside a delete.
    pub fn trash_memory(
        &self,
        folder: &std::path::Path,
        only_if_unused: bool,
        backend: &dyn DeleteBackend,
    ) -> Result<(), String> {
        let Some(adapter) = self
            .adapters
            .iter()
            .find(|adapter| adapter.owns_memory_folder(folder))
        else {
            return Err(format!(
                "{} isn't a memory folder Marduk can remove.",
                folder.display()
            ));
        };
        if only_if_unused && adapter.memory_in_use(folder) {
            return Err(format!(
                "{} is still used by sessions in its project.",
                folder.display()
            ));
        }
        backend.trash(&[folder.to_path_buf()])
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
        self.adapter_for(session)
            .map(|adapter| adapter.delete_plan(session))
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
                Some((_, session))
                    if deleted_with_selected_ancestor(&snapshot, session, &selected) =>
                {
                    Preview {
                        uid: uid.clone(),
                        session: Some(session.clone()),
                        result: Err(
                            "Deleted along with its parent session, which is also selected."
                                .to_string(),
                        ),
                    }
                }
                None => Preview {
                    uid: uid.clone(),
                    session: None,
                    result: Err(not_found()),
                },
                Some((index, session)) => {
                    let adapter = self.adapters[*index].as_ref();
                    let result = delete::preflight(adapter, session, now, self.running_window).map(
                        |mut plan| {
                            if plan.warning.is_none() {
                                plan.warning = descendants_warning(
                                    &snapshot,
                                    session,
                                    adapter.display_name(),
                                    plan.is_permanent(),
                                );
                            }
                            plan
                        },
                    );
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
                    None => DeleteOutcome::Refused {
                        reason: not_found(),
                    },
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
            Err(format!(
                "The project folder {} no longer exists.",
                dir.display()
            ))
        }
    }

    /// The path-independent `Session::key` of a scanned session.
    pub fn key_of(&self, uid: &str) -> Option<String> {
        self.snapshot
            .lock()
            .unwrap()
            .get(uid)
            .map(|(_, session)| session.key())
    }

    pub fn transcript(&self, uid: &str) -> Result<crate::transcript::Transcript, String> {
        let (index, session) = self
            .snapshot
            .lock()
            .unwrap()
            .get(uid)
            .cloned()
            .ok_or_else(not_found)?;
        self.adapters[index]
            .transcript(&session)
            .map_err(|error| format!("Couldn't read {}: {error}", session.primary_file.display()))
    }

    /// True when `path` is one of the files of a session in the current scan. Lets the UI
    /// reveal session files without being able to point at arbitrary paths.
    pub fn owns_path(&self, path: &std::path::Path) -> bool {
        self.snapshot.lock().unwrap().values().any(|(_, session)| {
            session.primary_file == path || session.files.iter().any(|file| file == path)
        })
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
                    None => ArchiveOutcome::Refused {
                        reason: not_found(),
                    },
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
    permanent: bool,
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
    let sessions = if count == 1 { "session" } else { "sessions" };
    (count > 0).then(|| {
        if permanent {
            format!(
                "{agent_name} also deletes {count} subagent {sessions} that this session started."
            )
        } else {
            format!("This also moves its {count} subagent {sessions} to the Trash.")
        }
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
        let old =
            filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
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

    /// Records what it was asked to trash, and actually removes it so later checks see that.
    #[derive(Default)]
    struct RecordingTrash(Mutex<Vec<std::path::PathBuf>>);
    impl DeleteBackend for RecordingTrash {
        fn trash(&self, paths: &[std::path::PathBuf]) -> Result<(), String> {
            for path in paths {
                if path.is_dir() {
                    fs::remove_dir_all(path).unwrap();
                } else {
                    fs::remove_file(path).unwrap();
                }
            }
            self.0.lock().unwrap().extend(paths.iter().cloned());
            Ok(())
        }
        fn run(&self, _command: &crate::adapter::AgentCommand) -> Result<(), String> {
            Ok(())
        }
    }

    /// One Claude Code project with a single, old session and a memory folder.
    fn catalog_with_claude_memory() -> (tempfile::TempDir, Catalog, std::path::PathBuf) {
        let config = tempfile::tempdir().unwrap();
        let project = config.path().join("projects").join("-work-app");
        let memory_dir = project.join("memory");
        fs::create_dir_all(&memory_dir).unwrap();
        fs::write(memory_dir.join("MEMORY.md"), "- [Git](git.md)").unwrap();
        fs::write(memory_dir.join("git.md"), "---\nname: git\n---\n").unwrap();
        let transcript = project.join("a9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f.jsonl");
        fs::write(
            &transcript,
            r#"{"type":"user","cwd":"/work/app","message":{"role":"user","content":"hi"}}"#,
        )
        .unwrap();
        let old =
            filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        filetime::set_file_mtime(&transcript, old).unwrap();
        let catalog = Catalog::new(
            vec![Box::new(crate::agents::claude::ClaudeAdapter::new(
                config.path().to_path_buf(),
            ))],
            Duration::from_secs(60),
        );
        (config, catalog, memory_dir)
    }

    #[test]
    fn offers_memory_with_the_last_session_and_trashes_it_once_unused() {
        let (_config, catalog, memory_dir) = catalog_with_claude_memory();
        let scans = catalog.scan();
        let id = uid(&scans[0].sessions[0]);
        let trash = RecordingTrash::default();

        let offered = catalog.memory_left_behind(std::slice::from_ref(&id));
        assert_eq!(offered.len(), 1);
        assert_eq!(offered[0].folder, memory_dir);
        let refused = catalog.trash_memory(&memory_dir, true, &trash).unwrap_err();
        assert!(refused.contains("still used"), "{refused}");

        catalog.delete(&[id], &trash, SystemTime::now());
        catalog.trash_memory(&memory_dir, true, &trash).unwrap();

        assert!(!memory_dir.exists());
    }

    #[test]
    fn reads_and_trashes_only_agent_memory() {
        let (config, catalog, memory_dir) = catalog_with_claude_memory();
        catalog.scan();
        let trash = RecordingTrash::default();

        assert_eq!(catalog.memory().len(), 1);
        assert!(
            catalog
                .read_memory_file(&memory_dir.join("git.md"))
                .unwrap()
                .contains("name: git")
        );
        let transcript = memory_dir
            .parent()
            .unwrap()
            .join("a9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f.jsonl");
        assert!(catalog.read_memory_file(&transcript).is_err());
        assert!(catalog.trash_memory(config.path(), false, &trash).is_err());
        assert!(trash.0.lock().unwrap().is_empty());

        catalog.trash_memory(&memory_dir, false, &trash).unwrap();
        assert_eq!(*trash.0.lock().unwrap(), vec![memory_dir]);
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
            let header = serde_json::json!({ "type": "session", "id": name, "cwd": cwd });
            fs::write(dir.join(name), header.to_string()).unwrap();
        }
        let catalog = Catalog::new(
            vec![Box::new(PiAdapter::new(root.path().to_path_buf()))],
            Duration::from_secs(60),
        );
        let scans = catalog.scan();
        let uid_of = |name: &str| uid(scans[0].sessions.iter().find(|s| s.id == name).unwrap());

        assert_eq!(
            catalog.project_dir(&uid_of("t_a.jsonl")).unwrap(),
            project.path()
        );
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
                format!(
                    r#"{{"type":"session_meta","payload":{{"id":"{}"{extra}}}}}"#,
                    uuid(n)
                ),
            )
            .unwrap();
            let old =
                filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
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
            &format!(
                r#","parent_thread_id":"{}","thread_source":"guardian_review""#,
                uuid(1)
            ),
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
                format!(
                    r#"{{"type":"session_meta","payload":{{"id":"{}"{extra}}}}}"#,
                    uuid(n)
                ),
            )
            .unwrap();
            let old =
                filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
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
            fs::write(
                dir.join(format!("t_{index}.jsonl")),
                "x".repeat(1000 * (index + 1)),
            )
            .unwrap();
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
    fn claude_subagents_are_view_only_children_counted_in_the_parents_delete() {
        use crate::agents::claude::ClaudeAdapter;
        let config = tempfile::tempdir().unwrap();
        let id = "a9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f";
        let project = config.path().join("projects").join("-w");
        let subagents = project.join(id).join("subagents");
        fs::create_dir_all(&subagents).unwrap();
        let old =
            filetime::FileTime::from_system_time(SystemTime::now() - Duration::from_secs(3600));
        let parent = project.join(format!("{id}.jsonl"));
        fs::write(
            &parent,
            r#"{"type":"user","cwd":"/w","message":{"role":"user","content":"main task"}}"#,
        )
        .unwrap();
        let child = subagents.join("agent-a1.jsonl");
        fs::write(
            &child,
            r#"{"type":"user","isSidechain":true,"message":{"role":"user","content":"look things up"}}"#,
        )
        .unwrap();
        fs::write(
            subagents.join("agent-a1.meta.json"),
            r#"{"description":"Research the API","agentType":"general-purpose"}"#,
        )
        .unwrap();
        for file in [&parent, &child] {
            filetime::set_file_mtime(file, old).unwrap();
        }
        let catalog = Catalog::new(
            vec![Box::new(ClaudeAdapter::new(config.path().to_path_buf()))],
            Duration::from_secs(60),
        );
        let scans = catalog.scan();
        let sessions = &scans[0].sessions;
        let sub = sessions.iter().find(|s| s.part_of_parent).unwrap();
        let main = sessions.iter().find(|s| !s.part_of_parent).unwrap();

        assert_eq!(
            sub.title.as_deref(),
            Some("Research the API · general-purpose")
        );
        assert_eq!(sub.parent_id.as_deref(), Some(id));
        assert_eq!(sub.files.len(), 2);
        assert!(matches!(
            catalog.delete_plan(sub),
            Some(DeletePlan::ReadOnly { .. })
        ));
        assert_eq!(
            catalog.transcript(&uid(sub)).unwrap().head[0].text,
            "look things up"
        );

        let preview = catalog.preview(&[uid(main)], SystemTime::now());
        assert_eq!(
            preview[0].result.as_ref().unwrap().warning.as_deref(),
            Some("This also moves its 1 subagent session to the Trash.")
        );
    }

    #[test]
    fn pi_sessions_cannot_be_archived() {
        let (_root, catalog) = catalog_with_old_pi_session();
        let scans = catalog.scan();
        let session = &scans[0].sessions[0];
        assert!(!catalog.can_archive(session));
        let outcome = catalog.toggle_archive(&[uid(session)], &NoopTrash, SystemTime::now());
        assert!(
            matches!(&outcome[0].1, ArchiveOutcome::Refused { reason } if reason.contains("can't archive"))
        );
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
