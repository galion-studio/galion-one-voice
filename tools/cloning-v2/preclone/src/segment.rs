// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Split a long take into target, other speaker, music, and noise.
//!
//! The target is an F0, not an embedding. With no hint, the target is the
//! speech F0 that occupies the most frames. A tie keeps the lower frequency.
//! Music is tonal energy that does not sit on that single harmonic grid.

use crate::dsp::{self, Feat};
use crate::profile::{Segment, SegmentKind};

const TARGET_SEMITONES: f64 = 2.5;
const MIN_KEPT_S: f64 = 0.30;
const MIN_SNR_DB: f64 = 6.0;
const SPEECH_HARM_MIN: f64 = 0.40;
const SPEECH_RESIDUAL_MAX: f64 = 0.62;
const MUSIC_FLAT_MAX: f64 = 0.40;
const MUSIC_RESIDUAL_MIN: f64 = 0.38;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lab {
    Noise = 0,
    Music = 1,
    Speech = 2,
}

pub(crate) fn select(
    samples: &[f32],
    sr: u32,
    noise_floor_dbfs: Option<f64>,
    target_f0_hz: Option<f64>,
) -> Vec<Segment> {
    let feats = dsp::frame_features(samples, sr);
    if feats.is_empty() {
        return Vec::new();
    }
    let dbs: Vec<f64> = feats.iter().map(|f| f.db).collect();
    let p10 = dsp::quantile(&dbs, 0.10);
    let p90 = dsp::quantile(&dbs, 0.90);
    let dr = p90 - p10;
    let floor = noise_floor_dbfs.unwrap_or(p10);
    // A steady take sits inside a couple of dB, so a gate above p10 would
    // reject the take itself. Spectral rules separate that texture.
    // A take with a real bed (dynamic range ≥ 12 dB) keeps frames 8 dB
    // above refcheck's quietest window.
    let gate_db = if dr < 12.0 {
        f64::NEG_INFINITY
    } else {
        floor + 8.0
    };
    let mut labs: Vec<Lab> = feats.iter().map(|f| label(f, gate_db)).collect();
    smooth(&mut labs);
    let target = target_f0(target_f0_hz, &feats, &labs);
    let apply_snr = dr >= 12.0;
    merge(samples, sr, &feats, &labs, target, floor, apply_snr)
}

fn label(f: &Feat, gate_db: f64) -> Lab {
    if f.db < gate_db {
        return Lab::Noise;
    }
    let tonal = f.flatness < MUSIC_FLAT_MAX;
    if tonal && f.residual_ratio >= MUSIC_RESIDUAL_MIN && f.harmonicity < 0.85 {
        return Lab::Music;
    }
    if f.harmonicity >= SPEECH_HARM_MIN && f.f0_hz > 0.0 && f.residual_ratio < SPEECH_RESIDUAL_MAX {
        return Lab::Speech;
    }
    if tonal && f.residual_ratio >= MUSIC_RESIDUAL_MIN {
        return Lab::Music;
    }
    Lab::Noise
}

fn smooth(labs: &mut [Lab]) {
    if labs.len() < 3 {
        return;
    }
    let src = labs.to_vec();
    for i in 0..src.len() {
        let a = i.saturating_sub(2);
        let b = (i + 3).min(src.len());
        let mut counts = [0u16; 3];
        for lab in &src[a..b] {
            counts[*lab as usize] += 1;
        }
        let mut best = src[i] as usize;
        let mut best_c = 0u16;
        for (idx, c) in counts.iter().enumerate() {
            if *c > best_c {
                best_c = *c;
                best = idx;
            }
        }
        labs[i] = match best {
            1 => Lab::Music,
            2 => Lab::Speech,
            _ => Lab::Noise,
        };
    }
}

fn target_f0(hint: Option<f64>, feats: &[Feat], labs: &[Lab]) -> f64 {
    if let Some(h) = hint {
        return h;
    }
    let mut bins = [0u32; 66];
    for (f, lab) in feats.iter().zip(labs.iter()) {
        if *lab != Lab::Speech || f.f0_hz <= 0.0 {
            continue;
        }
        let b = ((f.f0_hz - 70.0) / 5.0).floor();
        if (0.0..66.0).contains(&b) {
            bins[b as usize] += 1;
        }
    }
    let mut best_i = 0usize;
    let mut best_c = 0u32;
    for (i, c) in bins.iter().enumerate() {
        if *c > best_c {
            best_c = *c;
            best_i = i;
        }
    }
    if best_c == 0 {
        0.0
    } else {
        70.0 + best_i as f64 * 5.0 + 2.5
    }
}

fn semitones(a: f64, b: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return 99.0;
    }
    (12.0 * (a / b).log2()).abs()
}

fn merge(
    samples: &[f32],
    sr: u32,
    feats: &[Feat],
    labs: &[Lab],
    target: f64,
    floor_db: f64,
    apply_snr: bool,
) -> Vec<Segment> {
    let hop = dsp::ms_samples(sr, dsp::HOP_MS).max(1);
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < feats.len() {
        let lab = labs[i];
        let mut j = i + 1;
        while j < feats.len() && labs[j] == lab {
            j += 1;
        }
        let start = feats[i].start;
        let end = (feats[j - 1].start + hop).min(samples.len());
        let (kind, f0) = match lab {
            Lab::Noise => (SegmentKind::Noise, 0.0),
            Lab::Music => (SegmentKind::Music, median_f0(feats, i, j)),
            Lab::Speech => {
                let f0 = median_f0(feats, i, j);
                if target > 0.0 && semitones(f0, target) <= TARGET_SEMITONES {
                    (SegmentKind::Target, f0)
                } else {
                    (SegmentKind::Other, f0)
                }
            }
        };
        let snr = segment_snr(samples, start, end, floor_db);
        let dur = (end.saturating_sub(start)) as f64 / f64::from(sr);
        let kept =
            kind == SegmentKind::Target && dur >= MIN_KEPT_S && (!apply_snr || snr >= MIN_SNR_DB);
        if end > start {
            out.push(Segment {
                start_sample: start,
                end_sample: end,
                kind,
                kept,
                f0_hz: f0,
                snr_db: snr,
            });
        }
        i = j;
    }
    out
}

fn median_f0(feats: &[Feat], a: usize, b: usize) -> f64 {
    let xs: Vec<f64> = feats[a..b]
        .iter()
        .filter(|f| f.f0_hz > 0.0 && f.harmonicity >= 0.25)
        .map(|f| f.f0_hz)
        .collect();
    dsp::median(&xs)
}

fn segment_snr(samples: &[f32], a: usize, b: usize, floor_db: f64) -> f64 {
    if b <= a {
        return 0.0;
    }
    let mut acc = 0.0;
    for s in &samples[a..b] {
        let v = *s as f64;
        acc += v * v;
    }
    let db = db_power(acc / (b - a) as f64);
    let ratio = 10f64.powf((db - floor_db) / 10.0);
    if ratio <= 1.001 {
        0.0
    } else {
        10.0 * (ratio - 1.0).log10()
    }
}

fn db_power(power: f64) -> f64 {
    if power <= 1e-20 {
        -200.0
    } else {
        10.0 * power.log10()
    }
}
