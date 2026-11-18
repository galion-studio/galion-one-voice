// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Breath stage on top of the voicing track praatfan already produced.
//!
//! An inhale candidate is an unvoiced run that is louder than the clip's own
//! noise floor and about one breath long. That is the VAD-gap fallback.
//! A trained detector (YAMNet, when `workers/breath_worker.py` has a local
//! model) may replace these cues via [`apply_cues`]. CPPS is not computed
//! here: Praat's CPPS is GPL, and no MIT implementation is wired in.

use crate::Articulation;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct BreathCue {
    pub start_s: f64,
    pub end_s: f64,
    pub kind: String,
    pub loudness_dbfs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Lungs {
    pub breath_rate_per_min: f64,
    pub breath_group_s: f64,
    pub inhale_loudness_dbfs: Option<f64>,
    pub breathiness_cpps_db: Option<f64>,
    pub h1_h2_db: Option<f64>,
    pub n_inhales: usize,
    pub voiced_runs_per_s: f64,
    pub pause_ratio: f64,
    pub source: String,
    pub cues: Vec<BreathCue>,
}

pub fn h1_h2(art: &Articulation) -> Option<f64> {
    art.statics.iter().find_map(|(k, (_, v))| {
        let l = k.to_lowercase();
        if l.contains("h1") && l.contains("h2") {
            Some(*v)
        } else {
            None
        }
    })
}

fn runs(flags: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < flags.len() {
        if flags[i] {
            let s = i;
            while i < flags.len() && flags[i] {
                i += 1;
            }
            out.push((s, i));
        } else {
            i += 1;
        }
    }
    out
}

pub fn measure(art: &Articulation, cpps_db: Option<f64>) -> Lungs {
    let n = art.n_ticks.max(1);
    let dur = art.duration_s.max(crate::TICK_S);
    let voice = art
        .tracks
        .get("voicing")
        .map(|t| t.ticks.clone())
        .unwrap_or_default();
    let rms = art
        .tracks
        .get("rms_dbfs")
        .map(|t| t.ticks.clone())
        .unwrap_or_default();
    let mut floors: Vec<f64> = rms
        .iter()
        .flatten()
        .copied()
        .filter(|x| x.is_finite())
        .collect();
    floors.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let floor = if floors.is_empty() {
        -80.0
    } else {
        floors[(floors.len() as f64 * 0.20) as usize]
    };
    let voiced: Vec<bool> = (0..n)
        .map(|i| voice.get(i).copied().flatten().unwrap_or(0.0) >= 0.5)
        .collect();
    let unvoiced: Vec<bool> = voiced.iter().map(|v| !v).collect();
    let speech_db = {
        let mut voiced_rms = Vec::new();
        for (i, is_v) in voiced.iter().enumerate() {
            if *is_v {
                if let Some(v) = rms.get(i).copied().flatten() {
                    if v.is_finite() {
                        voiced_rms.push(v);
                    }
                }
            }
        }
        if voiced_rms.is_empty() {
            None
        } else {
            voiced_rms.sort_by(|a, b| a.partial_cmp(b).unwrap());
            Some(voiced_rms[voiced_rms.len() / 2])
        }
    };
    let tick_db = |i: usize| rms.get(i).copied().flatten().filter(|v| v.is_finite());
    let breath_tick: Vec<bool> = (0..n)
        .map(|i| {
            if voiced.get(i).copied().unwrap_or(false) {
                return false;
            }
            let Some(db) = tick_db(i) else { return false };
            let below_speech = speech_db.map(|s| db < s - 6.0).unwrap_or(true);
            db >= floor + 6.0 && db >= -55.0 && below_speech
        })
        .collect();
    let mut cues = Vec::new();
    for (a, b) in runs(&breath_tick) {
        let len_s = (b - a) as f64 * crate::TICK_S;
        if !(0.08..0.70).contains(&len_s) {
            continue;
        }
        let mut loud = Vec::new();
        for i in a..b {
            if let Some(v) = tick_db(i) {
                loud.push(v);
            }
        }
        if loud.is_empty() {
            continue;
        }
        let mean = loud.iter().sum::<f64>() / loud.len() as f64;
        cues.push(BreathCue {
            start_s: a as f64 * crate::TICK_S,
            end_s: b as f64 * crate::TICK_S,
            kind: "inhale".into(),
            loudness_dbfs: mean,
        });
    }
    let groups = runs(&voiced);
    let breath_group_s = if groups.is_empty() {
        0.0
    } else {
        groups
            .iter()
            .map(|(a, b)| (b - a) as f64 * crate::TICK_S)
            .sum::<f64>()
            / groups.len() as f64
    };
    let inhale_loudness_dbfs = if cues.is_empty() {
        None
    } else {
        Some(cues.iter().map(|c| c.loudness_dbfs).sum::<f64>() / cues.len() as f64)
    };
    let pause_ticks = unvoiced.iter().filter(|u| **u).count();
    Lungs {
        breath_rate_per_min: cues.len() as f64 / dur * 60.0,
        breath_group_s,
        inhale_loudness_dbfs,
        breathiness_cpps_db: cpps_db,
        h1_h2_db: h1_h2(art),
        n_inhales: cues.len(),
        voiced_runs_per_s: groups.len() as f64 / dur,
        pause_ratio: pause_ticks as f64 / n as f64,
        source: "praatfan-voicing-gap".into(),
        cues,
    }
}

/// Replace cues when an external detector wrote intervals in seconds.
pub fn apply_cues(lungs: &mut Lungs, cues: Vec<BreathCue>, source: &str, duration_s: f64) {
    lungs.n_inhales = cues.len();
    lungs.breath_rate_per_min = if duration_s > 0.0 {
        cues.len() as f64 / duration_s * 60.0
    } else {
        0.0
    };
    lungs.inhale_loudness_dbfs = if cues.is_empty() {
        None
    } else {
        Some(cues.iter().map(|c| c.loudness_dbfs).sum::<f64>() / cues.len() as f64)
    };
    lungs.cues = cues;
    lungs.source = source.into();
}

/// Splice the reference's own inhales in front of phrase starts that do not
/// already begin with one. This is the fallback when the cloner has no
/// breath token. Chatterbox in this repo has no `[breath]` tag.
pub fn splice_inhales(
    samples: &[f64],
    sr: f64,
    cues: &[BreathCue],
    phrase_starts_s: &[f64],
) -> Vec<f64> {
    if cues.is_empty() || phrase_starts_s.is_empty() || sr <= 0.0 {
        return samples.to_vec();
    }
    let cue = &cues[0];
    let a = (cue.start_s * sr).round().max(0.0) as usize;
    let b = (cue.end_s * sr).round() as usize;
    if b <= a || b > samples.len() {
        return samples.to_vec();
    }
    let inhale = samples[a..b].to_vec();
    let mut inserts: Vec<usize> = phrase_starts_s
        .iter()
        .map(|t| (*t * sr).round().max(0.0) as usize)
        .filter(|i| *i > inhale.len() && *i < samples.len())
        .collect();
    inserts.sort_unstable();
    inserts.dedup();
    let mut out = Vec::with_capacity(samples.len() + inserts.len() * inhale.len());
    let mut cursor = 0;
    for i in inserts {
        if i < cursor {
            continue;
        }
        let already = i >= inhale.len()
            && samples[i - inhale.len()..i]
                .iter()
                .zip(&inhale)
                .all(|(x, y)| (x - y).abs() < 1e-6);
        out.extend_from_slice(&samples[cursor..i]);
        if !already {
            out.extend_from_slice(&inhale);
        }
        cursor = i;
    }
    out.extend_from_slice(&samples[cursor..]);
    out
}
