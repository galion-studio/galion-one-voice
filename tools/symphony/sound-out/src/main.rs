// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-sound-out speak --text "..." [--backend piper|chatterbox|moshi]
//!                        [--room quiet|small|large|outdoor] [--distance 1]
//!                        [--noise-db 0] [--category neutral]
//!                        [--out out.wav]
//!
//! The WAV is written only where --out points. Nothing is downloaded.
#![forbid(unsafe_code)]

use galion_sound_out::contract::MomentFixture;
use galion_sound_out::environment::Room;
use galion_sound_out::{render, ChatterboxBackend, MoshiStub, PiperBackend};
use std::collections::HashMap;

fn args() -> (String, HashMap<String, String>) {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut m = HashMap::new();
    while let Some(k) = it.next() {
        if let Some(k) = k.strip_prefix("--") {
            let v = it.next().unwrap_or_default();
            m.insert(k.to_string(), v);
        }
    }
    (cmd, m)
}

fn run() -> Result<(), String> {
    let (cmd, m) = args();
    if cmd != "speak" {
        return Err("usage: galion-sound-out speak --text \"...\" [--backend piper] [--out file.wav]".into());
    }
    let text = m.get("text").cloned().unwrap_or_else(|| "Hello.".into());
    let mut moment = MomentFixture::new(&text);
    if let Some(c) = m.get("category") {
        moment.category = c.clone();
    }
    if let Some(room) = m.get("room") {
        moment.environment.room = match room.as_str() {
            "small" => Room::Small,
            "large" => Room::Large,
            "outdoor" => Room::Outdoor,
            _ => Room::Quiet,
        };
    }
    if let Some(d) = m.get("distance") {
        moment.environment.listener_distance_m = d.parse().map_err(|_| "distance is not a number")?;
    }
    if let Some(n) = m.get("noise-db") {
        moment.environment.noise_db = n.parse().map_err(|_| "noise-db is not a number")?;
    }
    let backend_name = m.get("backend").map(|s| s.as_str()).unwrap_or("piper");
    let device = m.get("device").map(|s| s.as_str());
    let out = match backend_name {
        "chatterbox" => {
            let mut b = ChatterboxBackend::stub();
            render(&moment, &mut b, &[], device)?
        }
        "moshi" => {
            let mut b = MoshiStub;
            render(&moment, &mut b, &[], device)?
        }
        "piper-real" => {
            let mut b = PiperBackend::real_if_present()?;
            render(&moment, &mut b, &[], device)?
        }
        _ => {
            let mut b = PiperBackend::stand_in();
            render(&moment, &mut b, &[], device)?
        }
    };
    if let Some(path) = m.get("out") {
        galion_lungs::wav::write_wav_mono_i16(
            std::path::Path::new(path),
            out.sample_rate,
            &out.samples,
        )?;
    }
    let report = serde_json::json!({
        "schema": out.schema,
        "backend": out.backend,
        "sample_rate": out.sample_rate,
        "samples": out.samples.len(),
        "frames": out.frame_count,
        "realtime_on_cpu": out.realtime_on_cpu,
        "weights_used": out.weights_used,
        "device": out.device,
        "breath_events": out.breath_events,
        "bursts": out.burst_count,
        "environment": out.environment,
        "stubbed": out.stubbed,
        "controls_echo": out.controls_echo,
    });
    println!("{}", serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?);
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("galion-sound-out: {err}");
        std::process::exit(1);
    }
}
