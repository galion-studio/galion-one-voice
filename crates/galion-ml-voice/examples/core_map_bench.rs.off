// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Core-map A/B: Piper clause latency alone and while the local LLM decodes.
//!
//! Spawns one warm [`PiperWorker`] (it pins itself from the core map in the
//! env) and talks to an already-running loopback llama-server
//! (`BENCH_LLM_URL`, e.g. `http://127.0.0.1:18080`) that was started through
//! `galion-pin llm`. Per rep:
//! 1. Piper alone: the clause, first-PCM and total ms.
//! 2. LLM alone: `/completion` with a fixed 48-token decode (`ignore_eos`).
//! 3. Both: the same decode, and after `BENCH_OVERLAP_MS` (default 350; the
//!    ~200 ms prompt eval is done, so Piper overlaps the decode, as in a turn) Piper speaks the clause.
//!
//! Prints one JSON line per clause. No PCM is written. Bench only, not shipped.
//!
//! ```text
//! cargo run --release -p galion-ml-voice --features speech --example core_map_bench
//! ```

use galion_ml_voice::speech::core_map::{self, CoreRole};
use galion_ml_voice::speech::piper_worker::{PiperProsody, PiperWorker};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const CLAUSES: &[(&str, &str)] = &[
    ("4w", "That sounds really hard."),
    ("12w", "You said your mother called this morning, and you are still shaken by it."),
];

fn env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.trim().is_empty())
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i]
}

fn num_after(body: &str, key: &str) -> Option<f64> {
    let i = body.find(key)? + key.len();
    let rest = body[i..].trim_start_matches([':', ' ', '"']);
    let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// One fixed-length decode. Returns (wall ms, prompt ms, decode tok/s).
fn llm_once(hostport: &str, seed: u32) -> Result<(f64, f64, f64), String> {
    let body = format!(
        "{{\"prompt\":\"<|im_start|>user\\nMy mother called this morning and I am still shaken. Talk to me for a bit. ({seed})<|im_end|>\\n<|im_start|>assistant\\n\",\"n_predict\":48,\"ignore_eos\":true,\"temperature\":0.7,\"seed\":{seed},\"cache_prompt\":false,\"stream\":false}}"
    );
    let t0 = Instant::now();
    let mut s = TcpStream::connect(hostport).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(60))).ok();
    write!(
        s,
        "POST /completion HTTP/1.0\r\nHost: {hostport}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| format!("write: {e}"))?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| format!("read: {e}"))?;
    let wall = t0.elapsed().as_secs_f64() * 1000.0;
    let prompt = num_after(&out, "\"prompt_ms\"").ok_or_else(|| format!("no timings: {}", &out[..out.len().min(200)]))?;
    let tps = num_after(&out, "\"predicted_per_second\"").ok_or("no predicted_per_second")?;
    Ok((wall, prompt, tps))
}

fn speak(w: &mut PiperWorker, text: &str) -> (f64, f64) {
    let t0 = Instant::now();
    let mut first = None;
    w.synthesize_stream(text, &mut |_pcm| {
        if first.is_none() {
            first = Some(t0.elapsed().as_secs_f64() * 1000.0);
        }
    })
    .expect("piper speak");
    (first.unwrap_or(f64::NAN), t0.elapsed().as_secs_f64() * 1000.0)
}

fn cpus_of(pid: u32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()
        .and_then(|s| s.lines().find(|l| l.starts_with("Cpus_allowed_list:")).map(|l| l[18..].trim().to_string()))
        .unwrap_or_default()
}

fn main() {
    let reps: usize = env("BENCH_REPS").and_then(|v| v.parse().ok()).unwrap_or(12);
    let overlap = env("BENCH_OVERLAP_MS").and_then(|v| v.parse().ok()).unwrap_or(350u64);
    let label = env("BENCH_LABEL").unwrap_or_else(|| "unlabelled".into());
    let llm = env("BENCH_LLM_URL").map(|u| u.trim_start_matches("http://").trim_end_matches('/').to_string());
    if let Some(h) = &llm {
        assert!(h.starts_with("127.0.0.1:") || h.starts_with("localhost:"), "loopback only");
    }
    let runtime = galion_ml_voice::speech::weights::piper_runtime().expect("Piper runtime (GALION_A21_RUNTIME)");
    let model = PathBuf::from(env("GALION_TTS_WEIGHTS").expect("GALION_TTS_WEIGHTS=<voice>.onnx"));
    let config = PathBuf::from(format!("{}.json", model.display()));
    let placed = core_map::placement(CoreRole::Piper);
    let mut w = PiperWorker::spawn(&runtime, &model, &config, &PiperProsody::from_env()).expect("spawn piper");
    for _ in 0..3 {
        for (_, c) in CLAUSES {
            speak(&mut w, c);
        }
    }
    let piper_cpus = cpus_of(w.pid());
    for (tag, clause) in CLAUSES {
        let (mut a_first, mut a_total, mut b_first, mut b_total) = (vec![], vec![], vec![], vec![]);
        let (mut l_alone, mut l_both, mut l_prompt) = (vec![], vec![], vec![]);
        for r in 0..reps {
            let (f, t) = speak(&mut w, clause);
            a_first.push(f);
            a_total.push(t);
            let Some(h) = llm.clone() else { continue };
            let (_, p, tps) = llm_once(&h, 1000 + r as u32).expect("llm alone");
            l_alone.push(tps);
            l_prompt.push(p);
            let h2 = h.clone();
            let seed = 2000 + r as u32;
            let job = std::thread::spawn(move || llm_once(&h2, seed));
            std::thread::sleep(Duration::from_millis(overlap));
            let (f, t) = speak(&mut w, clause);
            b_first.push(f);
            b_total.push(t);
            let (_, _, tps) = job.join().unwrap().expect("llm both");
            l_both.push(tps);
        }
        println!(
            "{{\"label\":\"{label}\",\"map\":\"{}\",\"piper_cpus\":\"{piper_cpus}\",\"piper_threads\":{},\"clause\":\"{tag}\",\"reps\":{reps},\
\"piper_alone_first_p50\":{:.1},\"piper_alone_first_p95\":{:.1},\"piper_alone_total_p50\":{:.1},\
\"piper_with_llm_first_p50\":{:.1},\"piper_with_llm_first_p95\":{:.1},\"piper_with_llm_total_p50\":{:.1},\
\"llm_alone_tps_p50\":{:.1},\"llm_with_piper_tps_p50\":{:.1},\"llm_with_piper_tps_min\":{:.1},\"llm_prompt_ms_p50\":{:.1}}}",
            placed.map.as_str(),
            std::env::var("GALION_PIPER_THREADS").ok().and_then(|t| t.parse::<u32>().ok()).unwrap_or(placed.threads),
            pct(&mut a_first, 0.5), pct(&mut a_first, 0.95), pct(&mut a_total, 0.5),
            pct(&mut b_first, 0.5), pct(&mut b_first, 0.95), pct(&mut b_total, 0.5),
            pct(&mut l_alone, 0.5), pct(&mut l_both.clone(), 0.5), pct(&mut l_both, 0.0), pct(&mut l_prompt, 0.5),
        );
    }
}
