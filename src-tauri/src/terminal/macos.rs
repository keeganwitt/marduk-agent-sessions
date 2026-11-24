use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use base64::prelude::{BASE64_STANDARD, Engine};
use core_foundation::array::{CFArray, CFArrayRef};
use core_foundation::base::TCFType;
use core_foundation::string::{CFString, CFStringRef};
use core_foundation::url::CFURL;
use marduk_core::agents::cli;

use super::{Invocation, TerminalPreference, detach};

const TERMINAL_ID: &str = "com.apple.Terminal";
const ITERM_ID: &str = "com.googlecode.iterm2";
const CMUX_ID: &str = "com.cmuxterm.app";

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    fn LSCopyDefaultRoleHandlerForContentType(content_type: CFStringRef, role: u32) -> CFStringRef;
    fn LSCopyApplicationURLsForBundleIdentifier(
        bundle_identifier: CFStringRef,
        error: *mut core_foundation::error::CFErrorRef,
    ) -> CFArrayRef;
}

pub fn automatic() -> Option<TerminalPreference> {
    let content_type = CFString::new("com.apple.terminal.shell-script");
    // Launch Services vends a retained CFString for kLSRolesAll, or null when no handler is set.
    let handler = unsafe {
        LSCopyDefaultRoleHandlerForContentType(content_type.as_concrete_TypeRef(), u32::MAX)
    };
    let handler =
        (!handler.is_null()).then(|| unsafe { CFString::wrap_under_create_rule(handler) });
    Some(
        handler
            .as_ref()
            .and_then(|id| terminal_for_id(&id.to_string()))
            .unwrap_or(TerminalPreference::MacTerminal),
    )
}

fn terminal_for_id(id: &str) -> Option<TerminalPreference> {
    match id {
        TERMINAL_ID => Some(TerminalPreference::MacTerminal),
        ITERM_ID => Some(TerminalPreference::ITerm2),
        CMUX_ID => Some(TerminalPreference::Cmux),
        "com.mitchellh.ghostty" => Some(TerminalPreference::Ghostty),
        "com.mitchellh.ghostty-dev" => Some(TerminalPreference::Ghostty),
        "com.github.wez.wezterm" => Some(TerminalPreference::WezTerm),
        "net.kovidgoyal.kitty" => Some(TerminalPreference::Kitty),
        "org.alacritty" => Some(TerminalPreference::Alacritty),
        _ => None,
    }
}

const MAC_TERMINAL_SCRIPT: &str = r#"
on run argv
    tell application "{app}"
        activate
        do script (item 1 of argv)
    end tell
end run
"#;

const ITERM_SCRIPT: &str = r#"
on run argv
    set commandLine to item 1 of argv
    tell application "{app}"
        activate
        set terminalWindow to (create window with default profile)
        tell current session of terminalWindow to write text commandLine
    end tell
end run
"#;

pub fn open(terminal: TerminalPreference, invocation: &Invocation) -> Result<(), String> {
    let terminal = if terminal == TerminalPreference::Automatic {
        automatic().unwrap_or(TerminalPreference::MacTerminal)
    } else {
        terminal
    };
    if let Some((name, bundle, args)) = cli_terminal(terminal) {
        let program = cli::find_program(name)
            .or_else(|| installed_app(bundle).map(|path| path.join("Contents/MacOS").join(name)))
            .filter(|path| cli::is_executable(path))
            .ok_or_else(|| {
                format!(
                    "Couldn't find {bundle}. Install it or choose another terminal in Settings."
                )
            })?;
        let mut command = Command::new(program);
        command.current_dir(&invocation.cwd);
        if terminal == TerminalPreference::Ghostty {
            command.args(["-e", "sh", "-c", &command_line(invocation)?]);
        } else {
            command
                .args(args)
                .arg(&invocation.cwd)
                .arg(if terminal == TerminalPreference::Alacritty {
                    "-e"
                } else {
                    "--"
                })
                .arg(&invocation.program)
                .args(&invocation.args);
            if let Some((key, value)) = &invocation.env {
                command.env(key, value);
            }
        }
        return detach(&mut command);
    }
    let line = command_line(invocation)?;
    if terminal == TerminalPreference::Cmux {
        return open_cmux(&line, &invocation.cwd);
    }

    let (id, title, template) = match terminal {
        TerminalPreference::MacTerminal => (TERMINAL_ID, "Terminal.app", MAC_TERMINAL_SCRIPT),
        TerminalPreference::ITerm2 => (ITERM_ID, "iTerm2", ITERM_SCRIPT),
        _ => return Err("This terminal is unavailable on macOS.".into()),
    };
    let app = application(id)
        .or_else(|| match terminal {
            TerminalPreference::MacTerminal => {
                Some(PathBuf::from("/System/Applications/Utilities/Terminal.app"))
            }
            TerminalPreference::ITerm2 => installed_app("iTerm.app"),
            _ => None,
        })
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            format!("Couldn't find {title}. Install it or choose another terminal in Settings.")
        })?;
    let app = app
        .to_str()
        .filter(|path| !path.chars().any(char::is_control))
        .ok_or("The terminal application path can't be passed to AppleScript.")?;
    let app = app.replace('\\', r"\\").replace('"', r#"\""#);
    let script = template.replace("{app}", &app);
    cli::run_capture(
        Path::new("/usr/bin/osascript"),
        &["-e", &script, &line],
        Duration::from_secs(90),
    )
    .map(|_| ())
}

fn cli_terminal(
    terminal: TerminalPreference,
) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    match terminal {
        TerminalPreference::Ghostty => Some(("ghostty", "Ghostty.app", &[])),
        TerminalPreference::WezTerm => Some((
            "wezterm",
            "WezTerm.app",
            &["start", "--always-new-process", "--cwd"],
        )),
        TerminalPreference::Kitty => Some(("kitty", "kitty.app", &["--directory"])),
        TerminalPreference::Alacritty => {
            Some(("alacritty", "Alacritty.app", &["--working-directory"]))
        }
        _ => None,
    }
}

fn application(id: &str) -> Option<PathBuf> {
    let id = CFString::new(id);
    // Launch Services returns a retained array of application URLs for the bundle identifier.
    let urls = unsafe {
        LSCopyApplicationURLsForBundleIdentifier(id.as_concrete_TypeRef(), std::ptr::null_mut())
    };
    let urls =
        (!urls.is_null()).then(|| unsafe { CFArray::<CFURL>::wrap_under_create_rule(urls) })?;
    urls.iter()
        .find_map(|url| url.to_path().filter(|path| path.is_dir()))
}

fn installed_app(name: &str) -> Option<PathBuf> {
    let from_home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Applications"));
    std::iter::once(PathBuf::from("/Applications"))
        .chain(from_home)
        .map(|dir| dir.join(name))
        .find(|path| path.is_dir())
}

fn shell_quote(value: &OsStr) -> Result<String, String> {
    let text = value
        .to_str()
        .filter(|text| !text.chars().any(char::is_control))
        .ok_or("This terminal can't open a session with control characters or non-UTF-8 paths.")?;
    Ok(format!("'{}'", text.replace('\'', "'\\''")))
}

fn command_line(invocation: &Invocation) -> Result<String, String> {
    let cwd = shell_quote(invocation.cwd.as_os_str())?;
    let mut script = format!("cd {cwd} && exec");
    if let Some((name, value)) = &invocation.env {
        script.push_str(&format!(
            " /usr/bin/env {name}={}",
            shell_quote(value.as_os_str())?
        ));
    }
    for argument in std::iter::once(invocation.program.as_os_str())
        .chain(invocation.args.iter().map(OsStr::new))
    {
        script.push(' ');
        script.push_str(&shell_quote(argument)?);
    }
    let encoded = BASE64_STANDARD.encode(script.as_bytes());
    Ok(format!(
        r#"/bin/sh -c 'exec /bin/sh -c "$(/usr/bin/printf %s {encoded} | /usr/bin/base64 -D)"'"#
    ))
}

fn open_cmux(line: &str, cwd: &Path) -> Result<(), String> {
    let program = cli::find_program("cmux")
        .or_else(|| {
            application(CMUX_ID)
                .or_else(|| installed_app("cmux.app"))
                .map(|app| app.join("Contents/Resources/bin/cmux"))
                .filter(|file| cli::is_executable(file))
        })
        .ok_or(
            "Couldn't find the cmux CLI. Install cmux or choose another terminal in Settings.",
        )?;
    let cwd = cwd.to_str().ok_or("cmux needs a UTF-8 project path.")?;
    let ready = || -> Result<bool, String> {
        let result = cli::run_capture(&program, &["ping"], Duration::from_secs(1));
        match result {
            Ok(_) => Ok(true),
            Err(error) if error.contains("Access denied") || error.contains("Authentication required") => {
                Err("cmux is refusing connections from Marduk. Its socket access setting must allow external apps, or choose another terminal.".into())
            }
            Err(_) => Ok(false),
        }
    };
    if !ready()? {
        let status = Command::new("/usr/bin/open")
            .args(["-b", CMUX_ID])
            .status()
            .map_err(|error| format!("Couldn't start cmux: {error}"))?;
        if !status.success() {
            return Err("Couldn't start cmux. Open cmux and try again.".into());
        }
        let start = Instant::now();
        while !ready()? {
            if start.elapsed() >= Duration::from_secs(5) {
                return Err(
                    "Couldn't connect to cmux. Check its socket access setting and try again."
                        .into(),
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    cli::run_capture(
        &program,
        &[
            "new-workspace",
            "--cwd",
            cwd,
            "--command",
            line,
            "--focus",
            "true",
        ],
        Duration::from_secs(5),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_supported_launch_services_handlers() {
        assert_eq!(
            terminal_for_id(TERMINAL_ID),
            Some(TerminalPreference::MacTerminal)
        );
        assert_eq!(terminal_for_id(ITERM_ID), Some(TerminalPreference::ITerm2));
        assert_eq!(terminal_for_id(CMUX_ID), Some(TerminalPreference::Cmux));
        assert_eq!(terminal_for_id("com.example.editor"), None);
    }

    #[test]
    fn shell_command_keeps_paths_and_arguments_literal() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().join("with space and 'quotes'");
        std::fs::create_dir(&cwd).unwrap();
        let program = dir.path().join("agent executable");
        marduk_core::agents::write_test_executable(
            &program,
            "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$1\" \"$MARDUK_TEST_DIR\"\n",
        );
        let arg = OsStr::new("$(echo substituted); 'quote'");
        let invocation = Invocation {
            program,
            args: vec![arg.into()],
            cwd: cwd.clone(),
            env: Some(("MARDUK_TEST_DIR", cwd.clone())),
        };
        let output = Command::new("/bin/sh")
            .args(["-c", &command_line(&invocation).unwrap()])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "{}\n{}\n{}\n",
                cwd.display(),
                arg.to_str().unwrap(),
                cwd.display()
            ),
        );
    }

    #[test]
    fn rejects_control_characters_in_terminal_input() {
        assert!(shell_quote(OsStr::new("line\nbreak")).is_err());
    }
}
