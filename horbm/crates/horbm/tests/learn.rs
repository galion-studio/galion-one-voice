// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Phase 7 gate: fresh config writes nothing. Granted config writes one
//! sourced row. Withdrawal leaves no row.
//!
//! Temp directories only. This file does not read `GALION_HORBM_KNOWLEDGE`
//! and does not open a card folder outside the scratch directory it creates.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    ClauseSource, LearnConfig, LearnPass, LEARN_PACK_DB, LEARN_PACK_LIVE, LEARN_PACK_NEXT,
    LEARN_PACK_RETIRING,
};

fn disk_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|err| err.into_inner())
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("horbm-f7-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn source(turn_id: u64) -> ClauseSource {
    ClauseSource {
        session: "sess-7".to_string(),
        turn_id,
        time: "2026-11-18T21:00:00Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn names(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for ent in rd.flatten() {
        out.push(ent.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    out
}

fn suspicious(dir: &Path) -> Vec<String> {
    names(dir)
        .into_iter()
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("horbm-mem")
                || lower.ends_with(".sqlite")
                || lower.ends_with(".sqlite-wal")
                || lower.ends_with(".sqlite-shm")
                || lower.ends_with(".db")
        })
        .collect()
}

fn sqlite_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let path = ent.path();
            if path.is_dir() {
                walk(&path, out);
                continue;
            }
            let name = ent.file_name().to_string_lossy().into_owned();
            let lower = name.to_ascii_lowercase();
            if lower.contains("sqlite") || lower.ends_with(".db") || lower.contains("horbm-mem") {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    walk(dir, &mut out);
    out.sort();
    out
}

#[test]
fn phase7_gate_fresh_writes_nothing_granted_writes_one_sourced_row_withdrawal_leaves_no_row() {
    let _guard = disk_lock();
    let root = scratch("gate");
    let pack = root.join("pack");
    let cwd = std::env::current_dir().expect("cwd");
    let before_cwd = suspicious(&cwd);
    let before_temp = suspicious(&std::env::temp_dir());
    let clause = "The kettle is on the hob.";
    let source = source(7);

    let fresh = LearnPass::open(LearnConfig::fresh(&pack));
    assert!(!fresh.is_granted());
    let denied = fresh
        .write_clause("user", clause, &source)
        .expect_err("fresh config");
    assert!(denied.is_denied(), "{denied}");
    assert!(!pack.exists(), "fresh config created {}", pack.display());
    fresh.withdraw().expect("withdraw fresh");
    assert!(!pack.exists());
    assert!(
        sqlite_under(&root).is_empty(),
        "fresh sqlite: {:?}",
        sqlite_under(&root)
    );

    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let row = learn
        .write_clause("user", clause, &source)
        .expect("granted write");
    let label = source.label().expect("label");
    assert_eq!(row.source, label);
    assert_eq!(row.text, clause);
    assert_eq!(row.said_by, "user");
    assert_eq!(row.date, source.time);
    assert!(!row.item_id.is_empty());
    assert!(row.source.contains("session=sess-7"));
    assert!(row.source.contains("turn=7"));
    assert!(row.source.contains("consent=learn-v1"));

    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1, "rows: {:?}", audit.rows);
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows[0], row);
    assert!(learn.live_pack().is_file());
    assert!(!pack.join(LEARN_PACK_NEXT).exists());
    assert!(!pack.join(LEARN_PACK_RETIRING).exists());
    assert!(!pack.join("revisions.sqlite").exists());
    assert!(!root.join("revisions.sqlite").exists());
    assert_eq!(
        names(&pack),
        vec![LEARN_PACK_LIVE.to_string()],
        "pack dirs: {:?}",
        names(&pack)
    );

    learn.withdraw().expect("withdraw");
    let after = learn.audit().expect("audit after withdraw");
    assert!(after.rows.is_empty());
    assert_eq!(after.archived, 0);
    assert!(!learn.live_pack().exists());
    assert!(!pack.join(LEARN_PACK_LIVE).exists());
    assert!(
        sqlite_under(&root).is_empty(),
        "withdrawal left {:?}",
        sqlite_under(&root)
    );
    learn.withdraw().expect("withdraw again");
    assert!(sqlite_under(&root).is_empty());

    assert_eq!(suspicious(&cwd), before_cwd, "cwd gained a diary file");
    assert_eq!(
        suspicious(&std::env::temp_dir()),
        before_temp,
        "temp gained a diary file"
    );
    drop(learn);
    assert!(sqlite_under(&root).is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn recall_is_not_a_learn_parameter_and_the_card_dir_stays_untouched() {
    let _guard = disk_lock();
    let root = scratch("separate");
    let cards = root.join("cards");
    let pack = root.join("pack");
    fs::create_dir_all(&cards).expect("cards");
    let before_cards = names(&cards);
    // Fresh learn stays deny. This pass has no recall parameter.
    let fresh = LearnPass::open_outside(LearnConfig::fresh(&pack), &[cards.clone()]);
    assert!(!fresh.is_granted());
    let denied = fresh
        .write_clause("user", "The kettle is on the hob.", &source(1))
        .expect_err("recall is not learn");
    assert!(denied.is_denied(), "{denied}");
    assert!(!pack.exists());
    assert_eq!(names(&cards), before_cards);

    let learn = LearnPass::open_outside(LearnConfig::granted(&pack), &[cards.clone()]);
    assert!(learn.is_granted());
    learn
        .write_clause("user", "The kettle is on the hob.", &source(2))
        .expect("learn without a recall flag");
    assert_eq!(names(&cards), before_cards, "card dir changed");
    assert!(!cards.join("revisions.sqlite").exists());
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.archived, 0);
    assert!(audit.rows[0].source.contains("turn=2"));
    learn.withdraw().expect("withdraw");
    assert!(learn.audit().expect("empty").rows.is_empty());
    assert!(sqlite_under(&root).is_empty(), "{:?}", sqlite_under(&root));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn empty_source_writes_nothing() {
    let _guard = disk_lock();
    let root = scratch("source");
    let pack = root.join("pack");
    let mut blank = source(3);
    blank.session = " ".to_string();
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let err = learn
        .write_clause("user", "The kettle is on the hob.", &blank)
        .expect_err("empty source");
    assert!(err.is_unsourced(), "{err}");
    assert!(!pack.exists());
    assert!(sqlite_under(&root).is_empty());

    let err = learn
        .write_clause("  ", "The kettle is on the hob.", &source(3))
        .expect_err("empty speaker");
    assert!(err.is_invalid(), "{err}");
    assert!(!pack.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn pack_swap_replaces_the_live_row() {
    let _guard = disk_lock();
    let root = scratch("swap");
    let pack = root.join("pack");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let first = learn
        .write_clause("user", "alpha-token the kettle is on the hob.", &source(4))
        .expect("first");
    assert!(first.source.contains("turn=4"));
    let second = learn
        .write_clause("user", "beta-token the kettle is off the hob.", &source(5))
        .expect("second");
    // The swap publishes a new sqlite file, so the diary id may start again.
    assert_ne!(first.text, second.text);
    assert!(second.source.contains("turn=5"));
    let audit = learn.audit().expect("audit");
    assert_eq!(audit.rows.len(), 1);
    assert_eq!(audit.archived, 0);
    assert_eq!(audit.rows[0].text, "beta-token the kettle is off the hob.");
    assert!(!audit.rows[0].text.contains("alpha-token"));
    assert!(!pack.join(LEARN_PACK_NEXT).exists());
    assert!(!pack.join(LEARN_PACK_RETIRING).exists());
    assert!(pack.join(LEARN_PACK_LIVE).join(LEARN_PACK_DB).is_file());
    assert!(!pack.join("revisions.sqlite").exists());
    learn.withdraw().expect("withdraw");
    assert!(sqlite_under(&root).is_empty(), "{:?}", sqlite_under(&root));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn card_folder_is_not_written() {
    let _guard = disk_lock();
    let root = scratch("cards");
    let cards = root.join("cards");
    fs::create_dir_all(&cards).expect("cards");
    fs::write(cards.join("manifest.json"), "{}\n").expect("manifest");
    let before = names(&cards);
    let learn = LearnPass::open_outside(LearnConfig::granted(&cards), &[cards.clone()]);
    let err = learn
        .write_clause("user", "The kettle is on the hob.", &source(6))
        .expect_err("card folder");
    assert!(err.is_refused(), "{err}");
    assert_eq!(names(&cards), before);
    assert!(!cards.join("revisions.sqlite").exists());
    assert!(sqlite_under(&root).is_empty(), "{:?}", sqlite_under(&root));
    learn
        .withdraw()
        .expect_err("withdraw must refuse the card folder");
    assert_eq!(
        fs::read_to_string(cards.join("manifest.json")).expect("manifest"),
        "{}\n"
    );
    let _ = fs::remove_dir_all(&root);
}
