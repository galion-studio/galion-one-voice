// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Rollout gate copied from the checks that already exist.
//!
//! Thresholds: `tools/emotion-v2/thresholds.toml`.
//! Comparisons: `tools/emotion-v2/harness/metrics.py` (`wer_passes`,
//! `ecapa_passes`, `meets_min`) and `judge_rollout` in
//! `tools/emotion-v2/harness/sweep.py`.
//! The same three numbers are recorded, not applied, in
//! `tools/symphony/listen-back` (`ROLLOUT_WER_MAX`, `ROLLOUT_ECAPA_MIN`,
//! `ROLLOUT_EMOTION_MIN`).
//!
//! ECAPA is a strict greater-than. A cosine of exactly 0.7057 is the
//! merge-v2 baseline and is not a pass. The swarm brief wrote `>=`. This
//! crate follows the locked comparison in the harness.

/// Inclusive ceiling. `tools/emotion-v2/harness/sweep.py` `WER_MAX`.
pub const WER_MAX: f64 = 0.018;
/// Strict floor. Equality fails. `ECAPA_MIN` in the same file.
pub const ECAPA_MIN: f64 = 0.7057;
/// Inclusive floor. `EMOTION_MIN`.
pub const EMOTION_MIN: f64 = 0.83;
/// `EMOTION_FORMULA` in the sweep harness.
pub const EMOTION_FORMULA: &str = "0.70*category_blend+0.30*emb_sim";
pub const CATEGORY_W: f64 = 0.70;
pub const EMB_W: f64 = 0.30;

/// Heartbeat period. `galion_heartbeat::FRAME_MS` and LOCKS `[clock] frame_ms`.
/// Also the streaming bar this sector is asked to measure against.
pub const STREAM_BAR_MS: f64 = 80.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    Unmeasured,
    Error,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::Unmeasured => "UNMEASURED",
            Status::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub name: String,
    pub status: Status,
    pub comparison: String,
    pub threshold: Option<f64>,
    pub value: Option<f64>,
    pub detail: String,
}

impl Check {
    pub fn unmeasured(name: &str, comparison: &str, threshold: Option<f64>, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            status: Status::Unmeasured,
            comparison: comparison.to_string(),
            threshold,
            value: None,
            detail: detail.to_string(),
        }
    }

    pub fn error(name: &str, comparison: &str, threshold: Option<f64>, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            status: Status::Error,
            comparison: comparison.to_string(),
            threshold,
            value: None,
            detail: detail.to_string(),
        }
    }
}

pub fn isclose(left: f64, right: f64, abs_tol: f64) -> bool {
    (left - right).abs() <= abs_tol
}

/// Inclusive. The ceiling itself passes.
pub fn wer_passes(wer_mean: f64, wer_max: f64) -> bool {
    wer_mean.is_finite() && (wer_mean <= wer_max || isclose(wer_mean, wer_max, 1e-12))
}

/// Strict. The baseline itself is not an improvement.
pub fn ecapa_passes(ecapa_mean: f64, ecapa_min: f64) -> bool {
    if !ecapa_mean.is_finite() {
        return false;
    }
    if isclose(ecapa_mean, ecapa_min, 1e-12) {
        return false;
    }
    ecapa_mean > ecapa_min
}

/// Inclusive lower bound. The target itself passes.
pub fn meets_min(score: f64, target: f64) -> bool {
    score.is_finite() && (score >= target || isclose(score, target, 1e-9))
}

pub fn emotion_match_value(category_blend: f64, emb_sim: f64) -> f64 {
    CATEGORY_W * category_blend + EMB_W * emb_sim
}

pub fn judge_wer(value: Option<f64>, detail: &str, ran: bool) -> Check {
    judge_metric("wer", "<=", WER_MAX, value, detail, ran, wer_passes)
}

pub fn judge_ecapa(value: Option<f64>, detail: &str, ran: bool) -> Check {
    judge_metric("ecapa", ">", ECAPA_MIN, value, detail, ran, ecapa_passes)
}

pub fn judge_emotion(value: Option<f64>, detail: &str, ran: bool) -> Check {
    judge_metric(
        "emotion_match",
        ">=",
        EMOTION_MIN,
        value,
        detail,
        ran,
        meets_min,
    )
}

fn judge_metric(
    name: &str,
    comparison: &str,
    threshold: f64,
    value: Option<f64>,
    detail: &str,
    ran: bool,
    accept: fn(f64, f64) -> bool,
) -> Check {
    let Some(value) = value else {
        return if ran {
            Check::error(name, comparison, Some(threshold), detail)
        } else {
            Check::unmeasured(name, comparison, Some(threshold), detail)
        };
    };
    if !value.is_finite() {
        return Check::error(
            name,
            comparison,
            Some(threshold),
            &format!("{detail} (non-finite value, not a measurement)"),
        );
    }
    let passed = accept(value, threshold);
    Check {
        name: name.to_string(),
        status: if passed { Status::Pass } else { Status::Fail },
        comparison: comparison.to_string(),
        threshold: Some(threshold),
        value: Some(value),
        detail: detail.to_string(),
    }
}

/// Same combination as `judge_rollout`. A missing number is not a pass.
/// `claimed` stays false: a measured pass is not a release.
pub fn judge_rollout(wer: &Check, ecapa: &Check, emotion: &Check) -> Rollout {
    let metrics = [wer, ecapa, emotion];
    if metrics.iter().any(|item| item.status == Status::Error) {
        return Rollout {
            status: Status::Error,
            pass: None,
            claimed: false,
            reason: "a scorer ran and failed; that is not a measurement and not a pass".into(),
        };
    }
    if metrics
        .iter()
        .any(|item| item.status != Status::Pass && item.status != Status::Fail)
    {
        let missing: Vec<&str> = metrics
            .iter()
            .filter(|item| item.status != Status::Pass && item.status != Status::Fail)
            .map(|item| item.name.as_str())
            .collect();
        return Rollout {
            status: Status::Unmeasured,
            pass: None,
            claimed: false,
            reason: format!(
                "unmeasured: {}. No number was invented, and an absent scorer is not a pass.",
                missing.join(", ")
            ),
        };
    }
    let pass = metrics.iter().all(|item| item.status == Status::Pass);
    Rollout {
        status: if pass { Status::Pass } else { Status::Fail },
        pass: Some(pass),
        claimed: false,
        reason: if pass {
            "measured against the locked rollout gate".into()
        } else {
            "measured; at least one guard missed the gate".into()
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rollout {
    pub status: Status,
    pub pass: Option<bool>,
    pub claimed: bool,
    pub reason: String,
}

pub fn judge_latency(name: &str, elapsed_ms: Option<f64>, detail: &str) -> Check {
    judge_metric(
        name,
        "<=",
        STREAM_BAR_MS,
        elapsed_ms,
        detail,
        elapsed_ms.is_some(),
        |v, bar| v.is_finite() && v <= bar,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_wer(value: f64) -> Check {
        judge_wer(Some(value), "test", true)
    }
    fn ok_ecapa(value: f64) -> Check {
        judge_ecapa(Some(value), "test", true)
    }
    fn ok_emotion(value: f64) -> Check {
        judge_emotion(Some(value), "test", true)
    }

    #[test]
    fn boundaries_match_the_emotion_harness() {
        let passing = judge_rollout(
            &ok_wer(0.018),
            &ok_ecapa(ECAPA_MIN + 1e-6),
            &ok_emotion(0.83),
        );
        assert_eq!(passing.status, Status::Pass);
        assert_eq!(passing.pass, Some(true));
        assert!(!passing.claimed);

        let ceiling = judge_rollout(&ok_wer(0.018 + 1e-6), &ok_ecapa(0.8), &ok_emotion(1.0));
        assert_eq!(ceiling.pass, Some(false));
        assert_eq!(ceiling.status, Status::Fail);

        let baseline = judge_rollout(&ok_wer(0.0), &ok_ecapa(ECAPA_MIN), &ok_emotion(1.0));
        assert_eq!(baseline.status, Status::Fail);
        assert_eq!(baseline.pass, Some(false));

        let short = judge_rollout(&ok_wer(0.0), &ok_ecapa(0.9), &ok_emotion(0.83 - 1e-4));
        assert_eq!(short.pass, Some(false));
    }

    #[test]
    fn absent_is_not_a_pass_and_carries_no_number() {
        let gate = judge_rollout(
            &Check::unmeasured("wer", "<=", Some(WER_MAX), "missing"),
            &Check::unmeasured("ecapa", ">", Some(ECAPA_MIN), "missing"),
            &Check::unmeasured("emotion_match", ">=", Some(EMOTION_MIN), "missing"),
        );
        assert_eq!(gate.status, Status::Unmeasured);
        assert_eq!(gate.pass, None);
        assert!(!gate.claimed);
        assert!(gate.reason.contains("unmeasured"));
    }

    #[test]
    fn emotion_formula_is_the_locked_weights() {
        let score = emotion_match_value(1.0, 0.0);
        assert!((score - 0.70).abs() < 1e-12);
        assert!(!meets_min(score, EMOTION_MIN));
        assert!(meets_min(0.83, 0.83));
        assert!(!ecapa_passes(0.7057, 0.7057));
        assert!(wer_passes(0.018, 0.018));
    }
}
