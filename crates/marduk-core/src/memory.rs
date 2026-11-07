//! Memory that agents keep on disk between sessions, for review in Marduk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

/// One agent's memory: per project for Claude Code, one global store for Codex.
#[derive(Clone, Debug, PartialEq)]
pub struct MemoryStore {
    pub agent: &'static str,
    pub agent_name: &'static str,
    pub folder: PathBuf,
    /// The project the memory belongs to; `None` for a global store.
    pub project_dir: Option<String>,
    /// Whether `project_dir` still exists; `None` when there's no project or it's unknown.
    pub project_exists: Option<bool>,
    /// The agent's sessions in that project, from the last scan.
    pub sessions: usize,
    pub files: Vec<MemoryFile>,
    pub problems: Vec<String>,
    /// How the agent treats this store, e.g. that it's generated and reset elsewhere.
    pub note: Option<String>,
    /// Whether Marduk may move the whole folder to the Trash.
    pub can_trash: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MemoryFile {
    pub path: PathBuf,
    pub title: String,
    pub description: Option<String>,
    /// What the file is for: a memory type (`feedback`, `project`, …) or a role (`index`).
    pub kind: Option<String>,
    /// The session the memory came from, when the agent records it.
    pub session_id: Option<String>,
    pub modified: Option<DateTime<Utc>>,
    pub size: u64,
}

/// `key: value` pairs from a leading `---` block, including indented (nested) keys; the
/// first occurrence of a key wins. Values lose surrounding quotes.
pub(crate) fn frontmatter(text: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("---") {
        return fields;
    }
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
            if !value.is_empty() {
                fields
                    .entry(key.trim().to_string())
                    .or_insert_with(|| value.to_string());
            }
        }
    }
    fields
}

/// Leading `key: value` header lines (Codex session summaries), up to the first blank line.
pub(crate) fn header_fields(text: &str) -> HashMap<String, String> {
    text.lines()
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_once(':'))
        .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
        .collect()
}

/// Targets of Markdown links, `[text](target)`, without any `#anchor`.
pub(crate) fn link_targets(text: &str) -> Vec<String> {
    text.match_indices("](")
        .filter_map(|(start, _)| {
            let rest = &text[start + 2..];
            let end = rest.find(')')?;
            let target = rest[..end].split('#').next().unwrap_or_default().trim();
            (!target.is_empty() && !target.contains("://")).then(|| target.to_string())
        })
        .collect()
}

pub(crate) fn modified(path: &Path) -> Option<DateTime<Utc>> {
    path.metadata()
        .and_then(|meta| meta.modified())
        .ok()
        .map(DateTime::<Utc>::from)
}

pub(crate) fn size(path: &Path) -> u64 {
    path.metadata().map(|meta| meta.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_frontmatter_including_nested_keys() {
        let text = "---\nname: git-identity\ndescription: \"Use the personal email\"\nmetadata:\n  type: feedback\n  originSessionId: a9d8\n---\nBody: not a field\n";

        let fields = frontmatter(text);

        assert_eq!(fields["name"], "git-identity");
        assert_eq!(fields["description"], "Use the personal email");
        assert_eq!(fields["type"], "feedback");
        assert_eq!(fields["originSessionId"], "a9d8");
        assert!(!fields.contains_key("Body"));
        assert!(frontmatter("no frontmatter: here").is_empty());
    }

    #[test]
    fn reads_header_lines_until_the_first_blank_line() {
        let fields = header_fields("thread_id: t1\ncwd: /work\n\nOutcome: later");

        assert_eq!(fields["thread_id"], "t1");
        assert_eq!(fields["cwd"], "/work");
        assert!(!fields.contains_key("Outcome"));
    }

    #[test]
    fn finds_local_link_targets() {
        let text = "- [Git](git.md) — hook\n- [Docs](https://example.com)\n- [Plan](plan.md#top)";

        assert_eq!(link_targets(text), vec!["git.md", "plan.md"]);
    }
}
