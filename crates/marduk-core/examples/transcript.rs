//! Read-only check of transcript parsing against this machine's largest session per agent.
//! Run with `cargo run --release -p marduk-core --example transcript`.

use std::time::Instant;

use marduk_core::RUNNING_WINDOW;
use marduk_core::catalog::{self, Catalog};

fn main() {
    let catalog = Catalog::new(marduk_core::default_adapters(Default::default()), RUNNING_WINDOW);
    let scans = catalog.scan();
    let notes = scans
        .iter()
        .flat_map(|scan| &scan.sessions)
        .filter_map(|session| catalog.transcript(&catalog::uid(session)).ok()?.note)
        .fold(
            std::collections::BTreeMap::<String, usize>::new(),
            |mut counts, note| {
                *counts.entry(note).or_default() += 1;
                counts
            },
        );
    for (note, count) in notes {
        println!("{count} transcripts noted: {note}");
    }
    for scan in scans {
        let Some(largest) = scan.sessions.iter().max_by_key(|session| session.size_bytes) else {
            continue;
        };
        let started = Instant::now();
        match catalog.transcript(&catalog::uid(largest)) {
            Ok(transcript) => {
                let messages = transcript.head.iter().chain(&transcript.tail);
                let (mut user, mut assistant, mut tools, mut context) = (0, 0, 0, 0);
                for message in messages {
                    match message.role {
                        marduk_core::Role::User => user += 1,
                        marduk_core::Role::Assistant => assistant += 1,
                        marduk_core::Role::Tool => tools += 1,
                        marduk_core::Role::Context => context += 1,
                    }
                }
                println!(
                    "{}: {:.1} MB in {:?} -> {user} user, {assistant} assistant, {tools} tool, {context} context, {} omitted",
                    scan.display_name,
                    largest.size_bytes as f64 / 1_048_576.0,
                    started.elapsed(),
                    transcript.omitted
                );
                if let Some(first) = transcript.head.iter().find(|m| m.role == marduk_core::Role::User) {
                    println!("  first prompt: {}", first.text.chars().take(100).collect::<String>());
                }
            }
            Err(error) => println!("{}: {error}", scan.display_name),
        }
    }
}
