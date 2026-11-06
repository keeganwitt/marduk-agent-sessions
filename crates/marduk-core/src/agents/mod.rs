pub mod claude;
pub mod cli;
pub mod codex;
pub mod copilot;
pub mod opencode;
pub mod pi;

use std::path::{Path, PathBuf};

pub(crate) fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub(crate) fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Finds an agent CLI on `PATH` or in common install folders. Apps launched from the macOS
/// Finder or Dock get a minimal `PATH` that excludes Homebrew and `~/.local/bin`.
pub(crate) fn find_program(name: &str) -> Option<PathBuf> {
    let home = home_dir();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    dirs.extend([
        home.join(".local/bin"),
        home.join(".bun/bin"),
        home.join(".npm-global/bin"),
        home.join(".cargo/bin"),
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    if let Some(app_data) = env_dir("APPDATA") {
        dirs.push(app_data.join("npm"));
    }
    find_in(name, &dirs)
}

fn find_in(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) { &["exe", "cmd", "bat"] } else { &[""] };
    dirs.iter().find_map(|dir| {
        candidates.iter().find_map(|ext| {
            let path = if ext.is_empty() {
                dir.join(name)
            } else {
                dir.join(format!("{name}.{ext}"))
            };
            is_executable(&path).then_some(path)
        })
    })
}

pub(crate) fn is_executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.is_file() && meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        meta.is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn finds_executables_in_order_and_skips_non_executables() {
        use std::os::unix::fs::PermissionsExt;
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join("tool"), "").unwrap();
        let exe = second.path().join("tool");
        std::fs::write(&exe, "").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

        let dirs = vec![first.path().to_path_buf(), second.path().to_path_buf()];

        assert_eq!(find_in("tool", &dirs), Some(exe));
        assert_eq!(find_in("missing", &dirs), None);
    }
}
