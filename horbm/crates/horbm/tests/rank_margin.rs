// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Spoken margin on the covering lookup.
//!
//! Every content token of the question has to be in the card. The shortest
//! haystack still ranks first. It is spoken only when it leads the next
//! covering card by `RANK_MARGIN` haystack tokens (2). A tie, a one-token
//! lead, or no cover speaks `I don't know.`

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horbm::{Brain, CardLookup, KnowledgeCards, RANK_MARGIN, UNKNOWN_REPLY};

fn real_card(id: &str, gist: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "title": "{gist}",
  "section": "horbm",
  "category": "fixture",
  "summary": "{gist}",
  "body": "{gist}",
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

fn write_pack(label: &str, cards: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "horbm-rank-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join("horbm")).unwrap();
    for (id, gist) in cards {
        std::fs::write(
            dir.join("horbm").join(format!("{id}.json")),
            real_card(id, gist),
        )
        .unwrap();
    }
    dir
}

fn finish(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn brain(dir: &Path) -> Brain {
    Brain::with_cards(KnowledgeCards::load(dir).unwrap().cards, None)
}

#[test]
fn a_clear_lead_speaks_the_shorter_card() {
    assert_eq!(RANK_MARGIN, 2);
    // `kettle-hob` haystack is {kettle, hob}: 2 tokens.
    // `kettle-hob-spare-drawer` haystack is {kettle, hob, spare, drawer}: 4.
    // Lead 2 is the margin. The shorter card is spoken.
    let dir = write_pack(
        "clear",
        &[
            ("kettle-hob", "The kettle is on the hob."),
            ("kettle-hob-spare-drawer", "The kettle hob spare drawer."),
        ],
    );
    let loaded = KnowledgeCards::load(&dir).unwrap();
    let hit = loaded.cards.lookup("where is the kettle").unwrap();
    assert_eq!(hit.id, "kettle-hob");
    assert_eq!(hit.gist, "The kettle is on the hob.");
    let spoken = brain(&dir).turn("where is the kettle");
    assert_eq!(spoken.reply, "The kettle is on the hob.");
    assert_eq!(spoken.sources, vec!["card:kettle-hob".to_string()]);
    finish(&dir);
}

#[test]
fn a_tie_says_i_dont_know() {
    // Both haystacks have two content tokens: {kettle, red} and {kettle, blue}.
    let dir = write_pack(
        "tie",
        &[
            ("kettle-red", "The kettle is red."),
            ("kettle-blue", "The kettle is blue."),
        ],
    );
    let loaded = KnowledgeCards::load(&dir).unwrap();
    assert!(loaded.cards.lookup("where is the kettle").is_none());
    let spoken = brain(&dir).turn("where is the kettle");
    assert_eq!(spoken.reply, "I don't know.");
    assert_eq!(spoken.reply, UNKNOWN_REPLY);
    assert_eq!(spoken.first_clause, UNKNOWN_REPLY);
    assert_eq!(spoken.sources, vec!["honest:unknown".to_string()]);
    finish(&dir);
}

#[test]
fn a_one_token_lead_says_i_dont_know() {
    assert_eq!(RANK_MARGIN, 2);
    // {kettle, hob} is 2. {kettle, hob, mug} is 3. Lead 1 is below the margin.
    let dir = write_pack(
        "near",
        &[
            ("kettle-hob", "The kettle is on the hob."),
            ("kettle-hob-mug", "The kettle hob mug."),
        ],
    );
    let loaded = KnowledgeCards::load(&dir).unwrap();
    assert!(loaded.cards.lookup("where is the kettle").is_none());
    let spoken = brain(&dir).turn("where is the kettle");
    assert_eq!(spoken.reply, "I don't know.");
    assert_eq!(spoken.reply, UNKNOWN_REPLY);
    assert_eq!(spoken.first_clause, UNKNOWN_REPLY);
    assert_eq!(spoken.sources, vec!["honest:unknown".to_string()]);
    finish(&dir);
}

#[test]
fn no_cover_says_i_dont_know() {
    let dir = write_pack("miss", &[("kettle-hob", "The kettle is on the hob.")]);
    let loaded = KnowledgeCards::load(&dir).unwrap();
    assert!(loaded.cards.lookup("who won the match").is_none());
    assert!(loaded.cards.lookup("where is the kettle violin").is_none());
    let spoken = brain(&dir).turn("who won the match");
    assert_eq!(spoken.reply, "I don't know.");
    assert_eq!(spoken.reply, UNKNOWN_REPLY);
    assert_eq!(spoken.first_clause, UNKNOWN_REPLY);
    assert_eq!(spoken.sources, vec!["honest:unknown".to_string()]);
    let partial = brain(&dir).turn("where is the kettle violin");
    assert_eq!(partial.reply, UNKNOWN_REPLY);
    finish(&dir);
}

#[test]
fn the_shortest_of_five_covering_cards_still_wins() {
    // Id order would put `kettle-zz` last. The ranker still sees it, because
    // the lookup sorts by haystack length and then keeps four. Lead is 2.
    let dir = write_pack(
        "five",
        &[
            ("kettle-aa", "Kettle aa bb cc."),
            ("kettle-bb", "Kettle bb dd ee."),
            ("kettle-cc", "Kettle cc ff gg."),
            ("kettle-dd", "Kettle dd hh ii."),
            ("kettle-zz", "Kettle zz."),
        ],
    );
    let loaded = KnowledgeCards::load(&dir).unwrap();
    let hit = loaded.cards.lookup("where is the kettle").unwrap();
    assert_eq!(hit.id, "kettle-zz");
    assert_eq!(hit.gist, "Kettle zz.");
    let spoken = brain(&dir).turn("where is the kettle");
    assert_eq!(spoken.reply, "Kettle zz.");
    assert_eq!(spoken.sources, vec!["card:kettle-zz".to_string()]);
    finish(&dir);
}
