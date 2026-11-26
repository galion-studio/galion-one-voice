// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! A seal keeps a heard line only when it teaches the previous miss, and the
//! learn pack keeps every such clause.
//!
//! Scratch directories live under the process temp dir and are removed when
//! the test ends. These tests do not change the process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{ClauseSource, LearnConfig, LearnPass, SessionSeal};

/// `LEARN_PACK_CAP` in `learn.rs`. 256, the same number as S111's `LEARNED_ROW_CAP`.
const LEARN_PACK_CAP: usize = 256;

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-taught-{name}-{}-{nanos}",
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

fn source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: format!("sess-{turn_id}"),
        turn_id,
        time: "2026-11-18T21:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn audit(pack: &Path) -> horbm::LearnAudit {
    LearnPass::open(LearnConfig::granted(pack))
        .audit()
        .expect("audit")
}

#[test]
fn a_line_after_a_miss_that_covers_it_is_kept() {
    let root = Scratch::new("kept");
    let pack = root.path().join("pack");
    let missed = "where is the kettle";
    let heard = "the kettle is on the stove";
    let clause = source(1);
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(seal.note_taught(missed, "user", heard, &clause));
    let row = seal.seal().expect("seal").expect("kept");
    assert_eq!(row.text, heard);
    assert_eq!(row.said_by, "user");
    assert_eq!(row.date, clause.time);
    assert_eq!(row.source, clause.label().expect("label"));
    let audit = audit(&pack);
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows[0], row);
}

#[test]
fn a_question_is_not_kept() {
    let root = Scratch::new("question");
    let pack = root.path().join("pack");
    let missed = "where is the kettle";
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(!seal.note_taught(missed, "user", "where is the kettle", &source(1)));
    assert!(!seal.note_taught(missed, "user", "Is the kettle on the stove", &source(2)));
    assert!(!seal.note_taught(missed, "user", "the kettle is on the stove?", &source(3)));
    assert!(seal.seal().expect("empty").is_none());
    assert!(!pack.exists(), "a question created {}", pack.display());
}

#[test]
fn a_line_with_no_miss_is_not_kept() {
    let root = Scratch::new("nomiss");
    let pack = root.path().join("pack");
    let heard = "the kettle is on the stove";
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(!seal.note_taught("", "user", heard, &source(1)));
    assert!(!seal.note_taught("   ", "user", heard, &source(2)));
    assert!(seal.seal().expect("empty").is_none());
    assert!(
        !pack.exists(),
        "a line with no miss created {}",
        pack.display()
    );
}

#[test]
fn a_line_that_does_not_cover_the_miss_is_not_kept() {
    let root = Scratch::new("cover");
    let pack = root.path().join("pack");
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(!seal.note_taught(
        "where is the kettle",
        "user",
        "the window is open",
        &source(1)
    ));
    assert!(seal.seal().expect("empty").is_none());
    assert!(
        !pack.exists(),
        "an uncovered line created {}",
        pack.display()
    );
}

#[test]
fn two_sessions_leave_two_rows() {
    let root = Scratch::new("two");
    let pack = root.path().join("pack");
    let kettle = "the kettle is on the stove";
    let mug = "the mug is in the cupboard";

    let mut first = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(first.note_taught("where is the kettle", "user", kettle, &source(1)));
    let row_a = first.seal().expect("first seal").expect("kettle");
    assert_eq!(row_a.text, kettle);
    drop(first);

    let mut second = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(second.note_taught("where is the mug", "user", mug, &source(2)));
    let row_b = second.seal().expect("second seal").expect("mug");
    assert_eq!(row_b.text, mug);

    let audit = audit(&pack);
    assert_eq!(audit.rows.len(), 2);
    assert_eq!(audit.archived, 0);
    assert!(audit
        .rows
        .iter()
        .any(|row| row.text == kettle && row.said_by == "user"));
    assert!(audit.rows.iter().any(|row| row == &row_b));
}

#[test]
fn the_same_clause_twice_is_one_row() {
    let root = Scratch::new("same");
    let pack = root.path().join("pack");
    let heard = "the kettle is on the stove";
    let mut seal = SessionSeal::open(LearnConfig::granted(&pack));
    assert!(seal.note_taught("where is the kettle", "user", heard, &source(1)));
    let row = seal.seal().expect("first").expect("row");

    assert!(seal.note_taught(
        "where is the kettle",
        "user",
        "  the kettle is on the stove  ",
        &source(9)
    ));
    let again = seal.seal().expect("second").expect("existing");
    assert_eq!(again.item_id, row.item_id);
    assert_eq!(again.text, row.text);
    assert_eq!(again.said_by, row.said_by);
    assert_eq!(again.source, row.source);
    assert_eq!(again.date, row.date);

    let audit = audit(&pack);
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows[0], row);
}

#[test]
fn cap_refuses_and_keeps_every_row() {
    let root = Scratch::new("cap");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let mut texts = Vec::with_capacity(LEARN_PACK_CAP);
    for n in 0..LEARN_PACK_CAP {
        let text = format!("clause {n} the kettle is on the stove");
        let row = learn
            .write_clause("user", &text, &source(n as u64))
            .expect("write");
        assert_eq!(row.text, text);
        texts.push(text);
    }
    let again = learn
        .write_clause("user", &texts[0], &source(9_000))
        .expect("duplicate at the cap");
    assert_eq!(again.text, texts[0]);
    let extra = "clause extra the kettle is beside the stove";
    let err = learn
        .write_clause("user", extra, &source(9_001))
        .expect_err("cap");
    assert!(err.is_refused(), "{err}");
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), LEARN_PACK_CAP);
    assert_eq!(audit.archived, 0);
    for text in &texts {
        assert!(
            audit
                .rows
                .iter()
                .any(|row| row.text == *text && row.said_by == "user"),
            "dropped {text}"
        );
    }
    assert!(audit.rows.iter().all(|row| row.text != extra));
}

#[test]
fn withdraw_still_leaves_no_row() {
    let root = Scratch::new("withdraw");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    learn
        .write_clause("user", "the kettle is on the stove", &source(1))
        .expect("kettle");
    learn
        .write_clause("user", "the mug is in the cupboard", &source(2))
        .expect("mug");
    assert_eq!(learn.audit().expect("before").rows.len(), 2);
    learn.withdraw().expect("withdraw");
    let after = learn.audit().expect("after");
    assert!(after.rows.is_empty());
    assert_eq!(after.archived, 0);
    assert!(!learn.live_pack().exists());
    learn.withdraw().expect("withdraw again");
    assert!(learn.audit().expect("still empty").rows.is_empty());
}
