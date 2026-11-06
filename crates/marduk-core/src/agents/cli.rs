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
        self.resolved
            .lock()
            .unwrap()
            .as_ref()
            .map(|(path, _)| path.clone())
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

/// Runs `program args…` and returns its standard output. Output is read while the process
/// runs; reading only after it exits would deadlock once the output fills the pipe buffer.
pub fn run_capture(
    program: &Path,
    args: &[&str],
    timeout: std::time::Duration,
) -> Result<String, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Couldn't run {}: {error}", program.display()))?;
    let mut stdout = child.stdout.take().ok_or("no stdout")?;
    let mut stderr = child.stderr.take().ok_or("no stderr")?;
    let stdout_reader = std::thread::spawn(move || {
        let mut output = String::new();
        let _ = stdout.read_to_string(&mut output);
        output
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut output = String::new();
        let _ = stderr.read_to_string(&mut output);
        output
    });
    let command_line = std::iter::once(program.display().to_string())
        .chain(args.iter().map(|arg| arg.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{command_line} didn't finish within {timeout:?}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    let output = stdout_reader.join().unwrap_or_default();
    let errors = stderr_reader.join().unwrap_or_default();
    if status.success() {
        Ok(output)
    } else {
        let detail: String = errors.trim().chars().take(300).collect();
        Err(format!("{command_line} failed ({status}): {detail}"))
    }
}

/// First line of `<program> --version`, to show which build Marduk will run.
pub fn version(program: &Path) -> Result<String, String> {
    let output = run_capture(program, &["--version"], VERSION_TIMEOUT)?;
    output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("{} --version printed nothing", program.display()))
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

    #[cfg(unix)]
    #[test]
    fn run_capture_handles_output_larger_than_the_pipe_buffer() {
        let output = run_capture(
            Path::new("/bin/sh"),
            &["-c", "head -c 1000000 /dev/zero | tr '\\0' x"],
            std::time::Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(output.len(), 1_000_000);
    }

    #[cfg(unix)]
    #[test]
    fn run_capture_times_out_and_reports_stderr() {
        let slow = run_capture(
            Path::new("/bin/sh"),
            &["-c", "sleep 5"],
            std::time::Duration::from_millis(200),
        );
        assert!(slow.unwrap_err().contains("didn't finish"));
        let failing = run_capture(
            Path::new("/bin/sh"),
            &["-c", "echo nope >&2; exit 4"],
            std::time::Duration::from_secs(5),
        );
        assert!(failing.unwrap_err().contains("nope"));
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
