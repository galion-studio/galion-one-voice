// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Listening set: persona × user emotion × persona mood history, on Piper.
//!
//! For each case the *user* line is synthesized (ljspeech-medium, loudness set
//! per emotion — synthetic user audio, labelled as such), run through the real
//! consent-gated voice detector (`GatedDetector<LoudnessEstimator>`, 80 ms
//! windows) and through `text_affect`, fused, and fed to the persona's
//! `PersonaMood` after that persona's mood history. The persona then speaks one
//! fixed reply line, so every measured difference is prosody, not words.
//!
//! ```text
//! GALION_A21_RUNTIME=<piper> cargo run --release -p galion-ml-voice --features speech --example emotion_sweep -- \
//!   --out results/emotion-sweep --user <ljspeech-medium.onnx> \
//!   --samantha <pack.onnx>[:speaker_id] --jarvis <pack.onnx>[:speaker_id]
//! ```
use galion_emotion::*;
use galion_ml_voice::emotive::{active_rms_dbfs, median_f0, EmotiveShaper};
use galion_ml_voice::speech::weights::{A21WeightTts, WeightStatus};
use std::io::Write;
use std::path::{Path, PathBuf};

const REPLY: &str = "I have moved the meeting to Thursday at four, and the notes are in your inbox.";
/// (name, user line, loudness gain for the synthetic user audio)
const USER: &[(&str, &str, f32)] = &[
    ("neutral", "Can you check my calendar for tomorrow?", 1.0),
    ("happy", "I got the job, this is amazing!", 1.7),
    ("sad", "My dog died yesterday and I feel so lonely.", 0.35),
    ("angry", "I am so angry, this is the third time it broke!", 2.0),
    ("anxious", "I'm really worried about the surgery tomorrow.", 1.3),
    ("calm", "Thanks, I'm relaxed now, it's a peaceful evening.", 0.4),
];
/// Piper's VITS sampling is stochastic (noise_scale/noise_w); metrics are the
/// mean over this many renders, the WAV is the first render.
const REPS: usize = 8;

/// (mean median-F0 Hz, mean speech s, mean active RMS dBFS, mean lead ms, first render)
fn measure(t: &mut A21WeightTts) -> (f32, f32, f32, f32, Vec<i16>) {
    let rate = t.sample_rate();
    let (mut f0, mut sp, mut db, mut ld) = (0.0, 0.0, 0.0, 0.0);
    let mut first = Vec::new();
    for i in 0..REPS {
        let pcm = t.synthesize_pcm(REPLY).expect("synth");
        f0 += median_f0(&pcm, rate, 60.0, 400.0).unwrap_or(0.0);
        sp += (pcm.len() - lead(&pcm) - trail(&pcm)) as f32 / rate as f32;
        db += active_rms_dbfs(&pcm, rate);
        ld += lead(&pcm) as f32 * 1000.0 / rate as f32;
        if i == 0 {
            first = pcm;
        }
    }
    let n = REPS as f32;
    (f0 / n, sp / n, db / n, ld / n, first)
}

const HISTORY: &[(&str, Option<usize>)] = &[("rest", None), ("after-joy", Some(1)), ("after-distress", Some(2))];

fn arg(name: &str) -> Option<String> {
    let a: Vec<String> = std::env::args().collect();
    a.iter().position(|x| x == name).and_then(|i| a.get(i + 1).cloned())
}

fn pack(spec: &str) -> (PathBuf, Option<u32>) {
    match spec.rsplit_once(':') {
        Some((p, sid)) if sid.chars().all(|c| c.is_ascii_digit()) && !sid.is_empty() => (PathBuf::from(p), sid.parse().ok()),
        _ => (PathBuf::from(spec), None),
    }
}

fn tts(path: &Path, speaker: Option<u32>) -> A21WeightTts {
    let st = WeightStatus::inspect(path).unwrap_or_else(|| panic!("no pack at {}", path.display()));
    let mut t = A21WeightTts::try_new(&st).unwrap_or_else(|e| panic!("{e}"));
    t.set_speaker(speaker);
    t
}

fn write_wav(path: &Path, pcm: &[i16], rate: u32) {
    let mut f = std::fs::File::create(path).unwrap();
    let n = (pcm.len() * 2) as u32;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + n).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&rate.to_le_bytes());
    h.extend_from_slice(&(rate * 2).to_le_bytes());
    h.extend_from_slice(&2u16.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&n.to_le_bytes());
    f.write_all(&h).unwrap();
    let mut b = Vec::with_capacity(pcm.len() * 2);
    for s in pcm {
        b.extend_from_slice(&s.to_le_bytes());
    }
    f.write_all(&b).unwrap();
}

/// Leading silence (samples below −45 dBFS-ish) — the pre-pause plus Piper's own lead-in.
fn lead(pcm: &[i16]) -> usize {
    pcm.iter().position(|&s| s.unsigned_abs() > 180).unwrap_or(pcm.len())
}
fn trail(pcm: &[i16]) -> usize {
    pcm.iter().rev().position(|&s| s.unsigned_abs() > 180).unwrap_or(pcm.len())
}

struct Row {
    persona: &'static str,
    user: &'static str,
    history: &'static str,
    detected_arousal: f32,
    fused_v: f32,
    fused_a: f32,
    mood: Mood,
    e: f32,
    p: PiperProsodyVector,
    f0: f32,
    speech_s: f32,
    lead_ms: f32,
    rms_db: f32,
    file: String,
}

fn main() {
    let out = PathBuf::from(arg("--out").unwrap_or_else(|| "results/emotion-sweep".into()));
    std::fs::create_dir_all(&out).unwrap();
    let (user_pack, _) = pack(&arg("--user").expect("--user <ljspeech-medium.onnx>"));
    let mut user_tts = tts(&user_pack, None);
    let mut gate = DefaultConsentGate::default();
    gate.grant(0, "en", Purpose::PersonalAssistant);

    // 1) User side: synthesize each user line once, detect from voice + text.
    let mut user_obs = Vec::new();
    let mut user_meta = Vec::new();
    for (name, line, gain) in USER {
        let raw = user_tts.synthesize_pcm(line).expect("user synth");
        let mut sh = EmotiveShaper::new(0.0, *gain);
        let mut pcm = vec![0i16; sh.max_out(raw.len())];
        let n = sh.process(&raw, &mut pcm);
        pcm.truncate(n);
        write_wav(&out.join(format!("user-{name}.wav")), &pcm, user_tts.sample_rate());
        let rate = user_tts.sample_rate();
        let win = (rate as usize * 80) / 1000;
        let mut det = GatedDetector::<LoudnessEstimator>::default();
        let mut last = None;
        let mut a_sum = 0.0;
        let mut a_n = 0.0;
        for (i, c) in pcm.chunks(win).enumerate() {
            let f: Vec<f32> = c.iter().map(|&s| s as f32 / 32768.0).collect();
            if let Ok(fr) = det.detect(&f, rate, (i * 80) as u32, &gate, Purpose::PersonalAssistant) {
                if fr.confidence > 0.0 {
                    a_sum += fr.arousal;
                    a_n += 1.0;
                    last = Some(EmotionFrame { arousal: a_sum / a_n, ..fr });
                }
            }
        }
        let voice = last.expect("voice frames");
        let text = text_affect(line);
        let fused = UserObservation::fuse(Some(&voice), det.valence_known(), Some(text));
        user_meta.push((voice.arousal, fused));
        user_obs.push(fused);
    }

    // 2) Persona side.
    let mut rows: Vec<Row> = Vec::new();
    let mut baselines = Vec::new();
    let mut refs = Vec::new();
    for persona in Persona::ALL {
        let flag = format!("--{}", persona.as_str());
        let (path, sid) = pack(&arg(&flag).unwrap_or_else(|| user_pack.display().to_string()));
        let mut t = tts(&path, sid);
        let rate = t.sample_rate();
        // V1 baseline: no plan (neutral scales, deterministic noise as in V1).
        let (base_f0, base_speech, base_db, _, base) = measure(&mut t);
        let bname = format!("{}-v1-baseline.wav", persona.as_str());
        write_wav(&out.join(&bname), &base, rate);
        baselines.push((persona.as_str(), t.voice().to_string(), sid, base_f0, base_speech, base_db, bname));
        // Persona reference: resting mood, no user observation. Same noise
        // regime as every persona case, so deltas isolate mood + user emotion.
        {
            use galion_emotion::ConditionedDecoder;
            let mut m = PersonaMood::new(persona);
            t.condition(&m.conditioning(0));
        }
        let (rf0, rsp, rdb, _, rpcm) = measure(&mut t);
        let rname = format!("{}-reference-rest.wav", persona.as_str());
        write_wav(&out.join(&rname), &rpcm, rate);
        refs.push((persona.as_str(), rf0, rsp, rdb, rname, t.plan().copied().unwrap()));
        for (hname, hist) in HISTORY {
            for (ui, (uname, _, _)) in USER.iter().enumerate() {
                let mut m = PersonaMood::new(persona);
                if let Some(h) = hist {
                    for k in 0..3u64 {
                        m.observe(&user_obs[*h], k * 3_000);
                    }
                }
                m.observe(&user_obs[ui], 9_000);
                let c = m.conditioning(9_500);
                use galion_emotion::ConditionedDecoder;
                t.condition(&c);
                let (f0, speech_s, rms_db, lead_ms, pcm) = measure(&mut t);
                let file = format!("{}-{}-{}.wav", persona.as_str(), uname, hname);
                write_wav(&out.join(&file), &pcm, rate);
                rows.push(Row {
                    persona: persona.as_str(),
                    user: uname,
                    history: hname,
                    detected_arousal: user_meta[ui].0,
                    fused_v: user_meta[ui].1.valence,
                    fused_a: user_meta[ui].1.arousal,
                    mood: c.mood,
                    e: c.empathy_e,
                    p: t.plan().copied().unwrap(),
                    f0,
                    speech_s,
                    lead_ms,
                    rms_db,
                    file,
                });
            }
        }
        eprintln!("{}: piper spawns over the sweep = {}", persona.as_str(), t.piper_spawns());
    }

    // 3) Report.
    let mut csv = String::from("persona,user_emotion,mood_history,voice_arousal,fused_valence,fused_arousal,mood_label,mood_v,mood_a,mood_d,e,length_scale,noise_scale,noise_w,sentence_silence_s,pitch_st,energy,pre_pause_ms,f0_hz,df0_vs_rest_st,speech_s,rate_vs_rest_pct,lead_ms,rms_dbfs,drms_vs_rest_db,file\n");
    let mut md = String::from("| persona | user | mood history | mood | e | length_scale | pitch st (plan) | energy | pause ms | F0 Hz | ΔF0 vs rest st | rate vs rest | ΔRMS dB | file |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for r in &rows {
        let b = refs.iter().find(|b| b.0 == r.persona).unwrap();
        let dst = if r.f0 > 0.0 && b.1 > 0.0 { 12.0 * (r.f0 / b.1).log2() } else { f32::NAN };
        let rate_pct = 100.0 * (b.2 / r.speech_s - 1.0);
        let drms = r.rms_db - b.3;
        csv.push_str(&format!(
            "{},{},{},{:.3},{:.3},{:.3},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.2},{:.3},{},{:.1},{:.2},{:.3},{:.1},{:.0},{:.2},{:.2},{}\n",
            r.persona, r.user, r.history, r.detected_arousal, r.fused_v, r.fused_a, r.mood.label(), r.mood.valence, r.mood.arousal, r.mood.dominance, r.e,
            r.p.length_scale, r.p.noise_scale, r.p.noise_w, r.p.sentence_silence_s, r.p.pitch_shift_st, r.p.energy, r.p.pre_pause_ms,
            r.f0, dst, r.speech_s, rate_pct, r.lead_ms, r.rms_db, drms, r.file
        ));
        md.push_str(&format!(
            "| {} | {} | {} | {} (v {:.2}, a {:.2}) | {:.2} | {:.3} | {:+.2} | {:.2} | {} | {:.1} | {:+.2} | {:+.1}% | {:+.1} | `{}` |\n",
            r.persona, r.user, r.history, r.mood.label(), r.mood.valence, r.mood.arousal, r.e, r.p.length_scale, r.p.pitch_shift_st, r.p.energy,
            r.p.pre_pause_ms, r.f0, dst, rate_pct, drms, r.file
        ));
    }
    let mut head = String::from("# Emotion sweep (persona × user emotion × mood history)\n\nReply line for every persona case: \"");
    head.push_str(REPLY);
    head.push_str(&format!("\"\n\nMetrics are means over {REPS} renders (Piper sampling is stochastic); each WAV is the first render. ΔF0 (semitones), rate and ΔRMS in the persona table are relative to that persona's own resting reference (resting mood, no user cue, same Piper noise regime). The V1 path (no plan: deterministic noise 0) is listed separately."));
    head.push_str("\n\n## V1 baselines (no persona plan)\n\n| persona | pack | speaker | F0 Hz | speech s | RMS dBFS | file |\n|---|---|---|---|---|---|---|\n");
    for b in &baselines {
        head.push_str(&format!("| {} | {} | {} | {:.1} | {:.3} | {:.1} | `{}` |\n", b.0, b.1, b.2.map(|s| s.to_string()).unwrap_or("-".into()), b.3, b.4, b.5, b.6));
    }
    head.push_str("\n## Persona resting references (the conditioning with no user cue)\n\n| persona | length_scale | noise_scale | noise_w | silence s | pitch st | energy | pre-pause ms | F0 Hz | speech s | RMS dBFS | file |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n");
    for r in &refs {
        let p = r.5;
        head.push_str(&format!("| {} | {:.3} | {:.3} | {:.3} | {:.2} | {:+.2} | {:.2} | {} | {:.1} | {:.3} | {:.1} | `{}` |\n", r.0, p.length_scale, p.noise_scale, p.noise_w, p.sentence_silence_s, p.pitch_shift_st, p.energy, p.pre_pause_ms, r.1, r.2, r.3, r.4));
    }
    head.push_str("\n## User side (synthetic user audio → consent-gated loudness detector + text cue → fused)\n\n| user | voice arousal (detector) | fused valence | fused arousal | conf |\n|---|---|---|---|---|\n");
    for (i, (n, _, _)) in USER.iter().enumerate() {
        head.push_str(&format!("| {} | {:.2} | {:+.2} | {:.2} | {:.2} |\n", n, user_meta[i].0, user_meta[i].1.valence, user_meta[i].1.arousal, user_meta[i].1.confidence));
    }
    head.push_str("\n## Persona cases\n\n");
    std::fs::write(out.join("sweep.csv"), csv).unwrap();
    std::fs::write(out.join("SWEEP.md"), head + &md).unwrap();
    println!("{} wavs + {} baselines + {} user clips -> {}", rows.len(), baselines.len(), USER.len(), out.display());
}
