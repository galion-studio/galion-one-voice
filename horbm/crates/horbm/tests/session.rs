// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A turn never writes the learn pack. Only `SessionSeal::seal` does, and
//! only when learn consent is granted.
//!
//! Scratch directories live under the process temp dir and are removed when
//! the test ends. These tests do not change the process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    Brain, CardHit, CardLookup, ClauseSource, LearnConfig, LearnPass, Persona, SessionSeal,
    LEARN_PACK_DB, LEARN_PACK_LIVE, LEARN_PACK_NEXT, LEARN_PACK_RETIRING, UNKNOWN_REPLY,
};

const NORTHWIND: &str = r#"
schema = "galion-horbm-persona/1"
version = 1
id = "northwind"
voice = "northwind"

[traits]
warmth = 0.61

[honesty]
never_fabricate = true
unknown = "I don't know."

[boundaries]
forbid = ["fake-sympathy", "unsourced-fact"]

[style]
brevity = 0.5
warmth = 0.62
max_sentences = 2

[baseline]
valence = 0.1
arousal = 0.31
dominance = 0.4
approach = 0.0
pitch_centre_hz = 216.0

[inner]
care = "heard"
want = "answer"
expect = "pause"
"#;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-session-{name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("scratch");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Kettle;

impl CardLookup for Kettle {
    fn lookup(&self, query: &str) -> Option<CardHit> {
        if query.to_ascii_lowercase().contains("kettle") {
            Some(CardHit {
                id: "kettle".into(),
                gist: "The kettle is on the hob.".into(),
            })
        } else {
            None
        }
    }
}

fn northwind() -> Persona {
    Persona::parse(NORTHWIND).expect("northwind persona")
}

fn kettle_brain() -> Brain {
    let brain = Brain::with_persona(Kettle, None, northwind());
    assert_eq!(brain.persona().id(), "northwind");
    brain
}

fn source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: "sess-7".to_string(),
        turn_id,
        time: "2026-11-18T21:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn is_pack_entry(name: &std::ffi::OsStr) -> bool {
    name == LEARN_PACK_DB
        || name == LEARN_PACK_LIVE
        || name == LEARN_PACK_NEXT
        || name == LEARN_PACK_RETIRING
}

/// `LEARN_PACK_*` files and directories anywhere under `dir`.
fn pack_entries(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if is_pack_entry(&ent.file_name()) {
                out.push(path.clone());
            }
            if path.is_dir() {
                walk(&path, out);
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

fn assert_no_pack(root: &Path, pack: &Path, when: &str) {
    assert!(!pack.exists(), "{when} created {}", pack.display());
    let found = pack_entries(root);
    assert!(found.is_empty(), "{when} left {found:?}");
}

fn run_turns(brain: &mut Brain, rounds: usize, root: &Path, pack: &Path) {
    for i in 0..rounds {
        let kettle = i % 2 == 0;
        let out = brain.turn(if kettle {
            "where is the kettle"
        } else {
            "I am so tired"
        });
        if kettle {
            assert_eq!(out.reply, "The kettle is on the hob.", "turn {i}");
        } else {
            assert_eq!(out.reply, UNKNOWN_REPLY, "turn {i}");
        }
        assert_no_pack(root, pack, &format!("turn {i}"));
    }
}

#[test]
fn a_turn_writes_no_learn_pack_when_learn_is_unset() {
    let root = Scratch::new("unset");
    let pack = root.path().join("pack");
    let pack_flag = pack.to_str().expect("utf-8 pack path").to_string();
    let config = LearnConfig::from_flags(None, Some(&pack_flag));
    assert!(!config.granted, "unset learn is fresh");
    assert_eq!(config.pack_dir, pack);

    let mut brain = kettle_brain();
    let mut seal = SessionSeal::open(config);
    seal.note("user", "The kettle is on the hob.", &source(1));
    run_turns(&mut brain, 4, root.path(), &pack);

    let err = seal.seal().expect_err("unset learn");
    assert!(err.is_denied(), "{err}");
    assert_no_pack(root.path(), &pack, "unset seal");
}

#[test]
fn a_held_clause_is_not_written_by_turns() {
    let root = Scratch::new("held");
    let pack = root.path().join("pack");
    let mut brain = kettle_brain();
    let held = source(4);
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    seal.note("user", "The kettle is on the hob.", &held);
    run_turns(&mut brain, 20, root.path(), &pack);

    let row = seal.seal().expect("seal").expect("held clause");
    let label = held.label().expect("label");
    assert_eq!(row.text, "The kettle is on the hob.");
    assert_eq!(row.source, label);
    assert!(pack.join(LEARN_PACK_LIVE).join(LEARN_PACK_DB).is_file());
    assert!(!pack.join(LEARN_PACK_NEXT).exists());
    assert!(!pack.join(LEARN_PACK_RETIRING).exists());
}

#[test]
fn seal_writes_exactly_one_sourced_row_and_a_second_seal_writes_nothing() {
    let root = Scratch::new("one");
    let pack = root.path().join("pack");
    let clause = source(7);
    let text = "The kettle is on the hob.";
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    seal.note("user", text, &clause);
    assert_no_pack(root.path(), &pack, "note");

    let written = seal.seal().expect("seal").expect("one row");
    let label = clause.label().expect("label");
    assert_eq!(written.source, label);

    let audit = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows[0].source, label);
    assert_eq!(audit.rows[0].text, text);
    assert_eq!(audit.rows[0].said_by, "user");
    assert_eq!(audit.rows[0].date, clause.time);
    assert_eq!(audit.rows[0], written);
    eprintln!(
        "MEASURE session_seal rows={} source={}",
        audit.rows.len(),
        audit.rows[0].source
    );

    assert!(seal.seal().expect("second seal").is_none());
    let again = LearnPass::open(LearnConfig::granted(&pack))
        .audit()
        .expect("second audit");
    assert_eq!(again.rows.len(), 1);
    assert_eq!(again.rows[0].source, label);
    assert_eq!(again.rows[0], written);
}

#[test]
fn a_denied_seal_writes_nothing_and_creates_no_file() {
    let root = Scratch::new("denied");
    let pack = root.path().join("pack");
    let config = LearnConfig::fresh(&pack);
    assert!(!config.granted);
    let mut seal = SessionSeal::open(config);
    seal.note("user", "The kettle is on the hob.", &source(2));
    let err = seal.seal().expect_err("denied learn");
    assert!(err.is_denied(), "{err}");
    assert_no_pack(root.path(), &pack, "denied seal");
}
