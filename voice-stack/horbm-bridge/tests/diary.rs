// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Person diary on [`galion_sam_horbm::CardEngine`].
//!
//! The flag is passed in. These tests do not set `GALION_HORBM_REMEMBER`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use galion_sam_horbm::{CardEngine, Pad, ReplyEngine};
use horbm::WriteOutcome;

const KETTLE: &str = "The kettle is on the hob.";
const PIN: &str = "remember that my favourite colour is blue";
const PINNED: &str = "my favourite colour is blue";
const ASK_PIN: &str = "what is my favourite colour";
const ASK_KETTLE: &str = "where is the kettle";

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
            "galion-card-diary-{label}-{n}-{}-{nanos}",
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

fn open_engine(scratch: &Scratch, remember: Option<&str>) -> CardEngine {
    write_kettle(&scratch.knowledge());
    CardEngine::open_with_remember(
        &scratch.memory(),
        &scratch.knowledge(),
        &scratch.cases(),
        remember,
    )
}

fn kettle_matches(
    text: &str,
    sources: &[String],
    mood: Pad,
    hit_text: &str,
    hit_sources: &[String],
    hit_mood: Pad,
) {
    assert_eq!(hit_text, text);
    assert_eq!(hit_sources, sources);
    assert_eq!(hit_mood, mood);
}

#[test]
fn unset_grant_creates_no_diary_and_the_kettle_reply_is_unchanged() {
    let flags = [None, Some(""), Some("yes"), Some("true")];
    let mut baseline: Option<(String, Vec<String>, Pad)> = None;
    for (index, flag) in flags.into_iter().enumerate() {
        let scratch = Scratch::new(&format!("deny-{index}"));
        let mut engine = open_engine(&scratch, flag);
        assert!(
            !scratch.memory().exists(),
            "{flag:?} created a diary before a turn"
        );
        let hit = engine.turn(ASK_KETTLE);
        assert_eq!(hit.text, KETTLE, "{flag:?}");
        assert!(
            hit.sources
                .iter()
                .all(|source| !source.starts_with("memory:")),
            "{flag:?} {}",
            hit.sources.join(" ")
        );
        match &baseline {
            None => baseline = Some((hit.text.clone(), hit.sources.clone(), hit.mood)),
            Some((text, sources, mood)) => {
                kettle_matches(text, sources, *mood, &hit.text, &hit.sources, hit.mood);
            }
        }
        let _asked = engine.turn(PIN);
        let recalled = engine.turn(ASK_PIN);
        assert!(
            !scratch.memory().exists(),
            "{flag:?} created {}",
            scratch.memory().display()
        );
        assert!(
            !recalled.text.to_ascii_lowercase().contains("blue"),
            "{flag:?} spoke a diary line: {}",
            recalled.text
        );
        assert!(
            recalled
                .sources
                .iter()
                .all(|source| !source.starts_with("memory:")),
            "{flag:?} {}",
            recalled.sources.join(" ")
        );
    }
}

#[test]
fn the_grant_word_attaches_the_person_diary() {
    let denied = Scratch::new("grant-baseline");
    let mut denied_engine = open_engine(&denied, None);
    let denied_hit = denied_engine.turn(ASK_KETTLE);
    drop(denied_engine);

    let scratch = Scratch::new("grant");
    let mut engine = open_engine(&scratch, Some(" grant "));
    assert!(
        !scratch.memory().exists(),
        "the grant opens the store; the file waits for a write"
    );
    let hit = engine.turn(ASK_KETTLE);
    assert_eq!(hit.text, KETTLE);
    assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
    kettle_matches(
        &denied_hit.text,
        &denied_hit.sources,
        denied_hit.mood,
        &hit.text,
        &hit.sources,
        hit.mood,
    );
    assert!(
        hit.sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "{}",
        hit.sources.join(" ")
    );

    let _pinned = engine.turn(PIN);
    assert!(
        scratch.memory().is_file(),
        "a granted turn records the person diary"
    );
    let recalled = engine.turn(ASK_PIN);
    assert_eq!(recalled.text, PINNED);
    assert!(
        recalled
            .sources
            .iter()
            .any(|source| source.starts_with("memory:")),
        "{}",
        recalled.sources.join(" ")
    );

    let again = engine.turn(ASK_KETTLE);
    assert_eq!(again.text, KETTLE);
    assert_eq!(again.sources, vec!["card:kettle".to_string()]);
    assert!(!again.text.to_ascii_lowercase().contains("blue"));
    assert!(!again.text.to_ascii_lowercase().contains("feeling"));
    assert!(
        again
            .sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "a world question must not cite the diary: {}",
        again.sources.join(" ")
    );
}

/// `brain` and `brain_mut` are the host path to the inner turn and to
/// `end_session`. Learn stays deny. Remember stays deny unless the flag is
/// the grant word.
#[test]
fn the_host_reaches_inner_turn_and_ends_the_diary_session() {
    let denied = Scratch::new("host-deny");
    let mut denied_engine = open_engine(&denied, None);
    assert_eq!(denied_engine.brain().person_id(), "person");
    let denied_inner = denied_engine.brain_mut().inner_turn(80, ASK_KETTLE, None);
    assert!(!denied_inner.learned, "learn stays deny on the inner turn");
    let denied_nudge = denied_engine
        .brain_mut()
        .inner_nudge(ASK_KETTLE, None)
        .expect("inner nudge");
    assert!(!denied_nudge.learned, "learn stays deny on the inner nudge");
    {
        let memory = denied_engine.brain_mut().person_memory().expect("diary");
        assert!(!memory.remember_granted());
        assert_eq!(memory.end_session("person", "t000099"), WriteOutcome::Refused);
    }
    assert!(!denied.memory().exists());

    let scratch = Scratch::new("host-grant");
    let mut engine = open_engine(&scratch, Some(" grant "));
    let person = engine.brain().person_id().to_string();
    assert_eq!(person, "person");
    let inner = engine.brain_mut().inner_turn(80, ASK_KETTLE, None);
    assert!(!inner.learned, "the remember grant is not the learn grant");
    let nudged = engine
        .brain_mut()
        .inner_nudge(ASK_KETTLE, None)
        .expect("inner nudge");
    assert!(!nudged.learned);
    let hit = engine.turn(ASK_KETTLE);
    assert_eq!(hit.text, KETTLE);
    assert_eq!(hit.sources, vec!["card:kettle".to_string()]);
    let _pinned = engine.turn(PIN);
    assert!(scratch.memory().is_file());
    let ended = {
        let memory = engine.brain_mut().person_memory().expect("granted diary");
        assert!(memory.remember_granted());
        memory.end_session(&person, "t000099")
    };
    assert!(
        matches!(ended, WriteOutcome::Stored { .. }),
        "end_session records the session: {ended:?}"
    );
    let again = engine.turn(ASK_KETTLE);
    assert_eq!(again.text, KETTLE);
    assert!(
        again
            .sources
            .iter()
            .all(|source| !source.starts_with("memory:")),
        "{}",
        again.sources.join(" ")
    );
}
