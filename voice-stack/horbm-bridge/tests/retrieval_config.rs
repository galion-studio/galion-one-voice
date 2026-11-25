// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The retrieval thread takes the typed remember word.
//!
//! These tests do not call `std::env::set_var`. `GALION_HORBM_REMEMBER` is
//! asserted unset. The diary and the retrieval cell follow the same word.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use galion_sam_horbm::{CardEngine, ReplyEngine, UNKNOWN_REPLY};
use horbm::{CellState, RetrievalConfig};

const KETTLE: &str = "The kettle is on the hob.";
const LAMP: &str = "The northwind lamp is lit.";

struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(label: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!(
            "galion-retrieval-config-{label}-{n}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn knowledge(&self) -> PathBuf {
        self.root.join("knowledge")
    }

    fn memory(&self) -> PathBuf {
        self.root.join("memory.db")
    }

    fn cases(&self) -> PathBuf {
        self.root.join("cases")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_kettle(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("kettle.md"),
        "# Kettle\n\nThe kettle is on the hob.\n",
    )
    .unwrap();
}

fn write_json(dir: &Path, name: &str, id: &str, gist: &str) {
    fs::create_dir_all(dir).unwrap();
    let body = format!(
        r#"{{
  "id": "{id}",
  "title": "{gist}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{gist}",
  "body": "{gist}",
  "claim_type": "fact",
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "fixture",
  "as_of_date": "2026-11-25",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
    );
    fs::write(dir.join(name), body).unwrap();
}

#[test]
fn granted_word_grants_retrieval_and_diary_without_env() {
    assert!(
        std::env::var("GALION_HORBM_REMEMBER").is_err(),
        "GALION_HORBM_REMEMBER is set in this process"
    );
    let scratch = Scratch::new("grant");
    fs::create_dir_all(scratch.knowledge()).unwrap();
    let mut engine = CardEngine::open_with_remember(
        &scratch.memory(),
        &scratch.knowledge(),
        &scratch.cases(),
        Some("grant"),
    );
    assert!(
        engine
            .brain_mut()
            .person_memory()
            .is_some_and(|memory| { horbm::PersonMemory::remember_granted(memory) }),
        "the diary did not attach"
    );
    assert!(
        !scratch.memory().exists(),
        "the grant opens the store; the file waits for a write"
    );
    let _pinned = engine.turn("remember that my favourite colour is blue");
    assert!(
        scratch.memory().is_file(),
        "a granted turn did not write the diary"
    );
    let cell = engine.brain().retrieval_cell().expect("retrieval thread");
    assert_ne!(cell.state, CellState::Idle);
}

#[test]
fn denied_word_keeps_both_closed() {
    for flag in [None, Some("yes"), Some("deny")] {
        let scratch = Scratch::new("deny");
        write_kettle(&scratch.knowledge());
        let mut engine = CardEngine::open_with_remember(
            &scratch.memory(),
            &scratch.knowledge(),
            &scratch.cases(),
            flag,
        );
        let hit = engine.turn("where is the kettle");
        assert_eq!(hit.text, KETTLE, "{flag:?}");
        let cell = engine.brain().retrieval_cell().expect("retrieval thread");
        assert_eq!(cell.state, CellState::Idle, "{flag:?}");
        assert!(!scratch.memory().exists(), "{flag:?}");
    }
}

#[test]
fn typed_config_is_used_as_given() {
    let scratch = Scratch::new("typed");
    let cards = scratch.root.join("cards");
    let custom = scratch.root.join("custom");
    write_json(&cards, "kettle.json", "kettle", KETTLE);
    write_json(&custom, "lamp.json", "lamp", LAMP);
    let config = RetrievalConfig {
        remember: true,
        knowledge_dir: Some(custom),
        fixture_rows: 0,
        keys_path: None,
    };
    let mut engine =
        CardEngine::open_with_retrieval(&scratch.memory(), &cards, &scratch.cases(), config);
    let lamp = engine.turn("where is the northwind lamp");
    assert_eq!(lamp.text, LAMP);
    assert!(
        lamp.sources.iter().any(|source| source == "retrieve:lamp"),
        "{:?}",
        lamp.sources
    );
    let kettle = engine.turn("where is the kettle");
    assert_eq!(kettle.text, UNKNOWN_REPLY);
    assert!(
        kettle
            .sources
            .iter()
            .all(|source| !source.contains("kettle")),
        "{:?}",
        kettle.sources
    );
}
