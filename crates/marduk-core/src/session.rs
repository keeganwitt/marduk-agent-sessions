use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};

/// Size and modification time of a session's primary file, captured at scan time so a
/// delete can detect that the agent has written to the session since.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    pub len: u64,
    pub modified: SystemTime,
}

impl Fingerprint {
    pub fn is_recent(&self, now: SystemTime, window: Duration) -> bool {
        match now.duration_since(self.modified) {
            Ok(age) => age < window,
            // A modification time in the future (clock skew) is treated as "just written".
            Err(_) => true,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub agent: &'static str,
    pub id: String,
    pub project_dir: Option<String>,
    pub title: Option<String>,
    /// When the session started; `None` when neither the transcript nor the filesystem says.
    pub created: Option<DateTime<Utc>>,
    pub last_activity: DateTime<Utc>,
    pub size_bytes: u64,
    pub archived: bool,
    pub primary_file: PathBuf,
    /// Every path that makes up the session, including `primary_file`.
    pub files: Vec<PathBuf>,
    pub fingerprint: Fingerprint,
    /// The session this one was spawned from, for agents that record subagent sessions.
    pub parent_id: Option<String>,
    /// True when the agent's own delete of the parent also deletes this session.
    pub deleted_with_parent: bool,
    /// Stored inside its parent session (Claude Code subagents): shown and readable, but
    /// only deleted, archived or labeled along with the parent.
    pub part_of_parent: bool,
}

impl Session {
    /// Path-independent identity, stable across an agent's own archive moves.
    pub fn key(&self) -> String {
        format!("{}:{}", self.agent, self.id)
    }

    pub fn is_running(&self, now: SystemTime, window: Duration) -> bool {
        self.fingerprint.is_recent(now, window)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint_aged(age: Duration, now: SystemTime) -> Fingerprint {
        Fingerprint {
            len: 1,
            modified: now - age,
        }
    }

    #[test]
    fn recent_within_window() {
        let now = SystemTime::now();
        assert!(
            fingerprint_aged(Duration::from_secs(10), now).is_recent(now, Duration::from_secs(60))
        );
    }

    #[test]
    fn not_recent_outside_window() {
        let now = SystemTime::now();
        assert!(
            !fingerprint_aged(Duration::from_secs(120), now)
                .is_recent(now, Duration::from_secs(60))
        );
    }

    #[test]
    fn future_mtime_counts_as_recent() {
        let now = SystemTime::now();
        let fp = Fingerprint {
            len: 1,
            modified: now + Duration::from_secs(30),
        };
        assert!(fp.is_recent(now, Duration::from_secs(60)));
    }
}
