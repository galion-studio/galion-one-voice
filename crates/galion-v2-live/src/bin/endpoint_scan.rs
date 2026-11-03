// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! v2-endpoint-scan: false-endpoint rate of the two-stage endpointer.
//!
//! Each WAV is one user turn (true end = last frame within 35 dB of peak,
//! same rule as v2-latency). Frames go through the real Vosk sidecar in
//! lockstep (`S` ack after every frame), so the transcript seen at frame i
//! is exactly what a live loop would have had. Then every endpointer
//! setting is replayed over the same frames:
//!   false speculate = a Speculate before the true end (a wasted, cancelled LLM call)
//!   false commit    = a Commit before the true end (we would talk over the user)
//! No timing claims come from this tool; it counts events. Usage:
//!   v2-endpoint-scan --out DIR WAV_OR_DIR...
use galion_v2_live::{asr::Asr, audio, endpoint::{EndpointCfg, Endpointer, EpEv}, stats::{pct, r1}};
use serde_json::json;

fn true_eos_ms(x: &[i16]) -> f64 {
    let fr = 320; let mut last = 0;
    let peak = x.chunks(fr).map(audio::rms_db).fold(-120.0, f64::max);
    for (i, f) in x.chunks(fr).enumerate() { if audio::rms_db(f) > peak - 35.0 { last = i; } }
    (last + 1) as f64 * 20.0
}

struct Clip { name: String, frames: Vec<Vec<i16>>, transcript: Vec<String>, eos: f64, secs: f64 }

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = "target/v2-endpoint-scan".to_string();
    let mut paths = vec![];
    let mut it = args.into_iter();
    while let Some(a) = it.next() { if a == "--out" { out = it.next().unwrap(); } else { paths.push(a); } }
    std::fs::create_dir_all(&out).unwrap();
    let mut wavs = vec![];
    for p in paths {
        let md = std::fs::metadata(&p).unwrap_or_else(|e| { eprintln!("{p}: {e}"); std::process::exit(2) });
        if md.is_dir() { let mut v: Vec<_> = std::fs::read_dir(&p).unwrap().filter_map(|e| e.ok()).map(|e| e.path().display().to_string()).filter(|s| s.ends_with(".wav")).collect(); v.sort(); wavs.extend(v); }
        else { wavs.push(p); }
    }
    let mut asr = Asr::spawn().unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
    let mut clips = vec![];
    for w in &wavs {
        let wav = audio::read_wav(w).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(2) });
        let x = audio::resample(&audio::to_mono(&wav), wav.rate, 16_000);
        let eos = true_eos_ms(&x);
        let mut frames: Vec<Vec<i16>> = x.chunks(320).map(|c| c.to_vec()).collect();
        for _ in 0..75 { frames.push(vec![0; 320]); }
        let (mut finals, mut partial) = (String::new(), String::new());
        let mut tr = Vec::with_capacity(frames.len());
        for f in &frames {
            asr.feed(f).unwrap(); asr.sync().unwrap();
            loop {
                let ev = asr.rx.recv_timeout(std::time::Duration::from_secs(10)).expect("asr sync timeout");
                if ev.sync { break; }
                if ev.fin { if !ev.text.is_empty() { if !finals.is_empty() { finals.push(' '); } finals.push_str(&ev.text); } partial.clear(); } else { partial = ev.text; }
            }
            tr.push(if partial.is_empty() { finals.clone() } else if finals.is_empty() { partial.clone() } else { format!("{finals} {partial}") });
        }
        asr.finalize().unwrap(); asr.sync().unwrap();
        loop { let ev = asr.rx.recv_timeout(std::time::Duration::from_secs(10)).expect("asr"); if ev.sync { break; } }
        eprintln!("{w}: {:.1}s eos {eos} ms: {}", x.len() as f64 / 16000.0, tr.last().unwrap());
        clips.push(Clip { name: w.rsplit('/').next().unwrap().to_string(), frames, transcript: tr, eos, secs: x.len() as f64 / 16000.0 });
    }
    let mut grid = vec![];
    for c in [60.0, 80.0, 100.0, 120.0, 160.0, 200.0, 240.0, 300.0, 400.0] { grid.push((60.0, c, c)); }
    for (c, h) in [(120.0, 400.0), (160.0, 500.0), (200.0, 600.0), (240.0, 600.0)] { for s in [40.0, 60.0, 80.0] { grid.push((s, c, h)); } }
    let mut res = vec![];
    for (s, c, h) in grid {
        let cfg = EndpointCfg { spec_ms: s, commit_ms: c, hold_ms: h };
        let (mut fs_clips, mut fs_ev, mut fc_clips, mut fc_ev, mut held) = (0, 0, 0, 0, 0);
        let (mut spec_lat, mut commit_lat) = (vec![], vec![]);
        let mut cut_names = vec![];
        for cl in &clips {
            // Replay the whole turn. A Commit is a cut-off only if the user
            // speaks again after it (a later VAD Start); the last Commit is the
            // turn end. Same for Speculate (wasted call) vs Resume/Start.
            let mut e = Endpointer::new(cfg);
            let mut evs: Vec<(f64, EpEv, bool)> = vec![];
            for (i, f) in cl.frames.iter().enumerate() {
                let ev = e.push(f, 20.0, &cl.transcript[i]);
                if ev != EpEv::None { evs.push((i as f64 * 20.0, ev, e.commit_window(&cl.transcript[i]) > c)); }
            }
            let last_start = evs.iter().rposition(|x| x.1 == EpEv::Start);
            let (mut fs, mut fc) = (0, 0);
            let mut end = None;
            for (k, (t, ev, hold)) in evs.iter().enumerate() {
                match ev {
                    EpEv::Commit => { if last_start.map_or(false, |ls| k < ls) { fc += 1; } else if end.is_none() { end = Some((*t, *hold)); } }
                    EpEv::Speculate => { if evs[k + 1..].iter().any(|x| x.1 == EpEv::Resume || x.1 == EpEv::Start) { fs += 1; } }
                    _ => {}
                }
            }
            if let Some((t, hold)) = end {
                commit_lat.push(t - cl.eos);
                let sp = evs.iter().filter(|x| x.1 == EpEv::Speculate && x.0 <= t).last().map(|x| x.0).unwrap_or(t);
                spec_lat.push(sp - cl.eos);
                if hold { held += 1; }
            }
            if fs > 0 { fs_clips += 1; } fs_ev += fs;
            if fc > 0 { fc_clips += 1; cut_names.push(cl.name.clone()); } fc_ev += fc;
        }
        let n = clips.len() as f64;
        let row = json!({"spec_ms": s, "commit_ms": c, "hold_ms": h, "semantic_hold": h > c, "clips": clips.len(),
            "false_commit_clip_rate": r1(fc_clips as f64 / n * 100.0), "false_commit_events": fc_ev,
            "false_spec_clip_rate": r1(fs_clips as f64 / n * 100.0), "false_spec_events": fs_ev, "false_spec_per_turn": r1(fs_ev as f64 / n * 10.0) / 10.0,
            "true_end_commits_held_by_semantic_rule": held,
            "eos_to_spec_ms": {"p50": r1(pct(&spec_lat, 50.0)), "p95": r1(pct(&spec_lat, 95.0))},
            "eos_to_commit_ms": {"p50": r1(pct(&commit_lat, 50.0)), "p95": r1(pct(&commit_lat, 95.0))}, "missed_commits": clips.len() - commit_lat.len(),
            "cut_off_clips": cut_names});
        eprintln!("{row}");
        res.push(row);
    }
    let speech_s: f64 = clips.iter().map(|c| c.secs).sum();
    let rep = json!({"tool": "v2-endpoint-scan", "clips": clips.len(), "speech_s": r1(speech_s), "frame_ms": 20,
        "transcripts": clips.iter().map(|c| json!({"clip": c.name, "eos_ms": c.eos, "text": c.transcript.last()})).collect::<Vec<_>>(), "grid": res});
    std::fs::write(format!("{out}/endpoint-scan.json"), serde_json::to_string_pretty(&rep).unwrap()).unwrap();
    eprintln!("wrote {out}/endpoint-scan.json");
}
