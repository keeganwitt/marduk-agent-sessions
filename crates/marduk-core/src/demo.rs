//! A made-up home folder with sessions from every supported agent, stored the way each agent
//! stores them. It's for screenshots and for trying Marduk without the agents installed.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::labels::LabelStore;

/// Marks a folder as a demo home, so writing a new demo may replace it.
const MARKER: &str = ".marduk-demo";

const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const KB: u64 = 1024;
const MB: u64 = 1024 * KB;

const WEB: &str = "Projects/acme-web";
const API: &str = "Projects/acme-api";
const WEATHER: &str = "Projects/weather-cli";
const PHOTOS: &str = "Projects/photo-sorter";
const RECIPES: &str = "Projects/recipes-app";
const DOTFILES: &str = "Projects/dotfiles";
const NOTES: &str = "Documents/notes";
/// A project whose folder was deleted; only its Claude Code memory is left.
const LEGACY: &str = "Projects/legacy-site";

const LABELS: [(&str, &str); 5] = [
    ("bug", "red"),
    ("docs", "blue"),
    ("keep", "green"),
    ("research", "purple"),
    ("review", "orange"),
];

const PLAN: &str = "I'll start by looking at how the project is laid out.";
const COMMAND: &str = "ls";
const LISTING: &str = "README.md\nsrc\ntests";
const ABANDONED: &str = "Here's a first draft, before you asked me to start over.";

pub struct DemoOptions {
    /// Ages in the demo are relative to this, so "Running" and "1m ago" stay true.
    pub now: SystemTime,
    /// Pads transcripts to realistic sizes. The padding is a hole in the file, which takes no
    /// disk space where the file system supports sparse files (APFS, ext4, but not NTFS).
    pub pad_sizes: bool,
}

pub struct DemoHome {
    root: PathBuf,
}

impl DemoHome {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Environment that points Marduk, and the agent CLIs it runs, at this home. Every agent's
    /// own variable is set too, so one already set in the user's shell can't leak real sessions in.
    pub fn env(&self) -> Vec<(&'static str, PathBuf)> {
        let data = self.root.join(".local").join("share");
        vec![
            ("MARDUK_HOME", self.root.clone()),
            ("MARDUK_DATA_DIR", self.root.join("marduk")),
            (
                "MARDUK_VSCODE_USER_DIR",
                self.root.join("vscode").join("User"),
            ),
            ("CLAUDE_CONFIG_DIR", self.root.join(".claude")),
            ("CODEX_HOME", self.root.join(".codex")),
            ("COPILOT_HOME", self.root.join(".copilot")),
            ("PI_CODING_AGENT_DIR", self.root.join(".pi").join("agent")),
            ("OPENCODE_DB", data.join("opencode").join("opencode.db")),
            ("KILO_DB", data.join("kilo").join("kilo.db")),
            ("XDG_DATA_HOME", data),
        ]
    }

    pub fn var(&self, name: &str) -> PathBuf {
        self.env()
            .into_iter()
            .find_map(|(key, path)| (key == name).then_some(path))
            .unwrap_or_else(|| panic!("{name} isn't a demo variable"))
    }

    /// Writes the demo, replacing an earlier demo in the same folder. Refuses any other
    /// non-empty folder, so a mistyped path can't wipe real data.
    pub fn write(&self, options: &DemoOptions) -> io::Result<()> {
        self.reset()?;
        let mut writer = Writer {
            home: self,
            options,
            labeled: Vec::new(),
        };
        writer.sessions()?;
        writer.vscode_chats()?;
        writer.claude_memory()?;
        writer.codex_memory()?;
        writer.save_labels()
    }

    fn reset(&self) -> io::Result<()> {
        if self.root.join(MARKER).is_file() {
            fs::remove_dir_all(&self.root)?;
        } else if fs::read_dir(&self.root).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "{} isn't empty and isn't a Marduk demo home",
                    self.root.display()
                ),
            ));
        }
        fs::create_dir_all(&self.root)?;
        fs::write(self.root.join(MARKER), "")
    }
}

/// A session, with ages in seconds before `DemoOptions::now`.
#[derive(Clone, Copy)]
struct Spec {
    title: &'static str,
    project: &'static str,
    created: u64,
    last: u64,
    size: u64,
    archived: bool,
    running: bool,
    /// A Codex Desktop chat started without a project; `project` is ignored.
    chat: bool,
    labels: &'static [&'static str],
}

const fn spec(
    title: &'static str,
    project: &'static str,
    created: u64,
    last: u64,
    size: u64,
) -> Spec {
    Spec {
        title,
        project,
        created,
        last,
        size,
        archived: false,
        running: false,
        chat: false,
        labels: &[],
    }
}

impl Spec {
    const fn archived(mut self) -> Self {
        self.archived = true;
        self
    }

    const fn chat(mut self) -> Self {
        self.chat = true;
        self
    }

    const fn running(mut self) -> Self {
        self.running = true;
        self
    }

    const fn labels(mut self, labels: &'static [&'static str]) -> Self {
        self.labels = labels;
        self
    }

    /// A subagent of this session, in the same project and archive state.
    const fn child(self, title: &'static str, created: u64, last: u64, size: u64) -> Spec {
        Spec {
            title,
            created,
            last,
            size,
            running: false,
            labels: &[],
            ..self
        }
    }

    /// When the n-th message after the first prompt was written.
    fn step(&self, n: u64) -> u64 {
        self.created.saturating_sub(n * 20).max(self.last)
    }
}

/// A Codex subagent's task name, nickname and role, which make its title.
struct CodexAgent {
    task: &'static str,
    nickname: &'static str,
    role: &'static str,
}

struct Writer<'a> {
    home: &'a DemoHome,
    options: &'a DemoOptions,
    labeled: Vec<(String, &'static [&'static str])>,
}

impl Writer<'_> {
    fn sessions(&mut self) -> io::Result<()> {
        let dark_mode = spec(
            "Add dark mode to the settings page",
            WEB,
            23 * HOUR,
            MINUTE,
            19 * MB,
        )
        .running()
        .labels(&["keep"]);
        self.claude(
            dark_mode,
            &[
                (
                    dark_mode.child("Find theme token usages", 2 * HOUR, 2 * HOUR, 644 * KB),
                    "Explore",
                ),
                (
                    dark_mode.child("Audit color contrast", 2 * HOUR, 2 * HOUR, 792 * KB),
                    "general-purpose",
                ),
                (
                    dark_mode.child(
                        "Research prefers-color-scheme support",
                        2 * HOUR,
                        2 * HOUR,
                        839 * KB,
                    ),
                    "general-purpose",
                ),
                (
                    dark_mode.child("Review CSS variables plan", 23 * HOUR, 23 * HOUR, 884 * KB),
                    "Plan",
                ),
            ],
        )?;
        self.claude(
            spec("Fix flaky login test", API, 26 * HOUR, HOUR, 5300 * KB).labels(&["bug"]),
            &[],
        )?;
        self.codex(
            spec(
                "Summarize changes for the release notes",
                WEB,
                3 * HOUR,
                2 * HOUR,
                2300 * KB,
            ),
            &[],
        )?;
        self.codex(
            spec(
                "Speed up image thumbnails",
                PHOTOS,
                3 * HOUR,
                3 * HOUR,
                226 * KB,
            )
            .archived(),
            &[],
        )?;
        self.codex(
            spec(
                "Add CSV export to reports",
                API,
                3 * HOUR + 20 * MINUTE,
                3 * HOUR,
                1100 * KB,
            )
            .archived(),
            &[],
        )?;
        let dates = spec(
            "Refactor date parsing",
            WEATHER,
            4 * HOUR,
            4 * HOUR,
            2900 * KB,
        );
        self.codex(
            dates,
            &[(
                dates.child("parser_tests", 4 * HOUR, 4 * HOUR, 310 * KB),
                CodexAgent {
                    task: "parser_tests",
                    nickname: "Kepler",
                    role: "explorer",
                },
            )],
        )?;
        let tuning = spec(
            "Tune the Postgres connection pool",
            API,
            26 * HOUR,
            3 * HOUR,
            186 * KB,
        );
        self.opencode(&[
            (tuning, None),
            (
                tuning.child("Benchmark pool sizes", 25 * HOUR, 3 * HOUR, 64 * KB),
                Some(tuning.title),
            ),
            (
                spec("Rename config keys", WEATHER, 10 * DAY, 9 * DAY, 92 * KB).archived(),
                None,
            ),
        ])?;
        self.pi(spec(
            "Write a README for weather-cli",
            WEATHER,
            27 * HOUR,
            4 * HOUR,
            1300 * KB,
        )
        .labels(&["docs"]))?;
        let review = spec(
            "Review PR 282: rate limiter",
            API,
            50 * HOUR,
            4 * HOUR,
            3400 * KB,
        )
        .labels(&["review"]);
        self.codex(
            review,
            &[(
                review.child("rate_limit_review", 4 * HOUR, 4 * HOUR, 420 * KB),
                CodexAgent {
                    task: "rate_limit_review",
                    nickname: "Volta",
                    role: "reviewer",
                },
            )],
        )?;
        let ci = spec(
            "Migrate CI to GitHub Actions",
            RECIPES,
            7 * DAY,
            4 * HOUR,
            11 * MB,
        );
        self.codex(
            ci,
            &[
                (
                    ci.child("workflow_matrix", 7 * DAY, 6 * DAY, MB),
                    CodexAgent {
                        task: "workflow_matrix",
                        nickname: "Ada",
                        role: "worker",
                    },
                ),
                (
                    ci.child("cache_setup", 7 * DAY, 6 * DAY, 512 * KB),
                    CodexAgent {
                        task: "cache_setup",
                        nickname: "Turing",
                        role: "worker",
                    },
                ),
                (
                    ci.child("release_job", 6 * DAY, 4 * HOUR, 780 * KB),
                    CodexAgent {
                        task: "release_job",
                        nickname: "Hopper",
                        role: "worker",
                    },
                ),
            ],
        )?;
        self.copilot(spec(
            "Add pagination to the users endpoint",
            API,
            4 * HOUR,
            4 * HOUR,
            245 * KB,
        ))?;
        self.claude(
            spec(
                "Set up a Homebrew bundle",
                DOTFILES,
                5 * HOUR,
                4 * HOUR,
                202 * KB,
            )
            .labels(&["keep"]),
            &[],
        )?;
        self.claude(
            spec(
                "Links don't open in the markdown view",
                WEB,
                5 * HOUR,
                4 * HOUR,
                1200 * KB,
            )
            .labels(&["bug"]),
            &[],
        )?;
        self.codex(
            spec(
                "Investigate memory leak in the worker",
                API,
                25 * HOUR,
                4 * HOUR,
                1700 * KB,
            )
            .archived(),
            &[],
        )?;
        let dashboard = spec(
            "Investigate slow dashboard queries",
            API,
            26 * HOUR,
            5 * HOUR,
            4 * MB,
        )
        .archived()
        .labels(&["research"]);
        self.codex(
            dashboard,
            &[
                (
                    dashboard.child("query_plans", 26 * HOUR, 6 * HOUR, 600 * KB),
                    CodexAgent {
                        task: "query_plans",
                        nickname: "Noether",
                        role: "explorer",
                    },
                ),
                (
                    dashboard.child("index_audit", 26 * HOUR, 6 * HOUR, 450 * KB),
                    CodexAgent {
                        task: "index_audit",
                        nickname: "Lovelace",
                        role: "explorer",
                    },
                ),
            ],
        )?;
        self.pi(spec(
            "Compare static site generators",
            NOTES,
            28 * HOUR,
            5 * HOUR,
            473 * KB,
        )
        .labels(&["research", "keep"]))?;
        self.claude(
            spec(
                "Plan the v2 data model",
                RECIPES,
                6 * HOUR,
                5 * HOUR,
                763 * KB,
            ),
            &[],
        )?;
        self.copilot(spec(
            "Translate UI strings to Spanish",
            RECIPES,
            6 * HOUR,
            6 * HOUR,
            252 * KB,
        ))?;
        self.claude(
            spec(
                "Write unit tests for the parser",
                WEATHER,
                6 * HOUR,
                6 * HOUR,
                1200 * KB,
            ),
            &[],
        )?;
        let orphan = spec("lint_fix", WEB, 2 * DAY, 2 * DAY, 380 * KB);
        self.codex_rollout(
            orphan,
            &uuid("lint_fix"),
            Some((
                &uuid("A session that was deleted"),
                &CodexAgent {
                    task: "lint_fix",
                    nickname: "Curie",
                    role: "worker",
                },
            )),
        )?;
        self.codex(
            spec("Explain this regex", "", 3 * DAY, 3 * DAY, 96 * KB).chat(),
            &[],
        )?;
        self.codex(
            spec(
                "Convert a CSV export to JSON",
                "",
                5 * DAY,
                5 * DAY,
                210 * KB,
            )
            .chat(),
            &[],
        )?;
        self.kilo(&[
            spec(
                "Sketch the onboarding flow",
                RECIPES,
                3 * DAY,
                2 * DAY,
                140 * KB,
            ),
            spec(
                "Fix broken image links",
                PHOTOS,
                16 * DAY,
                15 * DAY,
                310 * KB,
            ),
        ])?;
        self.claude(
            spec("Upgrade to React 19", WEB, 9 * DAY, 8 * DAY, 8 * MB),
            &[],
        )?;
        self.codex(
            spec(
                "Debug Docker build cache misses",
                API,
                12 * DAY,
                12 * DAY,
                2600 * KB,
            ),
            &[],
        )?;
        self.claude(
            spec(
                "Organize vacation photos by date",
                PHOTOS,
                21 * DAY,
                20 * DAY,
                3100 * KB,
            ),
            &[],
        )?;
        self.pi(spec(
            "Draft blog post on caching",
            NOTES,
            35 * DAY,
            34 * DAY,
            690 * KB,
        ))?;
        self.codex(
            spec("Set up Renovate", RECIPES, 48 * DAY, 47 * DAY, 930 * KB).archived(),
            &[],
        )?;
        self.claude(
            spec("Add OAuth login", API, 60 * DAY, 58 * DAY, 24 * MB),
            &[],
        )?;
        self.copilot(spec(
            "Explain the build script",
            DOTFILES,
            75 * DAY,
            75 * DAY,
            120 * KB,
        ))?;
        self.claude(
            spec(
                "Port weather-cli to Rust",
                WEATHER,
                95 * DAY,
                90 * DAY,
                41 * MB,
            ),
            &[],
        )
    }

    fn time(&self, ago: u64) -> SystemTime {
        self.options.now - Duration::from_secs(ago)
    }

    fn datetime(&self, ago: u64) -> DateTime<Utc> {
        self.time(ago).into()
    }

    fn stamp(&self, ago: u64) -> String {
        self.datetime(ago)
            .to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    fn millis(&self, ago: u64) -> i64 {
        self.datetime(ago).timestamp_millis()
    }

    fn project(&self, project: &str) -> io::Result<String> {
        let dir = project
            .split('/')
            .fold(self.home.root.clone(), |dir, part| dir.join(part));
        fs::create_dir_all(&dir)?;
        Ok(dir.to_string_lossy().into_owned())
    }

    fn label(&mut self, key: String, spec: &Spec) {
        if !spec.labels.is_empty() {
            self.labeled.push((key, spec.labels));
        }
    }

    /// One JSON record per line, the last one at the end of the padded size. Its modified time
    /// is the session's last activity, except for running sessions, which were just written.
    fn write_jsonl(&self, path: &Path, records: &[Value], spec: &Spec) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let (last, head) = records.split_last().expect("at least one record");
        let mut file = File::create(path)?;
        for record in head {
            serde_json::to_writer(&mut file, record)?;
            file.write_all(b"\n")?;
        }
        let mut tail = serde_json::to_vec(last)?;
        tail.push(b'\n');
        let end = file.stream_position()?;
        let padded_end = spec.size.saturating_sub(tail.len() as u64 + 1);
        if self.options.pad_sizes && padded_end > end {
            file.seek(SeekFrom::Start(padded_end))?;
            file.write_all(b"\n")?;
        }
        file.write_all(&tail)?;
        if !spec.running {
            file.set_modified(self.time(spec.last))?;
        }
        Ok(())
    }

    /// Database sessions are padded with a snapshot part, which transcripts don't show. Unlike
    /// a file's hole, this takes real disk space, so their sizes are kept small.
    fn filler(&self, spec: &Spec) -> Option<String> {
        (self.options.pad_sizes && spec.size > 0).then(|| "0".repeat(spec.size as usize))
    }

    fn set_modified(&self, path: &Path, ago: u64) -> io::Result<()> {
        File::options()
            .write(true)
            .open(path)?
            .set_modified(self.time(ago))
    }

    fn claude(&mut self, spec: Spec, subagents: &[(Spec, &str)]) -> io::Result<()> {
        let cwd = self.project(spec.project)?;
        let id = uuid(spec.title);
        let folder = self
            .home
            .var("CLAUDE_CONFIG_DIR")
            .join("projects")
            .join(claude_folder(&cwd));
        let mut records = self.claude_records(&spec, &id, &cwd, false);
        records.insert(
            1,
            json!({ "type": "ai-title", "aiTitle": spec.title, "sessionId": id }),
        );
        self.write_jsonl(&folder.join(format!("{id}.jsonl")), &records, &spec)?;
        for (subagent, agent_type) in subagents {
            let stem = format!("agent-{:016x}", fnv(subagent.title, 2));
            let dir = folder.join(&id).join("subagents");
            let records = self.claude_records(subagent, &id, &cwd, true);
            self.write_jsonl(&dir.join(format!("{stem}.jsonl")), &records, subagent)?;
            let meta = dir.join(format!("{stem}.meta.json"));
            fs::write(
                &meta,
                json!({ "description": subagent.title, "agentType": agent_type }).to_string(),
            )?;
            self.set_modified(&meta, subagent.last)?;
        }
        self.label(format!("claude:{id}"), &spec);
        Ok(())
    }

    fn claude_records(&self, spec: &Spec, id: &str, cwd: &str, sidechain: bool) -> Vec<Value> {
        let record = |kind: &str, ago: u64, message: Value| {
            json!({
                "type": kind,
                "sessionId": id,
                "cwd": cwd,
                "isSidechain": sidechain,
                "timestamp": self.stamp(ago),
                "message": message,
            })
        };
        vec![
            record(
                "user",
                spec.created,
                json!({ "role": "user", "content": spec.title }),
            ),
            record(
                "assistant",
                spec.step(1),
                json!({ "role": "assistant", "model": model("claude", spec.title), "content": [
                    { "type": "text", "text": PLAN },
                    { "type": "tool_use", "id": "toolu_1", "name": "Bash", "input": { "command": COMMAND } },
                ]}),
            ),
            record(
                "user",
                spec.step(2),
                json!({ "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": "toolu_1", "content": LISTING },
                ]}),
            ),
            record(
                "assistant",
                spec.last,
                json!({ "role": "assistant", "model": model("claude", spec.title), "content": [{ "type": "text", "text": summary(spec.title) }] }),
            ),
        ]
    }

    fn codex(&mut self, spec: Spec, subagents: &[(Spec, CodexAgent)]) -> io::Result<()> {
        let id = uuid(spec.title);
        self.codex_rollout(spec, &id, None)?;
        for (subagent, agent) in subagents {
            let child_id = uuid(&format!("{}/{}", spec.title, agent.task));
            self.codex_rollout(*subagent, &child_id, Some((&id, agent)))?;
        }
        self.label(format!("codex:{id}"), &spec);
        Ok(())
    }

    fn codex_rollout(
        &self,
        spec: Spec,
        id: &str,
        spawned_by: Option<(&str, &CodexAgent)>,
    ) -> io::Result<()> {
        let (cwd, workspace_root) = if spec.chat {
            let chats = self.home.root.join("Documents").join("Codex");
            let dir = chats
                .join(self.datetime(spec.created).format("%Y-%m-%d").to_string())
                .join(slug(spec.title));
            fs::create_dir_all(&dir)?;
            (
                dir.to_string_lossy().into_owned(),
                chats.to_string_lossy().into_owned(),
            )
        } else {
            let cwd = self.project(spec.project)?;
            (cwd.clone(), cwd)
        };
        let source = match spawned_by {
            Some((parent, agent)) => json!({ "subagent": { "thread_spawn": {
                "parent_thread_id": parent,
                "depth": 1,
                "agent_path": format!("/root/{}", agent.task),
                "agent_nickname": agent.nickname,
                "agent_role": agent.role,
            }}}),
            None => json!("cli"),
        };
        let item = |ago: u64, payload: Value| json!({ "timestamp": self.stamp(ago), "type": "response_item", "payload": payload });
        let event = |ago: u64, payload: Value| json!({ "timestamp": self.stamp(ago), "type": "event_msg", "payload": payload });
        let records = [
            json!({ "timestamp": self.stamp(spec.created), "type": "session_meta", "payload": {
                "id": id,
                "timestamp": self.stamp(spec.created),
                "cwd": cwd,
                "runtime_workspace_roots": [workspace_root],
                "originator": "codex_cli_rs",
                "source": source,
            }}),
            item(
                spec.created,
                json!({ "type": "message", "role": "user", "content": [{ "type": "input_text", "text": spec.title }] }),
            ),
            event(
                spec.created,
                json!({ "type": "user_message", "message": spec.title }),
            ),
            json!({ "timestamp": self.stamp(spec.created), "type": "turn_context",
                    "payload": { "cwd": cwd, "model": model("codex", spec.title) } }),
            item(
                spec.step(1),
                json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": PLAN }] }),
            ),
            item(
                spec.step(1),
                json!({ "type": "function_call", "name": "exec_command", "call_id": "call_1",
                        "arguments": json!({ "cmd": COMMAND }).to_string() }),
            ),
            item(
                spec.step(2),
                json!({ "type": "function_call_output", "call_id": "call_1", "output": LISTING }),
            ),
            item(
                spec.last,
                json!({ "type": "message", "role": "assistant", "content": [{ "type": "output_text", "text": summary(spec.title) }] }),
            ),
            event(spec.last, json!({ "type": "task_complete" })),
        ];
        let created = self.datetime(spec.created);
        let name = format!("rollout-{}-{id}.jsonl", created.format("%Y-%m-%dT%H-%M-%S"));
        let codex = self.home.var("CODEX_HOME");
        let path = if spec.archived {
            codex.join("archived_sessions").join(name)
        } else {
            codex
                .join("sessions")
                .join(created.format("%Y").to_string())
                .join(created.format("%m").to_string())
                .join(created.format("%d").to_string())
                .join(name)
        };
        self.write_jsonl(&path, &records, &spec)
    }

    fn copilot(&mut self, spec: Spec) -> io::Result<()> {
        let cwd = self.project(spec.project)?;
        let id = uuid(spec.title);
        let dir = self
            .home
            .var("COPILOT_HOME")
            .join("session-state")
            .join(&id);
        let event = |kind: &str, ago: u64, data: Value| json!({ "type": kind, "timestamp": self.stamp(ago), "data": data });
        let call =
            json!({ "toolCallId": "call_1", "name": "bash", "arguments": { "command": COMMAND } });
        let records = [
            event(
                "session.start",
                spec.created,
                json!({ "sessionId": id, "selectedModel": model("copilot", spec.title), "context": { "cwd": cwd } }),
            ),
            event(
                "user.message",
                spec.created,
                json!({ "content": spec.title }),
            ),
            event(
                "assistant.message",
                spec.step(1),
                json!({ "content": PLAN, "toolRequests": [call] }),
            ),
            event(
                "tool.execution_start",
                spec.step(1),
                json!({ "toolCallId": "call_1", "toolName": "bash", "arguments": { "command": COMMAND } }),
            ),
            event(
                "tool.execution_complete",
                spec.step(2),
                json!({ "toolCallId": "call_1", "success": true, "result": { "content": LISTING } }),
            ),
            event(
                "assistant.message",
                spec.last,
                json!({ "content": summary(spec.title) }),
            ),
        ];
        self.write_jsonl(&dir.join("events.jsonl"), &records, &spec)?;
        let workspace = dir.join("workspace.yaml");
        fs::write(
            &workspace,
            format!(
                "id: {id}\ncwd: {cwd}\nname: {}\ncreated_at: {}\nupdated_at: {}\n",
                json!(spec.title),
                self.stamp(spec.created),
                self.stamp(spec.last)
            ),
        )?;
        self.set_modified(&workspace, spec.last)?;
        self.label(format!("copilot:{id}"), &spec);
        Ok(())
    }

    /// Pi sessions are trees; each demo session has a reply the user rewound past, which Pi
    /// keeps in the file but isn't on the branch the user last saw.
    fn pi(&mut self, spec: Spec) -> io::Result<()> {
        let cwd = self.project(spec.project)?;
        let id = uuid(spec.title);
        let entry = |entry_id: &str, parent: Option<&str>, ago: u64, message: Value| {
            json!({ "type": "message", "id": entry_id, "parentId": parent,
                    "timestamp": self.stamp(ago), "message": message })
        };
        let records = [
            json!({ "type": "session", "version": 3, "id": id,
                    "timestamp": self.stamp(spec.created), "cwd": cwd }),
            json!({ "type": "model_change", "id": "m0", "parentId": null,
                    "timestamp": self.stamp(spec.created), "modelId": model("pi", spec.title) }),
            entry(
                "e1",
                None,
                spec.created,
                json!({ "role": "user", "content": [{ "type": "text", "text": spec.title }] }),
            ),
            entry(
                "e1b",
                Some("e1"),
                spec.step(1),
                json!({ "role": "assistant", "content": [{ "type": "text", "text": ABANDONED }] }),
            ),
            entry(
                "e2",
                Some("e1"),
                spec.step(1),
                json!({ "role": "assistant", "content": [
                    { "type": "text", "text": PLAN },
                    { "type": "toolCall", "id": "call_1", "name": "bash", "arguments": { "command": COMMAND } },
                ]}),
            ),
            entry(
                "e3",
                Some("e2"),
                spec.step(2),
                json!({ "role": "toolResult", "toolCallId": "call_1", "toolName": "bash",
                        "content": [{ "type": "text", "text": LISTING }] }),
            ),
            entry(
                "e4",
                Some("e3"),
                spec.last,
                json!({ "role": "assistant", "content": [{ "type": "text", "text": summary(spec.title) }] }),
            ),
        ];
        let folder = format!(
            "--{}--",
            cwd.trim_start_matches(['/', '\\'])
                .replace(['/', '\\', ':'], "-")
        );
        let name = format!(
            "{}_{id}.jsonl",
            self.datetime(spec.created).format("%Y-%m-%dT%H-%M-%S-%3fZ")
        );
        let path = self
            .home
            .var("PI_CODING_AGENT_DIR")
            .join("sessions")
            .join(folder)
            .join(name);
        self.write_jsonl(&path, &records, &spec)?;
        self.label(format!("pi:{id}"), &spec);
        Ok(())
    }

    fn database(&self, variable: &str, schema: &str) -> io::Result<Connection> {
        let path = self.home.var(variable);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let connection = Connection::open(path).map_err(io::Error::other)?;
        connection.execute_batch(schema).map_err(io::Error::other)?;
        Ok(connection)
    }

    /// OpenCode's current tables. Each session is given with the title of its parent, if any.
    fn opencode(&mut self, sessions: &[(Spec, Option<&str>)]) -> io::Result<()> {
        let connection = self.database(
            "OPENCODE_DB",
            "create table session_v2 (id text primary key, parent_id text, directory text not null,
               title text, time_created integer not null, time_updated integer not null,
               time_archived integer, model text);
             create table session_message (id text primary key, session_id text not null,
               type text not null, seq integer not null, time_created integer not null,
               time_updated integer not null, data text not null);
             create table session_part (message_id text not null, session_id text not null,
               ordinal integer not null, type text not null, call_id text,
               time_created integer not null, time_updated integer not null, data text not null);",
        )?;
        let sql = |result: rusqlite::Result<usize>| result.map(|_| ()).map_err(io::Error::other);
        for (spec, parent) in sessions {
            let id = opencode_id(spec.title);
            let cwd = self.project(spec.project)?;
            sql(connection.execute(
                "insert into session_v2 values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    id,
                    parent.map(opencode_id),
                    cwd,
                    spec.title,
                    self.millis(spec.created),
                    self.millis(spec.last),
                    spec.archived.then(|| self.millis(spec.last)),
                    json!({ "providerID": "anthropic", "modelID": model("opencode", spec.title) })
                        .to_string(),
                ],
            ))?;
            let message = |seq: i64, kind: &str, ago: u64, data: Value| {
                sql(connection.execute(
                    "insert into session_message values (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
                    params![
                        format!("msg_{id}_{seq}"),
                        id,
                        kind,
                        seq,
                        self.millis(ago),
                        data.to_string()
                    ],
                ))
            };
            let part = |seq: i64, ordinal: i64, kind: &str, ago: u64, data: Value| {
                sql(connection.execute(
                    "insert into session_part values (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7)",
                    params![
                        format!("msg_{id}_{seq}"),
                        id,
                        ordinal,
                        kind,
                        (kind == "tool").then_some("call_1"),
                        self.millis(ago),
                        data.to_string()
                    ],
                ))
            };
            message(1, "user", spec.created, json!({ "text": spec.title }))?;
            message(2, "assistant", spec.step(1), json!({ "content": [] }))?;
            part(2, 0, "text", spec.step(1), json!({ "text": PLAN }))?;
            part(
                2,
                1,
                "tool",
                spec.step(2),
                json!({ "id": "call_1", "name": "bash", "state": {
                    "status": "completed", "input": { "command": COMMAND }, "content": LISTING,
                }}),
            )?;
            message(3, "assistant", spec.last, json!({ "content": [] }))?;
            part(
                3,
                0,
                "text",
                spec.last,
                json!({ "text": summary(spec.title) }),
            )?;
            if let Some(filler) = self.filler(spec) {
                part(3, 1, "snapshot", spec.last, json!({ "snapshot": filler }))?;
            }
            self.label(format!("opencode:{id}"), spec);
        }
        Ok(())
    }

    /// Kilo still uses OpenCode's older `session`/`message`/`part` tables.
    fn kilo(&mut self, sessions: &[Spec]) -> io::Result<()> {
        let connection = self.database(
            "KILO_DB",
            "create table session (id text primary key, parent_id text, directory text not null,
               title text not null, time_created integer not null, time_updated integer not null);
             create table message (id text primary key, session_id text not null,
               time_created integer not null, time_updated integer not null, data text not null);
             create table part (id text primary key, message_id text not null,
               session_id text not null, time_created integer not null,
               time_updated integer not null, data text not null);",
        )?;
        let sql = |result: rusqlite::Result<usize>| result.map(|_| ()).map_err(io::Error::other);
        for spec in sessions {
            let id = opencode_id(spec.title);
            let cwd = self.project(spec.project)?;
            sql(connection.execute(
                "insert into session values (?1, null, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    cwd,
                    spec.title,
                    self.millis(spec.created),
                    self.millis(spec.last)
                ],
            ))?;
            let message = |seq: i64, role: &str, ago: u64| {
                sql(connection.execute(
                    "insert into message values (?1, ?2, ?3, ?3, ?4)",
                    params![
                        format!("msg_{id}_{seq}"),
                        id,
                        self.millis(ago),
                        json!({ "role": role }).to_string()
                    ],
                ))
            };
            let part = |seq: i64, ordinal: i64, ago: u64, data: Value| {
                sql(connection.execute(
                    "insert into part values (?1, ?2, ?3, ?4, ?4, ?5)",
                    params![
                        format!("prt_{id}_{seq}_{ordinal}"),
                        format!("msg_{id}_{seq}"),
                        id,
                        self.millis(ago),
                        data.to_string()
                    ],
                ))
            };
            message(1, "user", spec.created)?;
            part(
                1,
                0,
                spec.created,
                json!({ "type": "text", "text": spec.title }),
            )?;
            message(2, "assistant", spec.step(1))?;
            part(2, 0, spec.step(1), json!({ "type": "text", "text": PLAN }))?;
            part(
                2,
                1,
                spec.step(2),
                json!({ "type": "tool", "tool": "bash", "callID": "call_1", "state": {
                    "status": "completed", "input": { "command": COMMAND }, "output": LISTING,
                }}),
            )?;
            message(3, "assistant", spec.last)?;
            part(
                3,
                0,
                spec.last,
                json!({ "type": "text", "text": summary(spec.title) }),
            )?;
            if let Some(filler) = self.filler(spec) {
                part(
                    3,
                    1,
                    spec.last,
                    json!({ "type": "snapshot", "snapshot": filler }),
                )?;
            }
            self.label(format!("kilo:{id}"), spec);
        }
        Ok(())
    }

    /// Copilot Chat in VS Code: a chat in the older `.json` format and one in the newer `.jsonl`
    /// change log, each in its workspace's storage.
    fn vscode_chats(&mut self) -> io::Result<()> {
        let user = self.home.var("MARDUK_VSCODE_USER_DIR");
        let chats = [
            (
                spec("Explain the flaky snapshot test", WEB, 2 * DAY, 2 * DAY, 0),
                false,
            ),
            (
                spec(
                    "Rename props in the settings form",
                    RECIPES,
                    8 * DAY,
                    8 * DAY,
                    0,
                ),
                true,
            ),
        ];
        for (spec, log) in chats {
            let cwd = self.project(spec.project)?;
            let id = uuid(spec.title);
            let workspace = user
                .join("workspaceStorage")
                .join(format!("{:032x}", u128::from(fnv(spec.project, 5))));
            fs::create_dir_all(workspace.join("chatSessions"))?;
            let uri = format!(
                "file://{}{}",
                if cwd.starts_with('/') { "" } else { "/" },
                cwd.replace('\\', "/").replace(' ', "%20")
            );
            fs::write(
                workspace.join("workspace.json"),
                json!({ "folder": uri }).to_string(),
            )?;
            let request = json!({
                "requestId": "r1",
                "timestamp": self.millis(spec.created),
                "responseTimestamp": self.millis(spec.last),
                "modelId": format!("copilot/{}", model("vscode", spec.title)),
                "message": { "text": spec.title },
                "response": [
                    { "value": PLAN },
                    { "kind": "toolInvocationSerialized", "toolId": "run_in_terminal",
                      "toolCallId": "t1", "invocationMessage": { "value": COMMAND },
                      "resultDetails": { "input": COMMAND, "output": [{ "value": LISTING }] } },
                    { "value": summary(spec.title) },
                ],
            });
            let path = if log {
                let path = workspace.join("chatSessions").join(format!("{id}.jsonl"));
                let lines = [
                    json!({ "kind": 0, "v": { "version": 3, "sessionId": id,
                            "creationDate": self.millis(spec.created), "requests": [] } }),
                    json!({ "kind": 2, "k": ["requests"], "v": [request] }),
                ];
                fs::write(
                    &path,
                    lines
                        .iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )?;
                path
            } else {
                let path = workspace.join("chatSessions").join(format!("{id}.json"));
                fs::write(
                    &path,
                    json!({ "version": 3, "sessionId": id,
                            "creationDate": self.millis(spec.created),
                            "lastMessageDate": self.millis(spec.last),
                            "requests": [request] })
                    .to_string(),
                )?;
                path
            };
            self.set_modified(&path, spec.last)?;
            self.label(format!("vscode-copilot:{id}"), &spec);
        }
        Ok(())
    }

    /// Claude Code memory: a healthy project, one with a memory missing from its index, one
    /// whose project folder is gone, and an empty memory folder. `.claude.json` lists them all.
    fn claude_memory(&self) -> io::Result<()> {
        let config = self.home.var("CLAUDE_CONFIG_DIR");
        let path_of = |project: &str| {
            project
                .split('/')
                .fold(self.home.root.clone(), |dir, part| dir.join(part))
                .to_string_lossy()
                .into_owned()
        };
        let memory_dir = |project: &str| {
            config
                .join("projects")
                .join(claude_folder(&path_of(project)))
                .join("memory")
        };
        let memory = |id: &str, kind: &str, description: &str, body: &str, ago: u64| {
            format!(
                "---\nname: {id}\ndescription: {description}\nmetadata:\n  type: {kind}\n  \
                 originSessionId: {}\n  modified: {}\n---\n\n{body}\n",
                uuid("Add dark mode to the settings page"),
                self.stamp(ago)
            )
        };
        let write = |dir: &Path, files: &[(&str, String)]| -> io::Result<()> {
            fs::create_dir_all(dir)?;
            for (name, text) in files {
                fs::write(dir.join(name), text)?;
            }
            Ok(())
        };
        write(
            &memory_dir(WEB),
            &[
                (
                    "MEMORY.md",
                    "- [Theme tokens](theme-tokens.md) — colors come from CSS variables in tokens.css\n\
                     - [Test command](test-command.md) — run `pnpm test -- --run` before committing\n\
                     - [Design review](design-review.md) — UI changes need a screenshot in the PR\n"
                        .to_string(),
                ),
                (
                    "theme-tokens.md",
                    memory(
                        "theme-tokens",
                        "project",
                        "Colors come from CSS variables in src/styles/tokens.css",
                        "Never hard-code colors.\n\n**Why:** dark mode swaps the variables.\n\n**How to apply:** add new colors to tokens.css.",
                        2 * HOUR,
                    ),
                ),
                (
                    "test-command.md",
                    memory(
                        "test-command",
                        "feedback",
                        "Run pnpm test -- --run before committing",
                        "Watch mode never exits.\n\n**Why:** a hung commit hook.\n\n**How to apply:** always pass --run.",
                        DAY,
                    ),
                ),
                (
                    "design-review.md",
                    memory(
                        "design-review",
                        "reference",
                        "UI changes need a screenshot in the pull request",
                        "The design team reviews every UI change from screenshots.",
                        3 * DAY,
                    ),
                ),
            ],
        )?;
        write(
            &memory_dir(DOTFILES),
            &[
                (
                    "MEMORY.md",
                    "- [Brewfile](brewfile.md) — keep the Brewfile sorted\n".to_string(),
                ),
                (
                    "brewfile.md",
                    memory(
                        "brewfile",
                        "feedback",
                        "Keep the Brewfile sorted",
                        "Sort entries alphabetically.",
                        5 * HOUR,
                    ),
                ),
                (
                    "shell-prompt.md",
                    memory(
                        "shell-prompt",
                        "project",
                        "The prompt is Starship, configured in starship.toml",
                        "Starship, not oh-my-zsh.",
                        6 * HOUR,
                    ),
                ),
            ],
        )?;
        write(
            &memory_dir(LEGACY),
            &[
                (
                    "MEMORY.md",
                    "- [Deploys](deploys.md) — deploy with make deploy from the release branch\n"
                        .to_string(),
                ),
                (
                    "deploys.md",
                    memory(
                        "deploys",
                        "project",
                        "Deploy with make deploy from the release branch",
                        "Never deploy from main.",
                        120 * DAY,
                    ),
                ),
            ],
        )?;
        write(&memory_dir(WEATHER), &[])?;
        let projects: serde_json::Map<String, Value> =
            [WEB, API, WEATHER, PHOTOS, RECIPES, DOTFILES, LEGACY]
                .iter()
                .map(|project| (path_of(project), json!({})))
                .collect();
        fs::create_dir_all(&config)?;
        fs::write(
            config.join(".claude.json"),
            json!({ "projects": projects }).to_string(),
        )
    }

    /// A Codex memory store with memories turned on, including a summary of a session that
    /// still exists and one of a session that doesn't.
    fn codex_memory(&self) -> io::Result<()> {
        let codex = self.home.var("CODEX_HOME");
        let memories = codex.join("memories");
        let files = [
            (
                "memory_summary.md".to_string(),
                "# Memory summary\n\n- The user prefers small, reviewable pull requests.\n- acme-api runs its tests with `cargo nextest run`.\n".to_string(),
            ),
            (
                "MEMORY.md".to_string(),
                "# Task Group: Date parsing in weather-cli\n\n## Task 1: Refactor date parsing, success\n\n### keywords\n\nchrono, parsing\n".to_string(),
            ),
            (
                "raw_memories.md".to_string(),
                "- weather-cli dates are ISO 8601 with offsets.\n".to_string(),
            ),
            (
                format!(
                    "rollout_summaries/{}-Ab12-refactor_date_parsing.md",
                    self.datetime(4 * HOUR).format("%Y-%m-%dT%H-%M-%S")
                ),
                format!(
                    "thread_id: {}\ncwd: {}\n\nOutcome: success\n",
                    uuid("Refactor date parsing"),
                    self.project(WEATHER)?
                ),
            ),
            (
                format!(
                    "rollout_summaries/{}-Cd34-flaky_ci_cache.md",
                    self.datetime(40 * DAY).format("%Y-%m-%dT%H-%M-%S")
                ),
                format!(
                    "thread_id: {}\ncwd: {}\n\nOutcome: partial\n",
                    uuid("A deleted session about CI"),
                    self.project(RECIPES)?
                ),
            ),
            (
                "skills/run-nextest/SKILL.md".to_string(),
                "# Run nextest\n\nUse `cargo nextest run` in acme-api.\n".to_string(),
            ),
        ];
        for (name, text) in files {
            let path = memories.join(name);
            fs::create_dir_all(path.parent().unwrap_or(&memories))?;
            fs::write(path, text)?;
        }
        fs::write(codex.join("config.toml"), "[features]\nmemories = true\n")
    }

    fn save_labels(&self) -> io::Result<()> {
        let store = LabelStore::open(&self.home.var("MARDUK_DATA_DIR").join("labels.db"))
            .map_err(io::Error::other)?;
        let mut ids = HashMap::new();
        for (name, color) in LABELS {
            let label = store.create(name, color).map_err(io::Error::other)?;
            ids.insert(name, label.id);
        }
        for (key, names) in &self.labeled {
            let add: Vec<i64> = names.iter().map(|name| ids[name]).collect();
            store
                .apply(std::slice::from_ref(key), &add, &[], self.options.now)
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
}

/// A plausible model for a session, the same on every demo run.
fn model(agent: &str, title: &str) -> &'static str {
    let choices: &[&str] = match agent {
        "claude" => &["claude-opus-5-5", "claude-sonnet-5-5"],
        "codex" => &["gpt-6.1-sol", "gpt-6.1-terra", "gpt-6.1-luna"],
        _ => &["claude-sonnet-5-5", "gpt-6.1-sol"],
    };
    choices[(fnv(title, 4) % choices.len() as u64) as usize]
}

fn summary(title: &str) -> String {
    format!(
        "Done: **{title}**.\n\n- Updated the code in `src/`\n- Added tests for the new behavior\n\n\
         See [the project notes](https://example.com/notes) for details."
    )
}

/// Codex Desktop names a chat's folder after the start of its first prompt.
fn slug(title: &str) -> String {
    title
        .split_whitespace()
        .take(3)
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// Claude Code names a project's folder after its path, with every other character a dash.
fn claude_folder(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn fnv(seed: &str, salt: u64) -> u64 {
    seed.bytes()
        .fold(0xcbf2_9ce4_8422_2325 ^ salt, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// A UUID derived from `seed`, so a session keeps its id, and its labels, across demo runs.
fn uuid(seed: &str) -> String {
    let (a, b) = (fnv(seed, 0), fnv(seed, 1));
    format!(
        "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
        a >> 32,
        (a >> 16) & 0xffff,
        a & 0xfff,
        0x8000 | ((b >> 48) & 0x3fff),
        b & 0xffff_ffff_ffff
    )
}

fn opencode_id(title: &str) -> String {
    format!("ses_{:016x}", fnv(title, 3))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::adapter::AgentAdapter;
    use crate::agents::claude::ClaudeAdapter;
    use crate::agents::codex::CodexAdapter;
    use crate::agents::copilot::CopilotAdapter;
    use crate::agents::opencode::{self, OpenCodeAdapter};
    use crate::agents::pi::PiAdapter;
    use crate::session::Session;
    use crate::transcript::Role;

    /// Adapters configured the way `from_env` reads `DemoHome::env`.
    fn adapters(home: &DemoHome) -> Vec<Box<dyn AgentAdapter>> {
        let data = home.var("XDG_DATA_HOME");
        vec![
            Box::new(ClaudeAdapter::new(home.var("CLAUDE_CONFIG_DIR"))),
            Box::new(CodexAdapter::new(home.var("CODEX_HOME"), None)),
            Box::new(CopilotAdapter::new(
                home.var("COPILOT_HOME").join("session-state"),
            )),
            Box::new(OpenCodeAdapter::with_flavor(
                &opencode::OPENCODE,
                data.join("opencode"),
                Some(home.var("OPENCODE_DB")),
                None,
            )),
            Box::new(OpenCodeAdapter::with_flavor(
                &opencode::KILO,
                data.join("kilo"),
                Some(home.var("KILO_DB")),
                None,
            )),
            Box::new(PiAdapter::new(
                home.var("PI_CODING_AGENT_DIR").join("sessions"),
            )),
            Box::new(crate::agents::vscode_chat::VsCodeChatAdapter::new(vec![
                home.var("MARDUK_VSCODE_USER_DIR"),
            ])),
        ]
    }

    fn write_demo(pad_sizes: bool) -> (tempfile::TempDir, DemoHome, SystemTime) {
        let dir = tempfile::tempdir().unwrap();
        let home = DemoHome::new(dir.path().join("demo"));
        let now = SystemTime::now();
        home.write(&DemoOptions { now, pad_sizes }).unwrap();
        (dir, home, now)
    }

    fn scan(home: &DemoHome) -> Vec<(Box<dyn AgentAdapter>, Vec<Session>)> {
        adapters(home)
            .into_iter()
            .map(|adapter| {
                let result = adapter.scan();
                assert!(
                    result.complete && result.errors.is_empty(),
                    "{}: {:?}",
                    adapter.id(),
                    result.errors
                );
                (adapter, result.sessions)
            })
            .collect()
    }

    #[test]
    fn every_agent_reads_its_demo_sessions_and_transcripts() {
        let (_dir, home, _) = write_demo(false);

        for (adapter, sessions) in scan(&home) {
            assert!(!sessions.is_empty(), "{} has no sessions", adapter.id());
            for session in &sessions {
                let title = session.title.as_deref().unwrap_or_default();
                assert!(!title.is_empty(), "{} {}", adapter.id(), session.id);
                if adapter.id() != "kilo" {
                    assert!(
                        !session.models.is_empty(),
                        "{} {title} has no model",
                        adapter.id()
                    );
                }
                let project = PathBuf::from(session.project_dir.as_deref().unwrap());
                assert!(project.starts_with(home.root()) && project.is_dir());

                let transcript = adapter.transcript(session).unwrap();
                let messages: Vec<_> = transcript.head.iter().chain(&transcript.tail).collect();
                let has = |role: Role| messages.iter().any(|message| message.role == role);
                assert!(has(Role::User) && has(Role::Assistant), "{title}");
                assert!(
                    messages.iter().any(|message| message
                        .tool
                        .as_ref()
                        .and_then(|tool| tool.output.as_deref())
                        == Some(LISTING)),
                    "{} {title}: no tool output in {messages:?}",
                    adapter.id()
                );
                assert!(
                    messages
                        .iter()
                        .all(|message| !message.text.contains(ABANDONED))
                );
            }
        }
    }

    #[test]
    fn demo_has_running_archived_nested_orphaned_and_labeled_sessions() {
        let (_dir, home, now) = write_demo(false);
        let sessions: Vec<Session> = scan(&home)
            .into_iter()
            .flat_map(|(_, sessions)| sessions)
            .collect();
        let ids: HashSet<&str> = sessions.iter().map(|session| session.id.as_str()).collect();

        let running: Vec<_> = sessions
            .iter()
            .filter(|session| session.is_running(now, crate::RUNNING_WINDOW))
            .collect();
        assert_eq!(running.len(), 1);
        assert_eq!(
            running[0].title.as_deref(),
            Some("Add dark mode to the settings page")
        );
        assert!(sessions.iter().any(|session| session.archived));
        assert_eq!(
            sessions
                .iter()
                .filter(|session| session.part_of_parent)
                .count(),
            4
        );
        let orphans: Vec<_> = sessions
            .iter()
            .filter_map(|session| session.parent_id.as_deref())
            .filter(|parent| !ids.contains(parent))
            .collect();
        assert_eq!(orphans.len(), 1);
        assert!(
            sessions
                .iter()
                .any(|session| session.agent == "opencode" && session.parent_id.is_some())
        );
        let chats: Vec<_> = sessions
            .iter()
            .filter(|session| session.projectless)
            .collect();
        assert_eq!(chats.len(), 2);
        let chats_dir = home.root().join("Documents").join("Codex");
        assert!(chats.iter().all(|chat| {
            PathBuf::from(chat.project_dir.as_deref().unwrap()).starts_with(&chats_dir)
        }));

        let claude = ClaudeAdapter::new(home.var("CLAUDE_CONFIG_DIR"));
        let claude_sessions: Vec<Session> = sessions
            .iter()
            .filter(|session| session.agent == "claude")
            .cloned()
            .collect();
        let memory = claude.memory(&claude_sessions);
        assert_eq!(memory.len(), 4);
        let problem = |project: &str| {
            memory
                .iter()
                .find(|store| {
                    store
                        .project_dir
                        .as_deref()
                        .is_some_and(|dir| dir.ends_with(project))
                })
                .map(|store| store.problems.join(" "))
                .unwrap()
        };
        assert_eq!(problem("acme-web"), "");
        assert!(problem("dotfiles").contains("shell-prompt.md"));
        assert!(problem("legacy-site").contains("no longer exists"));
        assert!(problem("weather-cli").contains("empty"));
        let codex = CodexAdapter::new(home.var("CODEX_HOME"), None);
        let codex_memory = codex.memory(&sessions);
        assert_eq!(codex_memory.len(), 1);
        assert_eq!(codex_memory[0].sessions, 1);
        assert!(
            codex_memory[0]
                .note
                .as_deref()
                .unwrap()
                .starts_with("Codex treats")
        );

        let store = LabelStore::open(&home.var("MARDUK_DATA_DIR").join("labels.db")).unwrap();
        assert_eq!(store.labels().unwrap().len(), LABELS.len());
        let keys: HashSet<String> = sessions.iter().map(Session::key).collect();
        let assignments = store.assignments().unwrap();
        assert_eq!(assignments.len(), 8);
        assert!(assignments.keys().all(|key| keys.contains(key)));
    }

    #[cfg(unix)]
    #[test]
    fn padding_gives_sessions_their_listed_sizes() {
        let (_dir, home, _) = write_demo(true);
        let claude = ClaudeAdapter::new(home.var("CLAUDE_CONFIG_DIR")).scan();
        let port = claude
            .sessions
            .iter()
            .find(|session| session.title.as_deref() == Some("Port weather-cli to Rust"))
            .unwrap();

        assert_eq!(port.size_bytes, 41 * MB);
        assert!(claude.complete);
        let opencode = OpenCodeAdapter::with_flavor(
            &opencode::OPENCODE,
            home.var("XDG_DATA_HOME").join("opencode"),
            Some(home.var("OPENCODE_DB")),
            None,
        );
        let sessions = opencode.scan().sessions;
        let tuning = sessions
            .iter()
            .find(|session| session.title.as_deref() == Some("Tune the Postgres connection pool"))
            .unwrap();
        assert!(tuning.size_bytes > 186 * KB);
        let transcript = opencode.transcript(tuning).unwrap();
        assert!(
            transcript
                .head
                .iter()
                .chain(&transcript.tail)
                .all(|message| !message.text.contains("0000"))
        );
    }

    #[test]
    fn rewriting_replaces_an_earlier_demo() {
        let (_dir, home, now) = write_demo(false);
        let extra = home.root().join("extra.txt");
        fs::write(&extra, "").unwrap();

        home.write(&DemoOptions {
            now,
            pad_sizes: false,
        })
        .unwrap();

        assert!(!extra.exists());
        assert!(home.root().join(MARKER).is_file());
    }

    #[test]
    fn refuses_to_replace_a_folder_that_is_not_a_demo() {
        let dir = tempfile::tempdir().unwrap();
        let precious = dir.path().join("notes.txt");
        fs::write(&precious, "keep me").unwrap();

        let error = DemoHome::new(dir.path())
            .write(&DemoOptions {
                now: SystemTime::now(),
                pad_sizes: false,
            })
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&precious).unwrap(), "keep me");
    }

    #[test]
    fn ids_are_stable_and_well_formed() {
        assert_eq!(uuid("a"), uuid("a"));
        assert_ne!(uuid("a"), uuid("b"));
        assert!(crate::fs_util::looks_like_uuid(&uuid(
            "Fix flaky login test"
        )));
        assert!(
            opencode_id("x")
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        );
    }
}
