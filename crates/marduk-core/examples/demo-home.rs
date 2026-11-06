//! Writes a demo home of made-up sessions for every agent and prints the environment that
//! points Marduk at it: `cargo run -p marduk-core --example demo-home -- <dir> [--json]
//! [--no-padding]`. See docs/demo.md.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::SystemTime;

use marduk_core::demo::{DemoHome, DemoOptions};

const USAGE: &str = "usage: demo-home <dir> [--json] [--no-padding]";

fn main() -> ExitCode {
    let mut dir = None;
    let mut json = false;
    let mut pad_sizes = true;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--json" => json = true,
            "--no-padding" => pad_sizes = false,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if arg.starts_with('-') || dir.is_some() => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
            _ => dir = Some(PathBuf::from(arg)),
        }
    }
    let Some(dir) = dir else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };
    let dir = std::path::absolute(&dir).unwrap_or(dir);
    let home = DemoHome::new(dir);
    let options = DemoOptions {
        now: SystemTime::now(),
        pad_sizes,
    };
    if let Err(error) = home.write(&options) {
        eprintln!("Couldn't write the demo home: {error}");
        return ExitCode::FAILURE;
    }
    let env = home.env();
    if json {
        let map: serde_json::Map<String, serde_json::Value> = env
            .into_iter()
            .map(|(key, path)| (key.to_string(), path.to_string_lossy().into()))
            .collect();
        println!("{}", serde_json::Value::Object(map));
    } else {
        eprintln!("Wrote a demo home to {}", home.root().display());
        for (key, path) in env {
            println!("{key}={}", path.display());
        }
    }
    ExitCode::SUCCESS
}
