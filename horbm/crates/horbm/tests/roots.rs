// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Two card roots. Research style cards used here are built in memory.
//! The laptop research directory is not read.

use std::path::{Path, PathBuf};

use horbm::{
    blocks_answer, Brain, CardLookup, KnowledgeCards, MemoryExample, ResearchRoot, RetrievalConfig,
    RetrievalThread, RetrievedCells, UNKNOWN_REPLY,
};

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "horbm-roots-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).unwrap();
}

fn fact_card(id: &str, summary: &str, body: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{summary}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{summary}",
  "body": "{body}",
  "claim_type": "fact",
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "horbm",
  "as_of_date": "2026-11-24",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
    )
}

fn dialogue_card(id: &str, title: &str, body: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{title}",
  "section": "horbm",
  "category": "dialogue",
  "summary": "{title}",
  "body": "{body}",
  "claim_type": "interpretation",
  "research_only": true,
  "use_for": ["style", "empathy", "moves"],
  "not_for": ["answer", "cite"],
  "licence": "synthetic",
  "fixture": "synthetic",
  "sources": [],
  "evidence_strength": "none",
  "said_by": "horbm",
  "as_of_date": "2026-11-24",
  "status": "draft",
  "contested_views": [],
  "related": []
}}
"#
    )
}

fn barred_fact() -> &'static str {
    r#"{
  "id": "barred-kettle",
  "title": "Kettle aside",
  "section": "horbm",
  "category": "fixture",
  "summary": "The kettle is aside.",
  "body": "The kettle is aside.",
  "claim_type": "fact",
  "research_only": false,
  "not_for": ["answer"],
  "sources": [],
  "evidence_strength": "strong",
  "said_by": "horbm",
  "as_of_date": "2026-11-24",
  "status": "draft",
  "contested_views": [],
  "related": []
}
"#
}

#[test]
fn blocks_answer_is_research_only_or_not_for_answer() {
    assert!(!blocks_answer(false, &[]));
    assert!(blocks_answer(true, &[]));
    assert!(blocks_answer(false, &["answer".into()]));
    assert!(blocks_answer(false, &[" Answer ".into()]));
    assert!(!blocks_answer(false, &["cite".into()]));
    assert!(ResearchRoot::from_flag(None).is_empty());
    assert!(ResearchRoot::from_flag(Some("")).is_empty());
    assert!(ResearchRoot::from_flag(Some("/no/such/horbm-research-root")).is_empty());
}

#[test]
fn a_research_card_never_answers_and_a_mixed_dir_keeps_the_fact() {
    let dir = temp_dir("mixed");
    write(
        &dir,
        "kettle.json",
        &fact_card(
            "kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        ),
    );
    write(
        &dir,
        "soft-pace.json",
        &dialogue_card(
            "soft-pace",
            "Short line",
            "When they say i am tired, keep the line short.",
        ),
    );
    write(&dir, "barred-kettle.json", barred_fact());
    let loaded = KnowledgeCards::load(&dir).unwrap();
    let ids: Vec<&str> = loaded.cards.ids().collect();
    assert_eq!(ids, vec!["kettle"]);
    let hit = CardLookup::lookup(&loaded.cards, "where is the kettle").unwrap();
    assert_eq!(hit.id, "kettle");
    assert_eq!(hit.gist, "The kettle is on the hob.");
    assert!(CardLookup::lookup(&loaded.cards, "i am tired").is_none());

    let only = temp_dir("research-as-truth");
    write(
        &only,
        "soft-pace.json",
        &dialogue_card(
            "soft-pace",
            "Short line",
            "When they say i am tired, keep the line short.",
        ),
    );
    let loaded = KnowledgeCards::load(&only).unwrap();
    assert!(loaded.cards.is_empty());
    let mut brain = Brain::with_cards(loaded.cards, None);
    let out = brain.turn("i am tired");
    assert_ne!(out.arm, "answer");
    assert_ne!(out.arm, "cite");
    assert_eq!(out.reply, UNKNOWN_REPLY);
    assert_eq!(out.sources, vec!["honest:unknown".to_string()]);
    assert!(out.evidence.is_empty());
    assert!(!out.reply.to_ascii_lowercase().contains("short"));
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(only);
}

#[test]
fn an_unset_research_root_matches_the_truth_only_turn() {
    let dir = temp_dir("truth");
    write(
        &dir,
        "kettle.json",
        &fact_card(
            "kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        ),
    );
    let cards = KnowledgeCards::load(&dir).unwrap().cards;
    let mut plain = Brain::with_cards(cards.clone(), None);
    let mut empty = Brain::with_cards(cards, None).with_style(ResearchRoot::empty());
    for line in [
        "where is the kettle",
        "i am so tired",
        "something else",
        "cite that",
    ] {
        let left = plain.turn(line);
        let right = empty.turn(line);
        assert_eq!(left.reply, right.reply, "{line}");
        assert_eq!(left.arm, right.arm, "{line}");
        assert_eq!(left.sources, right.sources, "{line}");
        assert_eq!(left.evidence, right.evidence, "{line}");
        assert_eq!(left.first_clause, right.first_clause, "{line}");
        assert_eq!(left.rest, right.rest, "{line}");
    }
    assert_eq!(plain.turn("goodbye").reply, "Talk soon.");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn comfort_can_take_a_style_mark_and_still_says_the_miss() {
    let truth = temp_dir("truth-style");
    write(
        &truth,
        "kettle.json",
        &fact_card(
            "kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        ),
    );
    let style = ResearchRoot::from_examples(&[
        MemoryExample {
            id: "soft-pace",
            text: "When they say i am tired, keep the line short.",
            licence: "in-memory",
        },
        MemoryExample {
            id: "ask-plain",
            text: "When they ask who won the match, stay plain.",
            licence: "in-memory",
        },
        MemoryExample {
            id: "offer-back",
            text: "If they want something else, name the topic they already used.",
            licence: "in-memory",
        },
    ]);
    assert_eq!(style.len(), 3);
    assert_eq!(style.files_read(), 0);
    let mut brain =
        Brain::with_cards(KnowledgeCards::load(&truth).unwrap().cards, None).with_style(style);
    let tired = brain.turn("i am so tired");
    assert_eq!(tired.arm, "comfort");
    assert_eq!(tired.reply, "I don't know.");
    assert_eq!(tired.sources, vec!["honest:unknown".to_string()]);
    assert_eq!(tired.evidence, vec!["style:soft-pace".to_string()]);
    assert!(!tired.reply.to_ascii_lowercase().contains("short"));
    assert!(!tired
        .sources
        .iter()
        .any(|source| source.starts_with("style:")));

    let asked = brain.turn("who won the match");
    assert_eq!(asked.arm, "ask");
    assert_eq!(asked.reply, UNKNOWN_REPLY);
    assert_eq!(asked.evidence, vec!["style:ask-plain".to_string()]);
    assert!(!asked.reply.to_ascii_lowercase().contains("plain"));

    let _ = brain.turn("where is the kettle");
    let offer = brain.turn("something else");
    assert_eq!(offer.arm, "offer_topic");
    assert_eq!(offer.reply, "Back to kettle?");
    assert!(offer.evidence.iter().any(|item| item == "topic:kettle"));
    assert!(offer.evidence.iter().any(|item| item == "style:offer-back"));
    assert!(!offer.reply.to_ascii_lowercase().contains("topic they"));

    let cited = brain.turn("cite that");
    assert_eq!(cited.arm, "cite");
    assert_eq!(cited.reply, "The kettle is on the hob.");
    assert_eq!(cited.sources, vec!["card:kettle".to_string()]);
    assert!(!cited.evidence.iter().any(|item| item.starts_with("style:")));
    let _ = std::fs::remove_dir_all(truth);
}

#[test]
fn retrieval_never_puts_a_research_card_at_hits_zero() {
    let dir = temp_dir("retrieve-mixed");
    write(
        &dir,
        "kettle.json",
        &fact_card(
            "kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        ),
    );
    write(
        &dir,
        "soft-pace.json",
        &dialogue_card(
            "soft-pace",
            "Kettle",
            "When they say i am tired, the kettle is only a style note.",
        ),
    );
    let thread = RetrievalThread::spawn(RetrievalConfig {
        remember: true,
        knowledge_dir: Some(dir.clone()),
        fixture_rows: 0,
        keys_path: None,
    })
    .unwrap();
    thread.ask_wait("where is the kettle", 1, 1).unwrap();
    let cell = thread.cell();
    assert_eq!(cell.state, horbm::CellState::Hits);
    assert_eq!(cell.hits[0].item_id, "kettle");
    assert!(cell.hits.iter().all(|hit| hit.item_id != "soft-pace"));

    thread.ask_wait("i am tired", 2, 2).unwrap();
    let tired = thread.cell();
    assert_eq!(tired.state, horbm::CellState::Miss);
    assert!(tired.hits.is_empty());
    assert_eq!(tired.answer, UNKNOWN_REPLY);
    drop(thread);
    let _ = std::fs::remove_dir_all(dir);
}
