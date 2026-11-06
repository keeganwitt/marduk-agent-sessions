//! Read-only scan of this machine's agent sessions, for checking adapters against real data.
//! Run with `cargo run -p marduk-core --example scan`.

use std::time::SystemTime;

use marduk_core::RUNNING_WINDOW;
use marduk_core::catalog::Catalog;

fn main() {
    let catalog = Catalog::new(marduk_core::default_adapters(), RUNNING_WINDOW);
    let now = SystemTime::now();
    for scan in catalog.scan() {
        let bytes: u64 = scan.sessions.iter().map(|session| session.size_bytes).sum();
        let untitled = scan.sessions.iter().filter(|session| session.title.is_none()).count();
        let running = scan
            .sessions
            .iter()
            .filter(|session| session.is_running(now, RUNNING_WINDOW))
            .count();
        println!(
            "{}: {} sessions, {:.1} MB, {untitled} untitled, {running} running, complete={}",
            scan.display_name,
            scan.sessions.len(),
            bytes as f64 / 1_048_576.0,
            scan.complete
        );
        for error in scan.errors.iter().take(5) {
            println!("  error: {error}");
        }
        let mut sessions = scan.sessions;
        sessions.sort_by_key(|session| std::cmp::Reverse(session.last_activity));
        for session in sessions.iter().take(3) {
            println!(
                "  {} | {} | {} files | {}",
                session.last_activity.format("%Y-%m-%d %H:%M"),
                session.title.as_deref().unwrap_or("(untitled)"),
                session.files.len(),
                session.project_dir.as_deref().unwrap_or("-")
            );
        }
    }
}
