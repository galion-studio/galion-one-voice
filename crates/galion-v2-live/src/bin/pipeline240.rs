// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! v2-pipeline240: latency-240 lane 04 A/B of the full loop on the same
//! clips, Vosk sidecar, warm Piper and LLM as v2-latency.
//!
//! --mode baseline : the v2-latency full loop, unchanged (fixed 160 ms hang,
//!                   wait for ASR final, synthesize "Mm-hm." on the critical
//!                   path, then llm::stream = JSON + temp file + fresh curl,
//!                   HTTP/1.1 + TLS, first clause cap that rarely fires).
//! --mode fast     : two-stage endpointer (speculate at 60 ms of silence on
//!                   the partial transcript, commit at 160 ms / 500 ms if the
//!                   transcript sounds unfinished, cancel on resume), warm
//!                   pre-connected HTTP/2 curl with a prebuilt body, 3-word
//!                   first clause cap, cached floor-hold PCM in RAM.
//!                   Content audio may only play after commit: the reported
//!                   full-loop number is max(first PCM, commit).
//! No stub clocks: every time is a wall clock around a real engine.
use galion_v2_live::{asr::Asr, audio, endpoint::{norm, EndpointCfg, Endpointer, EpEv}, llm, now_ms,
    piper::{Piper, PiperPaths, PIPER_HZ}, stats::{r1, summary}, vad::{Vad, VadEv}, warm::{Prompt, Warm, WarmPool}};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SYSTEM: &str = "You are Samantha, a warm voice companion on a live call. Reply in one or two short spoken sentences. Start with a short reaction.";

fn true_eos_ms(x: &[i16]) -> f64 {
    let fr = 320; let mut last = 0;
    let peak = x.chunks(fr).map(audio::rms_db).fold(-120.0, f64::max);
    for (i, f) in x.chunks(fr).enumerate() { if audio::rms_db(f) > peak - 35.0 { last = i; } }
    (last + 1) as f64 * 20.0
}
fn ttfa_once(p: &mut Piper, text: &str) -> Vec<i16> {
    p.drain();
    let before = p.done.load(Ordering::SeqCst);
    p.say(text).unwrap();
    let mut v = p.first_pcm(10_000).expect("piper produced no PCM").pcm;
    for c in p.collect_until_done(before, 1, 15_000) { v.extend(c.pcm); }
    v
}
fn arg(args: &[String], k: &str) -> Option<String> { args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned() }

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let runs: usize = arg(&args, "--runs").and_then(|v| v.parse().ok()).unwrap_or(24);
    let mode = arg(&args, "--mode").unwrap_or_else(|| "fast".into());
    let out_dir = arg(&args, "--out").unwrap_or_else(|| format!("target/v2-pipeline240-{mode}"));
    let cap: usize = arg(&args, "--cap").and_then(|v| v.parse().ok()).unwrap_or(3);
    std::fs::create_dir_all(&out_dir).unwrap();
    let paths = PiperPaths::from_repo().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    let root = galion_v2_live::repo_root();
    let sdir = std::env::var("GALION_SAMPLES").unwrap_or_else(|_| root.join("../samples").display().to_string());
    let clips = ["voicezero-amy_koenig.wav", "voicezero-alan_davis_drake.wav", "armstrong-en.wav", "pdx-voice.wav"];
    let mut pcm16 = Vec::new();
    for c in clips {
        let w = audio::read_wav(&format!("{sdir}/{c}")).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
        pcm16.push((c, audio::resample(&audio::to_mono(&w), w.rate, 16_000)));
    }
    let mut cfg = llm::LlmCfg::from_env();
    cfg.max_tokens = 40;
    let mut asr = Asr::spawn().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    let mut p = Piper::spawn(&paths, 1.0, 0.667).unwrap();
    for _ in 0..3 { ttfa_once(&mut p, "Warming up."); }
    let report = match mode.as_str() {
        "baseline" => baseline(&mut asr, p, &cfg, &pcm16, runs, &out_dir),
        "fast" => fast(&mut asr, p, &cfg, &pcm16, runs, &out_dir, cap),
        m => { eprintln!("unknown --mode {m}"); std::process::exit(2) }
    };
    let path = format!("{out_dir}/pipeline240.json");
    std::fs::write(&path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    eprintln!("wrote {path}");
}

#[derive(Default)]
struct Series(std::collections::BTreeMap<&'static str, Vec<f64>>);
impl Series {
    fn push(&mut self, k: &'static str, v: f64) { self.0.entry(k).or_default().push(v); }
    fn json(&self) -> serde_json::Value { json!(self.0.iter().map(|(k, v)| (k.to_string(), summary(k, v))).collect::<serde_json::Map<_, _>>()) }
}

/// The v2-latency full loop, kept identical so the A/B runs in one session.
fn baseline(asr: &mut Asr, mut p: Piper, cfg: &llm::LlmCfg, pcm16: &[(&str, Vec<i16>)], runs: usize, out_dir: &str) -> serde_json::Value {
    let hang_ms = 160.0;
    let mut s = Series::default(); let mut log = String::new(); let mut cost_total = 0.0;
    for run in 0..runs {
        let (name, x) = &pcm16[run % pcm16.len()];
        let eos_rel = true_eos_ms(x);
        while asr.rx.try_recv().is_ok() {}
        let mut vad = Vad::new(hang_ms);
        let start = now_ms();
        let mut endpoint_at = None; let mut text = String::new();
        let mut frames: Vec<Vec<i16>> = x.chunks(320).map(|c| c.to_vec()).collect();
        for _ in 0..75 { frames.push(vec![0i16; 320]); }
        let mut asr_final_at = None;
        for (i, f) in frames.iter().enumerate() {
            let due = start + i as f64 * 20.0;
            while now_ms() < due { std::thread::sleep(Duration::from_micros(300)); }
            asr.feed(f).unwrap();
            let ev = vad.push(f, 20.0);
            if ev == VadEv::End && endpoint_at.is_none() && (i as f64 * 20.0) >= eos_rel - 40.0 { endpoint_at = Some(now_ms()); asr.finalize().unwrap(); break; }
        }
        let endpoint_at = endpoint_at.unwrap_or_else(|| { asr.finalize().unwrap(); now_ms() });
        let eos_abs = start + eos_rel;
        let deadline = now_ms() + 3000.0;
        while now_ms() < deadline {
            if let Ok(ev) = asr.rx.recv_timeout(Duration::from_millis(50)) {
                if ev.fin { if !ev.text.is_empty() { if !text.is_empty() { text.push(' '); } text.push_str(&ev.text); }
                            if asr_final_at.is_none() || ev.t_ms >= endpoint_at { asr_final_at = Some(ev.t_ms); }
                            if ev.t_ms >= endpoint_at { break; } }
            }
        }
        let asr_at = asr_final_at.unwrap_or(endpoint_at);
        let before_bc = p.done.load(Ordering::SeqCst);
        p.drain();
        let t_bc = p.say("Mm-hm.").unwrap();
        let bc = p.first_pcm(5000).unwrap();
        let _ = p.collect_until_done(before_bc, 1, 5000);
        let t_bc_done = now_ms();
        let msgs = json!([{"role":"system","content":SYSTEM}, {"role":"user","content": if text.is_empty() { "(unclear audio)".to_string() } else { text.clone() }}]);
        let t_llm = now_ms();
        let rx = llm::stream(cfg, msgs).unwrap();
        let t_spawned = now_ms();
        let mut cl = llm::Clauser::legacy(5);
        let (mut first_tok, mut first_clause, mut first_audio, mut said_first) = (None, None, None, None::<f64>);
        let mut reply = String::new(); let mut cost = None; let mut provider = String::new(); let mut spoken_first = String::new();
        for ev in rx.iter() {
            match ev {
                llm::LlmEv::Token(t, tok) => {
                    first_tok.get_or_insert(t); reply.push_str(&tok);
                    for c in cl.push(&tok) {
                        if first_clause.is_none() { first_clause = Some(now_ms()); spoken_first = c.clone(); p.drain(); said_first = Some(p.say(&c).unwrap()); first_audio = Some(p.first_pcm(5000).unwrap().t_ms); }
                    }
                }
                llm::LlmEv::Thought(..) => {}
                llm::LlmEv::Done(_, c, pr) => { cost = c; provider = pr; }
            }
        }
        if first_clause.is_none() { if let Some(c) = cl.flush() { first_clause = Some(now_ms()); spoken_first = c.clone(); p.drain(); said_first = Some(p.say(&c).unwrap()); first_audio = Some(p.first_pcm(5000).unwrap().t_ms); } }
        std::thread::sleep(Duration::from_millis(400)); p.drain();
        cost_total += cost.unwrap_or(0.0);
        let (Some(ft), Some(fc), Some(fa), Some(sf)) = (first_tok, first_clause, first_audio, said_first) else { eprintln!("run {run}: LLM returned nothing"); continue };
        let row = json!({"run": run, "clip": name, "asr": text, "reply": reply.trim(), "first_clause": spoken_first, "first_clause_words": spoken_first.split_whitespace().count(), "provider": provider, "cost_usd": cost,
            "eos_to_endpoint_ms": r1(endpoint_at - eos_abs), "asr_final_after_endpoint_ms": r1(asr_at - endpoint_at), "eos_to_asr_final_ms": r1(asr_at - eos_abs),
            "gap_asr_to_llm_request_ms": r1(t_llm - asr_at), "gap_bc_first_pcm_ms": r1(bc.t_ms - t_bc), "gap_bc_rest_of_synth_ms": r1(t_bc_done - bc.t_ms),
            "llm_spawn_ms": r1(t_spawned - t_llm), "llm_ttft_ms": r1(ft - t_llm), "tok_to_clause_ms": r1(fc - ft), "piper_ttfa_ms": r1(fa - sf),
            "eos_to_first_token_ms": r1(ft - eos_abs), "eos_to_first_clause_ms": r1(fc - eos_abs), "eos_to_first_content_audio_ms": r1(fa - eos_abs), "eos_to_backchannel_audio_ms": r1(bc.t_ms - eos_abs)});
        for k in ["eos_to_endpoint_ms", "asr_final_after_endpoint_ms", "eos_to_asr_final_ms", "gap_asr_to_llm_request_ms", "gap_bc_first_pcm_ms", "gap_bc_rest_of_synth_ms", "llm_spawn_ms", "llm_ttft_ms", "tok_to_clause_ms", "piper_ttfa_ms", "eos_to_first_token_ms", "eos_to_first_clause_ms", "eos_to_first_content_audio_ms", "eos_to_backchannel_audio_ms", "first_clause_words"] {
            s.push(k, row[k].as_f64().unwrap());
        }
        eprintln!("baseline {run}: {row}");
        log.push_str(&row.to_string()); log.push('\n');
    }
    std::fs::write(format!("{out_dir}/full-loop.ndjson"), &log).unwrap();
    json!({"mode": "baseline", "endpoint_hang_ms": hang_ms, "llm_model": cfg.model, "llm_provider_order": cfg.provider_order, "llm_cost_usd_total": cost_total, "runs": runs, "stages": s.json()})
}

#[derive(Default, Clone)]
struct RespOut { t_sent: f64, first_tok: Option<f64>, first_clause: Option<f64>, said: Option<f64>, first_pcm: Option<f64>, spoken_first: String, reply: String, cost: Option<f64>, provider: String }
struct Resp { cancel: Arc<AtomicBool>, committed: Arc<AtomicBool>, kill: Box<dyn Fn() + Send>, h: std::thread::JoinHandle<RespOut>, pub text: String, pub t_trigger: f64 }
impl Resp {
    /// Cancel without waiting (the thread ends once curl is dead); join it later, off the clock.
    fn abort_detached(self) -> std::thread::JoinHandle<RespOut> { self.cancel.store(true, Ordering::SeqCst); (self.kill)(); self.h }
}
/// LLM -> clauser -> warm Piper in its own thread so token/clause/PCM times are taken the moment they happen.
fn respond(w: Warm, body: String, text: String, t_trigger: f64, piper: Arc<Mutex<Piper>>, said_total: Arc<AtomicUsize>, cap: usize) -> Resp {
    let inflight = w.fire(&body).expect("warm fire");
    let kill: Box<dyn Fn() + Send> = Box::new(inflight.canceller());
    let cancel = Arc::new(AtomicBool::new(false));
    let committed = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone(); let k2 = committed.clone();
    let h = std::thread::spawn(move || {
        let mut o = RespOut { t_sent: inflight.t_sent, ..Default::default() };
        let mut cl = llm::Clauser::capped(cap);
        let speak = |c: String, o: &mut RespOut| {
            if o.first_clause.is_some() || c2.load(Ordering::SeqCst) { return; }
            o.first_clause = Some(now_ms()); o.spoken_first = c.clone();
            // speculative turns never reach the speaker before commit (no audio over a pausing user)
            while !k2.load(Ordering::SeqCst) { if c2.load(Ordering::SeqCst) { return; } std::thread::sleep(Duration::from_micros(500)); }
            let mut p = piper.lock().unwrap();
            // never mistake a cancelled turn's tail for our first PCM
            let need = said_total.load(Ordering::SeqCst);
            let t0 = now_ms();
            while p.done.load(Ordering::SeqCst) < need && now_ms() - t0 < 5000.0 { std::thread::sleep(Duration::from_millis(1)); }
            p.drain();
            o.said = Some(p.say(&c).unwrap()); said_total.fetch_add(1, Ordering::SeqCst);
            o.first_pcm = p.first_pcm(5000).map(|c| c.t_ms);
        };
        for ev in inflight.rx.iter() {
            match ev {
                llm::LlmEv::Token(t, tok) => {
                    if c2.load(Ordering::SeqCst) { continue; }
                    o.first_tok.get_or_insert(t); o.reply.push_str(&tok);
                    for c in cl.push(&tok) { speak(c, &mut o); }
                }
                llm::LlmEv::Thought(..) => {}
                llm::LlmEv::Done(_, c, pr) => { o.cost = c; o.provider = pr; }
            }
        }
        if o.first_clause.is_none() { if let Some(c) = cl.flush() { speak(c, &mut o); } }
        o
    });
    Resp { cancel, committed, kill, h, text, t_trigger }
}

fn fast(asr: &mut Asr, mut p: Piper, cfg: &llm::LlmCfg, pcm16: &[(&str, Vec<i16>)], runs: usize, out_dir: &str, cap: usize) -> serde_json::Value {
    let ep_cfg = EndpointCfg::from_env();
    // Force the ASR final at speculate time (faster check, but resets Vosk on every mid-turn pause).
    let fin_at_spec = std::env::var("GALION_EP_FINALIZE_AT_SPEC").map(|v| v == "1").unwrap_or(false);
    // Prepared line: floor-hold PCM synthesized once, played from RAM at commit (no synth on the turn).
    let t = now_ms(); let bc_pcm = ttfa_once(&mut p, "Mm-hm."); let bc_prep_ms = now_ms() - t;
    let piper = Arc::new(Mutex::new(p));
    let said_total = Arc::new(AtomicUsize::new(piper.lock().unwrap().done.load(Ordering::SeqCst)));
    let t = now_ms(); let prompt = Prompt::new(cfg, SYSTEM, 0.7); let prompt_build_us = (now_ms() - t) * 1000.0;
    let mut pool = WarmPool::new(cfg, 2).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    std::thread::sleep(Duration::from_millis(800)); // let the first spares finish TLS
    let mut s = Series::default(); let mut log = String::new(); let mut cost_total = 0.0;
    let (mut n_spec, mut n_false_spec, mut n_false_commit, mut n_restart, mut n_hit, mut n_turns) = (0, 0, 0, 0, 0, 0);
    let mut body_us = vec![];
    for run in 0..runs {
        let (name, x) = &pcm16[run % pcm16.len()];
        let eos_rel = true_eos_ms(x);
        pool.refill().unwrap();
        while asr.rx.try_recv().is_ok() {}
        let mut ep = Endpointer::new(ep_cfg);
        let (mut finals, mut partial) = (String::new(), String::new());
        let start = now_ms();
        let mut frames: Vec<Vec<i16>> = x.chunks(320).map(|c| c.to_vec()).collect();
        for _ in 0..75 { frames.push(vec![0i16; 320]); }
        let guard = eos_rel - 40.0;
        let mut resp: Option<Resp> = None;
        let mut stale: Vec<std::thread::JoinHandle<RespOut>> = vec![]; // aborted turns, joined off the clock
        let (mut spec_at, mut commit_at, mut asr_at) = (None::<f64>, None::<f64>, None::<f64>);
        let mut awaiting_final = false;
        let mut final_checked = false;
        let mut ignore_final = false; // a hold-check final whose turn then resumed
        let mut run_hold = 0;
        let mut spec_text_first: Option<String> = None;
        let mut spec_hit = false;
        let (mut run_false_spec, mut run_false_commit, mut run_restart, mut run_spec) = (0, 0, 0, 0);
        macro_rules! fire { ($text:expr, $t:expr) => {{
            let tb = now_ms(); let body = prompt.body(&$text); body_us.push((now_ms() - tb) * 1000.0);
            let r = respond(pool.take().unwrap(), body, $text.clone(), $t, piper.clone(), said_total.clone(), cap);
            pool.refill().unwrap(); r }} }
        // Final text arrived after a forced finalize: keep the speculative call if it matches, else restart now.
        macro_rules! on_final { ($t:expr) => {{
            awaiting_final = false; final_checked = true;
            let ft = if finals.is_empty() { "(unclear audio)".to_string() } else { finals.clone() };
            let same = resp.as_ref().map(|r| norm(&r.text) == norm(&ft)).unwrap_or(false);
            spec_hit = same;
            if !same {
                let committed = resp.as_ref().map(|r| r.committed.load(Ordering::SeqCst)).unwrap_or(commit_at.is_some());
                if let Some(r) = resp.take() { stale.push(r.abort_detached()); run_restart += 1; }
                let r = fire!(ft, $t);
                if committed { r.committed.store(true, Ordering::SeqCst); }
                resp = Some(r);
            }
        }} }
        for (i, f) in frames.iter().enumerate() {
            let due = start + i as f64 * 20.0;
            while now_ms() < due { std::thread::sleep(Duration::from_micros(300)); }
            asr.feed(f).unwrap();
            while let Ok(ev) = asr.rx.try_recv() {
                if ev.fin {
                    if !ev.text.is_empty() { if !finals.is_empty() { finals.push(' '); } finals.push_str(&ev.text); } partial.clear();
                    if awaiting_final && ignore_final { awaiting_final = false; ignore_final = false; }
                    else if awaiting_final { asr_at = Some(ev.t_ms); on_final!(now_ms()); }
                } else if !ev.sync { partial = ev.text; }
            }
            let tr = if partial.is_empty() { finals.clone() } else if finals.is_empty() { partial.clone() } else { format!("{finals} {partial}") };
            let t_rel = i as f64 * 20.0;
            match ep.push(f, 20.0, &tr) {
                EpEv::Speculate => {
                    // start the LLM on the partial right now, and force the ASR final in parallel to check it
                    let t_trig = now_ms();
                    if let Some(r) = resp.take() { stale.push(r.abort_detached()); }
                    resp = Some(fire!(tr, t_trig));
                    if fin_at_spec { asr.finalize().unwrap(); awaiting_final = true; }
                    run_spec += 1; spec_at = Some(t_trig); spec_text_first = Some(tr.clone());
                    if t_rel < guard { run_false_spec += 1; }
                }
                EpEv::Resume | EpEv::Start => { if let Some(r) = resp.take() { stale.push(r.abort_detached()); } spec_at = None; final_checked = false; if awaiting_final { ignore_final = true; } }
                EpEv::Hold => { if !awaiting_final { asr.finalize().unwrap(); awaiting_final = true; ignore_final = false; } run_hold += 1; }
                EpEv::Commit => {
                    if t_rel < guard { run_false_commit += 1; if let Some(r) = resp.take() { stale.push(r.abort_detached()); } spec_at = None; continue; }
                    commit_at = Some(now_ms());
                    if let Some(r) = &resp { r.committed.store(true, Ordering::SeqCst); }
                    break;
                }
                _ => {}
            }
        }
        let commit_at = commit_at.unwrap_or_else(now_ms);
        if !awaiting_final && !final_checked { asr.finalize().unwrap(); awaiting_final = true; }
        let eos_abs = start + eos_rel;
        let deadline = now_ms() + 3000.0;
        while awaiting_final && now_ms() < deadline {
            if let Ok(ev) = asr.rx.recv_timeout(Duration::from_millis(5)) {
                if ev.fin { if !ev.text.is_empty() { if !finals.is_empty() { finals.push(' '); } finals.push_str(&ev.text); } asr_at = Some(ev.t_ms); on_final!(now_ms()); }
            }
        }
        if let Some(r) = &resp { r.committed.store(true, Ordering::SeqCst); }
        let final_text = if finals.is_empty() { "(unclear audio)".to_string() } else { finals.clone() };
        let asr_at = asr_at.unwrap_or(commit_at);
        if spec_hit { n_hit += 1; }
        n_turns += 1; n_false_spec += run_false_spec; n_false_commit += run_false_commit; n_restart += run_restart; n_spec += run_spec;
        let r = resp.take().unwrap();
        let (trig, spec_text) = (r.t_trigger, r.text.clone());
        let spec_text_at_commit = spec_text_first.clone();
        let hit = spec_hit;
        let o = r.h.join().unwrap();
        std::thread::sleep(Duration::from_millis(400));
        let n_stale = stale.len();
        for h in stale { let _ = h.join(); }
        piper.lock().unwrap().drain();
        let wasted = 0.0; // killed calls report no usage; estimated below from the count
        let _ = n_stale;
        cost_total += o.cost.unwrap_or(0.0) + wasted;
        let (Some(ft), Some(fc), Some(fa), Some(sf)) = (o.first_tok, o.first_clause, o.first_pcm, o.said) else { eprintln!("run {run}: LLM returned nothing"); continue };
        let spec_rel = spec_at.map(|t| t - eos_abs);
        let gated = fa.max(commit_at);
        let row = json!({"run": run, "clip": name, "asr": final_text, "llm_input": spec_text, "spec_text_at_commit": spec_text_at_commit, "spec_hit": hit, "reply": o.reply.trim(), "first_clause": o.spoken_first,
            "first_clause_words": o.spoken_first.split_whitespace().count(), "provider": o.provider, "cost_usd": o.cost, "wasted_usd": wasted,
            "false_spec_this_turn": run_false_spec, "false_commit_this_turn": run_false_commit, "restarts_this_turn": run_restart, "hold_checks_this_turn": run_hold, "llm_calls_started": run_spec + run_restart + if run_spec == 0 { 1 } else { 0 },
            "eos_to_llm_trigger_ms": r1(trig - eos_abs), "eos_to_spec_ms": spec_rel.map(r1), "eos_to_commit_ms": r1(commit_at - eos_abs),
            "eos_to_asr_final_ms": r1(asr_at - eos_abs), "asr_final_after_spec_ms": spec_at.map(|t| r1(asr_at - t)),
            "gap_trigger_to_llm_request_ms": r1(o.t_sent - trig), "llm_ttft_ms": r1(ft - o.t_sent), "tok_to_clause_ms": r1(fc - ft), "piper_ttfa_ms": r1(fa - sf),
            "eos_to_first_token_ms": r1(ft - eos_abs), "eos_to_first_clause_ms": r1(fc - eos_abs),
            "eos_to_first_pcm_ready_ms": r1(fa - eos_abs), "eos_to_first_content_audio_ms": r1(gated - eos_abs), "commit_gate_bound": commit_at > fa,
            "eos_to_backchannel_audio_ms": r1(commit_at - eos_abs)});
        for k in ["eos_to_llm_trigger_ms", "eos_to_commit_ms", "eos_to_asr_final_ms", "gap_trigger_to_llm_request_ms", "llm_ttft_ms", "tok_to_clause_ms", "piper_ttfa_ms",
                  "eos_to_first_token_ms", "eos_to_first_clause_ms", "eos_to_first_pcm_ready_ms", "eos_to_first_content_audio_ms", "eos_to_backchannel_audio_ms", "first_clause_words"] {
            s.push(k, row[k].as_f64().unwrap());
        }
        eprintln!("fast {run}: {row}");
        log.push_str(&row.to_string()); log.push('\n');
    }
    std::fs::write(format!("{out_dir}/full-loop.ndjson"), &log).unwrap();
    json!({"mode": "fast", "finalize_at_spec": fin_at_spec, "endpointer": {"spec_ms": ep_cfg.spec_ms, "commit_ms": ep_cfg.commit_ms, "hold_ms": ep_cfg.hold_ms}, "first_clause_cap_words": cap,
        "llm_model": cfg.model, "llm_provider_order": cfg.provider_order, "llm_cost_usd_total": cost_total, "runs": runs, "turns": n_turns,
        "speculative_starts": n_spec, "spec_hit_turns": n_hit, "restarts_on_final": n_restart, "false_spec_events": n_false_spec, "false_commit_events": n_false_commit,
        "cold_pool_takes": pool.cold_takes, "prompt_build_us": r1(prompt_build_us), "body_build_us": summary("body build us", &body_us),
        "floor_hold_line": {"text": "Mm-hm.", "samples": bc_pcm.len(), "audio_ms": r1(bc_pcm.len() as f64 * 1000.0 / PIPER_HZ as f64), "prepared_once_ms": r1(bc_prep_ms), "turn_synth_ms": 0},
        "stages": s.json()})
}
