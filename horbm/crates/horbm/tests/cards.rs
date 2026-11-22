// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::{Path, PathBuf};

use horbm::{Brain, KnowledgeCards, UNKNOWN_REPLY};

fn real_card(id: &str, summary: &str, body: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{summary}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{summary}",
  "body": "{body}",
  "claim_type": "fact",
  "sources": [{{
    "title": "Galion knowledge fixture",
    "url": "https://github.com/galion-studio/galion-knowledge",
    "author": "Galion Labs",
    "date": "2026-11-18",
    "accessed": "2026-11-22"
  }}],
  "evidence_strength": "strong",
  "said_by": "Galion Labs",
  "as_of_date": "2026-11-18",
  "status": "draft",
  "contested_views": [],
  "related": ["fixture-gate"]
}}
"#
    )
}

fn write_pack(dir: &Path) {
    std::fs::create_dir_all(dir.join("horbm")).unwrap();
    std::fs::write(
        dir.join("horbm").join("fixture-kettle.json"),
        real_card(
            "fixture-kettle",
            "The kettle is on the hob.",
            "The kettle is on the hob.",
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("horbm").join("fixture-gate.json"),
        real_card(
            "fixture-gate",
            "The gate latch sticks.",
            "The gate latch sticks on the left.",
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("horbm").join("fixture.honesty.i-dont-know.json"),
        real_card(
            "fixture.honesty.i-dont-know",
            "Unknown is a complete answer.",
            "Say I don't know when the pack has no card.",
        ),
    )
    .unwrap();
}

#[test]
fn a_hit_names_the_card_and_a_miss_is_exactly_i_dont_know() {
    let dir = std::env::temp_dir().join(format!(
        "horbm-turn-cards-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    write_pack(&dir);
    let names_before = names(&dir.join("horbm"));
    let loaded = KnowledgeCards::load(&dir).unwrap();
    assert_eq!(loaded.cards.len(), 3);
    assert!(loaded
        .report
        .log_line()
        .starts_with("horbm cards: loaded 3 cards in "));
    let mut brain = Brain::with_cards(loaded.cards, None);
    let hit = brain.turn("where is the kettle");
    assert_eq!(hit.reply, "The kettle is on the hob.");
    assert_eq!(hit.sources, vec!["card:fixture-kettle".to_string()]);
    assert!(hit
        .evidence
        .iter()
        .any(|item| item == "card:fixture-kettle"));
    let dotted = brain.turn("unknown complete answer");
    assert_eq!(dotted.reply, "Unknown is a complete answer.");
    assert_eq!(
        dotted.sources,
        vec!["card:fixture.honesty.i-dont-know".to_string()]
    );
    let miss = brain.turn("who won the match");
    assert_eq!(miss.reply, "I don't know.");
    assert_eq!(miss.reply, UNKNOWN_REPLY);
    assert_eq!(miss.sources, vec!["honest:unknown".to_string()]);
    assert!(!miss.reply.to_ascii_lowercase().contains("ollama"));
    assert!(!dir.join("revisions.sqlite").exists());
    assert!(!dir.join("memory").exists());
    assert_eq!(names(&dir.join("horbm")), names_before);
    let _ = std::fs::remove_dir_all(&dir);
}

fn names(dir: &Path) -> Vec<PathBuf> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    names.sort();
    names
}
