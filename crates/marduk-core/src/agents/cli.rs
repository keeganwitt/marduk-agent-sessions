use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// User-chosen CLI locations, keyed by agent id. Shared between the app's settings and the
/// adapters so a change applies on the next scan.
#[derive(Default)]
pub struct CliOverrides(RwLock<HashMap<String, PathBuf>>);

impl CliOverrides {
    pub fn get(&self, agent: &str) -> Option<PathBuf> {
        self.0.read().unwrap().get(agent).cloned()
    }

    pub fn set(&self, agent: &str, path: Option<PathBuf>) {
        let mut paths = self.0.write().unwrap();
        match path {
            Some(path) => paths.insert(agent.to_string(), path),
            None => paths.remove(agent),
        };
    }

    pub fn all(&self) -> HashMap<String, PathBuf> {
        self.0.read().unwrap().clone()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliInfo {
    pub program: &'static str,
    pub install_url: &'static str,
    pub path: Option<PathBuf>,
    /// True when `path` came from the user's setting rather than auto-detection.
    pub overridden: bool,
}

enum Source {
    Fixed(Option<PathBuf>),
    Detect {
        agent: &'static str,
        overrides: Arc<CliOverrides>,
    },
}

/// An agent's command-line tool, resolved once per scan so a newly installed CLI or a
/// changed setting is picked up without restarting.
pub struct Cli {
    program: &'static str,
    install_url: &'static str,
    source: Source,
    resolved: Mutex<Option<(PathBuf, bool)>>,
}

impl Cli {
    pub fn fixed(program: &'static str, install_url: &'static str, path: Option<PathBuf>) -> Self {
        let cli = Self {
            program,
            install_url,
            source: Source::Fixed(path),
            resolved: Mutex::new(None),
        };
        cli.refresh();
        cli
    }

    pub fn detect(
        agent: &'static str,
        program: &'static str,
        install_url: &'static str,
        overrides: Arc<CliOverrides>,
    ) -> Self {
        let cli = Self {
            program,
            install_url,
            source: Source::Detect { agent, overrides },
            resolved: Mutex::new(None),
        };
        cli.refresh();
        cli
    }

    pub fn refresh(&self) {
        let resolved = match &self.source {
            Source::Fixed(path) => path.clone().map(|path| (path, false)),
            Source::Detect { agent, overrides } => match overrides.get(agent) {
                Some(path) => Some((path, true)),
                None => super::find_program(self.program).map(|path| (path, false)),
            },
        };
        *self.resolved.lock().unwrap() = resolved;
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.resolved.lock().unwrap().as_ref().map(|(path, _)| path.clone())
    }

    pub fn info(&self) -> CliInfo {
        let resolved = self.resolved.lock().unwrap().clone();
        CliInfo {
            program: self.program,
            install_url: self.install_url,
            overridden: resolved.as_ref().is_some_and(|(_, overridden)| *overridden),
            path: resolved.map(|(path, _)| path),
        }
    }

    pub fn missing_reason(&self, agent_name: &str, purpose: &str) -> String {
        format!(
            "Install the {agent_name} CLI (`{}`), or choose where it is in Settings, so Marduk can {purpose}.",
            self.program
        )
    }
}

pub fn is_executable(path: &Path) -> bool {
    super::is_executable(path)
}

const VERSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// First line of `<program> --version`, to show which build Marduk will run.
pub fn version(program: &Path) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Couldn't run {}: {error}", program.display()))?;
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > VERSION_TIMEOUT {
            let _ = child.kill();
            return Err(format!("{} --version didn't finish", program.display()));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let mut output = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut output);
    }
    let first_line = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    if status.success() && !first_line.is_empty() {
        Ok(first_line.to_string())
    } else {
        Err(format!("{} --version failed ({status})", program.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins_and_clearing_it_falls_back_to_detection() {
        let overrides = Arc::new(CliOverrides::default());
        let cli = Cli::detect(
            "fake",
            "marduk-test-no-such-program",
            "https://example.com",
            overrides.clone(),
        );
        assert_eq!(cli.path(), None);

        overrides.set("fake", Some(PathBuf::from("/opt/fake/bin/tool")));
        assert_eq!(cli.path(), None, "takes effect on the next refresh");
        cli.refresh();
        assert_eq!(
            cli.info(),
            CliInfo {
                program: "marduk-test-no-such-program",
                install_url: "https://example.com",
                path: Some(PathBuf::from("/opt/fake/bin/tool")),
                overridden: true,
            }
        );

        overrides.set("fake", None);
        cli.refresh();
        assert_eq!(cli.path(), None);
    }

    #[cfg(unix)]
    #[test]
    fn version_reads_the_first_line_and_reports_failures() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-cli");
        std::fs::write(&script, "#!/bin/sh\necho\necho fake-cli 1.2.3\necho more\n").unwrap();
        let failing = dir.path().join("broken-cli");
        std::fs::write(&failing, "#!/bin/sh\nexit 2\n").unwrap();
        for path in [&script, &failing] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert_eq!(version(&script).unwrap(), "fake-cli 1.2.3");
        assert!(version(&failing).unwrap_err().contains("failed"));
        assert!(version(&dir.path().join("missing")).is_err());
    }

    #[test]
    fn missing_reason_mentions_settings() {
        let cli = Cli::fixed("codex", "https://example.com", None);
        assert_eq!(
            cli.missing_reason("Codex", "delete Codex sessions"),
            "Install the Codex CLI (`codex`), or choose where it is in Settings, so Marduk can delete Codex sessions."
        );
    }
}
