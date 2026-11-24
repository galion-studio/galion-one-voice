// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::PathBuf;

use horbm_memory::{parse_cards, synthetic_card, Card, ShardManifest, Stack, Store, MAX_HITS};

fn cards_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/cards")
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "horbm-{name}-{}-{}",
        std::process::id(),
        name.len()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn open_stack(dir: &std::path::Path) -> Stack {
    Stack::open(dir.join("memory.sqlite"), dir.join("knowledge")).unwrap()
}

#[test]
fn imports_the_real_pack_and_keeps_every_field() {
    let dir = scratch("pack");
    let mut stack = open_stack(&dir);
    let first = stack.import_dir(cards_dir()).unwrap();
    assert!(first.cards >= 400, "cards {}", first.cards);
    assert!(first.rebuilt);
    assert!(first.core_rows > 0);
    assert!(first.shard_count > 1);
    let second = stack.import_dir(cards_dir()).unwrap();
    assert!(!second.rebuilt);
    assert_eq!(second.unchanged, first.cards);
    assert_eq!(second.revised, 0);
    assert_eq!(stack.revision_count().unwrap(), 0);

    let card = stack.latest_card("world.free-will-disputed").unwrap();
    assert_eq!(card.claim_type, "contested");
    assert!(card.contested_views.len() >= 3);
    assert!(card.document.contains("contested_views"));
    assert!(card.document.contains("legacy"));
    assert!(!card.sources.is_empty());
    assert!(!card.sources[0].url.is_empty());
    assert_eq!(card.evidence_strength, "moderate");
    assert!(!card.said_by.is_empty());
    assert!(!card.date.is_empty());

    let found = stack.query("compatibilist").unwrap();
    assert!(found.confident, "answer {}", found.answer);
    assert!(found.answer.is_empty());
    assert!(found
        .knowledge
        .iter()
        .any(|hit| hit.item_id == "world.free-will-disputed"));
    let hit = found
        .knowledge
        .iter()
        .find(|hit| hit.item_id == "world.free-will-disputed")
        .unwrap();
    assert_eq!(hit.pool, "knowledge");
    assert_eq!(hit.claim_type, "contested");
    assert_eq!(hit.evidence, "moderate");
    assert!(hit.contested_views.len() >= 3);
    assert!(!hit.said_by.is_empty());
    assert!(!hit.date.is_empty());
    assert!(hit.source.contains("http"));
    assert!(found.memory.iter().all(|hit| hit.pool == "memory"));
    assert!(found.knowledge.len() + found.memory.len() <= MAX_HITS);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn memory_is_not_returned_as_knowledge() {
    let dir = scratch("pools");
    let mut stack = open_stack(&dir);
    stack.import_dir(cards_dir()).unwrap();
    stack.push_turn(
        "please keep the word marigold in this call",
        "the person",
        "2026-11-18",
    );
    let memory = stack.query("marigold").unwrap();
    assert_eq!(memory.answer, "I don't know");
    assert!(memory.knowledge.is_empty());
    assert_eq!(memory.memory[0].pool, "memory");
    assert_eq!(memory.memory[0].claim_type, "user-said");
    assert_eq!(memory.memory[0].source, "session-ring");
    assert!(memory.memory.iter().all(|hit| hit.pool == "memory"));
    assert!(memory.gap_id.is_none());

    stack
        .remember(
            "the person said the private word marigold",
            "the person",
            "2026-11-18",
            "conversation",
        )
        .unwrap();
    let kept = stack.query("marigold").unwrap();
    assert!(kept
        .memory
        .iter()
        .any(|hit| hit.layer == "episode" && hit.pool == "memory"));
    assert!(kept.knowledge.iter().all(|hit| hit.pool == "knowledge"));
    assert!(kept
        .knowledge
        .iter()
        .all(|hit| hit.claim_type != "user-said"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_miss_is_logged_and_the_queue_ranks_it() {
    let dir = scratch("gaps");
    let mut stack = open_stack(&dir);
    let missed = stack.query("zzznothingmatches").unwrap();
    assert_eq!(missed.answer, "I don't know");
    assert!(!missed.confident);
    assert!(missed.knowledge.is_empty());
    assert!(missed.gap_id.is_none());
    stack.drain().unwrap();
    let queue = stack.gap_queue().unwrap();
    assert_eq!(queue[0].query, "zzznothingmatches");
    assert_eq!(queue[0].advice, "ask_galion_research");
    assert!(queue[0].detail.contains("Galion Research"));
    stack.query("zzznothingmatches").unwrap();
    stack.query("othergap").unwrap();
    stack.drain().unwrap();
    let queue = stack.gap_queue().unwrap();
    assert_eq!(queue[0].query, "zzznothingmatches");
    assert_eq!(queue[0].times, 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn low_confidence_names_the_nearest_cards() {
    let dir = scratch("near");
    let mut stack = open_stack(&dir);
    stack
        .import_cards(vec![synthetic_card(1), synthetic_card(2)], false)
        .unwrap();
    let missed = stack.query("sample zzznone").unwrap();
    assert_eq!(missed.answer, "I don't know");
    assert!(missed.gap_id.is_none());
    stack.drain().unwrap();
    let queue = stack.gap_queue().unwrap();
    assert_eq!(queue[0].advice, "revise_card");
    assert!(queue[0].detail.contains("sample-"));
    assert!(queue[0].nearest.contains("sample-"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unloaded_shard_is_the_gap_to_load() {
    let dir = scratch("shard");
    let mut stack = open_stack(&dir);
    let mut cards = Vec::new();
    for i in 0..20 {
        cards.push(synthetic_card(i));
    }
    stack.import_cards(cards, false).unwrap();
    assert!(stack.shard_count() >= 1);
    let unloaded = stack.unload_shard("world-history");
    assert!(unloaded);
    let marker = (0..20).find(|i| i % 5 == 0 && i % 4 == 0).unwrap();
    let response = stack.query(&format!("m{marker}")).unwrap();
    assert_eq!(response.answer, "I don't know");
    assert!(response.knowledge.is_empty());
    assert!(response.gap_id.is_none());
    stack.drain().unwrap();
    let queue = stack.gap_queue().unwrap();
    assert_eq!(queue[0].advice, "load_shard");
    assert!(queue[0].detail.contains("world-history"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn revising_a_card_archives_the_old_text_and_search_reads_the_new_one() {
    let dir = scratch("revise");
    let mut stack = open_stack(&dir);
    stack.import_dir(cards_dir()).unwrap();
    let current = stack.latest_card("world.free-will-disputed").unwrap();
    assert!(current.document.contains("Stanford"));
    let mut value: serde_json::Value = serde_json::from_str(&current.document).unwrap();
    value["summary"] = serde_json::json!("Revised summary token quokkaweek.");
    value["body"] = serde_json::json!("Revised body token quokkaweek.");
    let revised = parse_cards(&value, "world/world.free-will-disputed.json")
        .unwrap()
        .pop()
        .unwrap();
    let report = stack.apply_card(&revised).unwrap();
    assert!(report.rebuilt);
    assert_eq!(report.revised, 1);
    let found = stack.query("quokkaweek").unwrap();
    assert!(found.confident);
    assert_eq!(found.knowledge[0].item_id, "world.free-will-disputed");
    assert!(found.knowledge[0].text.contains("quokkaweek"));
    assert_eq!(found.knowledge[0].claim_type, "contested");
    assert!(found.knowledge[0].contested_views.len() >= 3);
    let old = stack.query("counted").unwrap();
    assert!(old
        .knowledge
        .iter()
        .all(|hit| hit.item_id != "world.free-will-disputed"));
    let archived = stack.revisions("world.free-will-disputed").unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].reason, "replaced");
    assert!(archived[0].document.contains("re-counted"));
    assert!(!archived[0].document.contains("quokkaweek"));
    let again = stack.apply_card(&revised).unwrap();
    assert!(!again.rebuilt);
    assert_eq!(
        stack.revisions("world.free-will-disputed").unwrap().len(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_card_removed_from_the_pack_is_archived_not_deleted() {
    let dir = scratch("absent");
    let mut stack = open_stack(&dir);
    let cards = vec![synthetic_card(1), synthetic_card(2)];
    stack.import_cards(cards.clone(), false).unwrap();
    stack.import_cards(vec![cards[0].clone()], false).unwrap();
    assert!(stack.query("m2").unwrap().knowledge.is_empty());
    let archived = stack.revisions("sample-2").unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].reason, "absent");
    assert!(archived[0].document.contains("marker m2"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_single_file_store_also_archives_a_changed_card() {
    let mut store = Store::open_in_memory().unwrap();
    let mut card = sample_store_card("keep", "first wording alpha");
    store.upsert_card(&card).unwrap();
    card = sample_store_card("keep", "second wording beta");
    store.upsert_card(&card).unwrap();
    let hits = store.search("beta").unwrap();
    assert_eq!(hits.hits[0].item_id, "keep");
    assert_eq!(hits.hits[0].pool, "knowledge");
    assert!(store.search("alpha").unwrap().hits.is_empty());
    let revisions = store.revisions_for("keep").unwrap();
    assert_eq!(revisions.len(), 1);
    assert!(revisions[0].document.contains("alpha"));
    store.upsert_card(&card).unwrap();
    assert_eq!(store.revision_count().unwrap(), 1);
}

fn sample_store_card(id: &str, summary: &str) -> Card {
    let raw = serde_json::json!({
        "id": id,
        "title": id,
        "section": "world",
        "category": "science",
        "summary": summary,
        "body": summary,
        "claim_type": "fact",
        "sources": [{
            "title": "fixture",
            "url": "https://example.test/keep",
            "author": "Ada Lovelace",
            "date": "1843",
            "accessed": "2026-11-18"
        }],
        "said_by": "Ada Lovelace",
        "as_of_date": "1843",
        "status": "draft",
        "contested_views": [],
        "related": []
    });
    parse_cards(&raw, "world/keep.json").unwrap().pop().unwrap()
}

#[test]
fn measured_corpus_at_the_target_size() {
    let _guard = big_index_lock();
    // Debug `cargo test` stays small. The release test is the 200k measurement.
    let n = if cfg!(debug_assertions) { 40 } else { 200_000 };
    let queries = if cfg!(debug_assertions) { 5 } else { 40 };
    let dir = scratch("measure");
    let mut stack = open_stack(&dir);
    let cards: Vec<Card> = (0..n).map(synthetic_card).collect();
    let expect_core = cards
        .iter()
        .filter(|card| card.claim_type == "fact")
        .count();
    let imported = stack.import_cards(cards, false).unwrap();
    assert_eq!(imported.cards, n);
    assert_eq!(imported.core_rows, expect_core);
    assert!(imported.shard_count >= 1);
    assert!(imported.shard_count <= 4);

    let mut selective = Vec::with_capacity(queries);
    for i in 0..queries {
        stack.clear_cache();
        let response = stack.query(&format!("m{}", (i * 17) % n)).unwrap();
        assert!(response.confident, "m{} {}", (i * 17) % n, response.answer);
        assert!(response.knowledge.iter().all(|hit| hit.pool == "knowledge"));
        assert!(response.memory.is_empty());
        selective.push(response.took_us as u64);
    }
    let mut broad = Vec::with_capacity(queries);
    for _ in 0..queries {
        stack.clear_cache();
        let response = stack.query("sample").unwrap();
        assert!(response.confident);
        assert!(!response.knowledge.is_empty());
        broad.push(response.took_us as u64);
    }
    stack.clear_cache();
    let spot = stack.query("m1").unwrap();
    let top_id = spot.knowledge[0].item_id.clone();
    let top_pool = spot.knowledge[0].pool.clone();
    let top_layer = spot.knowledge[0].layer.clone();
    let top_claim = spot.knowledge[0].claim_type.clone();
    let top_evidence = spot.knowledge[0].evidence.clone();
    let top_said = spot.knowledge[0].said_by.clone();
    let top_date = spot.knowledge[0].date.clone();
    let top_source = spot.knowledge[0].source.clone();
    assert!(spot.memory.is_empty());
    if n >= 200_000 {
        let rss = horbm_memory::rss_bytes();
        let disk = stack.disk_bytes().unwrap();
        let selective_p50_us = percentile(&mut selective, 0.50);
        let selective_p95_us = percentile(&mut selective, 0.95);
        let broad_p50_us = percentile(&mut broad, 0.50);
        let broad_p95_us = percentile(&mut broad, 0.95);
        let report = serde_json::json!({
            "rows": n,
            "core_rows": imported.core_rows,
            "shard_count": imported.shard_count,
            "queries": queries,
            "selective_p50_us": selective_p50_us,
            "selective_p95_us": selective_p95_us,
            "broad_p50_us": broad_p50_us,
            "broad_p95_us": broad_p95_us,
            "rss_bytes": rss,
            "disk_bytes": disk,
            "spot_id": top_id,
            "simd": horbm_memory::simd_lane(),
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/stack-measure.json");
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        eprintln!("{report}");
        assert!(
            selective_p95_us <= 10_000,
            "selective p95 {selective_p95_us} us exceeds 10 ms"
        );
        assert!(
            broad_p95_us <= 10_000,
            "broad p95 {broad_p95_us} us exceeds 10 ms"
        );
        assert!(rss > 0);
        // The int8 vectors alone are 256 bytes a row. A non-recursive
        // directory walk would miss index/ and report a much smaller file.
        assert!(disk > n as u64 * 256, "disk {disk}");
    }
    assert_eq!(top_id, "sample-1");
    assert_eq!(top_pool, "knowledge");
    assert_eq!(top_layer, "core");
    assert_eq!(top_claim, "fact");
    assert_eq!(top_evidence, "strong");
    assert_eq!(top_said, "Galion Labs");
    assert_eq!(top_date, "2026-11-18");
    assert!(top_source.contains("galion-knowledge"));

    let again = stack
        .import_cards((0..n).map(synthetic_card).collect(), false)
        .unwrap();
    assert!(!again.rebuilt);
    assert_eq!(again.unchanged, n);
    assert_eq!(stack.revision_count().unwrap(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// One published core card bundle, and the catalog version that names it.
///
/// `schema` is always `galion-shard-manifest/1`. That is the format.
/// `ScaleIndex` writes `version` `"1"` on a freshly built index because that
/// directory's first manifest is publication 1 of that index.
///
/// `version` on `v2/shards/manifest.json` is a different number: the
/// publication count of this catalog. Publication 1 covered the first core
/// digests. Publication 2 is the digest PR #13 recorded after the core pack
/// grew. Publication 3 is the digest after the history and religion cards
/// joined that pack. Publication 4 is the digest after the earth, life, and
/// health cards joined that pack. Publication 5 is the digest after the
/// physics, chemistry, astronomy, and energy gap fill. The version is looked up from the bundle, not copied back out of
/// the manifest, and it is not the schema's trailing 1.
struct CorePublication {
    version: &'static str,
    sha256: &'static str,
    bytes: u64,
    cards: usize,
}

const CORE_PUBLICATIONS: &[CorePublication] = &[
    CorePublication {
        version: "1",
        sha256: "1353a24a4c02a2034e17dc0c9fa716be1fdfdb10cd201da31bea709fd70b6d24",
        bytes: 1_014_355,
        cards: 436,
    },
    CorePublication {
        version: "1",
        sha256: "350085320a2e6c1e69fb713e017a70aef7692544dad77b6f964fe911497f52bd",
        bytes: 1_032_887,
        cards: 436,
    },
    CorePublication {
        version: "2",
        sha256: "08fe16e5e619fc34eec824312cd6efb38f1e57cf8b4b747b601a717b8ebe528e",
        bytes: 1_590_111,
        cards: 778,
    },
    CorePublication {
        version: "3",
        sha256: "2a625fc33126adbefbadba0aba32471a597374b367ad0c6840d345927df91a26",
        bytes: 2_016_490,
        cards: 1013,
    },
    CorePublication {
        version: "4",
        sha256: "5cfe36ca7a55506b232a49312856be46d6b6e20ffeb82f8889303fe179b9688d",
        bytes: 2_239_112,
        cards: 1146,
    },
    CorePublication {
        version: "5",
        sha256: "5dfa2a00ed3e64e079accae8f5a8ffb05cd2ffd37f7141bd6d7979fb3a60b44f",
        bytes: 3_060_127,
        cards: 1506,
    },
];

fn card_bundle(files: &[(String, Vec<u8>)]) -> (String, u64) {
    let mut ordered = files.to_vec();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));
    let mut bytes = Vec::new();
    for (rel, data) in &ordered {
        assert!(data.last() == Some(&b'\n'), "{rel} must end with a newline");
        bytes.extend(rel.as_bytes());
        bytes.push(b'\n');
        bytes.extend(data);
        bytes.push(b'\n');
    }
    (horbm_memory::sha256_hex(&bytes), bytes.len() as u64)
}

fn read_json_tree(dir: &std::path::Path, base: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    names.sort();
    for path in names {
        if path.is_dir() {
            out.extend(read_json_tree(&path, base));
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            out.push((rel, std::fs::read(&path).unwrap()));
        }
    }
    out
}

#[test]
fn addon_shard_manifest_matches_shard_manifest() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let text = std::fs::read_to_string(root.join("shards/manifest.json")).unwrap();
    let manifest: ShardManifest = serde_json::from_str(&text).expect("galion-shard-manifest/1");
    assert_eq!(manifest.schema, "galion-shard-manifest/1");
    let schema_generation = manifest.schema.rsplit('/').next().unwrap();
    assert_eq!(schema_generation, "1");
    assert!(manifest.immutable);
    assert!(!manifest.note.is_empty());
    assert_eq!(manifest.core_pack.id, "core");
    assert_eq!(manifest.core_pack.section, "core");
    assert_eq!(manifest.core_pack.category, "core");

    let core_dir = root.join("data/cards");
    let core_files = read_json_tree(&core_dir, &core_dir);
    let (core_sha, core_bytes) = card_bundle(&core_files);
    assert_eq!(manifest.core_pack.sha256, core_sha, "core digest is stale");
    assert_eq!(manifest.core_pack.bytes, core_bytes);
    let publication = CORE_PUBLICATIONS
        .iter()
        .find(|item| {
            item.sha256 == core_sha && item.bytes == core_bytes && item.cards == core_files.len()
        })
        .unwrap_or_else(|| {
            panic!(
                "core bundle {core_sha} ({} bytes, {} cards) is not a recorded publication. Publish the next catalog version and add that bundle here. Do not reuse an older version for a new digest.",
                core_bytes,
                core_files.len()
            )
        });
    assert_eq!(
        manifest.version, publication.version,
        "schema generation is {schema_generation}; catalog publication for this bundle is {}",
        publication.version
    );
    assert_ne!(
        manifest.version, schema_generation,
        "this core bundle is a later publication than the schema generation"
    );

    assert!(manifest.shards.len() >= 20);
    let mut ids = std::collections::BTreeSet::new();
    for shard in &manifest.shards {
        assert!(ids.insert(shard.id.clone()), "duplicate {}", shard.id);
        assert!(!shard.section.is_empty());
        assert!(!shard.category.is_empty());
        let card_dir = root.join("shards").join(&shard.id).join("cards");
        let files = if card_dir.is_dir() {
            std::fs::read_dir(&card_dir)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
                .map(|path| {
                    let name = path.file_name().unwrap().to_string_lossy().into_owned();
                    (format!("cards/{name}"), std::fs::read(&path).unwrap())
                })
                .collect()
        } else {
            Vec::new()
        };
        let (sha, bytes) = card_bundle(&files);
        assert_eq!(shard.sha256, sha, "{} digest is stale", shard.id);
        assert_eq!(shard.bytes, bytes, "{}", shard.id);
        let meta = root.join("shards").join(&shard.id).join("shard.json");
        let raw: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(meta).unwrap()).unwrap();
        assert_eq!(raw["sha256"].as_str(), Some(shard.sha256.as_str()));
        assert_eq!(raw["bytes"].as_u64(), Some(shard.bytes));
        assert_eq!(raw["schema"], "galion-shard-manifest/1");
        assert_eq!(raw["depends_on"][0], "core");
    }
}

#[test]
fn voice_hot_path_holds_under_background_writes() {
    let _guard = big_index_lock();
    // Debug `cargo test` stays small. The release test is the 200k gate.
    let n = if cfg!(debug_assertions) { 40 } else { 200_000 };
    let queries = if cfg!(debug_assertions) { 5 } else { 40 };
    let dir = scratch("voice");
    let mut stack = open_stack(&dir);
    let cards: Vec<Card> = (0..n).map(synthetic_card).collect();
    let imported = stack.import_cards(cards, false).unwrap();
    assert_eq!(imported.cards, n);

    let handle = stack.queue_handle();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_flag = std::sync::Arc::clone(&stop);
    let writer = std::thread::spawn(move || {
        let mut i = 0u64;
        while !stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
            if handle.pending() < 48 {
                handle.enqueue_background_mix(i);
                i += 1;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
    });

    let mut selective = Vec::with_capacity(queries);
    for i in 0..queries {
        stack.clear_cache();
        let started = std::time::Instant::now();
        let response = stack.query(&format!("m{}", (i * 17) % n)).unwrap();
        let elapsed = started.elapsed().as_micros() as u64;
        assert!(response.confident, "m{} {}", (i * 17) % n, response.answer);
        assert!(elapsed < 2_000_000, "query took {elapsed} us");
        selective.push(response.took_us as u64);
    }
    let mut broad = Vec::with_capacity(queries);
    for _ in 0..queries {
        stack.clear_cache();
        let response = stack.query("sample").unwrap();
        assert!(response.confident);
        assert!(!response.knowledge.is_empty());
        assert!(
            response.took_us < 2_000_000,
            "query took {} us",
            response.took_us
        );
        broad.push(response.took_us as u64);
    }
    stack.clear_cache();
    let spot = stack.query("m1").unwrap();
    assert_eq!(spot.knowledge[0].item_id, "sample-1");
    assert_eq!(spot.knowledge[0].pool, "knowledge");

    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    writer.join().unwrap();
    stack.drain().unwrap();

    if n >= 200_000 {
        let selective_p50_us = percentile(&mut selective, 0.50);
        let selective_p95_us = percentile(&mut selective, 0.95);
        let broad_p50_us = percentile(&mut broad, 0.50);
        let broad_p95_us = percentile(&mut broad, 0.95);
        let report = serde_json::json!({
            "rows": n,
            "queries": queries,
            "selective_p50_us": selective_p50_us,
            "selective_p95_us": selective_p95_us,
            "broad_p50_us": broad_p50_us,
            "broad_p95_us": broad_p95_us,
            "background": "persist, extract, summary, pin, revise",
        });
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/voice-measure.json");
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        eprintln!("{report}");
        assert!(
            selective_p95_us <= 10_000,
            "selective p95 {selective_p95_us} us exceeds 10 ms while the writer runs"
        );
        assert!(
            broad_p95_us <= 10_000,
            "broad p95 {broad_p95_us} us exceeds 10 ms while the writer runs"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn big_index_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let lock = LOCK.get_or_init(|| std::sync::Mutex::new(()));
    lock.lock().unwrap_or_else(|err| err.into_inner())
}

fn percentile(samples: &mut [u64], p: f64) -> u64 {
    samples.sort_unstable();
    if samples.is_empty() {
        return 0;
    }
    let rank = ((p * samples.len() as f64).ceil() as usize).saturating_sub(1);
    samples[rank.min(samples.len() - 1)]
}
