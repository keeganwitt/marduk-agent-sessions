pub mod claude;
pub mod cli;
pub mod codex;
pub mod copilot;
pub mod opencode;
pub mod pi;

use std::path::{Path, PathBuf};

/// `<root>/<folder>/*.jsonl`, the layout Claude Code and Pi both use. A missing root is an
/// agent that isn't installed; other read errors make the scan incomplete.
pub(crate) fn jsonl_two_levels(
    root: &Path,
    result: &mut crate::adapter::ScanResult,
) -> Vec<PathBuf> {
    let folders = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            result.record_error(format!("{}: {error}", root.display()));
            return Vec::new();
        }
    };
    let mut files = Vec::new();
    for folder in folders.flatten() {
        let folder = folder.path();
        if !folder.is_dir() {
            continue;
        }
        match std::fs::read_dir(&folder) {
            Ok(entries) => {
                files.extend(entries.flatten().map(|entry| entry.path()).filter(|path| {
                    path.extension().is_some_and(|ext| ext == "jsonl") && path.is_file()
                }))
            }
            Err(error) => result.record_error(format!("{}: {error}", folder.display())),
        }
    }
    files
}

/// Total size of `files`, for progress estimates.
pub(crate) fn total_size(files: &[PathBuf]) -> u64 {
    files
        .iter()
        .filter_map(|file| file.metadata().ok())
        .map(|meta| meta.len())
        .sum()
}

/// `MARDUK_HOME` moves only Marduk's idea of home, for the demo home; overriding `HOME` would
/// also move Cargo's and pnpm's caches for everything `tauri dev` starts.
pub(crate) fn home_dir() -> PathBuf {
    home_from(env_dir("MARDUK_HOME"))
}

fn home_from(marduk_home: Option<PathBuf>) -> PathBuf {
    marduk_home
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
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

#[cfg(not(unix))]
const WINDOWS_EXECUTABLES: [&str; 4] = ["exe", "cmd", "bat", "com"];

fn find_in(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let candidates: &[&str] = if cfg!(windows) {
        &["exe", "cmd", "bat", "com"]
    } else {
        &[""]
    };
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
    // Windows has no executable bit; what runs is decided by the file extension.
    #[cfg(not(unix))]
    {
        meta.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| WINDOWS_EXECUTABLES.contains(&ext.to_ascii_lowercase().as_str()))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

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

#[cfg(test)]
mod home_tests {
    use super::*;

    #[test]
    fn marduk_home_overrides_the_user_home() {
        assert_eq!(
            home_from(Some(PathBuf::from("demo"))),
            PathBuf::from("demo")
        );
        assert_eq!(home_from(None), dirs::home_dir().unwrap());
    }
}
