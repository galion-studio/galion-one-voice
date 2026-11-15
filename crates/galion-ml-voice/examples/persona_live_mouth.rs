// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Speak one matter line at the Samantha and Jarvis e0/e1 columns and print
//! what the mouth actually did. This host has no Piper pack, so the engine
//! is whatever `SpeechStack::detect` can open (Flite when the library is
//! present). Protocol-stub PCM is not used. WAVs are not written.
//!
//! `rate` is a pitch-preserving time stretch of the PCM. Pitch, loudness, and
//! the leading pause are applied in the same pass.
//!
//! ```text
//! GALION_SPEECH=flite cargo run -p galion-ml-voice --features speech --example persona_live_mouth
//! ```

use galion_ml_voice::speech::{FrameProsody, SpeechStack};

struct Column {
    persona: &'static str,
    e: f32,
    prosody: FrameProsody,
}

fn column(persona: &'static str, e: f32, pitch: f32, rate: f32, loudness: f32, pause: u16) -> Column {
    Column {
        persona,
        e,
        prosody: FrameProsody {
            pitch_st: pitch,
            pitch_range: 1.0,
            rate,
            energy: loudness,
            warmth: 0.5,
            pre_pause_ms: pause,
        },
    }
}

fn lead_samples(samples: &[f32]) -> usize {
    samples.iter().position(|s| s.abs() > 1.0e-3).unwrap_or(samples.len())
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
}

/// First autocorrelation peak between 70 Hz and 300 Hz. `None` if the window is short.
fn f0_hz(samples: &[f32], rate: u32) -> Option<f32> {
    let start = lead_samples(samples);
    let window = samples.get(start..)?;
    let n = window.len().min(rate as usize * 2 / 5);
    if n < rate as usize / 25 {
        return None;
    }
    let w = &window[..n];
    let min_lag = (rate as usize / 300).max(1);
    let max_lag = (rate as usize / 70).min(n / 2);
    if max_lag <= min_lag {
        return None;
    }
    let mut best = 0.0f32;
    let mut best_lag = 0usize;
    for lag in min_lag..max_lag {
        let mut acc = 0.0f32;
        for i in 0..(n - lag) {
            acc += w[i] * w[i + lag];
        }
        if acc > best {
            best = acc;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best <= 0.0 {
        None
    } else {
        Some(rate as f32 / best_lag as f32)
    }
}

fn main() {
    let line = "Your mother died this morning.";
    std::env::set_var("GALION_SPEECH", "flite");
    let mut stack = SpeechStack::detect();
    let (engine, label) = stack.reported();
    println!("engine={engine}");
    println!("label={label}");
    println!("text={line}");
    println!("rate_applied=false");
    println!("piper_pack=absent_on_this_run");
    println!("stub=false");
    if !stack.can_speak() {
        println!("verdict=BLOCKED reason=no speech backend");
        std::process::exit(2);
    }
    let columns = [
        column("samantha", 0.0, 0.0, 1.00, 1.00, 0),
        column("samantha", 1.0, -0.5, 0.92, 0.80, 250),
        column("jarvis", 0.0, -0.3, 0.96, 0.92, 80),
        column("jarvis", 1.0, -0.4, 0.94, 0.88, 120),
    ];
    let mut rows: Vec<(Column, u32, usize, u32, f32, Option<f32>)> = Vec::new();
    for col in columns {
        stack.set_frame_prosody(col.prosody);
        let audio = match stack.speak(line) {
            Ok(a) => a,
            Err(e) => {
                println!("verdict=FAIL persona={} e={} err={e}", col.persona, col.e);
                std::process::exit(1);
            }
        };
        let (id, _) = stack.reported();
        if id == "protocol-stub" || id == "unwired" || id == "tone" {
            println!("verdict=FAIL engine={id} reason=not a mouth");
            std::process::exit(1);
        }
        let lead = lead_samples(&audio.samples);
        let lead_ms = (lead as u64 * 1000 / u64::from(audio.sample_rate.max(1))) as u32;
        let speech = &audio.samples[lead..];
        let row_rms = rms(speech);
        let f0 = f0_hz(&audio.samples, audio.sample_rate);
        println!(
            "persona={} e={} engine={} rate_hz={} samples={} lead_ms={} rms={:.4} f0_hz={} pitch_st={} energy={} pause_ms={} rate_knob={}",
            col.persona,
            col.e,
            id,
            audio.sample_rate,
            audio.samples.len(),
            lead_ms,
            row_rms,
            f0.map(|h| format!("{h:.1}")).unwrap_or_else(|| "none".into()),
            col.prosody.pitch_st,
            col.prosody.energy,
            col.prosody.pre_pause_ms,
            col.prosody.rate,
        );
        rows.push((col, audio.sample_rate, audio.samples.len(), lead_ms, row_rms, f0));
    }
    let moved = |a: usize, b: usize| {
        let (left, right) = (&rows[a], &rows[b]);
        let lead_delta = (left.3 as i32 - right.3 as i32).abs();
        let rms_ratio = if right.4 > 1.0e-6 { left.4 / right.4 } else { 0.0 };
        println!(
            "delta {} e{} -> {} e{} lead_ms_delta={} rms_ratio={:.3} samples {} -> {}",
            left.0.persona, left.0.e, right.0.persona, right.0.e, lead_delta, rms_ratio, left.2, right.2
        );
        lead_delta >= 40 || (rms_ratio - 1.0).abs() > 0.05 || left.2 != right.2
    };
    let sam = moved(0, 1);
    let jar = moved(2, 3);
    if sam && jar {
        println!("verdict=MOUTH_MOVED engine={engine}");
    } else {
        println!("verdict=FAIL samantha_moved={sam} jarvis_moved={jar}");
        std::process::exit(1);
    }
}
