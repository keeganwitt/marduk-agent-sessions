use std::path::PathBuf;

use crate::session::Session;

pub trait AgentAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    /// Directories this agent's sessions live under. Deletes are confined to these.
    fn roots(&self) -> Vec<PathBuf>;
    fn scan(&self) -> ScanResult;
    fn delete_plan(&self, session: &Session) -> DeletePlan;
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
    },
    /// Delete through the agent's own CLI, which also cleans up its indexes. Permanent.
    AgentCommand {
        command: AgentCommand,
        files: Vec<PathBuf>,
    },
    ReadOnly {
        reason: String,
    },
}
