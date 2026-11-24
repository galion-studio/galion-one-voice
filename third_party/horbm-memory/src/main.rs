// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use horbm_memory::{
    parse_cards, rss_bytes, simd_lane, synthetic_card, Card, ScaleIndex, Stack, Store, CATEGORIES,
    EMB_DIM,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("horbm-memory: {err}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") && args.len() == 1 {
        print_help();
        return Ok(());
    }
    let db = take_db(&mut args)?;
    let knowledge = take_knowledge(&mut args)?;
    if args.is_empty() {
        print_help();
        return Err("missing command".into());
    }
    let cmd = args.remove(0);
    if matches!(
        cmd.as_str(),
        "import" | "rebuild" | "query" | "gaps" | "revisions" | "card" | "stack-bench"
    ) {
        return stack_cmd(&db, &knowledge, &cmd, &args);
    }
    let mut store = Store::open(&db).map_err(|e| e.to_string())?;
    match cmd.as_str() {
        "ingest" => {
            let dir = args.first().ok_or("ingest needs a directory")?;
            let report = store.ingest_dir(dir).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        "search" => {
            if args.is_empty() {
                return Err("search needs a query".into());
            }
            let query = args.join(" ");
            let response = store.search(&query).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response).map_err(|e| e.to_string())?
            );
        }
        "remember" => {
            let text = flag(&args, "--text")?;
            let said_by = flag(&args, "--said-by")?;
            let date = flag(&args, "--date")?;
            let source = flag(&args, "--source")?;
            let id = store
                .remember(&text, &said_by, &date, &source)
                .map_err(|e| e.to_string())?;
            println!("{{\"id\":\"{id}\",\"layer\":\"episode\"}}");
        }
        "diary" => {
            let sub = args.first().map(String::as_str).unwrap_or("");
            match sub {
                "add" => {
                    let text = flag(&args, "--text")?;
                    let date = flag(&args, "--date")?;
                    let said_by = flag_or(&args, "--said-by", "HORBM");
                    let source = flag_or(&args, "--source", "diary");
                    let id = store
                        .diary_add(&text, &date, &said_by, &source)
                        .map_err(|e| e.to_string())?;
                    println!("{{\"id\":\"{id}\",\"layer\":\"diary\"}}");
                }
                "list" => {
                    let entries = store.diary_list().map_err(|e| e.to_string())?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())?
                    );
                }
                _ => return Err("diary needs add or list".into()),
            }
        }
        "keep-session" => {
            let session = flag(&args, "--session")?;
            let on = args.iter().any(|arg| arg == "--on");
            let off = args.iter().any(|arg| arg == "--off");
            if on == off {
                return Err("keep-session needs --on or --off".into());
            }
            store
                .keep_whole_session(&session, on)
                .map_err(|err| err.to_string())?;
            store.drain().map_err(|err| err.to_string())?;
            let summary = store
                .session_summary(&session)
                .map_err(|err| err.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "session": session,
                    "keep_whole": on,
                    "summary": summary,
                })
            );
        }
        "summary" => {
            let session = flag_or(&args, "--session", "default");
            store.drain().map_err(|err| err.to_string())?;
            let summary = store
                .session_summary(&session)
                .map_err(|err| err.to_string())?;
            println!(
                "{}",
                serde_json::json!({
                    "session": session,
                    "summary": summary,
                })
            );
        }
        "jobs" => {
            let sub = args.first().map(String::as_str).unwrap_or("status");
            if sub == "drain" {
                store.drain().map_err(|err| err.to_string())?;
            }
            println!(
                "{}",
                serde_json::json!({
                    "pending": store.queue_len(),
                })
            );
        }
        "revise" => {
            let id = flag(&args, "--id")?;
            let text = flag(&args, "--text")?;
            let said_by = flag(&args, "--said-by")?;
            let date = flag(&args, "--date")?;
            let source = flag(&args, "--source")?;
            let new_id = store
                .revise_fact(&id, &text, &said_by, &date, &source)
                .map_err(|err| err.to_string())?;
            store.drain().map_err(|err| err.to_string())?;
            println!("{{\"id\":\"{new_id}\",\"revised\":\"{id}\",\"pinned\":true}}");
        }
        "forget" => {
            if args.iter().any(|a| a == "--withdraw") {
                return Err(
                    "there is no withdrawal command. forget --id and forget --mark are admin tools"
                        .into(),
                );
            } else if args.iter().any(|a| a == "--mark") {
                let id = flag(&args, "--id")?;
                store.mark(&id).map_err(|e| e.to_string())?;
                println!("{{\"marked\":\"{id}\"}}");
            } else {
                let id = flag(&args, "--id")?;
                store.forget(&id).map_err(|e| e.to_string())?;
                println!("{{\"archived\":\"{id}\"}}");
            }
        }
        "scale-bench" => {
            let n = flag_or(&args, "--n", "1000000")
                .parse::<usize>()
                .map_err(|_| "--n must be a number")?;
            let queries = flag_or(&args, "--queries", "40")
                .parse::<usize>()
                .map_err(|_| "--queries must be a number")?;
            let dir = PathBuf::from(flag_or(&args, "--dir", "/tmp/horbm-scale"));
            let report = scale_bench(&dir, n, queries).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        "bench" => {
            let n = flag_or(&args, "--n", "2000")
                .parse::<usize>()
                .map_err(|_| "--n must be a number")?;
            let queries = flag_or(&args, "--queries", "200")
                .parse::<usize>()
                .map_err(|_| "--queries must be a number")?;
            let report = bench(&mut store, n, queries).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        "help" => print_help(),
        other => return Err(format!("unknown command {other}")),
    }
    Ok(())
}

fn stack_cmd(
    db: &std::path::Path,
    knowledge: &std::path::Path,
    cmd: &str,
    args: &[String],
) -> Result<(), String> {
    if cmd == "stack-bench" {
        let n = flag_or(args, "--n", "200000")
            .parse::<usize>()
            .map_err(|_| "--n must be a number")?;
        let queries = flag_or(args, "--queries", "40")
            .parse::<usize>()
            .map_err(|_| "--queries must be a number")?;
        let report = stack_bench(knowledge, n, queries).map_err(|e| e.to_string())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
        return Ok(());
    }
    let mut stack = Stack::open(db, knowledge).map_err(|e| e.to_string())?;
    match cmd {
        "import" | "rebuild" => {
            let dir = args.first().ok_or("import needs a directory of cards")?;
            let report = if cmd == "rebuild" {
                stack.rebuild_dir(dir).map_err(|e| e.to_string())?
            } else {
                stack.import_dir(dir).map_err(|e| e.to_string())?
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
        }
        "query" => {
            if args.is_empty() {
                return Err("query needs words".into());
            }
            let response = stack.query(&args.join(" ")).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response).map_err(|e| e.to_string())?
            );
        }
        "gaps" => {
            let queue = stack.gap_queue().map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&queue).map_err(|e| e.to_string())?
            );
        }
        "revisions" => {
            let id = flag(args, "--id")?;
            let rows = stack.revisions(&id).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())?
            );
        }
        "card" => {
            let id = flag(args, "--id")?;
            let card = stack.latest_card(&id).map_err(|e| e.to_string())?;
            println!("{}", card.document);
        }
        other => return Err(format!("unknown command {other}")),
    }
    Ok(())
}

fn stack_bench(
    dir: &std::path::Path,
    n: usize,
    queries: usize,
) -> Result<serde_json::Value, String> {
    let db = dir.join("memory.sqlite");
    let mut stack = Stack::open(&db, dir).map_err(|e| e.to_string())?;
    let cards: Vec<Card> = (0..n).map(synthetic_card).collect();
    let core_expect = cards
        .iter()
        .filter(|card| card.claim_type == "fact")
        .count();
    let report = stack
        .import_cards(cards, false)
        .map_err(|e| e.to_string())?;
    let mut selective = Vec::with_capacity(queries);
    for i in 0..queries {
        stack.clear_cache();
        let response = stack
            .query(&format!("m{}", (i * 17) % n.max(1)))
            .map_err(|e| e.to_string())?;
        selective.push(response.took_us as u64);
    }
    let mut broad = Vec::with_capacity(queries);
    for _ in 0..queries {
        stack.clear_cache();
        let response = stack.query("sample").map_err(|e| e.to_string())?;
        broad.push(response.took_us as u64);
    }
    stack.clear_cache();
    let spot = stack.query("m0").map_err(|e| e.to_string())?;
    let top = spot.knowledge.first();
    Ok(serde_json::json!({
        "rows": n,
        "core_rows": report.core_rows,
        "core_expect": core_expect,
        "shard_count": report.shard_count,
        "rebuilt": report.rebuilt,
        "queries": queries,
        "selective": series_stats(&mut selective, "m0"),
        "broad": series_stats(&mut broad, "sample"),
        "rss_bytes": rss_bytes(),
        "disk_bytes": stack.disk_bytes().map_err(|e| e.to_string())?,
        "spot_pool": top.map(|hit| hit.pool.clone()),
        "spot_top_id": top.map(|hit| hit.item_id.clone()),
        "spot_said_by": top.map(|hit| hit.said_by.clone()),
        "spot_date": top.map(|hit| hit.date.clone()),
        "spot_source": top.map(|hit| hit.source.clone()),
        "spot_claim_type": top.map(|hit| hit.claim_type.clone()),
        "spot_evidence": top.map(|hit| hit.evidence.clone()),
        "spot_answer": spot.answer,
        "memory_hits": spot.memory.len(),
        "simd": simd_lane(),
        "cpu": cpu_model(),
        "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "note": "Stack::query microseconds. selective is m{i}. broad is sample, a token on every generated card. Cold queries clear the LRU first. The corpus is generated cards, not the knowledge pack."
    }))
}

fn bench(store: &mut Store, n: usize, queries: usize) -> Result<serde_json::Value, String> {
    let fixture_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let fixtures = if fixture_dir.is_dir() {
        store.ingest_dir(&fixture_dir).map_err(|e| e.to_string())?
    } else {
        return Err(format!("missing fixtures at {}", fixture_dir.display()));
    };
    let generated: Vec<Card> = (0..n).map(sample_card).collect();
    store.upsert_cards(&generated).map_err(|e| e.to_string())?;
    let warmup = 20.min(queries.max(1));
    for i in 0..warmup {
        let selective = selective_query(n, i);
        let _ = store.search(&selective).map_err(|e| e.to_string())?;
        let _ = store.search(&broad_query(n)).map_err(|e| e.to_string())?;
    }
    let mut selective = Vec::with_capacity(queries);
    let mut broad = Vec::with_capacity(queries);
    for i in 0..queries {
        let response = store
            .search(&selective_query(n, i))
            .map_err(|e| e.to_string())?;
        selective.push(response.took_us as u64);
        let response = store.search(&broad_query(n)).map_err(|e| e.to_string())?;
        broad.push(response.took_us as u64);
    }
    let spot = store
        .search(&if n == 0 {
            "bm25".to_string()
        } else {
            "m0".to_string()
        })
        .map_err(|e| e.to_string())?;
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    Ok(serde_json::json!({
        "corpus_fixtures": fixtures.upserted,
        "corpus_generated": n,
        "corpus_active": store.active_count().map_err(|e| e.to_string())?,
        "dim": EMB_DIM,
        "queries": queries,
        "warmup": warmup,
        "target_p95_us": 5_000,
        "selective": series_stats(&mut selective, &selective_query(n, 0)),
        "broad": series_stats(&mut broad, &broad_query(n)),
        "spot_query": spot.query,
        "spot_top_id": spot.hits.first().map(|h| h.item_id.clone()),
        "spot_top_said_by": spot.hits.first().map(|h| h.said_by.clone()),
        "spot_top_date": spot.hits.first().map(|h| h.date.clone()),
        "spot_top_source": spot.hits.first().map(|h| h.source.clone()),
        "simd": simd_lane(),
        "cpu": cpu_model(),
        "logical_cpus": cpus,
        "note": "Store::search microseconds on this process. selective queries use one rare token (m{i}, or bm25 when n is 0). broad repeats one token present in every generated card (sample), or the word the when n is 0. Generated cards are synthetic, not the knowledge pack."
    }))
}

fn scale_bench(
    dir: &std::path::Path,
    n: usize,
    queries: usize,
) -> Result<serde_json::Value, String> {
    let fixtures = fixture_cards().map_err(|e| e.to_string())?;
    let rows = fixtures.len() + n;
    let mut index = ScaleIndex::build(dir, fixtures.into_iter().chain((0..n).map(scale_card)))
        .map_err(|e| e.to_string())?;
    let mut selective_us = Vec::with_capacity(queries);
    for i in 0..queries {
        index.clear_cache();
        let response = index
            .search(&selective_query(n, i))
            .map_err(|e| e.to_string())?;
        selective_us.push(response.took_us as u64);
    }
    let mut broad_us = Vec::with_capacity(queries);
    for _ in 0..queries {
        index.clear_cache();
        let response = index.search("sample").map_err(|e| e.to_string())?;
        broad_us.push(response.took_us as u64);
    }
    index.clear_cache();
    let _ = index.search("sample").map_err(|e| e.to_string())?;
    let mut cached_us = Vec::with_capacity(queries);
    for _ in 0..queries {
        let response = index.search("sample").map_err(|e| e.to_string())?;
        cached_us.push(response.took_us as u64);
    }
    let spot = index.search_cold("m0").map_err(|e| e.to_string())?;
    let manifest = index.manifest().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "rows": rows,
        "core_rows": index.core_rows(),
        "shard_count": manifest.shards.len(),
        "queries": queries,
        "selective": series_stats(&mut selective_us, &selective_query(n, 0)),
        "broad": series_stats(&mut broad_us, "sample"),
        "broad_cached": series_stats(&mut cached_us, "sample"),
        "rss_bytes": rss_bytes(),
        "disk_bytes": index.disk_bytes().map_err(|e| e.to_string())?,
        "spot_top_id": spot.hits.first().map(|h| h.item_id.clone()),
        "spot_said_by": spot.hits.first().map(|h| h.said_by.clone()),
        "spot_date": spot.hits.first().map(|h| h.date.clone()),
        "spot_source": spot.hits.first().map(|h| h.source.clone()),
        "manifest_schema": manifest.schema,
        "manifest_immutable": manifest.immutable,
        "note": "Cold selective and broad searches clear the LRU before each query. broad_cached repeats one query. rows includes the fixture cards plus --n generated cards."
    }))
}

fn fixture_cards() -> Result<Vec<Card>, horbm_memory::Error> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut cards = Vec::new();
    let mut files = Vec::new();
    collect_json(&dir, &mut files).map_err(horbm_memory::Error::Io)?;
    for path in files {
        let raw = std::fs::read_to_string(&path)?;
        let value: serde_json::Value = serde_json::from_str(&raw)?;
        cards.extend(parse_cards(&value, &path.to_string_lossy())?);
    }
    Ok(cards)
}

fn collect_json(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

fn scale_card(i: usize) -> Card {
    let category = CATEGORIES[i % CATEGORIES.len()];
    let fact = i % 200 == 0;
    let raw = serde_json::json!({
        "id": format!("sample-{i}"),
        "title": format!("Sample {i} {category}"),
        "section": "world",
        "category": category,
        "summary": format!("Synthetic sample {i} about {category}. marker m{i}."),
        "body": format!("This generated card {i} exists so the benchmark has a corpus. Topic {category}. marker m{i}."),
        "claim_type": if fact { "fact" } else { "opinion" },
        "evidence_strength": if fact { "strong" } else { "none" },
        "sources": [{
            "title": "horbm-memory benchmark fixture",
            "url": "https://github.com/galion-studio/galion-knowledge",
            "author": "Galion Labs",
            "date": "2026-11-18"
        }],
        "said_by": "Galion Labs",
        "as_of_date": "2026-11-18",
        "status": "draft"
    });
    parse_cards(&raw, "bench/sample.json")
        .expect("sample card")
        .pop()
        .expect("one card")
}

fn selective_query(n: usize, i: usize) -> String {
    if n == 0 {
        "bm25".to_string()
    } else {
        format!("m{}", (i * 17) % n)
    }
}

fn broad_query(n: usize) -> String {
    if n == 0 {
        "the".to_string()
    } else {
        "sample".to_string()
    }
}

fn series_stats(samples: &mut [u64], example: &str) -> serde_json::Value {
    samples.sort_unstable();
    let sum: u64 = samples.iter().sum();
    let mean = if samples.is_empty() {
        0.0
    } else {
        sum as f64 / samples.len() as f64
    };
    let p95 = percentile(samples, 0.95);
    serde_json::json!({
        "example": example,
        "min_us": samples.first().copied().unwrap_or(0),
        "p50_us": percentile(samples, 0.50),
        "p95_us": p95,
        "p99_us": percentile(samples, 0.99),
        "max_us": samples.last().copied().unwrap_or(0),
        "mean_us": mean,
        "p95_within_5ms": p95 <= 5_000,
    })
}

fn sample_card(i: usize) -> Card {
    let topics = [
        "memory",
        "diary",
        "consent",
        "retrieval",
        "persona",
        "empathy",
    ];
    let topic = topics[i % topics.len()];
    let raw = serde_json::json!({
        "id": format!("sample-{i}"),
        "title": format!("Sample {i} {topic}"),
        "section": "world",
        "category": "science",
        "summary": format!("Synthetic sample {i} about {topic}. marker m{i}."),
        "body": format!("This generated card {i} exists so the benchmark has a corpus. Topic {topic}. marker m{i}."),
        "claim_type": "opinion",
        "sources": [{
            "title": "horbm-memory benchmark fixture",
            "url": "https://github.com/galion-studio/galion-knowledge",
            "author": "Galion Labs",
            "date": "2026-11-18"
        }],
        "evidence_strength": "none",
        "said_by": "Galion Labs",
        "as_of_date": "2026-11-18",
        "status": "draft"
    });
    parse_cards(&raw, "bench/sample.json")
        .expect("sample card")
        .pop()
        .expect("one card")
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((p * sorted.len() as f64).ceil() as usize).saturating_sub(1);
    sorted[rank.min(sorted.len() - 1)]
}

fn cpu_model() -> String {
    let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") else {
        return "unknown".into();
    };
    text.lines()
        .find_map(|line| {
            let (k, v) = line.split_once(':')?;
            if k.trim() == "model name" {
                Some(v.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".into())
}

fn take_knowledge(args: &mut Vec<String>) -> Result<PathBuf, String> {
    if let Some(pos) = args.iter().position(|a| a == "--knowledge") {
        let path = args
            .get(pos + 1)
            .cloned()
            .ok_or("--knowledge needs a directory")?;
        args.drain(pos..=pos + 1);
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from("horbm-knowledge"))
}

fn take_db(args: &mut Vec<String>) -> Result<PathBuf, String> {
    if let Some(pos) = args.iter().position(|a| a == "--db") {
        let path = args.get(pos + 1).cloned().ok_or("--db needs a path")?;
        args.drain(pos..=pos + 1);
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from("horbm-memory.sqlite"))
}

fn flag(args: &[String], name: &str) -> Result<String, String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
        .filter(|v| !v.starts_with("--"))
        .ok_or_else(|| format!("{name} is required"))
}

fn flag_or(args: &[String], name: &str, default: &str) -> String {
    flag(args, name).unwrap_or_else(|_| default.to_string())
}

fn print_help() {
    println!(
        "\
horbm-memory — HORBM memory and knowledge

  horbm-memory [--db PATH] [--knowledge DIR] import CARD_DIR
  horbm-memory [--db PATH] [--knowledge DIR] rebuild CARD_DIR
  horbm-memory [--db PATH] [--knowledge DIR] query WORDS...
  horbm-memory [--db PATH] [--knowledge DIR] gaps
  horbm-memory [--db PATH] [--knowledge DIR] revisions --id ID
  horbm-memory [--db PATH] [--knowledge DIR] card --id ID
  horbm-memory [--db PATH] ingest DIR
  horbm-memory [--db PATH] search QUERY...
  horbm-memory [--db PATH] remember --text TEXT --said-by WHO --date DATE --source SRC
  horbm-memory [--db PATH] diary add --text TEXT --date DATE [--said-by WHO] [--source SRC]
  horbm-memory [--db PATH] diary list
  horbm-memory [--db PATH] forget --id ID
  horbm-memory [--db PATH] forget --mark --id ID
  horbm-memory [--db PATH] keep-session --session ID --on|--off
  horbm-memory [--db PATH] summary [--session ID]
  horbm-memory [--db PATH] revise --id ID --text TEXT --said-by WHO --date DATE --source SRC
  horbm-memory [--db PATH] jobs [drain]
  horbm-memory [--db PATH] bench [--n N] [--queries N]
  horbm-memory scale-bench [--n N] [--queries N] [--dir PATH]
  horbm-memory stack-bench [--n N] [--queries N] [--knowledge DIR]

Default database: ./horbm-memory.sqlite
Default knowledge directory: ./horbm-knowledge
query returns at most 4 hits, split into knowledge and memory.
Memory hits include pinned and importance.
A knowledge miss is the answer \"I don't know\". The gap row is written
on the background queue. jobs drain waits for that queue.
forget --id and forget --mark are admin tools. forget --id archives a row.
The text stays and leaves retrieval. There is no withdrawal command.
keep-session --on writes a session summary and keeps the transcript searchable.
Card revisions and memory revisions are append-only. Search reads the latest text."
    );
}
