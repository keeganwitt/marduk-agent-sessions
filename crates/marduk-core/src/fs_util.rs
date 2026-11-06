use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::session::Fingerprint;

pub fn fingerprint(path: &Path) -> io::Result<Fingerprint> {
    let meta = fs::metadata(path)?;
    Ok(Fingerprint {
        len: meta.len(),
        modified: meta.modified()?,
    })
}

/// File creation time. Not every filesystem records it (e.g. some Linux setups).
pub fn created(path: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
    fs::metadata(path).ok()?.created().ok().map(Into::into)
}

/// Total size of a file, or of every file below a directory. Symlinks are not followed.
pub fn path_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|entry| path_size(&entry.path())).sum()
}

/// True when `path` resolves to a location strictly below one of `roots`.
/// Both sides are canonicalized, so `..` segments and symlinks cannot escape a root.
pub fn is_strictly_within(path: &Path, roots: &[PathBuf]) -> bool {
    let Ok(resolved) = path.canonicalize() else {
        return false;
    };
    roots
        .iter()
        .filter_map(|root| root.canonicalize().ok())
        .any(|root| resolved != root && resolved.starts_with(&root))
}

pub fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_size_sums_directory_tree() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a"), b"12345").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("b"), b"123").unwrap();
        assert_eq!(path_size(dir.path()), 8);
        assert_eq!(path_size(&dir.path().join("a")), 5);
        assert_eq!(path_size(&dir.path().join("missing")), 0);
    }

    #[test]
    fn within_root_accepts_children_and_rejects_root_itself() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        fs::write(&file, b"x").unwrap();
        let roots = vec![dir.path().to_path_buf()];
        assert!(is_strictly_within(&file, &roots));
        assert!(!is_strictly_within(dir.path(), &roots));
    }

    #[test]
    fn within_root_rejects_dotdot_escape() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("root");
        fs::create_dir(&root).unwrap();
        let outside = outer.path().join("outside");
        fs::write(&outside, b"x").unwrap();
        let sneaky = root.join("..").join("outside");
        assert!(!is_strictly_within(&sneaky, &[root]));
    }

    #[cfg(unix)]
    #[test]
    fn within_root_rejects_symlink_escape() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("root");
        fs::create_dir(&root).unwrap();
        let outside = outer.path().join("outside");
        fs::write(&outside, b"x").unwrap();
        let link = root.join("link");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        assert!(!is_strictly_within(&link, &[root]));
    }

    #[test]
    fn missing_path_is_not_within() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_strictly_within(
            &dir.path().join("nope"),
            &[dir.path().to_path_buf()]
        ));
    }

    #[test]
    fn uuid_detection() {
        assert!(looks_like_uuid("a9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f"));
        assert!(!looks_like_uuid("memory"));
        assert!(!looks_like_uuid("a9d8b9f2x3d8c-48bd-8b2d-ed6447b3e25f"));
        assert!(!looks_like_uuid("g9d8b9f2-3d8c-48bd-8b2d-ed6447b3e25f"));
    }
}
