// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Full-pack measurement. Runs when `GALION_HORBM_KNOWLEDGE` points at a
//! directory that loads at least 1,000 cards. Otherwise it prints that the
//! pack was not on this machine and returns. The fixture hit and miss stay
//! in `cards.rs`. Numbers printed here are from this process, not a laptop.

use std::time::Instant;

use horbm::{CardLookup, KnowledgeCards};

#[test]
fn full_pack_load_time_and_lookup_p95() {
    let Ok(raw) = std::env::var("GALION_HORBM_KNOWLEDGE") else {
        eprintln!("vm bench: GALION_HORBM_KNOWLEDGE unset; full pack was not measured");
        return;
    };
    let dir = std::path::PathBuf::from(raw.trim());
    if !dir.is_dir() {
        eprintln!(
            "vm bench: {} is not a directory; full pack was not measured",
            dir.display()
        );
        return;
    }
    let before = json_count(&dir);
    let loaded = KnowledgeCards::load(&dir).expect("pack load");
    let after = json_count(&dir);
    assert_eq!(before, after, "load must not write cards");
    assert!(!dir.join("revisions.sqlite").exists());
    if loaded.cards.len() < 1000 {
        eprintln!(
            "vm bench: loaded {} cards from {}; full pack was not measured",
            loaded.cards.len(),
            dir.display()
        );
        return;
    }
    assert_eq!(loaded.report.manifest, "sha256 ok");
    assert!(loaded.cards.lookup("zzzz-not-in-the-pack-9bad").is_none());
    let ids: Vec<String> = loaded.cards.ids().map(str::to_string).collect();
    assert_eq!(ids.len(), loaded.cards.len());
    for id in ids.iter().take(32) {
        let _ = loaded.cards.lookup(id);
    }
    let mut samples = Vec::with_capacity(ids.len() + 200);
    for id in &ids {
        let started = Instant::now();
        let _ = loaded.cards.lookup(id);
        samples.push(started.elapsed().as_nanos());
    }
    for n in 0..200 {
        let started = Instant::now();
        let miss = loaded.cards.lookup(&format!("zzzz-not-in-the-pack-{n}"));
        samples.push(started.elapsed().as_nanos());
        assert!(miss.is_none());
    }
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let load_ms = loaded.report.elapsed.as_secs_f64() * 1000.0;
    let p50 = percentile(&mut samples, 50);
    let p95 = percentile(&mut samples, 95);
    let max = samples.iter().copied().max().unwrap_or(0);
    eprintln!("vm bench: profile={profile} cards={}", loaded.cards.len());
    eprintln!(
        "vm bench: load_ms={load_ms:.3} manifest={} path={}",
        loaded.report.manifest,
        dir.display()
    );
    eprintln!(
        "vm bench: lookups={} p50_us={:.3} p95_us={:.3} max_us={:.3}",
        samples.len(),
        p50 as f64 / 1000.0,
        p95 as f64 / 1000.0,
        max as f64 / 1000.0
    );
    eprintln!("{}", loaded.report.log_line());
}

fn percentile(samples: &mut [u128], percent: usize) -> u128 {
    samples.sort_unstable();
    if samples.is_empty() {
        return 0;
    }
    let rank = (samples.len() * percent + 99) / 100;
    samples[rank.saturating_sub(1)]
}

fn json_count(dir: &std::path::Path) -> usize {
    fn walk(dir: &std::path::Path, n: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, n);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                *n += 1;
            }
        }
    }
    let mut n = 0;
    walk(dir, &mut n);
    n
}
