use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TerminalPreference {
    #[default]
    Automatic,
    MacTerminal,
    ITerm2,
    Cmux,
    WindowsTerminal,
    PowerShell,
    GnomeTerminal,
    Konsole,
    XfceTerminal,
    Kitty,
    Alacritty,
    Xterm,
    Ghostty,
    WezTerm,
    Mintty,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSettings {
    pub selected: TerminalPreference,
    pub automatic: Option<TerminalPreference>,
    pub options: &'static [TerminalPreference],
}

#[cfg(target_os = "macos")]
const OPTIONS: &[TerminalPreference] = &[
    TerminalPreference::MacTerminal,
    TerminalPreference::ITerm2,
    TerminalPreference::Cmux,
    TerminalPreference::Ghostty,
    TerminalPreference::WezTerm,
    TerminalPreference::Kitty,
    TerminalPreference::Alacritty,
];
#[cfg(windows)]
const OPTIONS: &[TerminalPreference] = &[
    TerminalPreference::WindowsTerminal,
    TerminalPreference::PowerShell,
    TerminalPreference::WezTerm,
    TerminalPreference::Alacritty,
    TerminalPreference::Mintty,
];
#[cfg(all(unix, not(target_os = "macos")))]
const OPTIONS: &[TerminalPreference] = &[
    TerminalPreference::GnomeTerminal,
    TerminalPreference::Konsole,
    TerminalPreference::XfceTerminal,
    TerminalPreference::Kitty,
    TerminalPreference::Alacritty,
    TerminalPreference::Xterm,
    TerminalPreference::Ghostty,
    TerminalPreference::WezTerm,
];

pub fn validate(terminal: TerminalPreference) -> Result<(), String> {
    if terminal == TerminalPreference::Automatic || OPTIONS.contains(&terminal) {
        Ok(())
    } else {
        Err("This terminal is unavailable on your operating system.".into())
    }
}

pub fn settings(selected: TerminalPreference) -> TerminalSettings {
    TerminalSettings {
        selected,
        automatic: automatic(),
        options: OPTIONS,
    }
}

pub struct Invocation {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Option<(&'static str, PathBuf)>,
}

impl Invocation {
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args).current_dir(&self.cwd);
        if let Some((name, value)) = &self.env {
            command.env(name, value);
        }
        command
    }
}

pub fn open(terminal: TerminalPreference, invocation: &Invocation) -> Result<(), String> {
    validate(terminal)?;
    platform::open(terminal, invocation)
}

fn automatic() -> Option<TerminalPreference> {
    platform::automatic()
}

fn detach(command: &mut Command) -> Result<(), String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("Couldn't open the terminal: {error}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(target_os = "macos")]
#[path = "terminal/macos.rs"]
mod platform;
#[cfg(windows)]
#[path = "terminal/windows.rs"]
mod platform;
#[cfg(all(unix, not(target_os = "macos")))]
#[path = "terminal/linux.rs"]
mod platform;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_identifiers_match_the_settings_picker() {
        assert_eq!(
            serde_json::to_string(&TerminalPreference::ITerm2).unwrap(),
            "\"iTerm2\""
        );
        assert_eq!(
            serde_json::to_string(&TerminalPreference::PowerShell).unwrap(),
            "\"powerShell\""
        );
    }

    #[test]
    fn accepts_automatic_and_platform_terminals() {
        assert!(validate(TerminalPreference::Automatic).is_ok());
        for &terminal in OPTIONS {
            assert!(validate(terminal).is_ok());
        }
        let foreign = if cfg!(windows) {
            TerminalPreference::MacTerminal
        } else {
            TerminalPreference::WindowsTerminal
        };
        assert!(validate(foreign).is_err());
    }
}
