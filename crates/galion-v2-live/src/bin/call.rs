// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! v2-call: a LIVE dual-band duplex call between two personas.
//!
//! Band L = Samantha (cloned from the CC0 Amy Koenig donor), band R = Jarvis
//! (cloned from the CC0 Alan Davis Drake donor). Each persona only *hears*
//! the other band: real streaming Vosk ASR + energy VAD on that band. Replies
//! are real LLM streams cut into clauses and voiced by the clone worker (or
//! warm Piper with `--lane piper`). A 20 ms wall-clock tick drives both bands
//! at once, so overlap, backchannels and barge-in happen on the line, not in
//! a script. Output: stereo WAV + NDJSON timing log + summary JSON.
use galion_v2_live::{asr::Asr, audio, clone::CloneWorker, llm, now_ms, piper::{Piper, PiperPaths, PIPER_HZ}, stats::{summary, r1}, vad::{Vad, VadEv}};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const HZ: u32 = 24_000;
const FRAME: usize = 480; // 20 ms @ 24 kHz

#[derive(Clone, Copy, PartialEq)]
enum Lane { Clone, Piper }

struct SynthReq { gen: u64, kind: &'static str, text: String, t_req: f64 }
struct SynthOut { gen: u64, kind: &'static str, text: String, pcm: Vec<i16>, t_req: f64, t_done: f64 }

fn strip_tags(s: &str) -> String {
    let mut o = String::new(); let mut depth = 0;
    for c in s.chars() { match c { '[' => depth += 1, ']' if depth > 0 => depth -= 1, _ if depth == 0 => o.push(c), _ => {} } }
    o.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Trim leading/trailing near-silence and normalise to a target peak.
fn tidy(mut x: Vec<i16>, gain: f32) -> Vec<i16> {
    let thr = 300i16;
    let a = x.iter().position(|s| s.abs() > thr).unwrap_or(0).saturating_sub(240);
    let b = x.iter().rposition(|s| s.abs() > thr).map(|p| (p + 1200).min(x.len())).unwrap_or(x.len());
    x = x[a..b.max(a)].to_vec();
    let peak = x.iter().map(|s| (*s as i32).abs()).max().unwrap_or(1).max(1) as f32;
    let k = (22000.0 / peak).min(4.0) * gain;
    x.iter().map(|&s| (s as f32 * k).clamp(-32767.0, 32767.0) as i16).collect()
}

fn synth_thread(lane: Lane, voice: String, reference: String, rx: Receiver<SynthReq>, tx: Sender<SynthOut>, ready: Sender<Result<f64, String>>, tmpdir: String) {
    std::thread::spawn(move || {
        enum E { C(CloneWorker), P(Piper) }
        let mut eng = match lane {
            Lane::Clone => match {
                // Pin each voice to its own cores (default: samantha 0-3, jarvis 4-7, 4 threads each).
                let key = voice.to_uppercase();
                let cpus = std::env::var(format!("GALION_CLONE_CPUS_{key}")).ok().or_else(|| Some(if voice == "jarvis" { "4-7".into() } else { "0-3".into() })).filter(|c| c != "none");
                let thr = std::env::var("GALION_CLONE_THREADS").ok().and_then(|t| t.parse().ok()).or(Some(4));
                CloneWorker::spawn_pinned(cpus.as_deref(), thr)
            }.and_then(|mut w| w.condition(&voice, &reference).map(|ms| (w, ms))) {
                Ok((w, ms)) => { let _ = ready.send(Ok(ms)); E::C(w) }
                Err(e) => { let _ = ready.send(Err(e)); return; }
            },
            Lane::Piper => {
                let ls = if voice == "jarvis" { 0.95 } else { 1.05 };
                match PiperPaths::from_repo().and_then(|p| Piper::spawn(&p, ls, 0.667)) { Ok(p) => { let _ = ready.send(Ok(0.0)); E::P(p) } Err(e) => { let _ = ready.send(Err(e)); return; } }
            }
        };
        let mut n = 0u64;
        for r in rx.iter() {
            n += 1;
            let pcm = match &mut eng {
                E::C(w) => { let _ = (&tmpdir, n); match w.say_pcm(&voice, &r.text) { Ok((p, _)) => p, Err(e) => { eprintln!("clone {voice}: {e}"); continue } } }
                E::P(p) => {
                    let t = strip_tags(&r.text); if t.is_empty() { continue; }
                    p.drain(); let before = p.done.load(std::sync::atomic::Ordering::SeqCst);
                    p.say(&t).unwrap();
                    let mut v: Vec<i16> = Vec::new();
                    if let Some(c) = p.first_pcm(5000) { v.extend(c.pcm); }
                    for c in p.collect_until_done(before, 1, 8000) { v.extend(c.pcm); }
                    audio::resample(&v, PIPER_HZ, HZ)
                }
            };
            let _ = tx.send(SynthOut { gen: r.gen, kind: r.kind, text: r.text, pcm, t_req: r.t_req, t_done: now_ms() });
        }
    });
}

struct Agent {
    name: &'static str, other: &'static str, system: String,
    play: VecDeque<i16>, gain: f32, bc_play: VecDeque<i16>, speaking_since: Option<f64>, in_voiced_since: Option<f64>, hold_since: Option<f64>, int_pending: bool, heard_voiced_ms: f64, paused: bool, pause_voiced: f64, pause_quiet: f64,
    gen: u64, synth_tx: Sender<SynthReq>, synth_rx: Receiver<SynthOut>,
    llm_rx: Option<Receiver<llm::LlmEv>>, clauser: llm::Clauser, reply_buf: String, spoken: Vec<String>,
    asr: Asr, vad: Vad, partial: String, heard_turn: String,
    history: Vec<(String, String)>,
    cache: Vec<(&'static str, String, Vec<i16>)>, // (kind, text, pcm)
    bc_i: usize, fill_i: usize,
    last_bc_ms: f64, words_at_last_bc: usize, interrupted_this_turn: bool, interrupt_words: usize,
    voiced_in_ms: f64, ducked: bool, barge_onset: Option<f64>,
    awaiting_reply_since: Option<f64>, endpoint_at: Option<f64>, first_audio_logged: bool,
    pending_synth: usize, turns: usize,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let secs: f64 = arg("--seconds").and_then(|v| v.parse().ok()).unwrap_or(90.0);
    let lane = if arg("--lane").as_deref() == Some("piper") { Lane::Piper } else { Lane::Clone };
    let out = arg("--out").unwrap_or_else(|| "target/v2-call".into());
    std::fs::create_dir_all(&out).unwrap();
    let tmp = format!("{out}/tmp"); std::fs::create_dir_all(&tmp).unwrap();
    let refs = (arg("--ref-samantha").unwrap_or_default(), arg("--ref-jarvis").unwrap_or_default());
    if lane == Lane::Clone && (refs.0.is_empty() || refs.1.is_empty()) { eprintln!("--ref-samantha/--ref-jarvis required for the clone lane"); std::process::exit(2); }
    let mut cfg = llm::LlmCfg::from_env(); cfg.max_tokens = 70;
    let log = Arc::new(Mutex::new(String::new()));
    let ev = |log: &Arc<Mutex<String>>, v: serde_json::Value| { let mut v = v; v["t_ms"] = json!(r1(now_ms())); let s = v.to_string(); eprintln!("{s}"); let mut l = log.lock().unwrap(); l.push_str(&s); l.push('\n'); };

    let tag_note = "You may begin at most one sentence with one sound tag from: [laugh] [chuckle] [sigh] [gasp]. Never use other brackets, emojis, stage directions, or lists.";
    let sam_sys = format!("You are Samantha, warm, playful and emotionally open, on a live voice call with your friend Jarvis. Speak naturally in 1-2 short sentences (under 25 words). React to what you actually heard; it is a speech transcript and may have recognition errors. {tag_note} Today: your rescue dog Biscuit came home after two days missing, and you're overjoyed; later you admit you're sad your best friend Lena moves to Lisbon on Friday.");
    let jar_sys = format!("You are Jarvis, dry, precise, quietly kind, with understated wit, on a live voice call with your friend Samantha. Speak in 1-2 short sentences (under 20 words). React to what you actually heard; it is a speech transcript and may have recognition errors. Ask a pointed question now and then. {tag_note}");

    if cfg.is_local() {
        // Local Qwen on loopback: open the keep-alive sockets and fill the prompt cache for both personas.
        for (who, sys) in [("samantha", &sam_sys), ("jarvis", &jar_sys)] {
            match galion_v2_live::local::prewarm(&cfg, sys) {
                Ok(ms) => ev(&log, json!({"ev":"llm_prewarm","who":who,"backend":"local","ms":r1(ms)})),
                Err(e) => { eprintln!("{e}"); std::process::exit(2) }
            }
        }
    }
    // Spawn engines.
    let mut agents: Vec<Agent> = Vec::new();
    for (name, other, sys, voice_ref, iw) in [("samantha", "jarvis", sam_sys, refs.0.clone(), 40usize), ("jarvis", "samantha", jar_sys, refs.1.clone(), 24usize)] {
        let (stx, srx) = channel(); let (otx, orx) = channel(); let (rtx, rrx) = channel();
        synth_thread(lane, name.to_string(), voice_ref, srx, otx, rtx, tmp.clone());
        let r = rrx.recv().unwrap();
        match r { Ok(ms) => ev(&log, json!({"ev":"engine_ready","who":name,"lane": if lane == Lane::Clone {"clone-nano"} else {"piper"},"cond_ms":ms})),
                  Err(e) => { eprintln!("{name}: {e}"); std::process::exit(2) } }
        let asr = Asr::spawn().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
        let mut vad = Vad::new(300.0); vad.min_voiced_ms = 40.0;
        agents.push(Agent { name, other, system: sys, play: VecDeque::new(), gain: 1.0, bc_play: VecDeque::new(), speaking_since: None, in_voiced_since: None, hold_since: None, int_pending: false, heard_voiced_ms: 0.0, paused: false, pause_voiced: 0.0, pause_quiet: 0.0, gen: 1, synth_tx: stx, synth_rx: orx,
            llm_rx: None, clauser: llm::Clauser::new(3), reply_buf: String::new(), spoken: vec![], asr, vad, partial: String::new(), heard_turn: String::new(),
            history: vec![], cache: vec![], bc_i: 0, fill_i: 0, last_bc_ms: -1e9, words_at_last_bc: 0, interrupted_this_turn: false, interrupt_words: iw,
            voiced_in_ms: 0.0, ducked: false, barge_onset: None, awaiting_reply_since: None, endpoint_at: None, first_audio_logged: true, pending_synth: 0, turns: 0 });
    }
    // Pre-generate cloned backchannels/fillers/interjections (real engine, cached so the floor can be held instantly).
    let cache_lines: [(&str, &[(&'static str, &str)]); 2] = [
        ("samantha", &[("bc", "Mm-hm."), ("bc", "Yeah."), ("bc", "Oh!"), ("fill", "Mm,"), ("fill", "Oh,"), ("fill", "Hmm,"), ("int", "Wait, wait,")]),
        ("jarvis", &[("bc", "Mm."), ("bc", "Quite."), ("bc", "Right."), ("fill", "Hm,"), ("fill", "Well,"), ("int", "Sorry, one thing,")]),
    ];
    for (i, (_, lines)) in cache_lines.iter().enumerate() {
        for (k, t) in lines.iter() { agents[i].synth_tx.send(SynthReq { gen: 0, kind: k, text: t.to_string(), t_req: now_ms() }).unwrap(); }
        for _ in 0..lines.len() {
            let o = agents[i].synth_rx.recv().unwrap();
            let g = if o.kind == "bc" { 0.55 } else { 0.9 };
            let mut pcm = tidy(o.pcm, g);
            if o.kind == "bc" { pcm.truncate((HZ as usize) * 45 / 100); }
            ev(&log, json!({"ev":"cache","who":agents[i].name,"kind":o.kind,"text":o.text,"audio_ms": pcm.len() * 1000 / HZ as usize, "synth_ms": r1(o.t_done - o.t_req)}));
            agents[i].cache.push((o.kind, o.text, pcm));
        }
    }

    let mut rec: Vec<i16> = Vec::new();
    let mut barges = vec![]; let mut ducks = vec![]; let mut gaps_first = vec![]; let mut gaps_content = vec![]; let mut synth_ms = vec![]; let mut ttft = vec![];
    let mut n_bc = 0; let mut n_int = 0; let mut overlap_frames = 0usize; let mut cost = 0.0;

    // Samantha opens the call.
    let start_reply = |a: &mut Agent, cfg: &llm::LlmCfg, user: String, log: &Arc<Mutex<String>>| {
        a.history.push(("user".into(), user));
        let mut msgs = vec![json!({"role":"system","content":a.system})];
        for (r, c) in a.history.iter().rev().take(12).rev() { msgs.push(json!({"role": r, "content": c})); }
        a.gen += 1; a.clauser = llm::Clauser::new(3); a.reply_buf.clear(); a.spoken.clear();
        match llm::stream(cfg, json!(msgs)) { Ok(rx) => { a.llm_rx = Some(rx); } Err(e) => { eprintln!("{e}"); std::process::exit(2) } }
        a.awaiting_reply_since = Some(now_ms()); a.first_audio_logged = false;
        let mut l = log.lock().unwrap(); l.push_str(&json!({"ev":"llm_start","who":a.name,"t_ms":r1(now_ms()),"gen":a.gen}).to_string()); l.push('\n');
    };
    start_reply(&mut agents[0], &cfg, "(The call just connected. Say hi to Jarvis and tell him your news.)".into(), &log);
    let t_start = now_ms();
    agents[0].endpoint_at = Some(t_start);
    let mut tick = 0u64;
    ev(&log, json!({"ev":"call_start","seconds":secs}));
    while now_ms() - t_start < secs * 1000.0 {
        let due = t_start + tick as f64 * 20.0;
        while now_ms() < due { std::thread::sleep(Duration::from_micros(400)); }
        tick += 1;
        let now = now_ms();
        // 1. brains: LLM tokens → clauses → synth; synth results → playback
        for i in 0..2 {
            let a = &mut agents[i];
            if let Some(rx) = &a.llm_rx {
                let mut done = false;
                while let Ok(e) = rx.try_recv() {
                    match e {
                        llm::LlmEv::Token(t, s) => {
                            if a.reply_buf.is_empty() { if let Some(t0) = a.awaiting_reply_since { ttft.push(t - t0); } }
                            a.reply_buf.push_str(&s);
                            for c in a.clauser.push(&s) { a.pending_synth += 1; a.synth_tx.send(SynthReq { gen: a.gen, kind: "say", text: c, t_req: now_ms() }).unwrap(); }
                        }
                        llm::LlmEv::Done(_, c, _) => { cost += c.unwrap_or(0.0); done = true; }
                        _ => {}
                    }
                }
                if done {
                    if let Some(c) = a.clauser.flush() { a.pending_synth += 1; a.synth_tx.send(SynthReq { gen: a.gen, kind: "say", text: c, t_req: now_ms() }).unwrap(); }
                    let reply = a.reply_buf.trim().to_string();
                    let (name, gen) = (a.name, a.gen);
                    a.history.push(("assistant".into(), reply.clone()));
                    a.llm_rx = None; a.turns += 1;
                    ev(&log, json!({"ev":"llm_done","who":name,"gen":gen,"reply":reply}));
                }
            }
            while let Ok(o) = agents[i].synth_rx.try_recv() {
                let a = &mut agents[i];
                a.pending_synth = a.pending_synth.saturating_sub(1);
                synth_ms.push(o.t_done - o.t_req);
                if o.gen != a.gen { let (n, t) = (a.name, o.text.clone()); ev(&log, json!({"ev":"synth_dropped","who":n,"text":t,"why":"cancelled by barge-in"})); continue; }
                let pcm = tidy(o.pcm, 1.0);
                let (n, t, ms, am) = (a.name, o.text.clone(), r1(o.t_done - o.t_req), pcm.len() * 1000 / HZ as usize);
                a.play.extend(pcm.iter()); a.spoken.push(o.text);
                ev(&log, json!({"ev":"clause_ready","who":n,"text":t,"synth_ms":ms,"audio_ms":am}));
            }
        }
        // 2. line: each band emits 20 ms
        let mut frames = [[0i16; FRAME]; 2];
        for i in 0..2 {
            let a = &mut agents[i];
            // polite floor-taking: don't start a turn on top of the other voice (interjections excepted), max 1.2 s
            let other_talking = a.voiced_in_ms > 0.0;
            if !a.play.is_empty() && a.speaking_since.is_none() && other_talking && !a.int_pending {
                let hs = *a.hold_since.get_or_insert(now);
                if now - hs < 1200.0 { for k in 0..FRAME { frames[i][k] = a.bc_play.pop_front().unwrap_or(0); } continue; }
            }
            a.hold_since = None;
            // paused by an overlapping voice: hold my queue, keep backchannel lane only
            if a.paused { for k in 0..FRAME { frames[i][k] = a.bc_play.pop_front().unwrap_or(0); } continue; }
            let audible = !a.play.is_empty();
            if audible && a.speaking_since.is_none() { a.speaking_since = Some(now); a.int_pending = false; }
            if !audible { a.speaking_since = None; }
            for k in 0..FRAME {
                let m = a.play.pop_front().map(|s| s as f32 * a.gain).unwrap_or(0.0) + a.bc_play.pop_front().map(|s| s as f32).unwrap_or(0.0);
                frames[i][k] = m.clamp(-32767.0, 32767.0) as i16;
            }
            if audible && !a.first_audio_logged {
                a.first_audio_logged = true;
                if let Some(ep) = a.endpoint_at { gaps_first.push(now - ep); }
                let n = a.name; ev(&log, json!({"ev":"first_audio","who":n,"since_endpoint_ms": a.endpoint_at.map(|e| r1(now - e))}));
            }
        }
        if frames[0].iter().any(|s| s.abs() > 500) && frames[1].iter().any(|s| s.abs() > 500) { overlap_frames += 1; }
        for k in 0..FRAME { rec.push(frames[0][k]); rec.push(frames[1][k]); }
        // 3. ears: each persona hears the other band
        for i in 0..2 {
            let heard = &frames[1 - i];
            let f16 = audio::resample(heard, HZ, 16_000);
            let a = &mut agents[i];
            a.asr.feed(&f16).unwrap();
            let vev = a.vad.push(&f16, 20.0);
            let voiced_now = audio::rms_db(&f16) > -45.0;
            a.voiced_in_ms = if voiced_now { a.voiced_in_ms + 20.0 } else { 0.0 };
            if voiced_now { a.in_voiced_since.get_or_insert(now - 20.0); a.heard_voiced_ms += 20.0; } else { a.in_voiced_since = None; }
            let speaking = !a.play.is_empty() && a.speaking_since.map_or(false, |s| a.in_voiced_since.map_or(false, |v| v >= s));
            // barge-in: other side keeps talking over me → duck at 80 ms, yield at 180 ms
            if speaking && voiced_now {
                if a.barge_onset.is_none() { a.barge_onset = a.in_voiced_since; }
                if a.voiced_in_ms >= 80.0 && !a.ducked { a.ducked = true; a.gain = 0.35; ducks.push(now - a.barge_onset.unwrap()); }
                if a.voiced_in_ms >= 180.0 && !a.paused {
                    // silence at <=200 ms (LOCKS barge_in), but hold the rest: a backchannel should not kill my turn
                    let lat = now + 20.0 - a.barge_onset.unwrap();
                    barges.push(lat); a.paused = true; a.pause_voiced = a.voiced_in_ms; a.pause_quiet = 0.0;
                    let n = a.name; ev(&log, json!({"ev":"barge_in_pause","who":n,"onset_to_silence_ms":r1(lat)}));
                }
            }
            if a.paused {
                if voiced_now { a.pause_voiced += 20.0; a.pause_quiet = 0.0; } else { a.pause_quiet += 20.0; }
                if a.pause_quiet >= 240.0 && a.pause_voiced < 800.0 {
                    a.paused = false; a.ducked = false; a.gain = 1.0; a.barge_onset = None; a.speaking_since = Some(now);
                    let (n, pv) = (a.name, a.pause_voiced); ev(&log, json!({"ev":"barge_in_resume","who":n,"overlap_voiced_ms":pv,"why":"short overlap (backchannel), floor kept"}));
                } else if a.pause_voiced >= 800.0 {
                    a.paused = false;
                    let dropped = a.play.len() * 1000 / HZ as usize;
                    a.play.clear(); a.speaking_since = None; a.gen += 1; a.llm_rx = None; a.pending_synth = 0; a.gain = 1.0; a.ducked = false; a.barge_onset = None;
                    let spoken = a.spoken.join(" ");
                    if let Some(last) = a.history.last_mut() { if last.0 == "assistant" { last.1 = format!("{spoken} — (you were cut off here)"); } }
                    let n = a.name; ev(&log, json!({"ev":"barge_in_yield","who":n,"committed_after_voiced_ms":800,"dropped_audio_ms":dropped}));
                }
            } else if !voiced_now && a.ducked && a.voiced_in_ms == 0.0 {
                a.ducked = false; a.gain = 1.0; a.barge_onset = None;
            } else if !speaking && !a.paused { a.barge_onset = None; a.ducked = false; a.gain = 1.0; }
            if vev == VadEv::Start && a.heard_turn.is_empty() { a.interrupted_this_turn = false; a.words_at_last_bc = 0; }
            if vev == VadEv::End { a.asr.finalize().unwrap(); a.endpoint_at = Some(now); }
        }
        // 4. ASR events → backchannel / interjection / reply
        for i in 0..2 {
            let evs: Vec<_> = agents[i].asr.rx.try_iter().collect();
            for e in evs {
                let a = &mut agents[i];
                if !e.fin {
                    a.partial = e.text.clone();
                    let words = (a.heard_turn.clone() + " " + &a.partial).split_whitespace().count();
                    let idle = a.play.is_empty() && a.llm_rx.is_none() && a.pending_synth == 0;
                    // backchannel: listener stays present while the other talks
                    if idle && words >= a.words_at_last_bc + 8 && now - a.last_bc_ms > 3500.0 {
                        let bcs: Vec<_> = a.cache.iter().filter(|c| c.0 == "bc").collect();
                        let c = bcs[a.bc_i % bcs.len()]; a.bc_i += 1;
                        let (t, pcm) = (c.1.clone(), c.2.clone());
                        a.bc_play.extend(pcm.iter()); a.last_bc_ms = now; a.words_at_last_bc = words; n_bc += 1;
                        let n = a.name; ev(&log, json!({"ev":"backchannel","who":n,"text":t,"heard_words":words}));
                    }
                    // interjection: Jarvis cuts in on a long run-on turn (content-driven, real LLM)
                    if idle && !a.interrupted_this_turn && words >= a.interrupt_words {
                        a.interrupted_this_turn = true; n_int += 1;
                        let ints: Vec<_> = a.cache.iter().filter(|c| c.0 == "int").collect();
                        if let Some(c) = ints.first() { let p = c.2.clone(); a.play.extend(p.iter()); a.int_pending = true; }
                        let heard = format!("{} {}", a.heard_turn, a.partial).trim().to_string();
                        let n = a.name; let o = a.other;
                        ev(&log, json!({"ev":"interject","who":n,"over":o,"heard_so_far":heard}));
                        a.endpoint_at = Some(now);
                        start_reply(a, &cfg, format!("{heard} ... (they are still talking; cut in briefly with one short question)"), &log);
                        a.heard_turn.clear(); a.partial.clear();
                    }
                } else {
                    if !e.text.is_empty() { if !a.heard_turn.is_empty() { a.heard_turn.push(' '); } a.heard_turn.push_str(&e.text); }
                    a.partial.clear();
                    let n = a.name; let ft = e.text.clone(); ev(&log, json!({"ev":"asr_final","who":n,"text":ft}));
                    // reply when the other side has really stopped (VAD endpoint) and I'm not already talking
                    let other_quiet = !agents[i].vad.speaking;
                    let a = &mut agents[i];
                    let idle = a.play.is_empty() && a.llm_rx.is_none() && a.pending_synth == 0;
                    let turn = a.heard_turn.trim().to_string();
                    let only_bc = turn.split_whitespace().count() <= 2 && (a.heard_voiced_ms < 900.0 || ["mm", "hm", "yeah", "right", "quite", "oh", "uh", "huh", "okay"].iter().any(|b| turn.to_lowercase().contains(b)));
                    if other_quiet && idle && !turn.is_empty() && !only_bc {
                        // floor hold: cached cloned filler right away, content follows
                        let fills: Vec<_> = a.cache.iter().filter(|c| c.0 == "fill").collect();
                        let c = fills[a.fill_i % fills.len()]; a.fill_i += 1;
                        let p = c.2.clone(); a.play.extend(p.iter());
                        start_reply(a, &cfg, turn, &log);
                        a.heard_turn.clear(); a.heard_voiced_ms = 0.0;
                    } else if only_bc && other_quiet { let n = a.name; ev(&log, json!({"ev":"ignored_short","who":n,"text":turn})); a.heard_turn.clear(); a.heard_voiced_ms = 0.0; }
                }
            }
        }
        // content gap: first non-filler clause audio after endpoint
        for i in 0..2 {
            let a = &agents[i];
            if let Some(t0) = a.awaiting_reply_since {
                if !a.spoken.is_empty() {
                    if let Some(ep) = a.endpoint_at { gaps_content.push(now - ep.min(t0)); }
                    agents[i].awaiting_reply_since = None;
                }
            }
        }
    }
    ev(&log, json!({"ev":"call_end"}));
    let wav = format!("{out}/call-stereo.wav");
    audio::write_wav(&wav, HZ, 2, &rec).unwrap();
    let mono: Vec<i16> = rec.chunks_exact(2).map(|c| ((c[0] as i32 + c[1] as i32) / 2) as i16).collect();
    audio::write_wav(&format!("{out}/call-mix.wav"), HZ, 1, &mono).unwrap();
    std::fs::write(format!("{out}/call-timing.ndjson"), log.lock().unwrap().as_str()).unwrap();
    let s = json!({"lane": if lane == Lane::Clone {"clone-nano (Chatterbox Nano CPU, CC0 donors)"} else {"piper A21 CPU"}, "seconds": secs,
        "llm_model": cfg.model, "llm_cost_usd": cost, "turns": {"samantha": agents[0].turns, "jarvis": agents[1].turns},
        "backchannels": n_bc, "interjections": n_int, "barge_in_silence": summary("barge-in onset→silence (pause)", &barges), "barge_in_duck": summary("barge-in onset→duck", &ducks),
        "overlap_s": r1(overlap_frames as f64 * 0.02), "endpoint_to_first_audio": summary("endpoint→first audio (filler/bc ok)", &gaps_first),
        "endpoint_to_first_content_clause": summary("endpoint→first content clause", &gaps_content), "llm_ttft": summary("llm ttft", &ttft),
        "synth_ms_per_clause": summary("synth per clause", &synth_ms), "wav": wav});
    std::fs::write(format!("{out}/call-summary.json"), serde_json::to_string_pretty(&s).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&s).unwrap());
    let _ = std::fs::remove_dir(&tmp);
}
