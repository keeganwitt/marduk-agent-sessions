pub mod adapter;
pub mod agents;
pub mod cache;
pub mod catalog;
pub mod delete;
pub mod demo;
pub mod fs_util;
mod jsonl;
pub mod labels;
pub mod session;
pub mod transcript;

pub use adapter::{
    AgentAdapter, AgentCommand, DeletePlan, Mechanics, NoProgress, ScanProgress, ScanResult,
};
pub use agents::cli::{CliInfo, CliOverrides};
pub use delete::{ArchiveOutcome, DeleteBackend, DeleteOutcome, PlannedDelete, SystemBackend};
pub use session::{Fingerprint, Session};
pub use transcript::{Message, Role, ToolCall, Transcript};

use std::time::Duration;

pub fn home_dir() -> std::path::PathBuf {
    agents::home_dir()
}

pub const RUNNING_WINDOW: Duration = Duration::from_secs(5 * 60);

pub fn default_adapters(
    overrides: std::sync::Arc<agents::cli::CliOverrides>,
) -> Vec<Box<dyn AgentAdapter>> {
    vec![
        Box::new(agents::claude::ClaudeAdapter::from_env()),
        Box::new(agents::codex::CodexAdapter::from_env(overrides.clone())),
        Box::new(agents::copilot::CopilotAdapter::from_env()),
        Box::new(agents::opencode::OpenCodeAdapter::from_env(
            &agents::opencode::OPENCODE,
            overrides.clone(),
        )),
        Box::new(agents::opencode::OpenCodeAdapter::from_env(
            &agents::opencode::KILO,
            overrides,
        )),
        Box::new(agents::pi::PiAdapter::from_env()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_agent_explains_how_it_deletes_and_archives() {
        for adapter in default_adapters(Default::default()) {
            let mechanics = adapter.mechanics();
            assert!(!mechanics.delete.trim().is_empty(), "{}", adapter.id());
            assert!(!mechanics.archive.trim().is_empty(), "{}", adapter.id());
            assert!(!adapter.roots().is_empty(), "{}", adapter.id());
        }
    }
}
