use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD};
use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;

use super::{config_dir, entries, idea_dirs, invalid_data, latest_copies, read_only};
use crate::adapter::{AgentAdapter, DeletePlan, Mechanics, ScanProgress, ScanResult};
use crate::cache::ParseCache;
use crate::fs_util;
use crate::jsonl::to_title;
use crate::session::{Fingerprint, Session};
use crate::transcript::{Role, Transcript, TranscriptBuilder, tool_input_text};

const ID: &str = "jetbrains-ai-assistant";
const DELETE: &str = "Delete chats through Chat History in the IntelliJ AI Chat tool window.";
const HISTORY_HEADER: &str = "AUI_EVENTS_V1";

pub struct AiAssistantAdapter {
    config: PathBuf,
    home: PathBuf,
    cache: ParseCache<Fingerprint, Arc<Workspace>>,
}

impl AiAssistantAdapter {
    pub fn new(config: PathBuf, home: PathBuf) -> Self {
        Self {
            config,
            home,
            cache: ParseCache::default(),
        }
    }

    pub fn from_env() -> Self {
        let home = super::super::home_dir();
        let config = super::super::env_dir("MARDUK_JETBRAINS_CONFIG_DIR")
            .unwrap_or_else(|| config_dir(&home));
        Self::new(config, home)
    }

    fn workspaces(installation: &Path, result: &mut ScanResult) -> Vec<PathBuf> {
        entries(&installation.join("workspace"), result, false)
            .into_iter()
            .filter(|p| p.extension().is_some_and(|ext| ext == "xml"))
            .collect()
    }

    fn load_workspace(&self, path: &Path, fingerprint: Fingerprint) -> io::Result<Arc<Workspace>> {
        self.cache
            .get_or_parse(path, fingerprint, || read_xml(path).map(Arc::new))
    }

    fn scan_workspace(
        &self,
        path: &Path,
        history: &Path,
        projects: &HashMap<String, String>,
        progress: &dyn ScanProgress,
        result: &mut ScanResult,
    ) -> io::Result<()> {
        let workspace_fingerprint = fs_util::fingerprint(path)?;
        progress.advance(workspace_fingerprint.len);
        let workspace = self.load_workspace(path, workspace_fingerprint)?;
        let workspace_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let project = projects
            .get(workspace_id)
            .cloned()
            .or_else(|| workspace.project_path());
        for chat in workspace.chats() {
            let Some(id) = chat
                .option("uid")
                .and_then(|option| option.value.as_deref())
            else {
                continue;
            };
            if !fs_util::looks_like_uuid(id) {
                result.record_error(format!("{}: invalid chat ID", path.display()));
                continue;
            }
            let event_file = history.join(format!("{id}.events"));
            let events = match fs_util::fingerprint(&event_file) {
                Ok(fingerprint) => Some(fingerprint),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => {
                    result.record_error(format!("{}: {error}", event_file.display()));
                    None
                }
            };
            let fingerprint = events.unwrap_or(workspace_fingerprint);
            let first_prompt = chat.messages().find_map(|message| {
                (message.role() == Some(Role::User))
                    .then(|| message.text())
                    .flatten()
            });
            let model = chat
                .option("chatModelId")
                .and_then(|option| option.value.as_deref());
            let agent_type = model.and_then(|model| model.strip_prefix("agent_"));
            let mut files = vec![path.to_path_buf()];
            if events.is_some() {
                files.insert(0, event_file);
            }
            let last_activity = chat
                .millis("modifiedAt")
                .unwrap_or_else(|| fingerprint.modified.into());
            result.sessions.push(Session {
                agent: ID,
                id: id.into(),
                project_dir: project.clone(),
                title: chat
                    .option("title")
                    .and_then(|option| option.title.as_ref())
                    .and_then(|title| title.option("text"))
                    .and_then(|option| option.value.as_deref())
                    .and_then(to_title)
                    .or_else(|| first_prompt.and_then(to_title)),
                created: chat
                    .option("statisticInformation")
                    .and_then(|option| option.statistics.as_ref())
                    .and_then(|stats| stats.millis("timestamp")),
                last_activity,
                size_bytes: events.map_or_else(
                    || {
                        chat.messages()
                            .filter_map(XmlOptions::text)
                            .map(|text| text.len() as u64)
                            .sum()
                    },
                    |fingerprint| fingerprint.len,
                ),
                archived: false,
                pinned: None,
                primary_file: files[0].clone(),
                files,
                fingerprint,
                parent_id: None,
                deleted_with_parent: false,
                part_of_parent: false,
                projectless: false,
                models: model
                    .filter(|model| agent_type.is_none() && !model.is_empty())
                    .map(|model| vec![model.into()])
                    .unwrap_or_default(),
                agent_type: agent_type.map(str::to_owned),
                empty: events.is_none()
                    && !chat
                        .messages()
                        .any(|message| message.role().is_some() && message.text().is_some()),
            });
        }
        Ok(())
    }
}

impl AgentAdapter for AiAssistantAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "JetBrains AI Assistant"
    }

    fn roots(&self) -> Vec<PathBuf> {
        vec![self.config.clone()]
    }

    fn estimate_scan_bytes(&self) -> u64 {
        let mut result = ScanResult::empty_complete();
        idea_dirs(&self.config, &mut result)
            .into_iter()
            .map(|installation| {
                super::super::total_size(&Self::workspaces(&installation, &mut result))
            })
            .sum()
    }

    fn scan_with(&self, progress: &dyn ScanProgress) -> ScanResult {
        let mut result = ScanResult::empty_complete();
        let mut seen = HashSet::new();
        for installation in idea_dirs(&self.config, &mut result) {
            let recent_path = installation.join("options/recentProjects.xml");
            let projects = match read_xml::<RecentProjects>(&recent_path) {
                Ok(recent) => recent.paths(&self.home),
                Err(error) => {
                    if error.kind() != io::ErrorKind::NotFound {
                        result.record_error(format!("{}: {error}", recent_path.display()));
                    }
                    HashMap::new()
                }
            };
            let history = installation.join("aia-task-history");
            for path in Self::workspaces(&installation, &mut result) {
                if let Err(error) =
                    self.scan_workspace(&path, &history, &projects, progress, &mut result)
                {
                    result.record_error(format!("{}: {error}", path.display()));
                }
                seen.insert(path);
            }
        }
        self.cache.retain(&seen);
        result.sessions = latest_copies(result.sessions);
        result
    }

    fn delete_plan(&self, _session: &Session) -> DeletePlan {
        read_only(DELETE)
    }

    fn mechanics(&self) -> Mechanics {
        Mechanics {
            delete: DELETE.into(),
            archive: "AI Assistant has no chat archive.".into(),
        }
    }

    fn transcript(&self, session: &Session) -> io::Result<Transcript> {
        if session
            .primary_file
            .extension()
            .is_some_and(|ext| ext == "events")
        {
            return read_events(&session.primary_file);
        }
        let path = &session.primary_file;
        let workspace = self.load_workspace(path, fs_util::fingerprint(path)?)?;
        let chat = workspace
            .chats()
            .find(|chat| {
                chat.option("uid")
                    .and_then(|option| option.value.as_deref())
                    == Some(&session.id)
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Chat is missing from the workspace",
                )
            })?;
        let mut builder = TranscriptBuilder::default();
        for message in chat.messages() {
            if let (Some(role), Some(text)) = (message.role(), message.text()) {
                builder.text(role, text, None);
            }
        }
        Ok(builder.finish())
    }
}

fn read_xml<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    quick_xml::de::from_reader(BufReader::new(File::open(path)?)).map_err(invalid_data)
}

#[derive(Deserialize)]
struct Workspace {
    #[serde(rename = "component", default)]
    components: Vec<WorkspaceComponent>,
}

impl Workspace {
    fn chats(&self) -> impl Iterator<Item = &XmlOptions> {
        self.components
            .iter()
            .find(|component| component.name == "ChatSessionStateTemp")
            .and_then(|component| {
                component
                    .options
                    .iter()
                    .find(|option| option.name == "chats")
            })
            .and_then(|option| option.list.as_ref())
            .into_iter()
            .flat_map(|list| &list.chats)
    }

    fn project_path(&self) -> Option<String> {
        let url = &self
            .components
            .iter()
            .find(|component| component.name == "ProjectRoots")?
            .roots
            .first()?
            .url;
        let path = url.strip_prefix("file://")?;
        Path::new(path).is_absolute().then(|| path.into())
    }
}

#[derive(Deserialize)]
struct WorkspaceComponent {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "project-root", default)]
    roots: Vec<XmlRoot>,
    #[serde(rename = "option", default)]
    options: Vec<XmlOption>,
}

#[derive(Deserialize)]
struct XmlRoot {
    #[serde(rename = "@url")]
    url: String,
}

#[derive(Deserialize)]
struct XmlOptions {
    #[serde(rename = "option", default)]
    options: Vec<XmlOption>,
}

impl XmlOptions {
    fn option(&self, name: &str) -> Option<&XmlOption> {
        self.options.iter().find(|option| option.name == name)
    }

    fn millis(&self, name: &str) -> Option<chrono::DateTime<chrono::Utc>> {
        DateTime::from_timestamp_millis(self.option(name)?.value.as_deref()?.parse().ok()?)
    }

    fn messages(&self) -> impl Iterator<Item = &Self> {
        self.option("messages")
            .and_then(|option| option.list.as_ref())
            .into_iter()
            .flat_map(|list| &list.messages)
    }

    fn role(&self) -> Option<Role> {
        match self
            .option("author")
            .and_then(|option| option.value.as_deref())
        {
            None | Some("User") => Some(Role::User),
            Some("Assistant") => Some(Role::Assistant),
            Some("System") => Some(Role::Context),
            _ => None,
        }
    }

    fn text(&self) -> Option<&str> {
        ["displayContent", "internalContent"]
            .iter()
            .filter_map(|name| self.option(name)?.value.as_deref())
            .find(|text| !text.trim().is_empty())
    }
}

#[derive(Deserialize)]
struct XmlOption {
    #[serde(rename = "@name", default)]
    name: String,
    #[serde(rename = "@value")]
    value: Option<String>,
    list: Option<XmlList>,
    #[serde(rename = "SerializedChatTitle")]
    title: Option<XmlOptions>,
    #[serde(rename = "ChatStatisticInformation")]
    statistics: Option<XmlOptions>,
}

#[derive(Deserialize)]
struct XmlList {
    #[serde(rename = "SerializedChat", default)]
    chats: Vec<XmlOptions>,
    #[serde(rename = "SerializedChatMessage", default)]
    messages: Vec<XmlOptions>,
}

#[derive(Deserialize)]
struct RecentProjects {
    #[serde(rename = "component", default)]
    components: Vec<RecentComponent>,
}

impl RecentProjects {
    fn paths(self, home: &Path) -> HashMap<String, String> {
        self.components
            .into_iter()
            .flat_map(|component| component.options)
            .flat_map(|option| option.map)
            .flat_map(|map| map.entries)
            .filter_map(|entry| {
                let id = entry.value?.info?.workspace_id?;
                let path = entry
                    .path
                    .strip_prefix("$USER_HOME$/")
                    .map(|relative| home.join(relative).to_string_lossy().into_owned())
                    .unwrap_or(entry.path);
                Path::new(&path).is_absolute().then_some((id, path))
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct RecentComponent {
    #[serde(rename = "option", default)]
    options: Vec<RecentOption>,
}

#[derive(Deserialize)]
struct RecentOption {
    map: Option<RecentMap>,
}

#[derive(Deserialize)]
struct RecentMap {
    #[serde(rename = "entry", default)]
    entries: Vec<RecentEntry>,
}

#[derive(Deserialize)]
struct RecentEntry {
    #[serde(rename = "@key", default)]
    path: String,
    value: Option<RecentValue>,
}

#[derive(Deserialize)]
struct RecentValue {
    #[serde(rename = "RecentProjectMetaInfo")]
    info: Option<RecentInfo>,
}

#[derive(Deserialize)]
struct RecentInfo {
    #[serde(rename = "@projectWorkspaceId")]
    workspace_id: Option<String>,
}

fn read_events(path: &Path) -> io::Result<Transcript> {
    let mut lines = BufReader::new(File::open(path)?).lines();
    if lines.next().transpose()?.as_deref() != Some(HISTORY_HEADER) {
        return Err(invalid_data("Unrecognized AI Assistant history format"));
    }

    let mut builder = TranscriptBuilder::default();
    let mut markdown = String::new();
    let mut markdown_step = String::new();
    let mut tools = HashSet::new();
    let mut results = HashSet::new();
    for line in lines {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let bytes = STANDARD.decode(line.trim()).map_err(invalid_data)?;
        let record: Value = serde_json::from_slice(&bytes).map_err(invalid_data)?;
        let block = &record["event"];
        let kind = block["kind"]
            .as_str()
            .and_then(|kind| kind.rsplit('.').next());
        let step = block["stepId"].as_str().unwrap_or_default();
        if kind == Some("MarkdownBlockUpdatedEvent") {
            if markdown_step != step {
                flush_markdown(&mut builder, &mut markdown);
                step.clone_into(&mut markdown_step);
            }
            if let Some(text) = block["text"].as_str() {
                text.clone_into(&mut markdown);
            }
            if let Some(chunk) = block["textChunk"].as_str() {
                markdown.push_str(chunk);
            }
            continue;
        }
        if let Some(prompt) = record["prompt"].as_str() {
            flush_markdown(&mut builder, &mut markdown);
            builder.text(Role::User, prompt, None);
            tools.clear();
            results.clear();
        } else if matches!(
            kind,
            Some("TerminalBlockUpdatedEvent" | "McpBlockUpdatedEvent" | "ToolBlockUpdatedEvent")
        ) {
            flush_markdown(&mut builder, &mut markdown);
            if tools.insert(step.to_owned()) {
                let name = block["toolName"]
                    .as_str()
                    .or(block["toolType"].as_str())
                    .unwrap_or(if kind == Some("TerminalBlockUpdatedEvent") {
                        "Terminal"
                    } else {
                        "Tool"
                    });
                let input = block["command"]
                    .as_str()
                    .or(block["text"].as_str())
                    .map(str::to_owned)
                    .unwrap_or_else(|| tool_input_text(&block["arguments"]));
                builder.tool_call(Some(step), name, &input, None);
            }
            if let Some(output) = block["output"].as_str().or(block["details"].as_str())
                && !output.trim().is_empty()
                && results.insert(step.to_owned())
            {
                builder.tool_result(Some(step), output, None);
            }
        }
    }
    flush_markdown(&mut builder, &mut markdown);
    Ok(builder.finish())
}

fn flush_markdown(builder: &mut TranscriptBuilder, markdown: &mut String) {
    builder.text(Role::Assistant, markdown, None);
    markdown.clear();
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;

    use serde_json::json;

    use super::*;

    const CHAT: &str = "f1bdae7b-ec2f-43a3-87d5-aa12ac62644c";
    const AGENT: &str = "805c4b09-e39b-48d3-8742-c1182a687904";

    fn write_workspace(config: &Path, modified_at: i64) {
        let workspace = config.join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(config.join("options")).unwrap();
        fs::write(
            config.join("options/recentProjects.xml"),
            r#"
            <application><component name="RecentProjectsManager">
              <option name="additionalInfo"><map><entry key="$USER_HOME$/Projects/demo">
                <value><RecentProjectMetaInfo projectWorkspaceId="workspace-id"/></value>
              </entry></map></option>
            </component></application>
        "#,
        )
        .unwrap();
        fs::write(workspace.join("workspace-id.xml"), format!(r#"
            <project><component name="ChatSessionStateTemp">
              <option name="chats"><list>
                <SerializedChat><option name="uid" value="{CHAT}"/>
                  <option name="modifiedAt" value="{modified_at}"/>
                  <option name="chatModelId" value="sample-model"/>
                  <option name="title"><SerializedChatTitle><option name="text" value="Renamed &amp; saved"/></SerializedChatTitle></option>
                  <option name="messages"><list>
                    <SerializedChatMessage><option name="displayContent" value="A &lt; B"/></SerializedChatMessage>
                    <SerializedChatMessage><option name="author" value="Assistant"/>
                      <option name="displayContent" value="Yes"/><option name="internalContent" value="internal prompt"/>
                    </SerializedChatMessage>
                  </list></option>
                </SerializedChat>
                <SerializedChat><option name="uid" value="{AGENT}"/>
                  <option name="modifiedAt" value="{modified_at}"/>
                  <option name="chatModelId" value="agent_acp.registry.codex-acp"/>
                  <option name="messages"><list><SerializedChatMessage>
                    <option name="displayContent" value="Find the test"/>
                  </SerializedChatMessage></list></option>
                </SerializedChat>
              </list></option>
            </component></project>
        "#)).unwrap();
    }

    #[test]
    fn reads_workspace_chats_and_prefers_the_newer_ide_copy() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        write_workspace(&root.join("IntelliJIdea2026.2"), 1_000);
        write_workspace(&root.join("IntelliJIdea2026.3"), 2_000);
        write_workspace(&root.join("Rider2026.4"), 3_000);

        let adapter = AiAssistantAdapter::new(root.into(), root.into());
        let scan = adapter.scan();
        assert!(scan.complete, "{:?}", scan.errors);
        assert_eq!(scan.sessions.len(), 2);
        let session = scan
            .sessions
            .iter()
            .find(|session| session.id == CHAT)
            .unwrap();
        assert_eq!(session.title.as_deref(), Some("Renamed & saved"));
        assert_eq!(session.last_activity.timestamp_millis(), 2_000);
        assert_eq!(session.models, ["sample-model"]);
        assert_eq!(
            session.project_dir.as_deref(),
            root.join("Projects/demo").to_str()
        );
        assert!(!session.empty);
        assert_eq!(adapter.delete_plan(session), read_only(DELETE));
        assert!(adapter.archive_command(session).is_none());
        let transcript = adapter.transcript(session).unwrap();
        assert_eq!(
            transcript
                .head
                .iter()
                .map(|m| (m.role, m.text.as_str()))
                .collect::<Vec<_>>(),
            [(Role::User, "A < B"), (Role::Assistant, "Yes")]
        );
    }

    #[test]
    fn replays_agent_history_without_repeating_inline_prompts() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("IdeaIC2026.3");
        write_workspace(&config, 2_000);
        let history = config.join("aia-task-history");
        fs::create_dir_all(&history).unwrap();
        let path = history.join(format!("{AGENT}.events"));
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{HISTORY_HEADER}").unwrap();
        for record in [
            json!({"type": "com.intellij.ml.llm.chat.shared.ChatSessionUserPromptEvent", "prompt": "Find the test"}),
            json!({"event": {"kind": "TerminalBlockUpdatedEvent", "stepId": "shell", "command": "ls"}}),
            json!({"event": {"kind": "TerminalBlockUpdatedEvent", "stepId": "shell", "command": "ls", "output": "test.rs"}}),
            json!({"event": {"kind": "com.intellij.ml.llm.aui.events.api.MarkdownBlockUpdatedEvent", "stepId": "reply", "textChunk": "Found "}}),
            json!({"event": {"kind": "com.intellij.ml.llm.aui.events.api.MarkdownBlockUpdatedEvent", "stepId": "reply", "text": "Found test.rs"}}),
        ] {
            writeln!(file, "{}", STANDARD.encode(record.to_string())).unwrap();
        }
        let adapter = AiAssistantAdapter::new(temp.path().into(), temp.path().into());
        let scan = adapter.scan();
        assert!(scan.complete, "{:?}", scan.errors);
        let session = scan
            .sessions
            .iter()
            .find(|session| session.id == AGENT)
            .unwrap();
        assert_eq!(session.primary_file, path);
        assert_eq!(
            session.agent_type.as_deref(),
            Some("acp.registry.codex-acp")
        );
        let transcript = adapter.transcript(session).unwrap();
        let [prompt, tool, answer] = transcript.head.as_slice() else {
            panic!("expected prompt, tool call and answer");
        };
        assert_eq!(prompt.text, "Find the test");
        assert_eq!(
            tool.tool.as_ref().unwrap().output.as_deref(),
            Some("test.rs")
        );
        assert_eq!(answer.text, "Found test.rs");
    }
}
