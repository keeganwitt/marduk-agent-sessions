//! Copilot Chat in VS Code (and VS Code Insiders and VSCodium, which store chats the same way):
//! one file per chat, `<User>/workspaceStorage/<workspace>/chatSessions/<id>.json|.jsonl`, and
//! `<User>/globalStorage/emptyWindowChatSessions/` for chats in a window with no folder open.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::{home_dir, total_size};
use crate::adapter::ScanProgress;
use crate::adapter::{AgentAdapter, DeletePlan, Mechanics, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl;
use crate::session::{Fingerprint, Session, note_model};
use crate::transcript::{Role, Transcript, TranscriptBuilder};

const PRODUCTS: [&str; 3] = ["Code", "Code - Insiders", "VSCodium"];

const HISTORY_WARNING: &str = "VS Code keeps its own chat history list, which may still show \
     this chat until you use Delete on it in VS Code.";

/// `<User>`, the folder VS Code keeps a user's settings and storage in, for `product`.
pub fn user_dir(home: &Path, product: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    let base = home.join("Library").join("Application Support");
    #[cfg(windows)]
    let base = super::env_dir("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = super::env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
    base.join(product).join("User")
}

pub struct VsCodeChatAdapter {
    user_dirs: Vec<PathBuf>,
    cache: ParseCache<Fingerprint, Parsed>,
}

impl VsCodeChatAdapter {
    pub fn new(user_dirs: Vec<PathBuf>) -> Self {
        Self {
            user_dirs,
            cache: ParseCache::default(),
        }
    }

    /// `MARDUK_VSCODE_USER_DIR` replaces every product's folder with one, for the demo home.
    pub fn from_env() -> Self {
        if let Some(dir) = super::env_dir("MARDUK_VSCODE_USER_DIR") {
            return Self::new(vec![dir]);
        }
        let home = home_dir();
        Self::new(
            PRODUCTS
                .iter()
                .map(|product| user_dir(&home, product))
                .collect(),
        )
    }

    /// Every chat file, with the files that belong to it and its workspace folder.
    fn locations(&self, result: &mut ScanResult) -> Vec<Location> {
        let mut locations = Vec::new();
        for user_dir in &self.user_dirs {
            let workspaces = user_dir.join("workspaceStorage");
            match fs::read_dir(&workspaces) {
                Ok(entries) => {
                    let mut dirs: Vec<PathBuf> = entries
                        .flatten()
                        .map(|entry| entry.path())
                        .filter(|path| path.is_dir())
                        .collect();
                    dirs.sort();
                    for workspace in dirs {
                        let project = workspace_folder(&workspace);
                        let editing = workspace.join("chatEditingSessions");
                        chat_files(
                            &workspace.join("chatSessions"),
                            project.as_deref(),
                            Some(&editing),
                            &mut locations,
                            result,
                        );
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => result.record_error(format!("{}: {error}", workspaces.display())),
            }
            chat_files(
                &user_dir
                    .join("globalStorage")
                    .join("emptyWindowChatSessions"),
                None,
                None,
                &mut locations,
                result,
            );
        }
        locations
    }

    fn read(
        &self,
        path: &Path,
        progress: &dyn ScanProgress,
    ) -> std::io::Result<(Fingerprint, Parsed)> {
        let fingerprint = fs_util::fingerprint(path)?;
        let parsed = self.cache.get_or_parse(path, fingerprint, || {
            read_state(path).map(|state| parse_state(&state))
        })?;
        progress.advance(fingerprint.len);
        Ok((fingerprint, parsed))
    }
}

/// A chat's files in one place: its `.jsonl` (preferred, as VS Code does) and/or `.json`, and
/// its edit checkpoints.
struct Location {
    id: String,
    primary: PathBuf,
    files: Vec<PathBuf>,
    project: Option<String>,
}

fn chat_files(
    dir: &Path,
    project: Option<&str>,
    editing: Option<&Path>,
    locations: &mut Vec<Location>,
    result: &mut ScanResult,
) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            result.record_error(format!("{}: {error}", dir.display()));
            return;
        }
    };
    let mut by_id: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext == "json" || ext == "jsonl")
        {
            let id = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            by_id.entry(id).or_default().push(path);
        }
    }
    let mut ids: Vec<String> = by_id.keys().cloned().collect();
    ids.sort();
    for id in ids {
        let mut files = by_id.remove(&id).unwrap_or_default();
        files.sort();
        let primary = files
            .iter()
            .find(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .unwrap_or(&files[0])
            .clone();
        if let Some(checkpoints) = editing.map(|dir| dir.join(&id)).filter(|dir| dir.is_dir()) {
            files.push(checkpoints);
        }
        locations.push(Location {
            id,
            primary,
            files,
            project: project.map(str::to_string),
        });
    }
}

/// The folder a workspace is for, from its `workspace.json`: a folder URI, or a
/// `.code-workspace` file whose first folder is used. Remote (non-`file:`) workspaces have none.
fn workspace_folder(workspace: &Path) -> Option<String> {
    let text = fs::read_to_string(workspace.join("workspace.json")).ok()?;
    let config: Value = serde_json::from_str(&text).ok()?;
    if let Some(folder) = config["folder"].as_str() {
        return file_uri_path(folder);
    }
    let file = PathBuf::from(file_uri_path(config["workspace"].as_str()?)?);
    let workspace_config: Value = serde_json::from_str(&fs::read_to_string(&file).ok()?).ok()?;
    let first = workspace_config["folders"][0]["path"].as_str()?;
    let path = file.parent()?.join(first);
    Some(path.to_string_lossy().into_owned())
}

/// `file:///Users/me/My%20App` → `/Users/me/My App`; `file:///c%3A/work` → `c:/work`.
fn file_uri_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);
    let windows_drive = decoded.len() > 2
        && decoded.starts_with('/')
        && decoded.as_bytes()[2] == b':'
        && decoded.as_bytes()[1].is_ascii_alphabetic();
    Some(if windows_drive {
        decoded[1..].to_string()
    } else {
        decoded
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16)
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A chat's state: the `.json` file itself, or the `.jsonl` change log replayed.
fn read_state(path: &Path) -> std::io::Result<Value> {
    if path.extension().is_some_and(|ext| ext == "jsonl") {
        let mut state = Value::Null;
        jsonl::for_each_line(path, |line| {
            if let Ok(entry) = serde_json::from_str::<Value>(line) {
                apply(&mut state, &entry);
            }
        })?;
        Ok(state)
    } else {
        let text = fs::read_to_string(path)?;
        serde_json::from_str(&text).map_err(std::io::Error::other)
    }
}

/// One change-log entry: `kind` 0 replaces the whole state, 1 sets the value at path `k`, 2
/// pushes `v`'s items onto the array at `k` (first truncating it to `i` if given), 3 deletes.
fn apply(state: &mut Value, entry: &Value) {
    let path: &[Value] = entry["k"].as_array().map(Vec::as_slice).unwrap_or_default();
    match entry["kind"].as_u64() {
        Some(0) => *state = entry["v"].clone(),
        Some(1) => *slot(state, path) = entry["v"].clone(),
        Some(2) => {
            let target = slot(state, path);
            if !target.is_array() {
                *target = Value::Array(Vec::new());
            }
            let array = target.as_array_mut().expect("just made an array");
            if let Some(length) = entry["i"].as_u64() {
                array.truncate(length as usize);
            }
            if let Some(items) = entry["v"].as_array() {
                array.extend(items.iter().cloned());
            }
        }
        Some(3) => {
            if let Some((last, parent)) = path.split_last() {
                match (slot(state, parent), last) {
                    (Value::Object(map), Value::String(key)) => {
                        map.remove(key);
                    }
                    (Value::Array(array), Value::Number(index)) => {
                        if let Some(index) = index.as_u64().map(|index| index as usize)
                            && index < array.len()
                        {
                            array.remove(index);
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// The value at `path` (object keys and array indices), created as needed.
fn slot<'a>(state: &'a mut Value, path: &[Value]) -> &'a mut Value {
    let mut current = state;
    for segment in path {
        current = match segment {
            Value::Number(index) => {
                let index = index.as_u64().unwrap_or(0) as usize;
                if !current.is_array() {
                    *current = Value::Array(Vec::new());
                }
                let array = current.as_array_mut().expect("just made an array");
                if array.len() <= index {
                    array.resize(index + 1, Value::Null);
                }
                &mut array[index]
            }
            other => {
                let key = other
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| other.to_string());
                if !current.is_object() {
                    *current = Value::Object(serde_json::Map::new());
                }
                current
                    .as_object_mut()
                    .expect("just made an object")
                    .entry(key)
                    .or_insert(Value::Null)
            }
        };
    }
    current
}

#[derive(Default, Clone)]
struct Parsed {
    id: Option<String>,
    title: Option<String>,
    first_prompt: Option<String>,
    created: Option<DateTime<Utc>>,
    last: Option<DateTime<Utc>>,
    models: Vec<String>,
    requests: usize,
}

fn millis(value: &Value) -> Option<DateTime<Utc>> {
    value.as_i64().and_then(DateTime::from_timestamp_millis)
}

fn parse_state(state: &Value) -> Parsed {
    let requests = state["requests"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut parsed = Parsed {
        id: state["sessionId"].as_str().map(str::to_string),
        title: state["customTitle"].as_str().map(str::to_string),
        created: millis(&state["creationDate"]),
        last: millis(&state["lastMessageDate"]),
        requests: requests.len(),
        ..Parsed::default()
    };
    for request in requests {
        if parsed.first_prompt.is_none() {
            parsed.first_prompt = request["message"]["text"]
                .as_str()
                .filter(|text| !text.trim().is_empty())
                .map(str::to_string);
        }
        if let Some(model) = request["modelId"].as_str() {
            note_model(&mut parsed.models, model.trim_start_matches("copilot/"));
        }
        for time in [
            &request["timestamp"],
            &request["responseTimestamp"],
            &request["modelState"]["completedAt"],
        ] {
            if let Some(time) = millis(time) {
                parsed.last = Some(parsed.last.map_or(time, |last| last.max(time)));
                parsed.created = Some(parsed.created.map_or(time, |created| created.min(time)));
            }
        }
    }
    parsed
}

impl AgentAdapter for VsCodeChatAdapter {
    fn id(&self) -> &'static str {
        "vscode-copilot"
    }

    fn display_name(&self) -> &'static str {
        "Copilot Chat (VS Code)"
    }

    fn roots(&self) -> Vec<PathBuf> {
        self.user_dirs.clone()
    }

    /// Chats without a request are drafts VS Code itself hides. A chat copied from an empty
    /// window into a workspace is one session whose files include both copies.
    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let locations = self.locations(&mut result);
        let mut seen = Vec::new();
        let mut by_id: HashMap<String, (Location, Fingerprint, Parsed)> = HashMap::new();
        let mut extra_files: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for location in locations {
            seen.push(location.primary.clone());
            let (fingerprint, parsed) = match self.read(&location.primary, progress) {
                Ok(read) => read,
                Err(error) => {
                    result.record_error(format!("{}: {error}", location.primary.display()));
                    continue;
                }
            };
            if parsed.requests == 0 {
                continue;
            }
            let id = parsed.id.clone().unwrap_or_else(|| location.id.clone());
            match by_id.remove(&id) {
                Some(existing) if existing.0.project.is_some() || location.project.is_none() => {
                    extra_files
                        .entry(id.clone())
                        .or_default()
                        .extend(location.files);
                    by_id.insert(id, existing);
                }
                Some(existing) => {
                    extra_files
                        .entry(id.clone())
                        .or_default()
                        .extend(existing.0.files);
                    by_id.insert(id, (location, fingerprint, parsed));
                }
                None => {
                    by_id.insert(id, (location, fingerprint, parsed));
                }
            }
        }
        for (id, (location, fingerprint, parsed)) in by_id {
            let mut files = location.files;
            files.extend(extra_files.remove(&id).unwrap_or_default());
            let title = [parsed.title, parsed.first_prompt]
                .into_iter()
                .flatten()
                .find_map(|text| jsonl::to_title(&text));
            result.sessions.push(Session {
                agent: "vscode-copilot",
                id,
                project_dir: location.project,
                title,
                created: parsed
                    .created
                    .or_else(|| fs_util::created(&location.primary)),
                last_activity: parsed.last.unwrap_or_else(|| fingerprint.modified.into()),
                size_bytes: files.iter().map(|file| fs_util::path_size(file)).sum(),
                archived: false,
                primary_file: location.primary,
                files,
                fingerprint,
                parent_id: None,
                deleted_with_parent: false,
                part_of_parent: false,
                projectless: false,
                models: parsed.models,
                agent_type: None,
                empty: false,
            });
        }
        self.cache.retain(&seen.into_iter().collect());
        result
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let locations = self.locations(&mut ScanResult::default());
        total_size(
            &locations
                .into_iter()
                .map(|location| location.primary)
                .collect::<Vec<_>>(),
        )
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: format!(
                "Moves the chat's file, and the checkpoints of files it edited, to the Trash. \
                 VS Code has no command to delete a chat. {HISTORY_WARNING}"
            ),
            archive: "VS Code has no chat archive.".to_string(),
        }
    }

    fn delete_plan(&self, session: &Session) -> DeletePlan {
        DeletePlan::Trash {
            files: session.files.clone(),
            warning: Some(HISTORY_WARNING.to_string()),
        }
    }

    fn transcript(&self, session: &Session) -> std::io::Result<Transcript> {
        Ok(build_transcript(&read_state(&session.primary_file)?))
    }
}

fn text_of(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(map) => map.get("value").and_then(text_of),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(text_of).collect();
            (!parts.is_empty()).then(|| parts.join("\n"))
        }
        _ => None,
    }
}

fn build_transcript(state: &Value) -> Transcript {
    let mut builder = TranscriptBuilder::default();
    let requests = state["requests"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    for request in requests {
        let asked = millis(&request["timestamp"]);
        if let Some(text) = request["message"]["text"].as_str() {
            builder.text(Role::User, text, asked);
        }
        let answered = millis(&request["responseTimestamp"])
            .or_else(|| millis(&request["modelState"]["completedAt"]))
            .or(asked);
        let old_results = &request["result"]["metadata"]["toolCallResults"];
        let mut reply = String::new();
        let flush = |reply: &mut String, builder: &mut TranscriptBuilder| {
            if !reply.trim().is_empty() {
                builder.text(Role::Assistant, reply, answered);
            }
            reply.clear();
        };
        for part in request["response"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            match part["kind"].as_str() {
                Some("toolInvocationSerialized") => {
                    flush(&mut reply, &mut builder);
                    let id = part["toolCallId"].as_str();
                    let input = text_of(&part["invocationMessage"])
                        .or_else(|| part["resultDetails"]["input"].as_str().map(str::to_string))
                        .unwrap_or_default();
                    builder.tool_call(
                        id,
                        part["toolId"].as_str().unwrap_or("tool"),
                        &input,
                        answered,
                    );
                    let output = text_of(&part["resultDetails"]["output"])
                        .or_else(|| id.and_then(|id| text_of(&old_results[id]["content"])));
                    if let Some(output) = output {
                        builder.tool_result(id, &output, answered);
                    }
                }
                None | Some("markdownContent") => {
                    if let Some(text) = part["value"].as_str() {
                        reply.push_str(text);
                    }
                }
                _ => {}
            }
        }
        flush(&mut reply, &mut builder);
    }
    builder.finish()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const ID: &str = "92fbbd03-adf8-4885-9078-f5f755c9be33";
    const NEW_ID: &str = "b523414b-288f-40a8-a19f-49be4f993958";

    fn chat_v3(id: &str) -> Value {
        json!({
            "version": 3,
            "sessionId": id,
            "creationDate": 1_762_400_000_000_i64,
            "lastMessageDate": 1_762_400_600_000_i64,
            "requests": [{
                "requestId": "r1",
                "timestamp": 1_762_400_000_000_i64,
                "modelId": "copilot/gpt-6.1-sol",
                "message": { "text": "Why does the build fail?" },
                "response": [
                    { "value": "Let me look at the " },
                    { "value": "logs." },
                    { "kind": "toolInvocationSerialized", "toolId": "run_in_terminal",
                      "toolCallId": "t1", "invocationMessage": { "value": "Running `make`" } },
                    { "value": "A missing header." }
                ],
                "result": { "metadata": { "toolCallResults": {
                    "t1": { "content": [{ "value": "fatal error: x.h not found" }] }
                } } }
            }]
        })
    }

    /// `file:///…` with forward slashes and spaces escaped, as VS Code writes on every OS.
    fn file_uri(path: &Path) -> String {
        let path = path
            .to_string_lossy()
            .replace('\\', "/")
            .replace(' ', "%20");
        format!(
            "file://{}{path}",
            if path.starts_with('/') { "" } else { "/" }
        )
    }

    /// One workspace with a `.json` chat (whose checkpoints exist) and a `.jsonl` chat, plus an
    /// empty draft, and an empty window holding a copy of the `.json` chat.
    fn fixture() -> (tempfile::TempDir, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let user = root.path().join("User");
        let workspace = user.join("workspaceStorage").join("dc517b8e");
        let project = root.path().join("My App");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(workspace.join("chatSessions")).unwrap();
        fs::write(
            workspace.join("workspace.json"),
            json!({ "folder": file_uri(&project) }).to_string(),
        )
        .unwrap();
        fs::write(
            workspace.join("chatSessions").join(format!("{ID}.json")),
            chat_v3(ID).to_string(),
        )
        .unwrap();
        fs::create_dir_all(workspace.join("chatEditingSessions").join(ID)).unwrap();
        let log = [
            json!({ "kind": 0, "v": { "sessionId": NEW_ID, "creationDate": 1_762_500_000_000_i64, "requests": [] } }),
            json!({ "kind": 2, "k": ["requests"], "v": [{ "message": { "text": "Draft that was replaced" } }] }),
            json!({ "kind": 2, "k": ["requests"], "i": 0, "v": [{
                "message": { "text": "Add a dark theme" }, "timestamp": 1_762_500_000_000_i64,
                "modelId": "copilot/claude-sonnet-5-5", "response": [] }] }),
            json!({ "kind": 1, "k": ["requests", 0, "response"], "v": [{ "value": "Done." }] }),
            json!({ "kind": 1, "k": ["requests", 0, "modelState"], "v": { "value": 1, "completedAt": 1_762_500_900_000_i64 } }),
            json!({ "kind": 1, "k": ["customTitle"], "v": "Dark theme" }),
            json!({ "kind": 1, "k": ["inputState"], "v": { "text": "half-typed" } }),
            json!({ "kind": 3, "k": ["inputState"] }),
        ];
        fs::write(
            workspace
                .join("chatSessions")
                .join(format!("{NEW_ID}.jsonl")),
            log.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        fs::write(
            workspace.join("chatSessions").join("empty.json"),
            json!({ "sessionId": "empty", "requests": [] }).to_string(),
        )
        .unwrap();
        let empty_window = user.join("globalStorage").join("emptyWindowChatSessions");
        fs::create_dir_all(&empty_window).unwrap();
        fs::write(
            empty_window.join(format!("{ID}.json")),
            chat_v3(ID).to_string(),
        )
        .unwrap();
        (root, user)
    }

    #[test]
    fn replays_the_change_log() {
        let mut state = Value::Null;
        for entry in [
            json!({ "kind": 0, "v": { "a": [1, 2, 3], "b": { "c": 1 } } }),
            json!({ "kind": 2, "k": ["a"], "i": 1, "v": [9] }),
            json!({ "kind": 1, "k": ["b", "d"], "v": "x" }),
            json!({ "kind": 3, "k": ["b", "c"] }),
            json!({ "kind": 1, "k": ["e", 1], "v": true }),
        ] {
            apply(&mut state, &entry);
        }

        assert_eq!(
            state,
            json!({ "a": [1, 9], "b": { "d": "x" }, "e": [null, true] })
        );
    }

    #[test]
    fn scans_chats_merging_copies_and_skipping_drafts() {
        let (root, user) = fixture();
        let adapter = VsCodeChatAdapter::new(vec![user.clone()]);

        let result = adapter.scan();

        assert!(result.complete, "{:?}", result.errors);
        let mut sessions = result.sessions;
        sessions.sort_by(|a, b| a.id.cmp(&b.id));
        assert_eq!(sessions.len(), 2);
        let old = sessions.iter().find(|session| session.id == ID).unwrap();
        assert_eq!(old.title.as_deref(), Some("Why does the build fail?"));
        assert_eq!(
            old.project_dir.as_deref().map(PathBuf::from),
            Some(root.path().join("My App"))
        );
        assert_eq!(old.models, ["gpt-6.1-sol"]);
        assert_eq!(old.last_activity.timestamp_millis(), 1_762_400_600_000);
        assert_eq!(old.files.len(), 3, "{:?}", old.files);
        assert!(
            old.files
                .iter()
                .any(|file| file.ends_with(ID) && file.is_dir())
        );
        assert!(
            old.files
                .iter()
                .any(|file| file.starts_with(user.join("globalStorage")))
        );
        let new = sessions
            .iter()
            .find(|session| session.id == NEW_ID)
            .unwrap();
        assert_eq!(new.title.as_deref(), Some("Dark theme"));
        assert_eq!(new.models, ["claude-sonnet-5-5"]);
        assert_eq!(new.last_activity.timestamp_millis(), 1_762_500_900_000);
    }

    #[test]
    fn reads_transcripts_from_both_formats() {
        let (_root, user) = fixture();
        let adapter = VsCodeChatAdapter::new(vec![user]);
        let sessions = adapter.scan().sessions;
        let transcript_of = |id: &str| {
            let session = sessions.iter().find(|session| session.id == id).unwrap();
            let transcript = adapter.transcript(session).unwrap();
            transcript
                .head
                .into_iter()
                .chain(transcript.tail)
                .collect::<Vec<_>>()
        };

        let old = transcript_of(ID);
        assert_eq!(old[0].role, Role::User);
        assert_eq!(old[1].text, "Let me look at the logs.");
        let tool = old[2].tool.as_ref().unwrap();
        assert_eq!(tool.name, "run_in_terminal");
        assert_eq!(tool.output.as_deref(), Some("fatal error: x.h not found"));
        assert_eq!(old[3].text, "A missing header.");
        let new = transcript_of(NEW_ID);
        assert_eq!(new[0].text, "Add a dark theme");
        assert_eq!(new[1].text, "Done.");
    }

    #[test]
    fn trashes_every_copy_with_a_history_warning() {
        let (_root, user) = fixture();
        let adapter = VsCodeChatAdapter::new(vec![user]);
        let session = adapter
            .scan()
            .sessions
            .into_iter()
            .find(|session| session.id == ID)
            .unwrap();

        match adapter.delete_plan(&session) {
            DeletePlan::Trash { files, warning } => {
                assert_eq!(files, session.files);
                assert!(warning.unwrap().contains("Delete on it in VS Code"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn decodes_file_uris() {
        assert_eq!(
            file_uri_path("file:///Users/me/My%20App").as_deref(),
            Some("/Users/me/My App")
        );
        assert_eq!(
            file_uri_path("file:///c%3A/work").as_deref(),
            Some("c:/work")
        );
        assert_eq!(file_uri_path("vscode-remote://ssh-remote+host/srv"), None);
    }
}
