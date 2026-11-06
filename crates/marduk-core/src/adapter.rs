use std::path::PathBuf;

use crate::session::Session;
use crate::transcript::Transcript;

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// Directories this agent's sessions live under. Deletes are confined to these.
    fn roots(&self) -> Vec<PathBuf>;
    /// Scans, reporting the bytes of each transcript as it's read.
    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult;
    fn scan(&self) -> ScanResult {
        self.scan_with(&NoProgress)
    }
    /// Bytes `scan_with` will report in total, found cheaply (listing files, not reading
    /// them), so progress can be shown against a known total.
    fn estimate_scan_bytes(&self) -> u64 {
        0
    }
    fn delete_plan(&self, session: &Session) -> DeletePlan;
    fn transcript(&self, session: &Session) -> std::io::Result<Transcript>;
    /// What "unchanged since the scan" and "still running" are judged by. File-based agents
    /// use the transcript file; database-backed agents use the session's own row.
    fn fingerprint(&self, session: &Session) -> std::io::Result<crate::session::Fingerprint> {
        crate::fs_util::fingerprint(&session.primary_file)
    }
    /// Whether the session is still where the scan found it, used to confirm that an agent's
    /// delete or archive command really took effect.
    fn still_present(&self, session: &Session) -> bool {
        session.primary_file.exists()
    }
    /// The agent's CLI, for agents that need it to delete or archive.
    fn cli(&self) -> Option<&crate::agents::cli::Cli> {
        None
    }
    /// The agent's own command to archive this session, or to unarchive it when it is
    /// already archived. `None` when the agent has no native archive.
    fn archive_command(&self, _session: &Session) -> Option<AgentCommand> {
        None
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
}

impl AgentCommand {
    pub fn command_line(&self) -> String {
        std::iter::once(self.program.display().to_string())
            .chain(self.args.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub trait ScanProgress: Sync {
    fn advance(&self, bytes: u64);
}

pub struct NoProgress;

impl ScanProgress for NoProgress {
    fn advance(&self, _bytes: u64) {}
}

#[derive(Debug, Default)]
pub struct ScanResult {
    pub sessions: Vec<Session>,
    /// False when part of the agent's storage could not be read, so absence from
    /// `sessions` must not be taken as proof that a session was deleted.
    pub complete: bool,
    pub errors: Vec<String>,
}

impl ScanResult {
    pub fn empty_complete() -> Self {
        Self {
            complete: true,
            ..Self::default()
        }
    }

    pub fn record_error(&mut self, error: String) {
        self.complete = false;
        self.errors.push(error);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeletePlan {
    Trash {
        files: Vec<PathBuf>,
        /// Extra consequences the user should know about before confirming.
        warning: Option<String>,
    },
    /// Delete through the agent's own CLI, which also cleans up its indexes. Permanent.
    AgentCommand {
        command: AgentCommand,
        files: Vec<PathBuf>,
        /// Extra consequences the user should know about before confirming.
        warning: Option<String>,
    },
    ReadOnly {
        reason: String,
    },
}
