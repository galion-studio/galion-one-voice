// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! `horbm-memory-audit` over a temp directory and the northwind fixture.
//! The binary is built for this integration test. These tests do not read a
//! default data root and do not open a network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{
    ClauseSource, EpisodicStore, LearnConfig, LearnPass, PersonMemory, PersonState, TrackedName,
    WriteOutcome,
};

fn audit_bin() -> &'static str {
    env!("CARGO_BIN_EXE_horbm-memory-audit")
}

fn scratch(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir =
        std::env::temp_dir().join(format!("horbm-audit-{name}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn run(args: &[impl AsRef<std::ffi::OsStr>]) -> Output {
    Command::new(audit_bin())
        .args(args)
        .output()
        .expect("spawn horbm-memory-audit")
}

fn json_lines(output: &Output) -> Vec<serde_json::Value> {
    let text = String::from_utf8(output.stdout.clone()).expect("stdout utf-8");
    if text.is_empty() {
        return Vec::new();
    }
    assert!(
        text.ends_with('\n'),
        "stdout is not line-terminated: {text:?}"
    );
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|err| panic!("{err} in {line}")))
        .collect()
}

fn names_of(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(dir) else {
        return out;
    };
    for entry in rd.flatten() {
        out.push(entry.file_name().to_string_lossy().into_owned());
    }
    out.sort();
    out
}

fn state(name: &str) -> PersonState {
    PersonState {
        names: vec![TrackedName {
            name: name.to_string(),
            class_label: "they".to_string(),
        }],
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
        feeling: None,
    }
}

fn source() -> ClauseSource {
    ClauseSource {
        session: "sess-audit".to_string(),
        turn_id: 3,
        time: "2026-11-25T00:00:09Z".to_string(),
        consent_record: "learn-v1".to_string(),
    }
}

fn grant_pin(db: &Path, person: &str, text: &str, at: &str) -> String {
    let mut store = EpisodicStore::open(db, true);
    let outcome = store.pin(person, text, at);
    let WriteOutcome::Stored { id } = outcome else {
        panic!("pin: {outcome:?}");
    };
    drop(store);
    id
}

const PIN_TEXT: &str = "the kettle stays on the hob";
const PIN_AT: &str = "2026-11-25T00:00:01Z";
const CLAUSE: &str = "The harbour light is green.";
const PERSON: &str = "northwind";

#[test]
fn list_shows_pin_state_row_and_learned_row() {
    let root = scratch("list");
    let db = root.join("person.sqlite");
    let pack = root.join("pack");
    {
        let mut store = EpisodicStore::open(&db, true);
        let pinned = store.pin(PERSON, PIN_TEXT, PIN_AT);
        assert!(matches!(pinned, WriteOutcome::Stored { .. }), "{pinned:?}");
        let noted = store.note_state(PERSON, &state("North"), "2026-11-25T00:00:02Z");
        assert!(matches!(noted, WriteOutcome::Stored { .. }), "{noted:?}");
        // Drop so the worker drains and the file is closed before the audit opens it.
    }
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    let written = learn
        .write_clause(PERSON, CLAUSE, &source())
        .expect("learn clause");

    let out = run(&[
        "list",
        "--memory",
        db.to_str().expect("utf-8 path"),
        "--learn",
        pack.to_str().expect("utf-8 path"),
    ]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(out.stderr.is_empty(), "stderr: {}", stderr(&out));
    let lines = json_lines(&out);
    let pin = lines
        .iter()
        .find(|line| line["kind"] == "row" && line["source"] == "pin")
        .expect("pin row");
    assert_eq!(pin["layer"], "episode");
    assert_eq!(pin["said_by"], PERSON);
    assert_eq!(pin["date"], PIN_AT);
    assert_eq!(pin["text"], PIN_TEXT);
    assert_eq!(pin["pinned"].as_bool(), Some(true));
    assert_eq!(pin["marked"].as_bool(), Some(false));
    assert_eq!(pin["current"].as_bool(), Some(true));
    assert!(pin.get("reason").is_none());
    let name = lines
        .iter()
        .find(|line| {
            line["kind"] == "row"
                && line["source"] == "state-tracker"
                && line["text"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("my name is "))
        })
        .expect("state row");
    assert_eq!(name["layer"], "diary");
    assert_eq!(name["said_by"], PERSON);
    assert_eq!(name["pinned"].as_bool(), Some(false));
    assert_eq!(name["marked"].as_bool(), Some(false));
    assert_eq!(name["current"].as_bool(), Some(true));
    let learned = lines
        .iter()
        .find(|line| line["kind"] == "learned")
        .expect("learned row");
    assert_eq!(learned["item_id"], written.item_id);
    assert_eq!(learned["said_by"], PERSON);
    assert_eq!(learned["date"], source().time);
    assert_eq!(learned["source"], written.source);
    assert_eq!(learned["text"], CLAUSE);
    assert!(learned.get("layer").is_none());
    assert!(learned.get("pinned").is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn forget_archives_and_second_list_shows_tombstone() {
    let root = scratch("forget");
    let db = root.join("person.sqlite");
    let id = grant_pin(&db, PERSON, PIN_TEXT, PIN_AT);
    let db_arg = db.to_str().expect("utf-8 path");

    let forgotten = run(&["forget", "--memory", db_arg, "--person", PERSON, &id]);
    assert_eq!(
        forgotten.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&forgotten)
    );
    assert_eq!(
        String::from_utf8_lossy(&forgotten.stdout).trim_end(),
        format!("Archived {{ id: \"{id}\" }}")
    );
    assert!(forgotten.stderr.is_empty(), "{}", stderr(&forgotten));

    let listed = run(&["list", "--memory", db_arg]);
    assert_eq!(listed.status.code(), Some(0), "stderr: {}", stderr(&listed));
    let lines = json_lines(&listed);
    assert!(
        lines.iter().all(|line| line["kind"] != "row"),
        "live row remained: {lines:?}"
    );
    let stone = lines
        .iter()
        .find(|line| line["kind"] == "tombstone" && line["item_id"] == id)
        .expect("tombstone");
    assert_eq!(stone["layer"], "episode");
    assert_eq!(stone["said_by"], PERSON);
    assert_eq!(stone["date"], PIN_AT);
    assert_eq!(stone["source"], "pin");
    assert_eq!(stone["text"], PIN_TEXT);
    assert_eq!(stone["reason"], "forget");
    assert!(stone.get("pinned").is_none());
    assert!(stone.get("marked").is_none());
    assert!(stone.get("current").is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn withdraw_learn_removes_packs() {
    let root = scratch("withdraw");
    let db = root.join("person.sqlite");
    let pack = root.join("pack");
    grant_pin(&db, PERSON, PIN_TEXT, PIN_AT);
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    learn
        .write_clause(PERSON, CLAUSE, &source())
        .expect("learn clause");
    assert!(learn.live_pack().is_file());
    drop(learn);

    let pack_arg = pack.to_str().expect("utf-8 path");
    let withdrawn = run(&["withdraw-learn", "--learn", pack_arg]);
    assert_eq!(
        withdrawn.status.code(),
        Some(0),
        "stderr: {}",
        stderr(&withdrawn)
    );
    assert!(withdrawn.stdout.is_empty(), "{:?}", withdrawn.stdout);
    assert!(withdrawn.stderr.is_empty(), "{}", stderr(&withdrawn));
    assert!(!pack.join("live").exists());
    assert!(!pack.join("next").exists());
    assert!(!pack.join("retiring").exists());
    assert!(!pack.join("live").join("learn-pack.sqlite").exists());

    let listed = run(&[
        "list",
        "--memory",
        db.to_str().expect("utf-8 path"),
        "--learn",
        pack_arg,
    ]);
    assert_eq!(listed.status.code(), Some(0), "stderr: {}", stderr(&listed));
    let lines = json_lines(&listed);
    assert!(
        lines.iter().all(|line| line["kind"] != "learned"),
        "learned row remained: {lines:?}"
    );
    assert!(lines.iter().any(|line| line["kind"] == "row"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_db_is_exit_2_and_creates_nothing() {
    let root = scratch("missing");
    let before = names_of(&root);
    let missing = root.join("absent").join("memory.sqlite");
    assert!(!missing.exists());
    assert!(!missing.parent().expect("parent").exists());

    let listed = run(&["list", "--memory", missing.to_str().expect("utf-8 path")]);
    assert_eq!(listed.status.code(), Some(2), "stderr: {}", stderr(&listed));
    assert!(listed.stdout.is_empty());
    assert!(!stderr(&listed).is_empty());
    assert!(!missing.exists());
    assert!(!missing.parent().expect("parent").exists());

    let forgotten = run(&[
        "forget",
        "--memory",
        missing.to_str().expect("utf-8 path"),
        "--person",
        PERSON,
        "ep-1",
    ]);
    assert_eq!(
        forgotten.status.code(),
        Some(2),
        "stderr: {}",
        stderr(&forgotten)
    );
    assert!(forgotten.stdout.is_empty());
    assert!(!missing.exists());
    assert!(!missing.parent().expect("parent").exists());
    assert_eq!(names_of(&root), before);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn person_filter_hides_other_people() {
    let root = scratch("person");
    let db = root.join("person.sqlite");
    let pack = root.join("pack");
    let guest = "guest";
    let guest_text = "guest keeps a blue notebook";
    grant_pin(&db, PERSON, PIN_TEXT, PIN_AT);
    grant_pin(&db, guest, guest_text, "2026-11-25T00:00:04Z");
    let learn = LearnPass::open(LearnConfig::granted(&pack));
    learn
        .write_clause(PERSON, CLAUSE, &source())
        .expect("northwind clause");
    learn
        .write_clause(guest, "Guest keeps the window shut.", &source())
        .expect("guest clause");
    drop(learn);

    let db_arg = db.to_str().expect("utf-8 path");
    let pack_arg = pack.to_str().expect("utf-8 path");
    let out = run(&[
        "list", "--memory", db_arg, "--learn", pack_arg, "--person", PERSON,
    ]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let lines = json_lines(&out);
    assert!(!lines.is_empty());
    assert!(lines.iter().all(|line| line["said_by"] == PERSON));
    let raw = String::from_utf8_lossy(&out.stdout);
    assert!(raw.contains(PIN_TEXT));
    assert!(raw.contains(CLAUSE));
    assert!(!raw.contains(guest_text));
    assert!(!raw.contains("Guest keeps the window shut."));

    let all = run(&["list", "--memory", db_arg, "--learn", pack_arg]);
    assert_eq!(all.status.code(), Some(0), "stderr: {}", stderr(&all));
    let all_raw = String::from_utf8_lossy(&all.stdout);
    assert!(all_raw.contains(guest_text));
    assert!(all_raw.contains("Guest keeps the window shut."));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn list_never_writes() {
    let root = scratch("readonly");
    let db = root.join("person.sqlite");
    grant_pin(&db, PERSON, PIN_TEXT, PIN_AT);
    let before = fs::read(&db).expect("read db");
    let meta = fs::metadata(&db).expect("stat db");
    let len = meta.len();
    let modified = meta.modified().expect("mtime");

    let out = run(&["list", "--memory", db.to_str().expect("utf-8 path")]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let lines = json_lines(&out);
    assert!(lines.iter().any(|line| line["text"] == PIN_TEXT));

    let after = fs::read(&db).expect("read db again");
    let again = fs::metadata(&db).expect("stat db again");
    assert_eq!(after, before);
    assert_eq!(again.len(), len);
    assert_eq!(again.modified().expect("mtime again"), modified);
    let _ = fs::remove_dir_all(&root);
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}
