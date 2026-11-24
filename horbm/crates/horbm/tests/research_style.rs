// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Synthetic style-example fixtures. Not a dataset. Nothing here is copied
//! from LCCC or any other corpus. The 100k cards are generated in the test
//! and deleted with the temp directory.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use horbm::{
    Brain, KnowledgeCards, ResearchRoot, StylePersona, StyleSearch, MAX_RESEARCH_BUNDLE_BYTES,
    STYLE_HIT_CAP, UNKNOWN_REPLY,
};

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "horbm-style-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn style_dir(root: &Path, dataset: &str) -> PathBuf {
    let dir = root
        .join("cards")
        .join("diary")
        .join("dialogue")
        .join(dataset);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_card(root: &Path, dataset: &str, name: &str, body: &str) {
    std::fs::write(style_dir(root, dataset).join(name), body).unwrap();
}

fn card(id: &str, body: &str) -> String {
    format!(
        r#"{{"id":"{id}","not_for":["answer","cite"],"licence":"synthetic","fixture":"synthetic","body":"{body}"}}"#
    )
}

fn samantha(root: &ResearchRoot, query: &str) -> StyleSearch {
    root.search(StylePersona::Samantha, query)
}

#[test]
fn unset_research_is_off_and_aria_reads_nothing() {
    assert!(std::env::var_os("GALION_HORBM_RESEARCH_DIR").is_none());
    let off = ResearchRoot::from_env();
    assert!(off.is_empty());
    assert_eq!(off.files_read(), 0);
    let searched = samantha(&off, "i am tired");
    assert!(searched.hits.is_empty());
    assert_eq!(searched.bundles_opened, 0);
    assert_eq!(off.files_read(), 0);

    let dir = temp_dir("aria");
    write_card(
        &dir,
        "synthetic",
        "soft.json",
        &card(
            "soft-pace",
            "When they say i am tired, keep the line short.",
        ),
    );
    let root = ResearchRoot::load(&dir).unwrap();
    assert_eq!(root.files_read(), 0);
    let aria = root.search(StylePersona::Aria, "i am tired");
    assert!(aria.hits.is_empty());
    assert_eq!(aria.bundles_opened, 0);
    assert!(root.opened_bundles().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn english_query_does_not_open_lccc_and_hits_are_not_answers() {
    let dir = temp_dir("lang");
    write_card(
        &dir,
        "synthetic",
        "soft.json",
        &card(
            "soft-pace",
            "When they say i am tired, the spare kettle sits in the cupboard.",
        ),
    );
    write_card(
        &dir,
        "lccc",
        "pack.json",
        &card(
            "lccc-leak",
            "When they say i am tired, lccc marker should stay unread.",
        ),
    );
    let outside = dir.join("outside.json");
    std::fs::write(
        &outside,
        card("outside-card", "outside the dialogue tree zephyr marker"),
    )
    .unwrap();
    let huge = style_dir(&dir, "synthetic").join("too-big.json");
    let mut file = std::fs::File::create(&huge).unwrap();
    file.write_all(&vec![b' '; (MAX_RESEARCH_BUNDLE_BYTES as usize) + 1])
        .unwrap();

    let root = ResearchRoot::load(&dir).unwrap();
    assert_eq!(root.files_read(), 0);
    assert_eq!(root.bundle_count(), 2);
    let stopwords = samantha(&root, "the and of");
    assert!(stopwords.hits.is_empty());
    assert_eq!(stopwords.bundles_opened, 0);
    assert_eq!(root.files_read(), 0);
    let found = samantha(&root, "i am tired");
    assert_eq!(found.bundles_skipped_lang, 1);
    assert_eq!(found.bundles_skipped_size, 1);
    assert_eq!(found.hits.len(), 1);
    assert!(found.hits.len() <= STYLE_HIT_CAP);
    let hit = &found.hits[0];
    assert_eq!(hit.id, "soft-pace");
    assert_eq!(hit.not_for, vec!["answer".to_string(), "cite".to_string()]);
    assert!(hit.tagged_not_for_answer_and_cite());
    assert!(!hit.allows_answer());
    assert!(!hit.allows_cite());
    assert_eq!(hit.licence, "synthetic");
    assert!(!root
        .opened_bundles()
        .iter()
        .any(|path| path.contains("lccc")));
    assert!(!root
        .opened_bundles()
        .iter()
        .any(|path| path.contains("too-big")));
    assert!(!root
        .opened_bundles()
        .iter()
        .any(|path| path.contains("outside")));
    assert!(found.hits.iter().all(|item| item.id != "lccc-leak"));
    assert!(found.hits.iter().all(|item| !item.text.contains("zephyr")));

    let again = samantha(&root, "i am tired");
    assert_eq!(again.bundles_opened, 0);
    assert_eq!(root.files_read(), 1);

    let mut brain = Brain::with_cards(KnowledgeCards::empty(), None).with_style(root);
    let tired = brain.turn("i am so tired");
    assert_ne!(tired.arm, "answer");
    assert_ne!(tired.arm, "cite");
    assert_eq!(tired.reply, UNKNOWN_REPLY);
    assert_eq!(tired.sources, vec!["honest:unknown".to_string()]);
    assert_eq!(tired.evidence, vec!["style:soft-pace".to_string()]);
    assert!(!tired.reply.to_ascii_lowercase().contains("cupboard"));
    assert!(!tired.reply.to_ascii_lowercase().contains("kettle"));
    assert!(tired
        .sources
        .iter()
        .all(|source| !source.starts_with("style:")));
    assert!(tired
        .sources
        .iter()
        .all(|source| !source.contains("soft-pace")));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_bad_card_is_skipped_and_near_copies_collapse() {
    let dir = temp_dir("schema");
    let bundle = r#"[
      {"id":"kept","license":"synthetic","not_for":["Answer","Cite"],"body":"keep the evening line short and warm"},
      {"id":"copy","licence":"synthetic","not_for":["answer","cite"],"body":"keep the evening line short and warm please"},
      {"id":"other","licence":"synthetic","not_for":["answer","cite"],"body":"keep the harbour gate warm"},
      {"id":"no-licence","not_for":["answer","cite"],"body":"keep the evening line short and warm extra"},
      {"id":"cite-only","licence":"synthetic","not_for":["cite"],"body":"keep the evening line short and warm extra"},
      {"id":"fact-card","licence":"synthetic","not_for":["answer","cite"],"claim_type":"fact","body":"keep the evening line short and warm extra"},
      {"id":"Not A Slug","licence":"synthetic","not_for":["answer","cite"],"body":"keep the evening line short and warm extra"}
    ]"#;
    write_card(&dir, "synthetic", "bundle.json", bundle);
    let root = ResearchRoot::load(&dir).unwrap();
    let found = samantha(&root, "keep the evening line short and warm");
    assert!(found.hits.len() <= STYLE_HIT_CAP);
    assert_eq!(found.hits[0].id, "kept");
    assert!(found.hits.iter().all(|hit| hit.id != "copy"));
    assert!(found.hits.iter().all(|hit| hit.id != "no-licence"));
    assert!(found.hits.iter().all(|hit| hit.id != "cite-only"));
    assert!(found.hits.iter().all(|hit| hit.id != "fact-card"));
    assert!(found.hits.iter().any(|hit| hit.id == "other"));
    assert!(found
        .hits
        .iter()
        .all(|hit| !hit.allows_answer() && !hit.allows_cite()));
    assert!(found
        .hits
        .iter()
        .all(|hit| hit.not_for == ["answer".to_string(), "cite".to_string()]));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_chinese_query_does_not_need_the_english_bundle() {
    let dir = temp_dir("zh");
    write_card(
        &dir,
        "synthetic",
        "soft.json",
        &card(
            "soft-pace",
            "When they say i am tired, keep the line short.",
        ),
    );
    write_card(
        &dir,
        "lccc",
        "pack.json",
        &card("lccc-sample", "synthetic han note \u{4f60}\u{597d}"),
    );
    let root = ResearchRoot::load(&dir).unwrap();
    let found = samantha(&root, "\u{4f60}\u{597d}");
    assert!(found.hits.iter().any(|hit| hit.id == "lccc-sample"));
    assert!(found.hits.iter().all(|hit| hit.lang == "zh"));
    assert!(!root
        .opened_bundles()
        .iter()
        .any(|path| path.contains("synthetic")));
    assert!(found.hits.iter().all(|hit| !hit.allows_answer()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn synthetic_hundred_thousand_cards_stay_off_lccc() {
    let dir = temp_dir("100k");
    let dataset = style_dir(&dir, "synthetic");
    let mut bundle_index = 0usize;
    let mut bundle =
        String::from("{\"licence\":\"synthetic\",\"fixture\":\"synthetic\",\"cards\":[");
    let mut count_in_bundle = 0usize;
    let mut first = true;
    for n in 0..100_000usize {
        let id = format!("syn-{n:06}");
        let body = format!("synthetic style sample {id} quiet pace");
        let line = card(&id, &body);
        if bundle.len() + line.len() + 2 > 200_000 && count_in_bundle > 0 {
            finish_bundle(&dataset, bundle_index, &mut bundle);
            bundle_index += 1;
            bundle =
                String::from("{\"licence\":\"synthetic\",\"fixture\":\"synthetic\",\"cards\":[");
            count_in_bundle = 0;
            first = true;
        }
        if !first {
            bundle.push(',');
        }
        first = false;
        bundle.push_str(&line);
        count_in_bundle += 1;
        if n == 4_242 {
            let copy = card(
                "syn-004242-b",
                "synthetic style sample syn-004242 quiet pace please",
            );
            bundle.push(',');
            bundle.push_str(&copy);
        }
    }
    if count_in_bundle > 0 {
        finish_bundle(&dataset, bundle_index, &mut bundle);
    }
    write_card(
        &dir,
        "lccc",
        "unread.json",
        &card(
            "lccc-unread",
            "synthetic style sample lccc marker quiet pace",
        ),
    );

    let root = ResearchRoot::load(&dir).unwrap();
    assert_eq!(root.files_read(), 0);
    let started = Instant::now();
    let first_search = samantha(&root, "syn-004242 quiet pace");
    let first_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let warm = samantha(&root, "syn-004242 quiet pace");
    let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "horbm research style: 100000 synthetic cards, first search {first_ms:.1} ms ({} us), warm search {warm_ms:.1} ms ({} us), hits {}, indexed {}",
        first_search.took_us,
        warm.took_us,
        first_search.hits.len(),
        root.len()
    );
    assert!(first_ms < 30_000.0, "first search {first_ms:.1} ms");
    assert!(warm_ms < 1_000.0, "warm search {warm_ms:.1} ms");
    assert!(!first_search.hits.is_empty());
    assert!(first_search.hits.len() <= STYLE_HIT_CAP);
    let survivor = first_search
        .hits
        .iter()
        .filter(|hit| hit.id == "syn-004242" || hit.id == "syn-004242-b")
        .count();
    assert_eq!(survivor, 1);
    assert!(first_search
        .hits
        .iter()
        .all(|hit| !hit.allows_answer() && !hit.allows_cite()));
    assert!(first_search
        .hits
        .iter()
        .all(|hit| hit.tagged_not_for_answer_and_cite()));
    assert!(!root
        .opened_bundles()
        .iter()
        .any(|path| path.contains("lccc")));
    assert_eq!(warm.bundles_opened, 0);
    assert!(root.len() >= 100_000);
    let _ = std::fs::remove_dir_all(dir);
}

fn finish_bundle(dir: &Path, index: usize, body: &mut String) {
    body.push_str("]}");
    let path = dir.join(format!("bundle-{index:04}.json"));
    assert!(
        body.len() as u64 <= MAX_RESEARCH_BUNDLE_BYTES,
        "{} is {} bytes",
        path.display(),
        body.len()
    );
    std::fs::write(path, body.as_bytes()).unwrap();
}
