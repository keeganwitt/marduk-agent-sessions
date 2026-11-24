mod ai_assistant;
mod air;

pub use ai_assistant::AiAssistantAdapter;
pub use air::AirAdapter;

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::adapter::{DeletePlan, ScanResult};
use crate::session::Session;

fn config_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Application Support");
    #[cfg(windows)]
    let base = super::env_dir("APPDATA").unwrap_or_else(|| home.join("AppData/Roaming"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = super::env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
    base.join("JetBrains")
}

fn system_dir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = home.join("Library/Caches");
    #[cfg(windows)]
    let base = super::env_dir("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData/Local"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = super::env_dir("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
    base.join("JetBrains")
}

fn entries(dir: &Path, result: &mut ScanResult, directories: bool) -> Vec<PathBuf> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            result.record_error(format!("{}: {error}", dir.display()));
            return Vec::new();
        }
    };
    let mut paths = Vec::new();
    for entry in entries {
        match entry.and_then(|entry| Ok((entry.path(), entry.file_type()?))) {
            Ok((path, kind))
                if (directories && kind.is_dir()) || (!directories && kind.is_file()) =>
            {
                paths.push(path);
            }
            Ok(_) => {}
            Err(error) => result.record_error(format!("{}: {error}", dir.display())),
        }
    }
    paths.sort();
    paths
}

fn idea_dirs(root: &Path, result: &mut ScanResult) -> Vec<PathBuf> {
    entries(root, result, true)
        .into_iter()
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            ["IntelliJIdea", "IdeaIC"]
                .iter()
                .filter_map(|prefix| name.strip_prefix(prefix))
                .any(|suffix| suffix.starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect()
}

fn read_only(reason: &str) -> DeletePlan {
    DeletePlan::ReadOnly {
        reason: reason.into(),
    }
}

fn invalid_data(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn latest_copies(sessions: Vec<Session>) -> Vec<Session> {
    let mut by_id: BTreeMap<String, Session> = BTreeMap::new();
    for session in sessions {
        let id = session.id.clone();
        if let Some(known) = by_id.get(&id)
            && (known.last_activity, &known.primary_file)
                >= (session.last_activity, &session.primary_file)
        {
            continue;
        }
        by_id.insert(id, session);
    }
    by_id.into_values().collect()
}
