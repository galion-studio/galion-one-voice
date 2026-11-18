// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! One plan: velum and murmur on the 80 ms grid, bursts on their own list.
//!
//! A tick's velum is the speaker's resting opening, plus a feeling term, plus
//! an explicit nasal span, plus a hum or "mm" which opens the port because
//! that is how those sounds leave the face. A laugh does not open it.
//!
//! `phon_nasal` never enters the sum. It is a speech-class posterior.

use serde::{Deserialize, Serialize};

use crate::baseline::Speaker;
use crate::bursts::{self, BurstAsk, BurstEvent};
use crate::config::Config;
use crate::feeling::OpenFeeling;
use crate::inventory::Inventory;
use crate::{DIRECTIONS, REUSES, TICK_MS};

#[derive(Clone, Debug, Deserialize)]
pub struct NasalSpan {
    pub start_s: f64,
    pub end_s: f64,
}

#[derive(Clone, Debug)]
pub struct TimedFeeling {
    pub at_s: f64,
    pub feeling: OpenFeeling,
}

#[derive(Clone, Debug)]
pub struct Request {
    pub text: String,
    pub feeling: OpenFeeling,
    pub trajectory: Vec<TimedFeeling>,
    pub nose: Vec<NasalSpan>,
    pub bursts: Vec<BurstAsk>,
    /// When set, burst-track amusement may place one laugh. Prosody does not.
    pub place_from_feeling: bool,
    pub speaker: Speaker,
}

impl Request {
    pub fn new(text: &str, feeling: OpenFeeling, speaker: Speaker) -> Self {
        Request {
            text: text.to_string(),
            feeling,
            trajectory: Vec::new(),
            nose: Vec::new(),
            bursts: Vec::new(),
            place_from_feeling: false,
            speaker,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NoseTick {
    pub t_ms: u32,
    pub velum_open: f64,
    pub velum_rest: f64,
    pub velum_delta: f64,
    pub murmur: f64,
    /// `rises`, `falls`, or `flat`. Chen's A1–P0 direction against this speaker's rest.
    pub a1_p0_direction: String,
    /// Config dB. Zero unless `a1_p0.db_per_velum` is set. Not a measurement.
    pub a1_p0_db_config: f64,
    /// Control for a later oral filter. This layer does not filter a vowel.
    pub antiresonance: f64,
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    pub intensity: f64,
    pub category: String,
    pub burst_top: Option<String>,
    pub gesture: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub schema: String,
    pub tick_ms: u32,
    pub duration_s: f64,
    pub n_ticks: usize,
    pub speech_s: f64,
    pub speaker_f0_hz: f64,
    pub velum_rest: f64,
    pub phon_nasal: Option<f64>,
    pub ticks: Vec<NoseTick>,
    pub bursts: Vec<BurstEvent>,
    pub gaps: Vec<String>,
    pub directions: Vec<String>,
    pub reuses: Vec<String>,
}

pub fn plan(req: &Request, cfg: &Config, inventory: &Inventory) -> Result<Plan, String> {
    cfg.check()?;
    req.speaker.check()?;
    req.feeling.check(inventory)?;
    for step in &req.trajectory {
        if !step.at_s.is_finite() || step.at_s < 0.0 {
            return Err("a feeling trajectory time must be finite and >= 0".into());
        }
        step.feeling.check(inventory)?;
    }
    for (i, span) in req.nose.iter().enumerate() {
        if !span.start_s.is_finite()
            || !span.end_s.is_finite()
            || span.end_s <= span.start_s
            || span.start_s < 0.0
        {
            return Err(format!("nasal span {i} needs 0 <= start < end"));
        }
    }

    let speech_s = speech_len(&req.text, cfg);
    let mut asks = req.bursts.clone();
    let mut gaps = base_gaps(&req.speaker);
    if req.place_from_feeling {
        if !asks
            .iter()
            .any(|a| crate::gestures::canonical(&a.gesture) == Some("laugh"))
        {
            if let Some(ask) = bursts::laugh_from_feeling(&req.feeling, cfg)? {
                asks.push(ask);
            }
        }
        if req.bursts.is_empty() {
            if let Some(note) = bursts::non_laugh_gap(&req.feeling) {
                gaps.push(note);
            }
        }
    }

    let events = bursts::place(
        &asks,
        speech_s,
        &req.speaker,
        &|t| feeling_at(req, t),
        cfg,
        inventory,
    )?;

    let mut end = speech_s;
    for ev in &events {
        end = end.max(ev.end_s);
    }
    for span in &req.nose {
        end = end.max(span.end_s);
    }
    if end <= 0.0 {
        end = cfg.tick_s;
    }
    let n_ticks = crate::config::ticks_covering(cfg.tick_s, end);
    let duration_s = n_ticks as f64 * cfg.tick_s;

    let mut ticks = Vec::with_capacity(n_ticks);
    for i in 0..n_ticks {
        let t = i as f64 * cfg.tick_s;
        let feel = feeling_at(req, t);
        let gesture = events
            .iter()
            .find(|e| t + 1e-9 >= e.start_s && t + 1e-9 < e.end_s)
            .map(|e| e.gesture.as_str());
        let mut velum = req.speaker.velum_rest + cfg.feeling.velum_per_intensity * feel.intensity;
        if span_covers(&req.nose, t, cfg.tick_s) {
            velum += cfg.velum.span_open;
        }
        match gesture {
            Some("hum") => velum = velum.max(cfg.velum.hum_open),
            Some("mm") => velum = velum.max(cfg.velum.mm_open),
            Some("laugh") => velum += cfg.velum.laugh_add,
            _ => {}
        }
        velum = velum.clamp(0.0, 1.0);
        let delta = velum - req.speaker.velum_rest;
        let direction = if delta > 1e-9 {
            "rises"
        } else if delta < -1e-9 {
            "falls"
        } else {
            "flat"
        };
        let burst_top =
            crate::feeling::exclusive_burst_top(&feel.burst).map(|(id, _)| id.to_string());
        ticks.push(NoseTick {
            t_ms: (i as u32) * TICK_MS,
            velum_open: velum,
            velum_rest: req.speaker.velum_rest,
            velum_delta: delta,
            murmur: velum,
            a1_p0_direction: direction.into(),
            a1_p0_db_config: cfg.a1_p0.db_per_velum * delta,
            antiresonance: delta.max(0.0),
            arousal: feel.arousal,
            valence: feel.valence,
            dominance: feel.dominance,
            intensity: feel.intensity,
            category: feel.category,
            burst_top,
            gesture: gesture.map(|s| s.to_string()),
        });
    }

    Ok(Plan {
        schema: "galion-nose/1".into(),
        tick_ms: TICK_MS,
        duration_s,
        n_ticks,
        speech_s,
        speaker_f0_hz: req.speaker.f0_hz,
        velum_rest: req.speaker.velum_rest,
        phon_nasal: req.speaker.phon_nasal,
        ticks,
        bursts: events,
        gaps,
        directions: DIRECTIONS.iter().map(|s| (*s).to_string()).collect(),
        reuses: REUSES.iter().map(|s| (*s).to_string()).collect(),
    })
}

fn base_gaps(speaker: &Speaker) -> Vec<String> {
    let mut gaps = vec![
        "TODO(S1): intent is the local galion-mne-intent-stub/0, not the galion-mne-contract crate"
            .into(),
        "phon_nasal is a speech-class posterior, not nasal resonance, and does not set velum_open"
            .into(),
        "A1-P0 dB is config. Chen (1997) gives the direction only.".into(),
        "No per-category nasalance was read. feeling.velum_per_intensity ships at 0.".into(),
        "Sigh acoustics are unknown. The four candidates are not four recipes.".into(),
        "Moan, gasp, hum and mm have no Cowen assignment in the sources read.".into(),
        "The oral antiresonance is a control. This layer does not filter a spoken vowel.".into(),
        "A cloner is not called. CPU synthesis and an optional runtime reference are the sound."
            .into(),
    ];
    if speaker.f0_hz <= 0.0 {
        gaps.push(
            "speaker F0 is missing. Voiced bursts are not given a Bachorowski sex mean.".into(),
        );
    }
    gaps
}

pub fn speech_len(text: &str, cfg: &Config) -> f64 {
    let chars = text.chars().filter(|c| !c.is_whitespace()).count();
    if chars == 0 {
        0.0
    } else {
        let raw = chars as f64 / cfg.speech.chars_per_s;
        crate::config::ticks_covering(cfg.tick_s, raw) as f64 * cfg.tick_s
    }
}

pub fn feeling_at(req: &Request, t: f64) -> OpenFeeling {
    let mut current = req.feeling.clone();
    let mut steps = req.trajectory.clone();
    steps.sort_by(|a, b| {
        a.at_s
            .partial_cmp(&b.at_s)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for step in steps {
        if step.at_s <= t + 1e-9 {
            current = step.feeling;
        }
    }
    current
}

fn span_covers(spans: &[NasalSpan], t: f64, tick_s: f64) -> bool {
    let t1 = t + tick_s;
    spans
        .iter()
        .any(|s| t < s.end_s - 1e-9 && t1 > s.start_s + 1e-9)
}
