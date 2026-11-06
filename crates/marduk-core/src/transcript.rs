use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};

const HEAD_MESSAGES: usize = 500;
const TAIL_MESSAGES: usize = 500;
const MAX_MESSAGE_CHARS: usize = 20_000;
const MAX_TOOL_CHARS: usize = 4_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    /// A tool call, with its output attached when the agent recorded one.
    Tool,
    /// Text the harness injected (system prompts, AGENTS.md, command wrappers).
    Context,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCall {
    pub name: String,
    pub output: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub role: Role,
    /// Message text, or the tool's input for tool calls.
    pub text: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub tool: Option<ToolCall>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transcript {
    /// The first messages of the session.
    pub head: Vec<Message>,
    /// Messages skipped between `head` and `tail` to keep long sessions viewable.
    pub omitted: usize,
    /// The last messages of the session.
    pub tail: Vec<Message>,
    /// True when some message or tool output was shortened.
    pub shortened: bool,
    /// How the transcript was read, when it wasn't the usual way.
    pub note: Option<String>,
}

/// Builds a transcript in one pass, keeping only the first and last messages of long
/// sessions, and attaching tool results to the call they answer.
#[derive(Default)]
pub struct TranscriptBuilder {
    head: Vec<Message>,
    tail: VecDeque<(usize, Message)>,
    total: usize,
    calls: HashMap<String, usize>,
    shortened: bool,
}

impl TranscriptBuilder {
    fn limit(&mut self, text: &str, max_chars: usize) -> String {
        if text.chars().count() <= max_chars {
            return text.to_string();
        }
        self.shortened = true;
        let mut shortened: String = text.chars().take(max_chars).collect();
        shortened.push_str("\n…");
        shortened
    }

    fn append(&mut self, message: Message) -> usize {
        let index = self.total;
        self.total += 1;
        if self.head.len() < HEAD_MESSAGES {
            self.head.push(message);
        } else {
            self.tail.push_back((index, message));
            if self.tail.len() > TAIL_MESSAGES {
                self.tail.pop_front();
            }
        }
        index
    }

    fn message_mut(&mut self, index: usize) -> Option<&mut Message> {
        if index < self.head.len() {
            return self.head.get_mut(index);
        }
        self.tail
            .iter_mut()
            .find(|(position, _)| *position == index)
            .map(|(_, message)| message)
    }

    pub fn text(&mut self, role: Role, text: &str, timestamp: Option<DateTime<Utc>>) {
        if text.trim().is_empty() {
            return;
        }
        let text = self.limit(text, MAX_MESSAGE_CHARS);
        self.append(Message {
            role,
            text,
            timestamp,
            tool: None,
        });
    }

    pub fn tool_call(
        &mut self,
        id: Option<&str>,
        name: &str,
        input: &str,
        timestamp: Option<DateTime<Utc>>,
    ) {
        let text = self.limit(input, MAX_TOOL_CHARS);
        let index = self.append(Message {
            role: Role::Tool,
            text,
            timestamp,
            tool: Some(ToolCall {
                name: name.to_string(),
                output: None,
            }),
        });
        if let Some(id) = id {
            self.calls.insert(id.to_string(), index);
        }
    }

    /// Attaches `output` to the call with this id, or records it on its own when the call
    /// is unknown or was skipped.
    pub fn tool_result(
        &mut self,
        id: Option<&str>,
        output: &str,
        timestamp: Option<DateTime<Utc>>,
    ) {
        let output = self.limit(output, MAX_TOOL_CHARS);
        let index = id.and_then(|id| self.calls.remove(id));
        if let Some(call) = index.and_then(|index| self.message_mut(index))
            && let Some(tool) = call.tool.as_mut()
        {
            tool.output = Some(output);
            return;
        }
        if index.is_some() {
            return;
        }
        self.append(Message {
            role: Role::Tool,
            text: String::new(),
            timestamp,
            tool: Some(ToolCall {
                name: "Tool result".to_string(),
                output: Some(output),
            }),
        });
    }

    pub fn finish(self) -> Transcript {
        let omitted = self.total - self.head.len() - self.tail.len();
        Transcript {
            head: self.head,
            omitted,
            tail: self.tail.into_iter().map(|(_, message)| message).collect(),
            shortened: self.shortened,
            note: None,
        }
    }
}

/// Tool inputs are often JSON; show strings as-is and everything else as pretty JSON.
pub fn tool_input_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// Text of a tool result, which is a string, an array of content blocks, or other JSON.
pub fn tool_output_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Array(_) => crate::jsonl::content_text(value).unwrap_or_default(),
        other => tool_input_text(other),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn attaches_tool_results_to_their_calls() {
        let mut builder = TranscriptBuilder::default();
        builder.text(Role::User, "list files", None);
        builder.tool_call(Some("t1"), "bash", "ls", None);
        builder.tool_result(Some("t1"), "a.txt", None);
        builder.tool_result(Some("unknown"), "orphan", None);

        let transcript = builder.finish();

        assert_eq!(transcript.head.len(), 3);
        assert_eq!(
            transcript.head[1].tool,
            Some(ToolCall {
                name: "bash".into(),
                output: Some("a.txt".into())
            })
        );
        assert_eq!(
            transcript.head[2].tool.as_ref().unwrap().output.as_deref(),
            Some("orphan")
        );
    }

    #[test]
    fn skips_empty_text() {
        let mut builder = TranscriptBuilder::default();
        builder.text(Role::User, "  ", None);
        assert!(builder.finish().head.is_empty());
    }

    #[test]
    fn keeps_first_and_last_messages_of_long_sessions() {
        let mut builder = TranscriptBuilder::default();
        let total = HEAD_MESSAGES + TAIL_MESSAGES + 7;
        for index in 0..total {
            builder.text(Role::User, &index.to_string(), None);
        }

        let transcript = builder.finish();

        assert_eq!(transcript.head.len(), HEAD_MESSAGES);
        assert_eq!(transcript.tail.len(), TAIL_MESSAGES);
        assert_eq!(transcript.omitted, 7);
        assert_eq!(transcript.head[0].text, "0");
        assert_eq!(
            transcript.tail.last().unwrap().text,
            (total - 1).to_string()
        );
    }

    #[test]
    fn result_for_a_skipped_call_is_dropped_with_it() {
        let mut builder = TranscriptBuilder::default();
        for _ in 0..HEAD_MESSAGES {
            builder.text(Role::User, "x", None);
        }
        builder.tool_call(Some("gone"), "bash", "ls", None);
        for _ in 0..TAIL_MESSAGES {
            builder.text(Role::User, "y", None);
        }
        builder.tool_result(Some("gone"), "out", None);

        let transcript = builder.finish();

        assert_eq!(transcript.omitted, 1);
        assert!(transcript.tail.iter().all(|message| message.tool.is_none()));
    }

    #[test]
    fn shortens_long_tool_output() {
        let mut builder = TranscriptBuilder::default();
        builder.tool_call(Some("t"), "bash", "ls", None);
        builder.tool_result(Some("t"), &"x".repeat(MAX_TOOL_CHARS + 10), None);
        let transcript = builder.finish();
        assert!(transcript.shortened);
        let output = transcript.head[0]
            .tool
            .as_ref()
            .unwrap()
            .output
            .as_ref()
            .unwrap();
        assert_eq!(output.chars().count(), MAX_TOOL_CHARS + 2);
    }

    #[test]
    fn tool_text_helpers() {
        assert_eq!(tool_input_text(&json!("ls -la")), "ls -la");
        assert_eq!(
            tool_input_text(&json!({"cmd": "ls"})),
            "{\n  \"cmd\": \"ls\"\n}"
        );
        assert_eq!(
            tool_output_text(&json!([{"type": "text", "text": "out"}])),
            "out"
        );
    }
}
