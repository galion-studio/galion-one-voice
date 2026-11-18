// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Room context and the measurements refcheck already knows how to make.
//! Noise floor and RT60 are `galion-refcheck`'s functions, called as a library.
//! A dry clip with no tail leaves RT60 empty; that is a measurement limit, not a zero.

use galion_refcheck::dsp::{estimate_rt60, noise_floor_dbfs};
use galion_refcheck::spec::{ReverbSpec, WindowSpec};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Context {
    pub room: String,
    pub proximity: String,
    pub distance_m: Option<f64>,
    pub lombard: bool,
    pub noise_dbfs: Option<f64>,
    /// Requested room decay. The measured tail is `Environment.rt60_s`.
    pub target_rt60_s: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Environment {
    pub noise_floor_dbfs: Option<f64>,
    pub snr_db: Option<f64>,
    pub rt60_s: Option<f64>,
    pub rt60_note: Option<String>,
    pub effects: Context,
}

struct Room {
    name: &'static str,
    rt60_s: f64,
    noise_floor_dbfs: f64,
    proximity: &'static str,
    distance_m: f64,
    lombard: bool,
}

const ROOMS: [Room; 4] = [
    Room {
        name: "studio",
        rt60_s: 0.20,
        noise_floor_dbfs: -68.0,
        proximity: "close",
        distance_m: 0.30,
        lombard: false,
    },
    Room {
        name: "close",
        rt60_s: 0.15,
        noise_floor_dbfs: -65.0,
        proximity: "close",
        distance_m: 0.25,
        lombard: false,
    },
    Room {
        name: "cafe",
        rt60_s: 0.45,
        noise_floor_dbfs: -48.0,
        proximity: "table",
        distance_m: 1.20,
        lombard: true,
    },
    Room {
        name: "hall",
        rt60_s: 1.10,
        noise_floor_dbfs: -55.0,
        proximity: "far",
        distance_m: 4.0,
        lombard: false,
    },
];

pub fn context_for(name: Option<&str>, measured_noise: Option<f64>) -> Result<Context, String> {
    let Some(name) = name else {
        return Ok(Context {
            room: "unknown".into(),
            proximity: "unknown".into(),
            distance_m: None,
            lombard: false,
            noise_dbfs: measured_noise,
            target_rt60_s: None,
        });
    };
    let room = ROOMS
        .iter()
        .find(|r| r.name == name)
        .ok_or_else(|| format!("unknown env {name} (studio, close, cafe, hall)"))?;
    Ok(Context {
        room: room.name.into(),
        proximity: room.proximity.into(),
        distance_m: Some(room.distance_m),
        lombard: room.lombard,
        noise_dbfs: Some(room.noise_floor_dbfs),
        target_rt60_s: Some(room.rt60_s),
    })
}

pub fn measure(samples: &[f64], sr: f64, requested: Option<&str>) -> Result<Environment, String> {
    let pcm: Vec<f32> = samples.iter().map(|s| *s as f32).collect();
    let sr_u = sr.round().max(1.0) as u32;
    let window = WindowSpec {
        max_dbfs: 0.0,
        window_ms: 400,
        hop_ms: 100,
    };
    let noise = noise_floor_dbfs(&pcm, sr_u, &window);
    let snr = noise.and_then(|floor| speech_level_db(&pcm).map(|speech| speech - floor));
    let reverb = ReverbSpec {
        max_rt60_s: 5.0,
        fit_start_db: -5.0,
        fit_end_db: -20.0,
        frame_ms: 10,
        hop_ms: 5,
    };
    let (rt60, note) = match estimate_rt60(&pcm, sr_u, &reverb) {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(e)),
    };
    let effects = context_for(requested, noise)?;
    Ok(Environment {
        noise_floor_dbfs: noise,
        snr_db: snr,
        rt60_s: rt60,
        rt60_note: note,
        effects,
    })
}

fn speech_level_db(x: &[f32]) -> Option<f64> {
    if x.is_empty() {
        return None;
    }
    let win = 400.min(x.len());
    if win == 0 {
        return None;
    }
    let mut levels = Vec::new();
    let mut i = 0;
    while i + win <= x.len() {
        let e = x[i..i + win]
            .iter()
            .map(|s| (*s as f64) * (*s as f64))
            .sum::<f64>()
            / win as f64;
        if e > 0.0 {
            levels.push(10.0 * e.log10());
        }
        i += win / 2;
    }
    if levels.is_empty() {
        return None;
    }
    levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(levels[(levels.len() as f64 * 0.90) as usize])
}
