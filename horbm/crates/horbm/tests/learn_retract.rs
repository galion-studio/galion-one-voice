// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One learned clause can be taken back. Withdrawal still removes the whole pack.
//!
//! Scratch directories live under the process temp dir and are removed when
//! the test ends. These tests do not change the process environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    ClauseSource, LearnConfig, LearnPass, LearnedRow, LEARN_PACK_NEXT, LEARN_PACK_RETIRING,
};

/// `LEARN_PACK_CAP` in `learn.rs`. 256, the same number as S111's `LEARNED_ROW_CAP`.
const LEARN_PACK_CAP: usize = 256;

const PERSON: &str = "northwind";
const OAR: &str = "The spare oar is in the locker.";
const HOB: &str = "The kettle is on the hob.";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "horbm-learn-retract-{name}-{}-{nanos}",
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
        time: format!("2026-11-26T00:{turn_id:04}Z"),
        consent_record: "learn-v1".to_string(),
    }
}

fn carries(row: &LearnedRow, said_by: &str, text: &str, source: &ClauseSource) -> bool {
    row.said_by == said_by
        && row.text == text
        && row.date == source.time
        && row.source == source.label().expect("label")
        && !row.item_id.is_empty()
}

/// FNV-1a of the live database bytes. Same bytes, same hash.
fn fnv(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn pack_files(pack: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, u64)>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                walk(&path, root, out);
                continue;
            }
            let bytes =
                fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            out.push((rel, fnv(&bytes)));
        }
    }
    walk(pack, pack, &mut out);
    out.sort();
    out
}

#[test]
fn retract_middle_of_three_leaves_the_other_two_in_order() {
    let root = Scratch::new("middle");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let first = source(1);
    let middle = source(2);
    let third = source(3);
    learn.write_clause(PERSON, OAR, &first).expect("oar");
    learn.write_clause(PERSON, HOB, &middle).expect("hob");
    learn.write_clause("user", HOB, &third).expect("user hob");

    assert!(learn
        .retract_clause("  northwind  ", "  The kettle is on the hob.  ")
        .expect("retract middle"));

    let audit = learn.audit().expect("audit");
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows.len(), 2, "{:?}", audit.rows);
    assert!(
        carries(&audit.rows[0], PERSON, OAR, &first),
        "{:?}",
        audit.rows[0]
    );
    assert!(
        carries(&audit.rows[1], "user", HOB, &third),
        "{:?}",
        audit.rows[1]
    );
    assert!(audit
        .rows
        .iter()
        .all(|row| !(row.said_by == PERSON && row.text == HOB)));
}

#[test]
fn unknown_clause_leaves_the_pack_byte_identical() {
    let root = Scratch::new("unknown");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let clause = source(4);
    learn.write_clause(PERSON, OAR, &clause).expect("write");
    let db = learn.live_pack();
    let before = fs::read(&db).expect("db");
    let before_hash = fnv(&before);
    let before_pack = pack_files(&pack);

    assert!(!learn
        .retract_clause(PERSON, "The window is shut.")
        .expect("unknown text"));
    assert!(!learn.retract_clause("user", OAR).expect("other speaker"));

    let after = fs::read(&db).expect("db after");
    let after_hash = fnv(&after);
    assert_eq!(after_hash, before_hash, "db hash changed");
    assert!(
        after == before,
        "unknown retract rewrote the db (before {} bytes {before_hash:016x}, after {} bytes {after_hash:016x})",
        before.len(),
        after.len()
    );
    assert_eq!(pack_files(&pack), before_pack);
    assert_eq!(learn.audit().expect("audit").rows.len(), 1);
}

#[test]
fn retract_last_row_leaves_an_empty_live_pack() {
    let root = Scratch::new("last");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    learn.write_clause(PERSON, HOB, &source(5)).expect("write");
    assert!(learn.retract_clause(PERSON, HOB).expect("retract last"));

    let audit = learn.audit().expect("empty audit");
    assert!(audit.rows.is_empty(), "{:?}", audit.rows);
    assert_eq!(audit.archived, 0);
    assert!(learn.live_pack().is_file(), "empty pack file stays");
    assert!(!pack.join(LEARN_PACK_NEXT).exists());
    assert!(!pack.join(LEARN_PACK_RETIRING).exists());

    let bytes = fs::read(learn.live_pack()).expect("empty db");
    assert!(!learn.retract_clause(PERSON, HOB).expect("already gone"));
    assert_eq!(fs::read(learn.live_pack()).expect("db stayed"), bytes);
    assert!(learn.audit().expect("still empty").rows.is_empty());
    assert!(learn.live_pack().is_file());
}

#[test]
fn denied_retract_creates_no_file() {
    let root = Scratch::new("denied");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::fresh(&pack));
    assert!(!learn.is_granted());
    let err = learn.retract_clause(PERSON, HOB).expect_err("denied");
    assert!(err.is_denied(), "{err}");
    assert!(!pack.exists(), "denied retract created {}", pack.display());
}

#[test]
fn retract_then_write_keeps_the_cap() {
    let root = Scratch::new("cap");
    let pack = root.path().join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let mut texts = Vec::with_capacity(LEARN_PACK_CAP);
    for n in 0..LEARN_PACK_CAP {
        let text = format!("clause {n} the kettle is on the stove");
        learn
            .write_clause(PERSON, &text, &source(n as u64))
            .expect("fill");
        texts.push(text);
    }
    let dropped = texts[1].clone();
    assert!(learn.retract_clause(PERSON, &dropped).expect("free a slot"));
    let opened = learn.audit().expect("after retract");
    assert_eq!(opened.rows.len(), LEARN_PACK_CAP - 1);
    assert_eq!(opened.archived, 0);
    assert!(opened.rows.iter().all(|row| row.text != dropped));

    let extra = "clause extra the kettle is beside the stove";
    learn
        .write_clause(PERSON, extra, &source(9_001))
        .expect("write into the freed slot");
    let full = learn.audit().expect("full again");
    assert_eq!(full.rows.len(), LEARN_PACK_CAP);
    assert_eq!(full.archived, 0);
    assert!(full
        .rows
        .iter()
        .any(|row| row.text == extra && row.said_by == PERSON));
    assert!(full.rows.iter().all(|row| row.text != dropped));
    for text in texts.iter().filter(|text| *text != &dropped) {
        assert!(
            full.rows
                .iter()
                .any(|row| row.text == *text && row.said_by == PERSON),
            "missing {text}"
        );
    }

    let overflow = "clause overflow the kettle is beside the stove";
    let err = learn
        .write_clause(PERSON, overflow, &source(9_002))
        .expect_err("cap");
    assert!(err.is_refused(), "{err}");
    let still = learn.audit().expect("still at the cap");
    assert_eq!(still.rows.len(), LEARN_PACK_CAP);
    assert!(still.rows.iter().all(|row| row.text != overflow));
    assert!(still.rows.iter().all(|row| row.text != dropped));
}
