use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde_json::Value;

const TITLE_MAX_CHARS: usize = 120;

/// Calls `on_line` with each non-empty line of a JSONL file. Invalid UTF-8 lines are skipped.
pub fn for_each_line(path: &Path, mut on_line: impl FnMut(&str)) -> io::Result<()> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            return Ok(());
        }
        if let Ok(line) = std::str::from_utf8(&buffer) {
            let line = line.trim();
            if !line.is_empty() {
                on_line(line);
            }
        }
    }
}

pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.with_timezone(&Utc))
}

/// Plain text of a message `content` that is either a string or an array of content blocks.
pub fn content_text(content: &Value) -> Option<String> {
    let text = match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    if text.trim().is_empty() { None } else { Some(text) }
}

pub fn to_title(text: &str) -> Option<String> {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return None;
    }
    if collapsed.chars().count() <= TITLE_MAX_CHARS {
        return Some(collapsed);
    }
    let mut truncated: String = collapsed.chars().take(TITLE_MAX_CHARS - 1).collect();
    truncated.push('…');
    Some(truncated)
}

pub fn later(current: Option<DateTime<Utc>>, candidate: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    match (current, candidate) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn content_text_handles_strings_and_blocks() {
        assert_eq!(content_text(&json!("hi")), Some("hi".into()));
        let blocks = json!([{"type": "text", "text": "a"}, {"type": "image"}, {"type": "text", "text": "b"}]);
        assert_eq!(content_text(&blocks), Some("a\nb".into()));
        assert_eq!(content_text(&json!([{"type": "tool_result"}])), None);
        assert_eq!(content_text(&json!("   ")), None);
    }

    #[test]
    fn title_collapses_whitespace_and_truncates() {
        assert_eq!(to_title("  fix\n the   bug "), Some("fix the bug".into()));
        let long = "x".repeat(200);
        let title = to_title(&long).unwrap();
        assert_eq!(title.chars().count(), TITLE_MAX_CHARS);
        assert!(title.ends_with('…'));
        assert_eq!(to_title(" \n "), None);
    }

    #[test]
    fn parses_rfc3339_with_offsets() {
        let parsed = parse_timestamp("2026-11-05T10:00:00+02:00").unwrap();
        assert_eq!(parsed.to_rfc3339(), "2026-11-05T08:00:00+00:00");
        assert!(parse_timestamp("yesterday").is_none());
    }

    #[test]
    fn for_each_line_skips_blank_and_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.jsonl");
        std::fs::write(&path, b"a\n\n\xff\xfe\nb").unwrap();
        let mut lines = Vec::new();
        for_each_line(&path, |line| lines.push(line.to_string())).unwrap();
        assert_eq!(lines, vec!["a", "b"]);
    }
}
