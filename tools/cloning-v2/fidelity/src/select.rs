//! Best-segment reference selection and length-adaptive conditioning.
//!
//! Ideal donor length is 50 s. A short best piece (~6 s, the current Samantha and Jarvis
//! references) is kept as-is: it is not tiled out to 50 s. The single best segment is
//! placed first so a model that only reads the first 6 s still hears it.

use serde::Serialize;

use crate::analyze::{self, Analysis};
use crate::audio::{self, TARGET_SR};
use crate::embed::{self, EMBED_DIM};

pub const IDEAL_S: f32 = 50.0;
pub const SHORT_BELOW_S: f32 = 8.0;
pub const LM_S: f32 = 6.0;
pub const SPEAKER_TOKEN_S: f32 = 10.0;
const HARD_MIN_SCORE: f64 = 0.15;

#[derive(Clone, Debug)]
pub struct Config {
    pub target_sr: u32,
    pub ideal_s: f32,
    pub short_below_s: f32,
    pub lm_s: f32,
    pub speaker_token_s: f32,
    pub window_s: f32,
    pub hop_s: f32,
    pub min_window_s: f32,
    pub xfade_ms: f32,
    pub min_score_short: f32,
    pub min_score_long: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target_sr: TARGET_SR,
            ideal_s: IDEAL_S,
            short_below_s: SHORT_BELOW_S,
            lm_s: LM_S,
            speaker_token_s: SPEAKER_TOKEN_S,
            window_s: 2.0,
            hop_s: 0.5,
            min_window_s: 0.8,
            xfade_ms: 15.0,
            min_score_short: 0.20,
            min_score_long: 0.35,
        }
    }
}

/// `0.50 * snr_norm + 0.30 * voiced_ratio + 0.20 * pitch_coverage`, with `snr_norm = clamp(snr_db/30, 0, 1)`.
pub fn segment_score(snr_db: f64, voiced_ratio: f64, pitch_coverage: f64) -> f64 {
    let snr_n = (snr_db / 30.0).clamp(0.0, 1.0);
    0.50 * snr_n + 0.30 * voiced_ratio.clamp(0.0, 1.0) + 0.20 * pitch_coverage.clamp(0.0, 1.0)
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentMeta {
    pub input_index: usize,
    pub start_s: f64,
    pub end_s: f64,
    pub dur_s: f64,
    pub snr_db: f64,
    pub voiced_ratio: f64,
    pub pitch_coverage: f64,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlanMeta {
    pub schema: &'static str,
    pub mode: String,
    pub sample_rate: u32,
    pub inputs: usize,
    pub input_s: f64,
    pub ideal_s: f64,
    pub short_below_s: f64,
    pub assembled_s: f64,
    pub lm_conditioning_s: f64,
    pub speaker_token_s: f64,
    pub speaker_pool_s: f64,
    pub repeated: bool,
    pub recommended_timbre_strength: f64,
    pub timbre_applied: bool,
    pub min_score: f64,
    pub segments: Vec<SegmentMeta>,
    pub embedding_dim: usize,
    pub embedding: Vec<f32>,
    pub notes: Vec<String>,
}

pub struct Prepared {
    pub samples: Vec<f32>,
    pub meta: PlanMeta,
}

#[derive(Clone, Debug)]
struct Cand {
    input_index: usize,
    start: usize,
    end: usize,
    snr_db: f64,
    voiced_ratio: f64,
    pitch_coverage: f64,
    score: f64,
}

pub fn prepare(inputs: &[(Vec<f32>, u32)], cfg: &Config) -> Result<Prepared, String> {
    if cfg.target_sr < 8_000 {
        return Err("target sample rate must be at least 8 kHz".into());
    }
    let mut audio: Vec<Vec<f32>> = Vec::new();
    for (samples, sr) in inputs {
        if samples.iter().any(|v| v.is_finite() && v.abs() > 0.0) {
            audio.push(audio::resample_linear(
                &audio::sanitize(samples),
                *sr,
                cfg.target_sr,
            ));
        }
    }
    if audio.is_empty() {
        return Err("no usable reference audio".into());
    }
    let sr = cfg.target_sr;
    let input_s: f64 = audio.iter().map(|y| y.len() as f64 / sr as f64).sum();
    let short = input_s < cfg.short_below_s as f64;
    let mode = if short { "short" } else { "long" };
    let cap_s = if short {
        input_s as f32
    } else {
        cfg.ideal_s.min(input_s as f32)
    };
    let min_score = if short {
        cfg.min_score_short
    } else {
        cfg.min_score_long
    };
    let analyses: Vec<Analysis> = audio.iter().map(|y| analyze::analyze(y, sr)).collect();
    let mut cands = Vec::new();
    let win = ((cfg.window_s * sr as f32) as usize).max(1);
    let hop = ((cfg.hop_s * sr as f32) as usize).max(1);
    let min_win = ((cfg.min_window_s * sr as f32) as usize).max(1);
    let tiny = ((0.40 * sr as f32) as usize).max(1);
    for (index, y) in audio.iter().enumerate() {
        let mut windows = Vec::new();
        if y.len() <= win {
            if y.len() >= tiny {
                windows.push((0usize, y.len()));
            }
        } else {
            let mut s = 0usize;
            let mut last_end = 0usize;
            while s + win <= y.len() {
                windows.push((s, s + win));
                last_end = s + win;
                s += hop;
            }
            if y.len().saturating_sub(last_end) >= min_win {
                windows.push((y.len() - min_win, y.len()));
            }
        }
        for (start, end) in windows {
            let region = analyze::region_stats(y, &analyses[index], start, end);
            let score = segment_score(region.snr_db, region.voiced_ratio, region.pitch_coverage);
            cands.push(Cand {
                input_index: index,
                start,
                end,
                snr_db: region.snr_db,
                voiced_ratio: region.voiced_ratio,
                pitch_coverage: region.pitch_coverage,
                score,
            });
        }
    }
    if cands.is_empty() {
        return Err("reference audio is shorter than 0.4 s".into());
    }
    cands.sort_by(|a, b| {
        score_rank(b.score)
            .cmp(&score_rank(a.score))
            .then(a.input_index.cmp(&b.input_index))
            .then(a.start.cmp(&b.start))
    });
    let xfade_s = cfg.xfade_ms / 1000.0;
    let mut chosen: Vec<Cand> = Vec::new();
    consider(
        &cands,
        &mut chosen,
        min_score as f64,
        cap_s,
        sr,
        xfade_s,
        None,
    );
    let floor_s = if short {
        (input_s as f32 * 0.45).min(cap_s)
    } else {
        cfg.lm_s.min(input_s as f32).min(cap_s)
    };
    if assembled_estimate(&chosen, sr, xfade_s) < floor_s - 1e-3 {
        consider(
            &cands,
            &mut chosen,
            HARD_MIN_SCORE,
            cap_s,
            sr,
            xfade_s,
            Some(floor_s),
        );
    }
    if chosen.is_empty() {
        chosen.push(cands[0].clone());
    }
    let best_i = chosen
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| {
            score_rank(a.score)
                .cmp(&score_rank(b.score))
                .then(b.input_index.cmp(&a.input_index))
                .then(b.start.cmp(&a.start))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);
    let best = chosen.remove(best_i);
    chosen.sort_by(|a, b| {
        a.input_index
            .cmp(&b.input_index)
            .then(a.start.cmp(&b.start))
    });
    chosen.insert(0, best);

    let xf = ((cfg.xfade_ms * sr as f32) / 1000.0).round().max(1.0) as usize;
    let parts: Vec<Vec<f32>> = chosen
        .iter()
        .map(|c| audio[c.input_index][c.start..c.end].to_vec())
        .collect();
    let mut samples = crossfade_concat(&parts, xf);
    let cap_n = ((cap_s as f64) * sr as f64).round() as usize;
    if samples.len() > cap_n && cap_n > 8 {
        let fade = (0.010 * sr as f32) as usize;
        edge_fade_tail(&mut samples[..cap_n], fade);
        samples.truncate(cap_n);
    }
    let fade = ((0.005 * sr as f32) as usize).min(samples.len() / 4);
    edge_fade(&mut samples, fade);

    let mut emb_parts = Vec::new();
    let mut weights = Vec::new();
    for cand in &chosen {
        let piece = &audio[cand.input_index][cand.start..cand.end];
        emb_parts.push(embed::embedding(piece, sr));
        weights.push((cand.end - cand.start) as f32);
    }
    let segment_mean = embed::average_embeddings(&emb_parts, Some(&weights));
    let assembled_emb = embed::embedding(&samples, sr);
    let embedding = if segment_mean.len() == EMBED_DIM && assembled_emb.len() == EMBED_DIM {
        embed::average_embeddings(&[segment_mean, assembled_emb], None)
    } else {
        assembled_emb
    };

    let assembled_s = samples.len() as f64 / sr as f64;
    let recommended = if short { 0.35 } else { 0.55 };
    let segments = chosen
        .iter()
        .map(|c| SegmentMeta {
            input_index: c.input_index,
            start_s: c.start as f64 / sr as f64,
            end_s: c.end as f64 / sr as f64,
            dur_s: (c.end - c.start) as f64 / sr as f64,
            snr_db: c.snr_db,
            voiced_ratio: c.voiced_ratio,
            pitch_coverage: c.pitch_coverage,
            score: c.score,
        })
        .collect();
    let meta = PlanMeta {
        schema: "galion-fidelity-plan/1",
        mode: mode.to_string(),
        sample_rate: sr,
        inputs: audio.len(),
        input_s,
        ideal_s: cfg.ideal_s as f64,
        short_below_s: cfg.short_below_s as f64,
        assembled_s,
        lm_conditioning_s: assembled_s.min(cfg.lm_s as f64),
        speaker_token_s: assembled_s.min(cfg.speaker_token_s as f64),
        speaker_pool_s: assembled_s,
        repeated: false,
        recommended_timbre_strength: recommended,
        timbre_applied: false,
        min_score: min_score as f64,
        segments,
        embedding_dim: embedding.len(),
        embedding,
        notes: vec![
            "LM conditioning is the prefix of the assembled wav. The highest-scoring segment is placed first.".into(),
            "Speaker tokens use the prefix up to 10 s. The speaker embedding pools the full assembled clip (capped at ideal_s).".into(),
            "Short references (under 8 s, including a ~6 s best piece) are not repeated to fill 50 s.".into(),
            "Multi-reference averaging assumes one speaker. Do not average Samantha with Jarvis.".into(),
            "Spectral-envelope correction is an opt-in post layer on the clone, not baked into this reference.".into(),
            "This tool does not evaluate consent. Clone-time consent stays a single galion_consent::evaluate in crates/galion-consent.".into(),
        ],
    };
    Ok(Prepared { samples, meta })
}

fn score_rank(score: f64) -> i64 {
    (score * 100.0).round() as i64
}

fn overlaps(a: &Cand, b: &Cand) -> bool {
    a.input_index == b.input_index && a.start < b.end && b.start < a.end
}

fn assembled_estimate(chosen: &[Cand], sr: u32, xfade_s: f32) -> f32 {
    if chosen.is_empty() {
        return 0.0;
    }
    let sum: f32 = chosen
        .iter()
        .map(|c| (c.end - c.start) as f32 / sr as f32)
        .sum();
    let fade = xfade_s * (chosen.len() - 1) as f32;
    (sum - fade).max(0.0)
}

fn consider(
    cands: &[Cand],
    chosen: &mut Vec<Cand>,
    min_score: f64,
    cap_s: f32,
    sr: u32,
    xfade_s: f32,
    stop_after_s: Option<f32>,
) {
    for cand in cands {
        if cand.score + 1e-12 < min_score {
            continue;
        }
        if chosen.iter().any(|other| overlaps(other, cand)) {
            continue;
        }
        let acc = assembled_estimate(chosen, sr, xfade_s);
        if let Some(stop) = stop_after_s {
            if !chosen.is_empty() && acc >= stop - 1e-3 {
                break;
            }
        }
        let dur = (cand.end - cand.start) as f32 / sr as f32;
        let extra = if chosen.is_empty() {
            dur
        } else {
            (dur - xfade_s).max(0.0)
        };
        if !chosen.is_empty() && acc + extra > cap_s + 0.02 {
            continue;
        }
        chosen.push(cand.clone());
    }
}

pub fn crossfade_concat(parts: &[Vec<f32>], xf: usize) -> Vec<f32> {
    let mut out = Vec::new();
    for part in parts {
        if out.is_empty() || out.len() < xf || part.len() < xf {
            out.extend_from_slice(part);
            continue;
        }
        let start = out.len() - xf;
        for i in 0..xf {
            let t = i as f32 / xf as f32;
            out[start + i] = out[start + i] * (1.0 - t) + part[i] * t;
        }
        out.extend_from_slice(&part[xf..]);
    }
    out
}

fn edge_fade(y: &mut [f32], n: usize) {
    let n = n.min(y.len() / 2);
    for i in 0..n {
        let g = i as f32 / n.max(1) as f32;
        y[i] *= g;
        let j = y.len() - 1 - i;
        y[j] *= g;
    }
}

fn edge_fade_tail(y: &mut [f32], n: usize) {
    let n = n.min(y.len());
    let start = y.len() - n;
    for i in 0..n {
        let g = 1.0 - (i as f32 / n.max(1) as f32);
        y[start + i] *= g;
    }
}
