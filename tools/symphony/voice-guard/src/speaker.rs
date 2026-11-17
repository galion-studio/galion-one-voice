// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Speaker drift against a pluggable baseline.
//!
//! TractPrint v2 is `galion_articulation::score`. The ECAPA cosine is the
//! caller's. This file does not run galion-ecapa and does not load its weights.

use crate::checks::{self, Prepared};
use crate::config::Config;
use crate::report::{round6, Check};
use galion_articulation::acoustic;
use galion_articulation::score;

pub struct Baseline<'a> {
    pub samples: &'a [f32],
    pub sample_rate_hz: u32,
}

pub fn drift(
    clip: &Prepared,
    baseline: Option<&Baseline<'_>>,
    embedding_cos: Option<f64>,
    cfg: &Config,
) -> Check {
    let method = "TractPrint v2 (galion-articulation score). Optional embedding cosine is caller-supplied from galion-ecapa";
    if baseline.is_none() && embedding_cos.is_none() {
        return checks::stub(
            "speaker_drift",
            "no baseline clip and no embedding cosine. TractPrint v2 and galion-ecapa were not run",
            method,
        );
    }
    if let Some(cos) = embedding_cos {
        if !cos.is_finite() {
            return Check {
                name: "speaker_drift",
                status: "fail",
                score: Some(0.0),
                value: None,
                limit: Some(cfg.speaker.min_embedding_cos),
                unit: "cosine",
                reason: "embedding cosine is not finite".into(),
                frames: Vec::new(),
                method,
            };
        }
        if cos <= cfg.speaker.min_embedding_cos {
            return Check {
                name: "speaker_drift",
                status: "fail",
                score: Some(round6(cos.clamp(0.0, 1.0))),
                value: Some(round6(cos)),
                limit: Some(cfg.speaker.min_embedding_cos),
                unit: "cosine",
                reason: format!(
                    "ECAPA cosine {cos:.4} is not above {}",
                    cfg.speaker.min_embedding_cos
                ),
                frames: whole(clip),
                method,
            };
        }
    }
    let Some(base) = baseline else {
        let cos = embedding_cos.unwrap_or(0.0);
        return Check {
            name: "speaker_drift",
            status: "pass",
            score: Some(round6(cos.clamp(0.0, 1.0))),
            value: Some(round6(cos)),
            limit: Some(cfg.speaker.min_embedding_cos),
            unit: "cosine",
            reason: format!("caller ECAPA cosine {cos:.4} is above the merge-v2 gate. No audio baseline, so TractPrint was not run"),
            frames: Vec::new(),
            method,
        };
    };
    if base.sample_rate_hz != clip.sample_rate_hz {
        return Check {
            name: "speaker_drift",
            status: "fail",
            score: Some(0.0),
            value: Some(base.sample_rate_hz as f64),
            limit: Some(clip.sample_rate_hz as f64),
            unit: "Hz",
            reason: format!(
                "baseline {} Hz does not match the clip {} Hz",
                base.sample_rate_hz, clip.sample_rate_hz
            ),
            frames: whole(clip),
            method,
        };
    }
    if base.samples.is_empty() || clip.clean.is_empty() {
        return Check {
            name: "speaker_drift",
            status: "fail",
            score: Some(0.0),
            value: None,
            limit: Some(cfg.speaker.min_speaker),
            unit: "speaker",
            reason: "baseline or clip is empty; TractPrint cannot run".into(),
            frames: whole(clip),
            method,
        };
    }
    let clip_f64: Vec<f64> = clip.clean.iter().map(|s| *s as f64).collect();
    let base_f64: Vec<f64> = base.samples.iter().copied().map(f64::from).collect();
    let (mut reference, _) = acoustic::analyse(&base_f64, clip.sample_rate_hz as f64);
    let (mut rendered, _) = acoustic::analyse(&clip_f64, clip.sample_rate_hz as f64);
    reference.finish();
    rendered.finish();
    let scored = score::compare_with(&reference, &rendered, embedding_cos);
    if scored.features.iter().all(|f| f.part != "speaker") {
        return Check {
            name: "speaker_drift",
            status: "fail",
            score: Some(0.0),
            value: None,
            limit: Some(cfg.speaker.min_speaker),
            unit: "speaker",
            reason: "TractPrint measured no speaker features (F0, vocal-tract length, F1-F3)"
                .into(),
            frames: whole(clip),
            method,
        };
    }
    let pass = scored.speaker >= cfg.speaker.min_speaker;
    Check {
        name: "speaker_drift",
        status: if pass { "pass" } else { "fail" },
        score: Some(round6(scored.speaker.clamp(0.0, 1.0))),
        value: Some(round6(scored.speaker)),
        limit: Some(cfg.speaker.min_speaker),
        unit: "speaker",
        reason: if pass {
            format!(
                "TractPrint speaker {:.3} is at or above {}",
                scored.speaker, cfg.speaker.min_speaker
            )
        } else {
            format!(
                "TractPrint speaker {:.3} is under {} (speaker drift)",
                scored.speaker, cfg.speaker.min_speaker
            )
        },
        frames: if pass { Vec::new() } else { whole(clip) },
        method,
    }
}

/// Median HNR on voiced frames, from the same praatfan pass TractPrint uses.
pub fn buzz(clip: &Prepared, min_hnr_db: f64) -> Check {
    let method = "praatfan harmonicity cc via galion-articulation (Boersma 1993 HNR)";
    if clip.clean.is_empty() {
        return checks::stub("buzz", "empty clip", method);
    }
    let samples: Vec<f64> = clip.clean.iter().map(|s| *s as f64).collect();
    let (mut art, _) = acoustic::analyse(&samples, clip.sample_rate_hz as f64);
    art.finish();
    let Some(summary) = art.summary.get("hnr_db") else {
        return checks::stub(
            "buzz",
            "no voiced HNR. Unvoiced audio is not scored as buzz",
            method,
        );
    };
    let Some(median) = summary.q.get(4).copied().filter(|v| v.is_finite()) else {
        return checks::stub("buzz", "HNR median is not finite", method);
    };
    let pass = median >= min_hnr_db;
    Check {
        name: "buzz",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(median)),
        limit: Some(min_hnr_db),
        unit: "dB",
        reason: if pass {
            format!("median HNR {median:.2} dB is at or above {min_hnr_db:.0}")
        } else {
            format!("median HNR {median:.2} dB is under {min_hnr_db:.0} (buzz: noise dominates harmonics)")
        },
        frames: if pass { Vec::new() } else { whole(clip) },
        method,
    }
}

fn whole(clip: &Prepared) -> Vec<crate::report::FrameSpan> {
    if clip.n_frames == 0 {
        Vec::new()
    } else {
        vec![crate::report::FrameSpan {
            start: 0,
            end: clip.n_frames,
        }]
    }
}
