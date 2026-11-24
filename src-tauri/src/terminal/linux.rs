use std::path::PathBuf;
use std::process::Command;

use marduk_core::agents::cli::find_program;

use super::{Invocation, OPTIONS, TerminalPreference, detach};

fn launcher(terminal: TerminalPreference) -> Option<(&'static str, &'static [&'static str])> {
    match terminal {
        TerminalPreference::GnomeTerminal => Some(("gnome-terminal", &["--"])),
        TerminalPreference::Konsole => Some(("konsole", &["-e"])),
        TerminalPreference::XfceTerminal => Some(("xfce4-terminal", &["-x"])),
        TerminalPreference::Kitty => Some(("kitty", &["--"])),
        TerminalPreference::Alacritty => Some(("alacritty", &["-e"])),
        TerminalPreference::Xterm => Some(("xterm", &["-e"])),
        TerminalPreference::Ghostty => Some(("ghostty", &["-e"])),
        TerminalPreference::WezTerm => Some(("wezterm", &["start", "--always-new-process", "--"])),
        _ => None,
    }
}

pub fn automatic() -> Option<TerminalPreference> {
    if find_program("xdg-terminal-exec").is_some() || find_program("x-terminal-emulator").is_some()
    {
        return None;
    }
    OPTIONS
        .iter()
        .copied()
        .find(|&terminal| installed(terminal).is_some())
}

fn installed(terminal: TerminalPreference) -> Option<(PathBuf, &'static [&'static str])> {
    let (name, separator) = launcher(terminal)?;
    find_program(name).map(|path| (path, separator))
}

pub fn open(terminal: TerminalPreference, invocation: &Invocation) -> Result<(), String> {
    let (program, prefix): (_, &[&str]) = if terminal == TerminalPreference::Automatic {
        find_program("xdg-terminal-exec")
            .map(|path| (path, &["--"][..]))
            .or_else(|| find_program("x-terminal-emulator").map(|path| (path, &["-e"][..])))
            .or_else(|| automatic().and_then(installed))
    } else {
        installed(terminal)
    }
    .ok_or("Couldn't find the terminal. Choose an installed terminal in Settings.")?;
    let mut command = Command::new(program);
    command.args(prefix).current_dir(&invocation.cwd);
    add_invocation(&mut command, invocation);
    detach(&mut command)
}

fn add_invocation(command: &mut Command, invocation: &Invocation) {
    command
        .args([
            "/bin/sh",
            "-c",
            "cd \"$1\" && shift && exec \"$@\"",
            "marduk",
        ])
        .arg(&invocation.cwd);
    if let Some((name, value)) = &invocation.env {
        let mut setting = std::ffi::OsString::from(format!("{name}="));
        setting.push(value);
        command.arg("/usr/bin/env").arg(setting);
    }
    command.arg(&invocation.program).args(&invocation.args);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_linux_setting_has_an_argv_launcher() {
        assert!(OPTIONS.iter().all(|&terminal| launcher(terminal).is_some()));
    }

    #[test]
    fn launch_preserves_the_project_directory_and_literal_arguments() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().join("project's files");
        std::fs::create_dir(&cwd).unwrap();
        let invocation = Invocation {
            program: "/bin/sh".into(),
            args: [
                "-c",
                "printf '%s\\n' \"$PWD\" \"$1\" \"$MARDUK_TEST_DIR\"",
                "agent",
                "$HOME's files",
            ]
            .into_iter()
            .map(Into::into)
            .collect(),
            cwd: cwd.clone(),
            env: Some(("MARDUK_TEST_DIR", cwd.clone())),
        };
        let mut args = Command::new("/usr/bin/env");
        add_invocation(&mut args, &invocation);
        let output = args.output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{}\n$HOME's files\n{}\n", cwd.display(), cwd.display())
        );
    }
}
