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
        labels: &[],
    }
}

impl Spec {
    const fn archived(mut self) -> Self {
        self.archived = true;
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
                json!({ "role": "assistant", "content": [
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
                json!({ "role": "assistant", "content": [{ "type": "text", "text": summary(spec.title) }] }),
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
        let cwd = self.project(spec.project)?;
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
                json!({ "sessionId": id, "context": { "cwd": cwd } }),
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
               time_archived integer);
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
                "insert into session_v2 values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    parent.map(opencode_id),
                    cwd,
                    spec.title,
                    self.millis(spec.created),
                    self.millis(spec.last),
                    spec.archived.then(|| self.millis(spec.last)),
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

fn summary(title: &str) -> String {
    format!(
        "Done: **{title}**.\n\n- Updated the code in `src/`\n- Added tests for the new behavior\n\n\
         See [the project notes](https://example.com/notes) for details."
    )
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
