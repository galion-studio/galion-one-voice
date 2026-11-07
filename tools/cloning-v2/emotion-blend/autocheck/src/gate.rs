//! Adoption gate: keep a candidate only if WER <= wer_max AND ECAPA > ecapa_min.
//! Defaults follow the Samantha merge-v2 production baseline (WER 0.018, ECAPA 0.7057).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GateThresholds {
    pub wer_max: f64,
    pub ecapa_min: f64,
}

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            wer_max: 0.018,
            ecapa_min: 0.7057,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GateScores {
    pub wer_mean: f64,
    pub ecapa_mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateResult {
    pub keep: bool,
    pub wer_ok: bool,
    pub ecapa_ok: bool,
    pub wer_mean: f64,
    pub ecapa_mean: f64,
    pub wer_max: f64,
    pub ecapa_min: f64,
    pub reason: String,
}

/// Strict inequality on ECAPA (must *improve* over the baseline).
/// Inclusive on WER (must stay at or below the allowed word-error rate).
pub fn evaluate(scores: GateScores, thr: GateThresholds) -> GateResult {
    let wer_ok = scores.wer_mean <= thr.wer_max;
    let ecapa_ok = scores.ecapa_mean > thr.ecapa_min;
    let keep = wer_ok && ecapa_ok;
    let reason = if keep {
        format!(
            "KEEP: WER {:.4} <= {:.4} and ECAPA {:.4} > {:.4}",
            scores.wer_mean, thr.wer_max, scores.ecapa_mean, thr.ecapa_min
        )
    } else if !wer_ok && !ecapa_ok {
        format!(
            "REJECT: WER {:.4} > {:.4} and ECAPA {:.4} <= {:.4}",
            scores.wer_mean, thr.wer_max, scores.ecapa_mean, thr.ecapa_min
        )
    } else if !wer_ok {
        format!(
            "REJECT: WER {:.4} > {:.4} (ECAPA {:.4} ok vs {:.4})",
            scores.wer_mean, thr.wer_max, scores.ecapa_mean, thr.ecapa_min
        )
    } else {
        format!(
            "REJECT: ECAPA {:.4} <= {:.4} (WER {:.4} ok vs {:.4})",
            scores.ecapa_mean, thr.ecapa_min, scores.wer_mean, thr.wer_max
        )
    };
    GateResult {
        keep,
        wer_ok,
        ecapa_ok,
        wer_mean: scores.wer_mean,
        ecapa_mean: scores.ecapa_mean,
        wer_max: thr.wer_max,
        ecapa_min: thr.ecapa_min,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thr_sam() -> GateThresholds {
        GateThresholds::default()
    }

    #[test]
    fn samantha_exact_baseline_rejected_ecapa_not_strictly_better() {
        // Production merged Samantha sits exactly on the ECAPA floor — must NOT keep.
        let r = evaluate(
            GateScores {
                wer_mean: 0.018,
                ecapa_mean: 0.7057,
            },
            thr_sam(),
        );
        assert!(!r.keep);
        assert!(r.wer_ok);
        assert!(!r.ecapa_ok);
        assert!(r.reason.contains("REJECT"));
    }

    #[test]
    fn keep_when_wer_at_cap_and_ecapa_improves() {
        let r = evaluate(
            GateScores {
                wer_mean: 0.018,
                ecapa_mean: 0.7058,
            },
            thr_sam(),
        );
        assert!(r.keep);
        assert!(r.wer_ok && r.ecapa_ok);
    }

    #[test]
    fn reject_when_wer_above_cap_even_if_ecapa_better() {
        let r = evaluate(
            GateScores {
                wer_mean: 0.019,
                ecapa_mean: 0.72,
            },
            thr_sam(),
        );
        assert!(!r.keep);
        assert!(!r.wer_ok);
        assert!(r.ecapa_ok);
    }

    #[test]
    fn reject_when_both_fail() {
        let r = evaluate(
            GateScores {
                wer_mean: 0.05,
                ecapa_mean: 0.70,
            },
            thr_sam(),
        );
        assert!(!r.keep);
        assert!(!r.wer_ok && !r.ecapa_ok);
        assert!(r.reason.contains("WER") && r.reason.contains("ECAPA"));
    }

    #[test]
    fn jarvis_thresholds_custom() {
        let thr = GateThresholds {
            wer_max: 0.019,
            ecapa_min: 0.7331,
        };
        // Exact floor → reject (strict ECAPA).
        let r = evaluate(
            GateScores {
                wer_mean: 0.019,
                ecapa_mean: 0.7331,
            },
            thr,
        );
        assert!(!r.keep);
        let r2 = evaluate(
            GateScores {
                wer_mean: 0.019,
                ecapa_mean: 0.7332,
            },
            thr,
        );
        assert!(r2.keep);
    }

    #[test]
    fn floating_edge_wer_just_over() {
        let r = evaluate(
            GateScores {
                wer_mean: 0.0180001,
                ecapa_mean: 0.80,
            },
            thr_sam(),
        );
        assert!(!r.keep);
        assert!(!r.wer_ok);
    }
}
