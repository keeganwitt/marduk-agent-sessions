pub mod adapter;
pub mod agents;
pub mod catalog;
pub mod delete;
pub mod fs_util;
mod jsonl;
pub mod session;

pub use adapter::{AgentAdapter, AgentCommand, DeletePlan, ScanResult};
pub use delete::{ArchiveOutcome, DeleteBackend, DeleteOutcome, PlannedDelete, SystemBackend};
pub use session::{Fingerprint, Session};

use std::time::Duration;

pub const RUNNING_WINDOW: Duration = Duration::from_secs(5 * 60);

pub fn default_adapters() -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(agents::claude::ClaudeAdapter::from_env()),
        Box::new(agents::codex::CodexAdapter::from_env()),
        Box::new(agents::pi::PiAdapter::from_env()),
    ]
}
