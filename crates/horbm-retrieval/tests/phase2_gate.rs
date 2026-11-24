// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 2 gate.
//!
//! With `remember` denied, the cell is empty and nothing is on disk.
//! A 1,000-row fixture returns in the test, untimed.
//!
//! Proving command, from the repo root:
//! cargo test -p horbm-retrieval -- --test-threads=1

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm_retrieval::{run_retrieval, FixtureRow, RunRequest, SessionConsent};

#[test]
fn locks_remember_default_is_deny() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../LOCKS.toml");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    let section = text
        .split("[consent]")
        .nth(1)
        .unwrap_or_else(|| panic!("LOCKS.toml has no [consent]"));
    let section = section.split("\n[").next().unwrap_or(section);
    assert!(
        section.contains("remember = \"deny\""),
        "LOCKS.toml [consent] remember must stay deny"
    );
    assert!(!SessionConsent::default().remember());
    assert!(!SessionConsent::denied().remember());
}

#[test]
fn remember_denied_cell_empty_nothing_on_disk_and_fixture_returns() {
    let scratch = Scratch::new("deny");
    let knowledge = scratch.0.join("knowledge");
    copy_dir(&fixture_root(), &knowledge);
    let memory_db = scratch.0.join("memory.db");
    let rows: Vec<FixtureRow> = (0..1000)
        .map(|i| FixtureRow {
            card_id: format!("fixture.row.{i:04}"),
            clause: format!("fixture clause t{i:04}"),
            source: "ram-fixture".to_string(),
        })
        .collect();
    assert_eq!(rows.len(), 1000);

    let outcome = run_retrieval(RunRequest {
        knowledge_dir: knowledge.clone(),
        memory_db: Some(memory_db.clone()),
        consent: SessionConsent::default(),
        stable_prefix: "t0042".to_string(),
        extra_rows: rows,
    })
    .expect("retrieval");

    assert!(
        outcome.cell.generation >= 1,
        "publish did not run; an empty cell at generation 0 would be a false pass"
    );
    assert!(
        outcome.cell.hits.is_empty(),
        "remember denied must publish an empty cell"
    );
    assert_eq!(outcome.memory_db.as_str(), "not created");
    assert_eq!(outcome.revisions.as_str(), "not created");
    assert!(!memory_db.exists());
    assert!(!knowledge.join("revisions.sqlite").exists());
    assert!(!scratch.0.join("revisions.sqlite").exists());
    let hit = outcome
        .hits
        .iter()
        .find(|hit| hit.card_id == "fixture.row.0042")
        .expect("1,000-row fixture did not return t0042");
    assert_eq!(hit.source, "ram-fixture");
    assert!(hit.score > 0.0);
    assert!(hit.clause.contains("t0042"));
    assert_eq!(outcome.report.cards, 2);
    assert_eq!(outcome.report.manifest, "sha256 ok");

    let anchor = run_retrieval(RunRequest {
        knowledge_dir: knowledge,
        memory_db: Some(memory_db),
        consent: SessionConsent::denied(),
        stable_prefix: "phase2anchor".to_string(),
        extra_rows: Vec::new(),
    })
    .expect("fixture query");
    assert!(anchor.cell.hits.is_empty());
    assert_eq!(anchor.cell.generation, 1);
    let card = anchor
        .hits
        .iter()
        .find(|hit| hit.card_id == "world.fixture.anchor")
        .expect("committed fixture card did not return");
    assert!(card.source.contains("HORBM fixture"));
    assert!(card.clause.contains("phase2anchor"));
    assert!(!scratch.0.join("memory.db").exists());
}

#[test]
fn remember_granted_publishes_hits_and_still_writes_nothing() {
    let scratch = Scratch::new("grant");
    let knowledge = scratch.0.join("knowledge");
    copy_dir(&fixture_root(), &knowledge);
    let memory_db = scratch.0.join("memory.db");
    let outcome = run_retrieval(RunRequest {
        knowledge_dir: knowledge,
        memory_db: Some(memory_db.clone()),
        consent: SessionConsent::granted_remember(),
        stable_prefix: "phase2anchor".to_string(),
        extra_rows: Vec::new(),
    })
    .expect("grant");
    assert!(!outcome.cell.hits.is_empty());
    assert_eq!(outcome.cell.hits[0].card_id, "world.fixture.anchor");
    assert_eq!(outcome.memory_db.as_str(), "not created");
    assert!(!memory_db.exists());
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/mini")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "horbm-retrieval-{label}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        let file_type = entry.file_type().unwrap();
        if file_type.is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}
