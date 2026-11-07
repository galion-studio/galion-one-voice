//! Gate logic (pure, unit-tested).
//!
//! A fix is KEPT only if ALL configured checks pass:
//!   WER            <= wer_max            (default 0.018; inclusive)
//!   ECAPA          >  ecapa_threshold    (default 0.7057; strict)
//!   emotion_match  >= emo_min            (when emotion gate enabled; higher is better)
//! Optionally (`require_beat_prefix`) the ECAPA bar is raised to
//! max(ecapa_threshold, voice's own pre-fix ECAPA).
//! Missing / NaN metrics always fail. Winner = highest ECAPA among kept fixes,
//! ties broken by lower WER, then fix order (a < b < c).
//!
//! Emotion match is skipped (treated as pass / not required) when
//! `emotion_enabled` is false — e.g. Jarvis with no emotion-moment config.

use crate::emotion_match::DEFAULT_EMO_MIN;

pub const DEFAULT_WER_MAX: f64 = 0.018;
pub const DEFAULT_ECAPA_THRESHOLD: f64 = 0.7057;
const EPS: f64 = 1e-9;

#[derive(Clone, Debug, PartialEq)]
pub struct GateCfg {
    pub wer_max: f64,
    pub ecapa_threshold: f64,
    pub require_beat_prefix: bool,
    /// When false, emotion_match is not gated (moment config absent / --skip-emotion).
    pub emotion_enabled: bool,
    pub emo_min: f64,
}

impl Default for GateCfg {
    fn default() -> Self {
        GateCfg {
            wer_max: DEFAULT_WER_MAX,
            ecapa_threshold: DEFAULT_ECAPA_THRESHOLD,
            require_beat_prefix: false,
            emotion_enabled: false,
            emo_min: DEFAULT_EMO_MIN,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub wer: f64,
    pub ecapa: f64,
    pub emotion_match: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    pub fix: String,
    pub keep: bool,
    pub wer: Option<f64>,
    pub ecapa: Option<f64>,
    pub emotion_match: Option<f64>,
    pub wer_max: f64,
    pub ecapa_bar: f64,
    pub emo_min: f64,
    pub emotion_enabled: bool,
    pub wer_ok: bool,
    pub ecapa_ok: bool,
    pub emotion_ok: bool,
    pub reasons: Vec<String>,
}

pub fn ecapa_bar(cfg: &GateCfg, prefix: Option<&Metrics>) -> f64 {
    match (cfg.require_beat_prefix, prefix) {
        (true, Some(p)) if p.ecapa.is_finite() => cfg.ecapa_threshold.max(p.ecapa),
        _ => cfg.ecapa_threshold,
    }
}

pub fn evaluate(cfg: &GateCfg, prefix: Option<&Metrics>, fix: &str, m: Option<&Metrics>) -> Decision {
    let bar = ecapa_bar(cfg, prefix);
    let mut reasons = Vec::new();
    let (wer_ok, ecapa_ok, emotion_ok, wer, ecapa, emotion_match) = match m {
        None => {
            reasons.push("not run / no metrics".to_string());
            (false, false, !cfg.emotion_enabled, None, None, None)
        }
        Some(m) => {
            let wer_ok = m.wer.is_finite() && m.wer >= 0.0 && m.wer <= cfg.wer_max + EPS;
            let ecapa_ok = m.ecapa.is_finite() && m.ecapa > bar + EPS;
            let emotion_ok = if !cfg.emotion_enabled {
                true
            } else {
                match m.emotion_match {
                    Some(e) if e.is_finite() => e + EPS >= cfg.emo_min,
                    _ => false,
                }
            };
            if !m.wer.is_finite() { reasons.push("WER missing/NaN".into()); }
            else if !wer_ok { reasons.push(format!("WER {:.5} > max {:.5}", m.wer, cfg.wer_max)); }
            if !m.ecapa.is_finite() { reasons.push("ECAPA missing/NaN".into()); }
            else if !ecapa_ok { reasons.push(format!("ECAPA {:.4} not > bar {:.4}", m.ecapa, bar)); }
            if cfg.emotion_enabled {
                match m.emotion_match {
                    None => reasons.push("emotion_match missing".into()),
                    Some(e) if !e.is_finite() => reasons.push("emotion_match NaN".into()),
                    Some(e) if e + EPS < cfg.emo_min => {
                        reasons.push(format!("emotion_match {:.4} < min {:.4}", e, cfg.emo_min));
                    }
                    _ => {}
                }
            }
            if wer_ok && ecapa_ok && emotion_ok {
                let mut ok = format!("WER {:.5} <= {:.5} and ECAPA {:.4} > {:.4}", m.wer, cfg.wer_max, m.ecapa, bar);
                if cfg.emotion_enabled {
                    ok.push_str(&format!(" and emotion_match {:.4} >= {:.4}", m.emotion_match.unwrap_or(f64::NAN), cfg.emo_min));
                }
                reasons.push(ok);
            }
            (wer_ok, ecapa_ok, emotion_ok, Some(m.wer), Some(m.ecapa), m.emotion_match)
        }
    };
    Decision {
        fix: fix.to_string(),
        keep: wer_ok && ecapa_ok && emotion_ok,
        wer, ecapa, emotion_match,
        wer_max: cfg.wer_max, ecapa_bar: bar, emo_min: cfg.emo_min,
        emotion_enabled: cfg.emotion_enabled,
        wer_ok, ecapa_ok, emotion_ok, reasons,
    }
}

/// Index of the winning decision (kept, highest ECAPA, then lower WER, then earliest).
pub fn pick_winner(ds: &[Decision]) -> Option<usize> {
    let mut best: Option<usize> = None;
    for (i, d) in ds.iter().enumerate() {
        if !d.keep { continue; }
        best = match best {
            None => Some(i),
            Some(j) => {
                let (bi, bj) = (d.ecapa.unwrap(), ds[j].ecapa.unwrap());
                if bi > bj + EPS { Some(i) }
                else if (bi - bj).abs() <= EPS && d.wer.unwrap() + EPS < ds[j].wer.unwrap() { Some(i) }
                else { Some(j) }
            }
        };
    }
    best
}

pub fn winner_name(persona: &str, fix: &str, exists: &dyn Fn(&str) -> bool) -> String {
    let base = format!("own-{}-v2-blend-{}", persona, fix);
    if !exists(&base) { return base; }
    let mut k = 2;
    loop {
        let n = format!("{}-{}", base, k);
        if !exists(&n) { return n; }
        k += 1;
    }
}

pub fn persona_of(voice: &str) -> String {
    let s = voice.strip_suffix(".cbxvoice").unwrap_or(voice);
    let s = s.strip_prefix("own-").unwrap_or(s);
    let s = match s.find("-v2") { Some(i) if i > 0 => &s[..i], _ => s };
    s.to_string()
}

pub fn is_protected(voice: &str) -> bool {
    let v = voice.strip_suffix(".cbxvoice").unwrap_or(voice);
    v == "own-samantha" || v == "own-jarvis" || voice.contains(".bak")
}

pub fn watch_eligible(file_name: &str) -> bool {
    file_name.starts_with("own-") && file_name.ends_with(".cbxvoice") && !file_name.contains(".bak") && !file_name.contains("-v2-blend-")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(wer: f64, ecapa: f64) -> Metrics { Metrics { wer, ecapa, emotion_match: None } }
    fn me(wer: f64, ecapa: f64, emo: f64) -> Metrics { Metrics { wer, ecapa, emotion_match: Some(emo) } }

    #[test]
    fn keeps_when_both_pass() {
        let d = evaluate(&GateCfg::default(), None, "a", Some(&m(0.010, 0.7100)));
        assert!(d.keep && d.wer_ok && d.ecapa_ok && d.emotion_ok);
    }
    #[test]
    fn wer_boundary_inclusive() {
        let d = evaluate(&GateCfg::default(), None, "a", Some(&m(0.018, 0.72)));
        assert!(d.keep, "WER == 0.018 must pass (<=)");
        let d = evaluate(&GateCfg::default(), None, "a", Some(&m(0.0181, 0.72)));
        assert!(!d.keep && !d.wer_ok && d.ecapa_ok);
    }
    #[test]
    fn wer_raw_merge_v2_value_passes() {
        assert!(evaluate(&GateCfg::default(), None, "b", Some(&m(0.01764, 0.71))).wer_ok);
    }
    #[test]
    fn ecapa_strictly_greater() {
        let d = evaluate(&GateCfg::default(), None, "a", Some(&m(0.0, 0.7057)));
        assert!(!d.keep && !d.ecapa_ok, "ECAPA == threshold must fail (>)");
        assert!(evaluate(&GateCfg::default(), None, "a", Some(&m(0.0, 0.7058))).keep);
    }
    #[test]
    fn both_fail() {
        let d = evaluate(&GateCfg::default(), None, "c", Some(&m(0.05, 0.69)));
        assert!(!d.keep && !d.wer_ok && !d.ecapa_ok);
        assert_eq!(d.reasons.len(), 2);
    }
    #[test]
    fn missing_or_nan_fails() {
        assert!(!evaluate(&GateCfg::default(), None, "a", None).keep);
        assert!(!evaluate(&GateCfg::default(), None, "a", Some(&m(f64::NAN, 0.8))).keep);
        assert!(!evaluate(&GateCfg::default(), None, "a", Some(&m(0.0, f64::NAN))).keep);
        assert!(!evaluate(&GateCfg::default(), None, "a", Some(&m(-0.1, 0.8))).keep);
    }
    #[test]
    fn per_voice_threshold() {
        let cfg = GateCfg { ecapa_threshold: 0.7331, ..Default::default() };
        assert!(!evaluate(&cfg, None, "a", Some(&m(0.01, 0.7200))).keep);
        assert!(evaluate(&cfg, None, "a", Some(&m(0.01, 0.7400))).keep);
    }
    #[test]
    fn custom_wer_max() {
        let cfg = GateCfg { wer_max: 0.03, ..Default::default() };
        assert!(evaluate(&cfg, None, "a", Some(&m(0.025, 0.71))).keep);
    }
    #[test]
    fn beat_prefix_raises_bar_only_when_enabled() {
        let pre = m(0.02, 0.7200);
        let off = GateCfg::default();
        assert!(evaluate(&off, Some(&pre), "a", Some(&m(0.01, 0.7100))).keep);
        let on = GateCfg { require_beat_prefix: true, ..Default::default() };
        let d = evaluate(&on, Some(&pre), "a", Some(&m(0.01, 0.7100)));
        assert!(!d.keep);
        assert!((d.ecapa_bar - 0.72).abs() < 1e-12);
        let low = m(0.02, 0.69);
        assert!((evaluate(&on, Some(&low), "a", Some(&m(0.0, 0.71))).ecapa_bar - 0.7057).abs() < 1e-12);
    }
    #[test]
    fn winner_highest_ecapa_then_wer_then_order() {
        let cfg = GateCfg::default();
        let ds = vec![
            evaluate(&cfg, None, "a", Some(&m(0.010, 0.7150))),
            evaluate(&cfg, None, "b", Some(&m(0.005, 0.7200))),
            evaluate(&cfg, None, "c", Some(&m(0.030, 0.7500))),
        ];
        assert_eq!(pick_winner(&ds), Some(1));
        let ds = vec![
            evaluate(&cfg, None, "a", Some(&m(0.010, 0.7200))),
            evaluate(&cfg, None, "b", Some(&m(0.005, 0.7200))),
        ];
        assert_eq!(pick_winner(&ds), Some(1), "tie on ECAPA -> lower WER");
        let ds = vec![
            evaluate(&cfg, None, "a", Some(&m(0.005, 0.7200))),
            evaluate(&cfg, None, "b", Some(&m(0.005, 0.7200))),
        ];
        assert_eq!(pick_winner(&ds), Some(0), "full tie -> earliest");
    }
    #[test]
    fn no_winner_when_none_pass() {
        let cfg = GateCfg::default();
        let ds = vec![evaluate(&cfg, None, "a", Some(&m(0.05, 0.75))), evaluate(&cfg, None, "b", None)];
        assert_eq!(pick_winner(&ds), None);
        assert_eq!(pick_winner(&[]), None);
    }
    #[test]
    fn naming_and_persona() {
        assert_eq!(persona_of("own-samantha-v2-emo-r2"), "samantha");
        assert_eq!(persona_of("own-bob.cbxvoice"), "bob");
        assert_eq!(persona_of("own-mary-ann"), "mary-ann");
        let taken = |n: &str| n == "own-bob-v2-blend-a" || n == "own-bob-v2-blend-a-2";
        assert_eq!(winner_name("bob", "a", &taken), "own-bob-v2-blend-a-3");
        assert_eq!(winner_name("bob", "c", &taken), "own-bob-v2-blend-c");
    }
    #[test]
    fn protection_and_watch_filter() {
        assert!(is_protected("own-samantha") && is_protected("own-jarvis.cbxvoice"));
        assert!(is_protected("own-jarvis.cbxvoice.bak-pre-v2-20261105"));
        assert!(!is_protected("own-samantha-v2-emo-r2"));
        assert!(watch_eligible("own-new.cbxvoice"));
        assert!(!watch_eligible("own-new-v2-blend-a.cbxvoice"));
        assert!(!watch_eligible("own-samantha-fp16.cbxvoice.bak"));
        assert!(!watch_eligible("piper-jarvis.cbxvoice"));
    }

    #[test]
    fn emotion_gate_skipped_when_disabled() {
        let cfg = GateCfg::default(); // emotion_enabled=false
        assert!(evaluate(&cfg, None, "a", Some(&m(0.01, 0.71))).keep);
    }

    #[test]
    fn emotion_gate_requires_threshold_when_enabled() {
        let cfg = GateCfg { emotion_enabled: true, emo_min: 0.60, ..Default::default() };
        assert!(!evaluate(&cfg, None, "a", Some(&m(0.01, 0.71))).keep, "missing emo fails");
        assert!(!evaluate(&cfg, None, "a", Some(&me(0.01, 0.71, 0.59))).keep);
        let d = evaluate(&cfg, None, "a", Some(&me(0.01, 0.71, 0.60)));
        assert!(d.keep && d.emotion_ok);
        assert!(evaluate(&cfg, None, "a", Some(&me(0.01, 0.71, 0.70))).keep);
    }

    #[test]
    fn emotion_boundary_inclusive() {
        let cfg = GateCfg { emotion_enabled: true, emo_min: 0.55, ..Default::default() };
        assert!(evaluate(&cfg, None, "a", Some(&me(0.01, 0.71, 0.55))).keep);
    }
}
