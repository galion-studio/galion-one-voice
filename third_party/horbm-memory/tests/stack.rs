// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use horbm_memory::{Card, Store, MAX_HITS};

fn card(id: &str, summary: &str, body: &str) -> Card {
    let raw = serde_json::json!({
        "id": id,
        "title": id,
        "section": "world",
        "category": "science",
        "summary": summary,
        "body": body,
        "claim_type": "fact",
        "sources": [{
            "title": format!("{id} source"),
            "url": format!("https://example.test/{id}"),
            "author": "Ada Lovelace",
            "date": "1843"
        }],
        "said_by": "Ada Lovelace",
        "as_of_date": "1843",
        "status": "draft"
    });
    horbm_memory::parse_cards(&raw, "world/card.json")
        .unwrap()
        .pop()
        .unwrap()
}

#[test]
fn search_returns_at_most_four_sourced_hits() {
    let mut store = Store::open_in_memory().unwrap();
    for i in 0..10 {
        store
            .upsert_card(&card(
                &format!("card-{i}"),
                &format!("shared token xylophone number {i}"),
                "A sourced sentence about xylophone retrieval.",
            ))
            .unwrap();
    }
    let response = store.search("xylophone").unwrap();
    assert!(response.hits.len() <= MAX_HITS);
    assert_eq!(response.hits.len(), MAX_HITS);
    for hit in &response.hits {
        assert!(!hit.said_by.is_empty());
        assert!(!hit.date.is_empty());
        assert!(!hit.source.is_empty());
        assert_ne!(hit.said_by, "unknown");
        assert!(hit.source.contains("example.test"));
        assert_eq!(hit.date, "1843");
    }
}

#[test]
fn rare_token_wins_bm25() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .upsert_card(&card("plain", "a quiet note", "nothing unusual here"))
        .unwrap();
    store
        .upsert_card(&card(
            "rare",
            "the quokka rests",
            "A quokka is the rare animal in this fixture.",
        ))
        .unwrap();
    let response = store.search("quokka").unwrap();
    assert_eq!(response.hits[0].item_id, "rare");
}

#[test]
fn ring_is_ram_until_remember() {
    let mut store = Store::open_in_memory().unwrap();
    store.push_turn(
        "please keep the word marigold in this call",
        "the person",
        "2026-11-18",
    );
    let live = store.search("marigold").unwrap();
    assert_eq!(live.hits[0].layer, "session");
    assert_eq!(live.hits[0].source, "session-ring");
    assert_eq!(live.hits[0].said_by, "the person");
    assert_eq!(store.active_count().unwrap(), 0);

    let again = Store::open_in_memory().unwrap();
    let gone = again.search("marigold").unwrap();
    assert!(gone.hits.is_empty());

    let id = store
        .remember(
            "please keep the word marigold in this call",
            "the person",
            "2026-11-18",
            "conversation",
        )
        .unwrap();
    assert!(id.starts_with("ep-"));
    let kept = store.search("marigold").unwrap();
    assert!(kept
        .hits
        .iter()
        .any(|h| h.layer == "episode" && h.item_id == id));
    assert_eq!(
        kept.hits.iter().find(|h| h.item_id == id).unwrap().date,
        "2026-11-18"
    );
}

#[test]
fn diary_is_append_only_and_ordered() {
    let mut store = Store::open_in_memory().unwrap();
    let a = store
        .diary_add("first entry about rain", "2026-11-01", "HORBM", "diary")
        .unwrap();
    let b = store
        .diary_add("second entry about tea", "2026-11-02", "HORBM", "diary")
        .unwrap();
    let list = store.diary_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].item_id, a);
    assert_eq!(list[1].item_id, b);
    assert_eq!(list[0].date, "2026-11-01");
    assert!(store
        .search("tea")
        .unwrap()
        .hits
        .iter()
        .any(|h| h.item_id == b));
}

#[test]
fn mark_hides_and_forget_archives() {
    let mut store = Store::open_in_memory().unwrap();
    store
        .upsert_card(&card("keep", "alpha stays", "alpha"))
        .unwrap();
    store
        .upsert_card(&card("drop", "beta leaves", "beta"))
        .unwrap();
    store.mark("drop").unwrap();
    let hidden = store.search("beta").unwrap();
    assert!(hidden.hits.iter().all(|h| h.item_id != "drop"));
    let list_still = store.active_count().unwrap();
    assert_eq!(list_still, 1);
    store.forget("keep").unwrap();
    assert!(store.search("alpha").unwrap().hits.is_empty());
    assert!(store.forget("missing").is_err());
    let archived = store.archive_list().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].item_id, "keep");
    assert_eq!(archived[0].text, "alpha");
    assert_eq!(archived[0].said_by, "Ada Lovelace");
    assert_eq!(archived[0].date, "1843");
    assert_eq!(archived[0].reason, "forget");
    assert!(archived[0].source.contains("example.test"));
}

#[test]
fn a_term_in_every_row_stays_off_the_postings_scan() {
    let mut store = Store::open_in_memory().unwrap();
    let mut cards = Vec::new();
    for i in 0..80 {
        cards.push(card(
            &format!("c{i}"),
            &format!("sample note uniq{i}"),
            "sample",
        ));
    }
    store.upsert_cards(&cards).unwrap();
    let common = store.search("sample").unwrap();
    assert!(!common.hits.is_empty());
    assert!(common.hits.len() <= MAX_HITS);
    let rare = store.search("uniq3").unwrap();
    assert_eq!(rare.hits[0].item_id, "c3");
}

#[test]
fn archived_row_survives_reopen() {
    let dir = std::env::temp_dir().join(format!("horbm-archive-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("memory.sqlite");
    let id = {
        let mut store = Store::open(&path).unwrap();
        let id = store
            .remember(
                "keep the word cedar",
                "the person",
                "2026-11-18",
                "conversation",
            )
            .unwrap();
        store.forget(&id).unwrap();
        assert!(store.search("cedar").unwrap().hits.is_empty());
        id
    };
    let store = Store::open(&path).unwrap();
    let archived = store.archive_list().unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].item_id, id);
    assert_eq!(archived[0].text, "keep the word cedar");
    assert_eq!(archived[0].reason, "forget");
    assert!(store.search("cedar").unwrap().hits.is_empty());
    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ingest_is_idempotent_and_reads_fixtures() {
    let mut store = Store::open_in_memory().unwrap();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");
    let first = store.ingest_dir(dir).unwrap();
    assert!(first.upserted >= 4);
    let second = store.ingest_dir(dir).unwrap();
    assert_eq!(first.upserted, second.upserted);
    assert_eq!(store.active_count().unwrap(), first.upserted);
    let hits = store.search("bm25").unwrap();
    assert!(!hits.hits.is_empty());
    assert!(hits.hits.len() <= MAX_HITS);
    for hit in &hits.hits {
        assert!(!hit.said_by.is_empty());
        assert!(!hit.date.is_empty());
        assert!(!hit.source.is_empty());
    }
}

#[test]
fn remember_refuses_an_empty_field() {
    let mut store = Store::open_in_memory().unwrap();
    assert!(store.remember("", "a", "2026-11-18", "s").is_err());
    assert!(store.remember("text", "a", "", "s").is_err());
    assert_eq!(store.active_count().unwrap(), 0);
}
