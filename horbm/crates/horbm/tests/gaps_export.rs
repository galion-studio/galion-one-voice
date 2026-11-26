// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `horbm-gaps` over two synthetic turn logs, one of them rotated.
//!
//! The lines are fixtures. No research directory is read and no card text
//! is included.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LIVE: &str = r#"{"ts":"2026-11-01T00:00:03","session":"s","kind":"gap","turn":1,"heard":"Who won the match","reason":"no_card","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:09","session":"s","kind":"gap","turn":2,"heard":"who \t won   the match","reason":"research_miss","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:04","session":"s","kind":"gap","turn":3,"heard":"withheld","reason":"no_card","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:05","session":"s","kind":"gap","turn":4,"heard":"","reason":"no_card","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:06","session":"s","kind":"gap","turn":5,"heard":"why is the sky blue","reason":"research_answered","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:07","session":"s","kind":"reply","turn":5,"text":"I don't know."}

{this is not json
{"kind":"gap","heard":"orphan line","ts":"2026-11-01T00:00:15"}
{"ts":"2026-11-01T00:00:08","session":"s","kind":"gap","turn":6,"heard":"Where is the kettle","reason":"card_unspeakable","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:02","session":"s","kind":"gap","turn":7,"heard":"alpha lamp","reason":"research_unreachable","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:14","session":"s","kind":"gap","turn":8,"heard":"alpha lamp","reason":"research_unreachable","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:16","session":"s","kind":"gap","turn":9,"heard":"Say  \"Hello\"","reason":"no_card","advice":"ask_galion_research"}
"#;

const ROTATED: &str = r#"{"ts":"2026-11-01T00:00:00","session":"s","kind":"gap","turn":1,"heard":"who won the Match","reason":"no_card","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:01","session":"s","kind":"gap","turn":2,"heard":"WHO won the match","reason":"research_timeout","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:10","session":"s","kind":"gap","turn":3,"heard":"where is the kettle","reason":"research_off","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:11","session":"s","kind":"gap","turn":4,"heard":"   ","reason":"no_card","advice":"ask_galion_research"}
{"ts":"2026-11-01T00:00:12","session":"s","kind":"gap","turn":5,"heard":"zeta cord","reason":"no_card"}
{"ts":"2026-11-01T00:00:13","session":"s","kind":"session_end"}
{"ts":"2026-11-01T00:00:17","session":"s","kind":"follow_up","turn":3,"outcome":"miss","sources":[],"text":"later"}
"#;

const EXPECTED: &str = r#"{"heard":"who won the match","reasons":["no_card","research_miss","research_timeout"],"count":4,"first_ts":"2026-11-01T00:00:00","last_ts":"2026-11-01T00:00:09"}
{"heard":"alpha lamp","reasons":["research_unreachable"],"count":2,"first_ts":"2026-11-01T00:00:02","last_ts":"2026-11-01T00:00:14"}
{"heard":"where is the kettle","reasons":["card_unspeakable","research_off"],"count":2,"first_ts":"2026-11-01T00:00:08","last_ts":"2026-11-01T00:00:10"}
{"heard":"say \"hello\"","reasons":["no_card"],"count":1,"first_ts":"2026-11-01T00:00:16","last_ts":"2026-11-01T00:00:16"}
{"heard":"zeta cord","reasons":["no_card"],"count":1,"first_ts":"2026-11-01T00:00:12","last_ts":"2026-11-01T00:00:12"}
"#;

#[test]
fn duplicates_merge_and_closed_lines_are_skipped() {
    let dir = scratch("merge");
    let (live, rotated) = write_logs(&dir);
    let first_out = dir.join("gaps-a.jsonl");
    let second_out = dir.join("gaps-b.jsonl");
    let first = gaps(&[&live, &rotated], &first_out);
    let second = gaps(&[&rotated, &live], &second_out);
    let stderr = String::from_utf8_lossy(&first.stderr);
    assert!(first.status.success(), "{stderr}");
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert!(
        first.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&first.stdout)
    );
    assert!(
        second.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&second.stdout)
    );
    assert_eq!(stderr.as_ref(), "horbm-gaps: malformed 2\n");
    assert_eq!(first.stderr, second.stderr);
    let written = std::fs::read(&first_out).unwrap();
    assert_eq!(written, std::fs::read(&second_out).unwrap());
    assert_eq!(written, EXPECTED.as_bytes());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_input_exits_2() {
    let dir = scratch("missing");
    let live = dir.join("turns.jsonl");
    std::fs::write(&live, LIVE).unwrap();
    let missing = dir.join("turns.jsonl.1");
    let out = dir.join("gaps.jsonl");
    let output = gaps(&[&live, &missing], &out);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out.exists(), "a missing input must not write an output");
    let _ = std::fs::remove_dir_all(&dir);
}

fn gaps(logs: &[&Path], out: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_horbm-gaps"))
        .args(logs)
        .arg("--out")
        .arg(out)
        .output()
        .expect("run horbm-gaps")
}

fn write_logs(dir: &Path) -> (PathBuf, PathBuf) {
    let live = dir.join("turns.jsonl");
    let rotated = dir.join("turns.jsonl.1");
    std::fs::write(&live, LIVE).unwrap();
    std::fs::write(&rotated, ROTATED).unwrap();
    (live, rotated)
}

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "horbm-gaps-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
