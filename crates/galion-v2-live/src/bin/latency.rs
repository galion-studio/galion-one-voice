// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! v2-latency: Gate 1. Real CPU Piper (A21) TTFA cold/warm, chunking and
//! core-count effects, and the full loop on real CC0 speech:
//! end of speech → VAD/endpoint → streaming Vosk final → real LLM stream
//! (first clause) → warm Piper first PCM. No stub clocks anywhere.
use galion_v2_live::{audio, asr::Asr, llm, now_ms, piper::{Piper, PiperPaths, PIPER_HZ}, stats::{summary, r1}, vad::{Vad, VadEv}, warm::{Prompt, WarmPool}};
use serde_json::json;
use std::time::Duration;

fn cpu_spec() -> serde_json::Value {
    let ci = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = ci.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split(':').nth(1)).unwrap_or("?").trim().to_string();
    let flags = ci.lines().find(|l| l.starts_with("flags")).unwrap_or("");
    let mem = std::fs::read_to_string("/proc/meminfo").unwrap_or_default().lines().next().unwrap_or("").to_string();
    json!({"model": model, "logical_cpus": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
           "avx2": flags.contains(" avx2"), "avx512f": flags.contains(" avx512f"), "mem": mem.split_whitespace().nth(1).unwrap_or("?").to_string() + " kB",
           "kernel": std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().trim()})
}

fn ttfa_once(p: &mut Piper, text: &str) -> (f64, f64) {
    p.drain();
    let before = p.done.load(std::sync::atomic::Ordering::SeqCst);
    let t = p.say(text).unwrap();
    let first = p.first_pcm(10_000).expect("piper produced no PCM");
    let rest = p.collect_until_done(before, 1, 15_000);
    let n: usize = first.pcm.len() + rest.iter().map(|c| c.pcm.len()).sum::<usize>();
    (first.t_ms - t, n as f64 * 1000.0 / PIPER_HZ as f64)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = args.iter().position(|a| a == "--runs").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(20);
    let do_loop = !args.iter().any(|a| a == "--no-loop");
    let only_loop = args.iter().any(|a| a == "--only-loop");
    let out_dir = args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| "target/v2-latency".into());
    std::fs::create_dir_all(&out_dir).unwrap();
    let paths = PiperPaths::from_repo().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    let mut report = json!({"cpu": cpu_spec(), "piper": {"bin": "rhasspy piper 2023.11.14-2 (A21 pin)", "voice": "en_US-ljspeech-medium", "rate_hz": PIPER_HZ}, "runs": runs});

    if only_loop {
        report["full_loop"] = full_loop(&paths, runs, &out_dir);
        let path = format!("{out_dir}/latency.json");
        std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        eprintln!("wrote {path}");
        return;
    }
    // 1. Cold: fresh process each run (spawn → model load → first PCM).
    let mut cold_total = Vec::new(); let mut cold_first_req = Vec::new();
    for _ in 0..runs {
        let mut p = Piper::spawn(&paths, 1.0, 0.667).unwrap();
        let spawned = p.spawned_ms;
        let t = p.say("Hello.").unwrap();
        let c = p.first_pcm(20_000).expect("cold piper produced no PCM");
        cold_total.push(c.t_ms - spawned);
        cold_first_req.push(c.t_ms - t);
    }
    report["cold_spawn_to_first_pcm_ms"] = summary("cold spawn→first PCM", &cold_total);
    eprintln!("cold: {}", report["cold_spawn_to_first_pcm_ms"]);

    // 2. Warm: one process, warmed with 3 throwaway clauses, then N runs per clause length.
    let clauses = [("1w", "Yes."), ("2w", "Hi there."), ("4w", "I hear you, Mate."), ("8w", "That sounds hard, tell me what happened next."),
                   ("20w", "I was thinking about what you said this morning, and I think you were right about the plan for the weekend trip.")];
    let mut warm = serde_json::Map::new();
    let mut p = Piper::spawn(&paths, 1.0, 0.667).unwrap();
    for _ in 0..3 { ttfa_once(&mut p, "Warming up the voice."); }
    for (k, text) in clauses {
        let mut v = Vec::new(); let mut aud = 0.0;
        for _ in 0..runs { let (ttfa, a) = ttfa_once(&mut p, text); v.push(ttfa); aud = a; }
        let mut s = summary(&format!("warm TTFA {k}"), &v); s["audio_ms"] = json!(r1(aud)); s["text"] = json!(text);
        eprintln!("warm {k}: {s}");
        warm.insert(k.into(), s);
    }
    report["warm_ttfa_ms"] = json!(warm);
    drop(p);

    // 3. Core-count lever (ONNX intra-op threads are not exposed by the piper CLI; pin cores with taskset).
    let mut cores = serde_json::Map::new();
    let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    for c in [1usize, 2, 4, ncpu] {
        if c > ncpu { continue; }
        let tp = std::env::temp_dir().join(format!("piper-taskset-{c}.sh"));
        std::fs::write(&tp, format!("#!/bin/sh\nexec taskset -c 0-{} {} \"$@\"\n", c - 1, paths.bin)).unwrap();
        let _ = std::process::Command::new("chmod").arg("+x").arg(&tp).status();
        let pp = PiperPaths { bin: tp.display().to_string(), model: paths.model.clone() };
        let mut p = Piper::spawn(&pp, 1.0, 0.667).unwrap();
        for _ in 0..3 { ttfa_once(&mut p, "Warming up the voice."); }
        let mut v4 = Vec::new(); let mut v20 = Vec::new();
        for _ in 0..runs { v4.push(ttfa_once(&mut p, clauses[2].1).0); }
        for _ in 0..(runs / 2).max(5) { v20.push(ttfa_once(&mut p, clauses[4].1).0); }
        cores.insert(format!("{c}_cores"), json!({"4w": summary("4w", &v4), "20w": summary("20w", &v20)}));
        eprintln!("cores {c}: 4w p50 {} 20w p50 {}", r1(galion_v2_live::stats::pct(&v4, 50.0)), r1(galion_v2_live::stats::pct(&v20, 50.0)));
    }
    report["core_pinning"] = json!(cores);

    if do_loop { report["full_loop"] = full_loop(&paths, runs, &out_dir); }
    let path = format!("{out_dir}/latency.json");
    std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    eprintln!("wrote {path}");
}

/// Last voiced 20 ms frame (true end of speech) in a 16 kHz clip, in ms.
fn true_eos_ms(x: &[i16]) -> f64 {
    let fr = 320; let mut last = 0;
    let peak = x.chunks(fr).map(audio::rms_db).fold(-120.0, f64::max);
    for (i, f) in x.chunks(fr).enumerate() { if audio::rms_db(f) > peak - 35.0 { last = i; } }
    (last + 1) as f64 * 20.0
}

fn full_loop(paths: &PiperPaths, runs: usize, out_dir: &str) -> serde_json::Value {
    let root = galion_v2_live::repo_root();
    let sdir = std::env::var("GALION_SAMPLES").unwrap_or_else(|_| root.join("../samples").display().to_string());
    let clips = ["voicezero-amy_koenig.wav", "voicezero-alan_davis_drake.wav", "armstrong-en.wav", "pdx-voice.wav"];
    let mut pcm16 = Vec::new();
    for c in clips {
        let w = audio::read_wav(&format!("{sdir}/{c}")).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
        let m = audio::resample(&audio::to_mono(&w), w.rate, 16_000);
        pcm16.push((c, m));
    }
    let hang_ms: f64 = std::env::var("GALION_ENDPOINT_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(160.0);
    let mut cfg = llm::LlmCfg::from_env();
    cfg.max_tokens = 40;
    let mut asr = Asr::spawn().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    let mut p = Piper::spawn(paths, 1.0, 0.667).unwrap();
    for _ in 0..3 { ttfa_once(&mut p, "Warming up."); }
    // latency-240: one pre-connected HTTP/2 curl (TLS done, headers sent, waiting for
    // the body) replaces the per-turn curl spawn + TCP + TLS. GALION_WARM=0 = old path.
    const SYSTEM: &str = "You are Samantha, a warm voice companion on a live call. Reply in one or two short spoken sentences. Start with a short reaction.";
    let use_warm = std::env::var("GALION_WARM").map(|v| v != "0").unwrap_or(true);
    let prompt = Prompt::new(&cfg, SYSTEM, 0.7);
    let mut pool = if use_warm { Some(WarmPool::new(&cfg, 1).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) })) } else { None };
    let mut bc_wait = Vec::new();
    // Backchannel lane: a cached Piper "Mm-hm." is queued at endpoint; its first audio is the floor hold.
    let mut rows = Vec::new();
    let (mut eos_to_endpoint, mut eos_to_asr, mut eos_to_tok, mut eos_to_clause, mut eos_to_audio, mut llm_ttft, mut piper_ttfa, mut eos_to_bc) =
        (vec![], vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
    let mut cost_total = 0.0;
    let mut log = String::new();
    for run in 0..runs {
        let (name, x) = &pcm16[run % pcm16.len()];
        let eos_rel = true_eos_ms(x);
        while asr.rx.try_recv().is_ok() {}
        let mut vad = Vad::new(hang_ms);
        if let Some(pl) = pool.as_mut() { pl.refill().unwrap(); }
        let start = now_ms();
        let mut endpoint_at = None; let mut text = String::new();
        // feed in real time: 20 ms frames, then 1.5 s of trailing room silence
        let mut frames: Vec<Vec<i16>> = x.chunks(320).map(|c| c.to_vec()).collect();
        for _ in 0..75 { frames.push(vec![0i16; 320]); }
        let mut asr_final_at = None;
        for (i, f) in frames.iter().enumerate() {
            let due = start + i as f64 * 20.0;
            while now_ms() < due { std::thread::sleep(Duration::from_micros(300)); }
            asr.feed(f).unwrap();
            let ev = vad.push(f, 20.0);
            if ev == VadEv::End && endpoint_at.is_none() && (i as f64 * 20.0) >= eos_rel - 40.0 {
                endpoint_at = Some(now_ms());
                asr.finalize().unwrap();
                break;
            }
        }
        let endpoint_at = endpoint_at.unwrap_or_else(|| { asr.finalize().unwrap(); now_ms() });
        let eos_abs = start + eos_rel;
        // ASR: take the forced final (or a vosk final that arrived earlier)
        let deadline = now_ms() + 3000.0;
        while now_ms() < deadline {
            if let Ok(ev) = asr.rx.recv_timeout(Duration::from_millis(50)) {
                if ev.fin { if !ev.text.is_empty() { if !text.is_empty() { text.push(' '); } text.push_str(&ev.text); }
                            if asr_final_at.is_none() || ev.t_ms >= endpoint_at { asr_final_at = Some(ev.t_ms); }
                            if ev.t_ms >= endpoint_at { break; } }
            }
        }
        let asr_at = asr_final_at.unwrap_or(endpoint_at);
        // LLM first, at ASR final (latency-240 fix: this used to wait for the
        // whole "Mm-hm." synth + a 30 ms idle poll, ~68 ms p50 on the turn).
        let user = if text.is_empty() { "(unclear audio)".to_string() } else { text.clone() };
        let t_llm = now_ms();
        let rx = match pool.as_mut() {
            Some(pl) => pl.take().unwrap().fire(&prompt.body(&user)).unwrap().rx,
            None => llm::stream(&cfg, json!([{"role":"system","content": SYSTEM}, {"role":"user","content": user}])).unwrap(),
        };
        // backchannel (floor hold) runs while the LLM request is in flight — real Piper on the same warm process
        let before_bc = p.done.load(std::sync::atomic::Ordering::SeqCst);
        p.drain();
        let _tbc = p.say("Mm-hm.").unwrap();
        let bc = p.first_pcm(5000).unwrap();
        // Backchannel is streamed (its first PCM is the floor hold); no 30 ms idle poll.
        // Piper is one process, so the first clause only waits until "Mm-hm." is done, if it isn't yet.
        let bc_done = |p: &Piper| p.done.load(std::sync::atomic::Ordering::SeqCst) > before_bc;
        // first clause capped at 3 words (2-4 word budget); the cap now fires on leading-space tokens
        let mut cl = llm::Clauser::new(std::env::var("GALION_FIRST_CLAUSE_WORDS").ok().and_then(|v| v.parse().ok()).unwrap_or(3));
        let (mut first_tok, mut first_clause, mut first_audio, mut said_first) = (None, None, None, None::<f64>);
        let mut reply = String::new(); let mut cost = None; let mut provider = String::new();
        let mut spoken_first = String::new();
        for ev in rx.iter() {
            match ev {
                llm::LlmEv::Token(t, s) => {
                    first_tok.get_or_insert(t); reply.push_str(&s);
                    for c in cl.push(&s) {
                        if first_clause.is_none() {
                            first_clause = Some(now_ms()); spoken_first = c.clone();
                            let w0 = now_ms(); let dl = w0 + 5000.0;
                            while !bc_done(&p) && now_ms() < dl { std::thread::sleep(Duration::from_micros(500)); }
                            bc_wait.push(now_ms() - w0);
                            p.drain();
                            said_first = Some(p.say(&c).unwrap());
                            let ch = p.first_pcm(5000).unwrap();
                            first_audio = Some(ch.t_ms);
                        }
                    }
                }
                llm::LlmEv::Thought(..) => {}
                llm::LlmEv::Done(_, c, pr) => { cost = c; provider = pr; }
            }
        }
        if first_clause.is_none() { if let Some(c) = cl.flush() { first_clause = Some(now_ms()); spoken_first = c.clone();
            while !bc_done(&p) { std::thread::sleep(Duration::from_micros(500)); } p.drain(); said_first = Some(p.say(&c).unwrap()); first_audio = Some(p.first_pcm(5000).unwrap().t_ms); } }
        std::thread::sleep(Duration::from_millis(400)); p.drain();
        cost_total += cost.unwrap_or(0.0);
        let (Some(ft), Some(fc), Some(fa), Some(sf)) = (first_tok, first_clause, first_audio, said_first) else { eprintln!("run {run}: LLM returned nothing"); continue };
        eos_to_endpoint.push(endpoint_at - eos_abs); eos_to_asr.push(asr_at - eos_abs); eos_to_tok.push(ft - eos_abs);
        eos_to_clause.push(fc - eos_abs); eos_to_audio.push(fa - eos_abs); llm_ttft.push(ft - t_llm); piper_ttfa.push(fa - sf); eos_to_bc.push(bc.t_ms - eos_abs);
        let row = json!({"run": run, "clip": name, "asr": text, "reply": reply.trim(), "first_clause": spoken_first, "provider": provider, "cost_usd": cost,
            "eos_to_endpoint_ms": r1(endpoint_at - eos_abs), "eos_to_asr_final_ms": r1(asr_at - eos_abs), "llm_ttft_ms": r1(ft - t_llm), "warm_http2": use_warm, "gap_asr_to_llm_request_ms": r1(t_llm - asr_at), "first_clause_words": spoken_first.split_whitespace().count(),
            "eos_to_first_token_ms": r1(ft - eos_abs), "eos_to_first_clause_ms": r1(fc - eos_abs), "piper_ttfa_ms": r1(fa - sf),
            "eos_to_first_content_audio_ms": r1(fa - eos_abs), "eos_to_backchannel_audio_ms": r1(bc.t_ms - eos_abs)});
        eprintln!("loop {run}: {row}");
        log.push_str(&row.to_string()); log.push('\n');
    }
    std::fs::write(format!("{out_dir}/full-loop.ndjson"), &log).unwrap();
    rows.push(0);
    json!({"endpoint_hang_ms": hang_ms, "warm_http2": use_warm, "bc_wait_before_first_clause": summary("bc wait", &bc_wait), "llm_model": cfg.model, "llm_provider_order": cfg.provider_order, "llm_cost_usd_total": cost_total,
        "eos_to_endpoint": summary("eos→endpoint", &eos_to_endpoint), "eos_to_asr_final": summary("eos→asr final", &eos_to_asr),
        "llm_ttft": summary("llm ttft", &llm_ttft), "eos_to_first_token": summary("eos→first token", &eos_to_tok),
        "eos_to_first_clause": summary("eos→first clause", &eos_to_clause), "piper_ttfa_in_loop": summary("piper ttfa in loop", &piper_ttfa),
        "eos_to_first_content_audio": summary("eos→first content audio (FULL LOOP)", &eos_to_audio),
        "eos_to_backchannel_audio": summary("eos→backchannel audio", &eos_to_bc)})
}
