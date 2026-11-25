// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Research style cards. The laptop directory is our own private research
//! and development cards, to be published later through knowledge.galion.app.
//! Nothing in this file is a dataset. A test that needs those cards reads
//! `GALION_HORBM_RESEARCH_DIR` and returns when it is unset.

use std::path::PathBuf;
use std::time::Instant;

use horbm::{
    Brain, KnowledgeCards, MemoryExample, ResearchRoot, StylePersona, STYLE_HIT_CAP, UNKNOWN_REPLY,
};

fn laptop_dir() -> Option<PathBuf> {
    let raw = std::env::var("GALION_HORBM_RESEARCH_DIR").ok()?;
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

#[test]
fn unset_research_reads_nothing_and_returns_no_hits() {
    let off = ResearchRoot::from_flag(None);
    assert!(off.is_empty());
    assert_eq!(off.files_read(), 0);
    let searched = off.search(StylePersona::Samantha, "i am tired");
    assert!(searched.hits.is_empty());
    assert_eq!(searched.bundles_opened, 0);
    assert_eq!(off.files_read(), 0);
    assert!(ResearchRoot::from_flag(Some("")).is_empty());
    assert!(ResearchRoot::from_flag(Some("   ")).files_read() == 0);

    if std::env::var_os("GALION_HORBM_RESEARCH_DIR").is_none() {
        let env = ResearchRoot::from_env();
        assert!(env.is_empty());
        assert_eq!(env.files_read(), 0);
        assert!(env
            .search(StylePersona::Samantha, "i am tired")
            .hits
            .is_empty());
        assert_eq!(env.files_read(), 0);
    }
}

#[test]
fn an_in_memory_card_is_refused_as_an_answer_and_as_a_citation() {
    let root = ResearchRoot::from_examples(&[MemoryExample {
        id: "soft-pace",
        text: "The spare kettle sits in the cupboard.",
        licence: "in-memory",
    }]);
    assert_eq!(root.files_read(), 0);
    let aria = root.search(StylePersona::Aria, "spare kettle cupboard");
    assert!(aria.hits.is_empty());
    assert_eq!(root.files_read(), 0);

    let found = root.search(StylePersona::Samantha, "spare kettle cupboard");
    assert_eq!(found.bundles_opened, 0);
    assert_eq!(root.files_read(), 0);
    assert_eq!(found.hits.len(), 1);
    let hit = &found.hits[0];
    assert_eq!(hit.id, "soft-pace");
    assert_eq!(hit.not_for, ["answer".to_string(), "cite".to_string()]);
    assert!(hit.tagged_not_for_answer_and_cite());
    assert!(!hit.allows_answer());
    assert!(!hit.allows_cite());

    let mut brain = Brain::with_cards(KnowledgeCards::empty(), None).with_style(root);
    let out = brain.turn("where is the spare kettle in the cupboard");
    assert_ne!(out.arm, "answer");
    assert_ne!(out.arm, "cite");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert!(!out.reply.to_ascii_lowercase().contains("cupboard"));
    assert!(out
        .sources
        .iter()
        .all(|source| !source.starts_with("style:")));
    assert!(out
        .sources
        .iter()
        .all(|source| !source.contains("soft-pace")));
}

#[test]
fn laptop_research_root_does_not_answer_or_open_lccc_for_english() {
    let Some(dir) = laptop_dir() else {
        eprintln!("GALION_HORBM_RESEARCH_DIR unset; skipping laptop research cards");
        return;
    };
    let root = ResearchRoot::load(&dir).expect("laptop research root");
    assert_eq!(root.files_read(), 0);
    let aria = root.search(StylePersona::Aria, "hello");
    assert!(aria.hits.is_empty());
    assert_eq!(root.files_read(), 0);

    let found = root.search(StylePersona::Samantha, "hello");
    assert!(found.hits.len() <= STYLE_HIT_CAP);
    assert!(found.hits.iter().all(|hit| {
        !hit.allows_answer()
            && !hit.allows_cite()
            && hit.tagged_not_for_answer_and_cite()
            && hit.not_for == ["answer".to_string(), "cite".to_string()]
    }));
    assert!(root
        .opened_bundles()
        .iter()
        .all(|path| !path.to_ascii_lowercase().contains("lccc")));
}

#[test]
fn laptop_research_root_latency() {
    let Some(dir) = laptop_dir() else {
        eprintln!("GALION_HORBM_RESEARCH_DIR unset; skipping laptop research latency");
        return;
    };
    let root = ResearchRoot::load(&dir).expect("laptop research root");
    let started = Instant::now();
    let first = root.search(StylePersona::Samantha, "hello");
    let first_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let warm = root.search(StylePersona::Samantha, "hello");
    let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "horbm research style: laptop root indexed {} cards, first search {first_ms:.1} ms ({} us), warm search {warm_ms:.1} ms ({} us), hits {}",
        root.len(),
        first.took_us,
        warm.took_us,
        first.hits.len()
    );
    assert!(first.hits.len() <= STYLE_HIT_CAP);
    assert!(first
        .hits
        .iter()
        .all(|hit| !hit.allows_answer() && !hit.allows_cite()));
    assert_eq!(warm.bundles_opened, 0);
    assert!(root
        .opened_bundles()
        .iter()
        .all(|path| !path.to_ascii_lowercase().contains("lccc")));
}
