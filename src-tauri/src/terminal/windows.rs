use std::ffi::OsStr;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

use base64::prelude::{BASE64_STANDARD, Engine};
use marduk_core::agents::cli::find_program;

use super::{Invocation, TerminalPreference, detach};

const CREATE_NEW_CONSOLE: u32 = 0x00000010;

pub fn automatic() -> Option<TerminalPreference> {
    None
}

fn encode_utf16(value: impl Iterator<Item = u16>) -> String {
    let bytes: Vec<u8> = value.flat_map(u16::to_le_bytes).collect();
    BASE64_STANDARD.encode(bytes)
}

fn powershell_value(value: &OsStr) -> String {
    use std::os::windows::ffi::OsStrExt;
    let encoded = encode_utf16(value.encode_wide());
    format!("([Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('{encoded}')))")
}

fn encoded_command(invocation: &Invocation) -> Result<String, String> {
    let program = powershell_value(invocation.program.as_os_str());
    let args = invocation
        .args
        .iter()
        .map(|arg| powershell_value(arg))
        .collect::<Vec<_>>();
    let mut script = format!(
        "Set-Location -LiteralPath {} -ErrorAction Stop; ",
        powershell_value(invocation.cwd.as_os_str())
    );
    if let Some((key, value)) = &invocation.env {
        script.push_str(&format!(
            "$env:{key} = {}; ",
            powershell_value(value.as_os_str())
        ));
    }
    if matches!(invocation.program.extension().and_then(OsStr::to_str), Some(ext) if ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat"))
        && invocation.args.iter().any(|arg| {
            arg.to_string_lossy()
                .contains(['%', '!', '"', '&', '|', '<', '>', '^', '(', ')', '\r', '\n'])
        })
    {
        return Err("This Windows batch launcher can't safely pass the session path. Choose a native executable in Agent CLIs.".into());
    }
    script.push_str(&format!("& {program} {}", args.join(" ")));
    Ok(encode_utf16(script.encode_utf16()))
}

pub fn open(terminal: TerminalPreference, invocation: &Invocation) -> Result<(), String> {
    let system32 = std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("System32"));
    let powershell = system32
        .as_ref()
        .map(|dir| dir.join("WindowsPowerShell/v1.0/powershell.exe"))
        .filter(|file| file.is_file())
        .or_else(|| find_program("powershell"))
        .ok_or("Couldn't find Windows PowerShell.")?;
    let mut command = if terminal == TerminalPreference::WindowsTerminal {
        let wt = find_program("wt")
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .map(|dir| dir.join("Microsoft/WindowsApps/wt.exe"))
                    .filter(|file| file.is_file())
            })
            .ok_or("Couldn't find Windows Terminal. Install it or choose another terminal in Settings.")?;
        let mut command = Command::new(wt);
        command.arg("new-tab").arg(&powershell);
        command
    } else if let Some((name, args)) = cli_terminal(terminal) {
        let launcher = find_program(name)
            .or_else(|| {
                if terminal != TerminalPreference::Mintty {
                    return None;
                }
                let mut root = PathBuf::from(std::env::var_os("SystemDrive")?);
                root.push(std::path::MAIN_SEPARATOR_STR);
                let path = root.join("cygwin64/bin/mintty.exe");
                path.is_file().then_some(path)
            })
            .ok_or_else(|| {
                format!("Couldn't find {name}. Install it or choose another terminal in Settings.")
            })?;
        let mut command = Command::new(launcher);
        command.args(args).arg(&powershell);
        command
    } else if terminal == TerminalPreference::PowerShell {
        let conhost = system32
            .map(|dir| dir.join("conhost.exe"))
            .filter(|file| file.is_file())
            .ok_or("Couldn't find Windows Console Host.")?;
        let mut command = Command::new(conhost);
        command.arg(&powershell).creation_flags(CREATE_NEW_CONSOLE);
        command
    } else {
        let mut command = Command::new(powershell);
        command.creation_flags(CREATE_NEW_CONSOLE);
        command
    };
    command
        .args(["-NoExit", "-EncodedCommand", &encoded_command(invocation)?])
        .current_dir(&invocation.cwd);
    detach(&mut command)
}

fn cli_terminal(terminal: TerminalPreference) -> Option<(&'static str, &'static [&'static str])> {
    match terminal {
        TerminalPreference::WezTerm => Some(("wezterm", &["start", "--always-new-process", "--"])),
        TerminalPreference::Alacritty => Some(("alacritty", &["-e"])),
        TerminalPreference::Mintty => Some(("mintty", &["--pcon", "on", "-e"])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_value_preserves_unicode_without_interpolation() {
        let original = "résumé’s `$args";
        let expression = powershell_value(OsStr::new(original));
        let encoded = expression.split('\'').nth(1).unwrap();
        assert_eq!(encoded, encode_utf16(original.encode_utf16()));
        assert!(!expression.contains("$args"));
    }
}
