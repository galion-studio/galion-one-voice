// Galion Voice — Perpetual Open MIT License
//! Affect *estimator* seam (A11 wave 2): what turns a PCM window into raw
//! valence / arousal / dominance / confidence, **without** consent or smoothing.
//!
//! ```text
//!  caller PCM ─► GatedDetector<E>  (gate FIRST, input checks, seq, EMA)  ─► EmotionFrame
//!                     │
//!                     └─ E: AffectEstimator   ← swap this, nothing else changes
//!                          • LoudnessEstimator   heuristic, RMS/ZCR (runtime stub; no valence)
//!                          • SyntheticEstimator  deterministic fixture modes (tests / console / Rika)
//!                          • ReferenceEstimator  replays an OFFLINE label track (e.g. a SenseVoice-Small
//!                                                dev-oracle run done elsewhere) — reference only
//!                          • (later) learned affect head on student Mimi latents, `.gq` weights
//! ```
//!
//! Why: the A9 `EnergyStubDetector` hard-coded `valence = 0.0` inside the
//! detector. Valence now comes from [`Estimate::valence`], an `Option`:
//! `None` = "this estimator has no valence opinion" (the frame carries 0.0 and
//! [`GatedDetector::valence_known`] is false), so a later estimator can supply
//! it without touching the consent / store / band path.
//!
//! Hard rules (unchanged from A9): the gate is checked before `pcm` is read;
//! no model is loaded, downloaded or trained here; SenseVoice-Small is an
//! **offline reference only** (A9 §6, K16: no onnx / sherpa-onnx at runtime) —
//! [`ReferenceEstimator`] just replays labels someone produced offline.

use crate::consent::{ConsentGate, Purpose};
use crate::detect::{AffectDetector, DetectError};
use crate::frame::EmotionFrame;

/// Where an estimate comes from (shown in diagnostics; gates may refuse some kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimatorKind {
    /// Cheap DSP heuristic (loudness). Runtime-allowed stub.
    Heuristic,
    /// Deterministic fixture generator. Tests / demos only.
    Synthetic,
    /// Replay of an offline reference run (dev oracle). Never a runtime model.
    OfflineReference,
    /// Learned head on student latents (not built yet).
    Learned,
}

impl EstimatorKind {
    pub fn label(self) -> &'static str {
        match self {
            EstimatorKind::Heuristic => "heuristic",
            EstimatorKind::Synthetic => "synthetic",
            EstimatorKind::OfflineReference => "offline-reference",
            EstimatorKind::Learned => "learned",
        }
    }
}

/// Raw, unsmoothed estimate for one window. Ranges as in [`EmotionFrame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Estimate {
    /// `None` = no valence opinion (frame carries 0.0). Loudness cannot know valence.
    pub valence: Option<f32>,
    pub arousal: f32,
    pub dominance: f32,
    pub confidence: f32,
}

impl Estimate {
    pub const UNKNOWN: Estimate = Estimate { valence: None, arousal: 0.3, dominance: 0.5, confidence: 0.0 };
}

/// PCM window → raw affect. No consent logic here: [`GatedDetector`] owns it.
pub trait AffectEstimator {
    fn kind(&self) -> EstimatorKind;
    fn name(&self) -> &'static str;
    /// `pcm` is non-empty mono f32 in [-1, 1], `sample_rate > 0`, consent already Allow.
    fn estimate(&mut self, pcm: &[f32], sample_rate: u32, t_ms: u32) -> Estimate;
    /// Drop internal state (withdrawal / hang-up).
    fn reset(&mut self) {}
}

// ---------------------------------------------------------------------------
// Loudness (today's runtime stub)
// ---------------------------------------------------------------------------

/// RMS / zero-crossing heuristic — exactly the A9 `EnergyStubDetector` maths.
/// Arousal ~ loudness, dominance ~ loudness − ZCR, **valence unknown**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoudnessEstimator {
    /// RMS that maps to arousal 1.0 (~shouting).
    pub full_scale_rms: f32,
    /// Below this RMS the window is silence → confidence 0.
    pub silence_rms: f32,
    /// Confidence reported for non-silent windows (low on purpose).
    pub confidence: f32,
}

impl Default for LoudnessEstimator {
    fn default() -> Self {
        Self { full_scale_rms: 0.3, silence_rms: 1e-4, confidence: 0.3 }
    }
}

/// Frame RMS and zero-crossing rate (crossings per sample).
pub fn rms_zcr(pcm: &[f32]) -> (f32, f32) {
    if pcm.is_empty() {
        return (0.0, 0.0);
    }
    let n = pcm.len() as f32;
    let rms = (pcm.iter().map(|x| x * x).sum::<f32>() / n).sqrt();
    let zc = pcm.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count() as f32 / n;
    (rms, zc)
}

/// Acoustic features this crate already computes for one window.
/// Recognition reads these. It does not load weights.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AcousticFeatures {
    pub rms: f32,
    pub zcr: f32,
}

impl AcousticFeatures {
    pub fn from_pcm(pcm: &[f32]) -> Self {
        let (rms, zcr) = rms_zcr(pcm);
        Self { rms, zcr }
    }
}

impl LoudnessEstimator {
    /// Same mapping as [`AffectEstimator::estimate`]. Valence stays `None`.
    pub fn estimate_features(&self, features: AcousticFeatures) -> Estimate {
        let arousal = (features.rms / self.full_scale_rms).min(1.0);
        Estimate {
            valence: None,
            arousal,
            dominance: 0.4 + 0.3 * arousal - 0.2 * features.zcr.min(1.0),
            confidence: if features.rms < self.silence_rms { 0.0 } else { self.confidence },
        }
    }
}

impl AffectEstimator for LoudnessEstimator {
    fn kind(&self) -> EstimatorKind {
        EstimatorKind::Heuristic
    }
    fn name(&self) -> &'static str {
        "loudness"
    }
    fn estimate(&mut self, pcm: &[f32], _sample_rate: u32, _t_ms: u32) -> Estimate {
        self.estimate_features(AcousticFeatures::from_pcm(pcm))
    }
}

// ---------------------------------------------------------------------------
// Synthetic fixtures
// ---------------------------------------------------------------------------

/// Deterministic fixture modes (one step per call = one 80 ms frame).
#[derive(Debug, Clone, PartialEq)]
pub enum SyntheticMode {
    /// Valence 0, arousal 0.3, confidence 0 — reads as "unknown".
    Neutral,
    /// Fixed VAD regardless of audio.
    Constant { valence: f32, arousal: f32, confidence: f32 },
    /// Loudness arousal/dominance from the PCM, plus a fixed valence.
    LoudnessWithValence { valence: f32 },
    /// Valence = amp·sin(2π·frame/period); arousal from loudness. Exercises sign changes.
    Sweep { period_frames: u32, valence_amp: f32, confidence: f32 },
    /// Cycle through (valence, arousal, confidence) per frame.
    Script(Vec<(f32, f32, f32)>),
}

impl SyntheticMode {
    /// Parse a CLI name: `neutral`, `sweep`, `joy`, `angry`, `sad`, `tired`, `calm`, `valence:<v>`.
    pub fn parse(s: &str) -> Option<SyntheticMode> {
        Some(match s {
            "neutral" => SyntheticMode::Neutral,
            "sweep" => SyntheticMode::Sweep { period_frames: 50, valence_amp: 0.8, confidence: 0.6 },
            "joy" => SyntheticMode::Constant { valence: 0.7, arousal: 0.55, confidence: 0.7 },
            "angry" => SyntheticMode::Constant { valence: -0.7, arousal: 0.85, confidence: 0.7 },
            "sad" => SyntheticMode::Constant { valence: -0.6, arousal: 0.4, confidence: 0.7 },
            "tired" => SyntheticMode::Constant { valence: -0.4, arousal: 0.15, confidence: 0.7 },
            "calm" => SyntheticMode::Constant { valence: 0.2, arousal: 0.2, confidence: 0.7 },
            _ => {
                let v: f32 = s.strip_prefix("valence:")?.parse().ok()?;
                if !(-1.0..=1.0).contains(&v) {
                    return None;
                }
                SyntheticMode::LoudnessWithValence { valence: v }
            }
        })
    }
}

/// Fixture estimator: valence without a model, for tests and the console.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticEstimator {
    pub mode: SyntheticMode,
    loud: LoudnessEstimator,
    frame: u32,
}

impl SyntheticEstimator {
    pub fn new(mode: SyntheticMode) -> Self {
        Self { mode, loud: LoudnessEstimator::default(), frame: 0 }
    }
}

impl AffectEstimator for SyntheticEstimator {
    fn kind(&self) -> EstimatorKind {
        EstimatorKind::Synthetic
    }
    fn name(&self) -> &'static str {
        match self.mode {
            SyntheticMode::Neutral => "synthetic-neutral",
            SyntheticMode::Constant { .. } => "synthetic-constant",
            SyntheticMode::LoudnessWithValence { .. } => "synthetic-valence",
            SyntheticMode::Sweep { .. } => "synthetic-sweep",
            SyntheticMode::Script(_) => "synthetic-script",
        }
    }
    fn estimate(&mut self, pcm: &[f32], sr: u32, t_ms: u32) -> Estimate {
        let k = self.frame;
        self.frame = self.frame.wrapping_add(1);
        let loud = self.loud.estimate(pcm, sr, t_ms);
        match &self.mode {
            SyntheticMode::Neutral => Estimate { valence: Some(0.0), ..Estimate::UNKNOWN },
            SyntheticMode::Constant { valence, arousal, confidence } => {
                Estimate { valence: Some(*valence), arousal: *arousal, dominance: 0.5, confidence: *confidence }
            }
            SyntheticMode::LoudnessWithValence { valence } => Estimate { valence: Some(*valence), ..loud },
            SyntheticMode::Sweep { period_frames, valence_amp, confidence } => {
                let p = (*period_frames).max(1) as f32;
                let v = valence_amp * (std::f32::consts::TAU * (k as f32) / p).sin();
                Estimate { valence: Some(v), confidence: *confidence, ..loud }
            }
            SyntheticMode::Script(steps) if !steps.is_empty() => {
                let (v, a, c) = steps[k as usize % steps.len()];
                Estimate { valence: Some(v), arousal: a, dominance: 0.5, confidence: c }
            }
            SyntheticMode::Script(_) => Estimate::UNKNOWN,
        }
    }
    fn reset(&mut self) {
        self.frame = 0;
    }
}

// ---------------------------------------------------------------------------
// Offline reference (SenseVoice-Small dev-oracle seam) — replay only
// ---------------------------------------------------------------------------

/// Categorical emotion labels as emitted by SenseVoice-style SER tags
/// (`<|HAPPY|>`, `<|SAD|>`, …). Parsing only; no model here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefLabel {
    Neutral,
    Happy,
    Sad,
    Angry,
    Fearful,
    Disgusted,
    Surprised,
    Unknown,
}

impl RefLabel {
    /// Accepts `HAPPY`, `happy`, `<|HAPPY|>`; `EMO_UNKNOWN` / anything else → `Unknown`.
    pub fn parse(s: &str) -> RefLabel {
        let t = s.trim().trim_start_matches("<|").trim_end_matches("|>").to_ascii_uppercase();
        match t.as_str() {
            "NEUTRAL" => RefLabel::Neutral,
            "HAPPY" => RefLabel::Happy,
            "SAD" => RefLabel::Sad,
            "ANGRY" => RefLabel::Angry,
            "FEARFUL" => RefLabel::Fearful,
            "DISGUSTED" => RefLabel::Disgusted,
            "SURPRISED" => RefLabel::Surprised,
            _ => RefLabel::Unknown,
        }
    }

    /// Fixed label → (valence, arousal, dominance) table (A9 §6 stage A).
    /// `None` for `Unknown`.
    pub fn vad(self) -> Option<(f32, f32, f32)> {
        Some(match self {
            RefLabel::Neutral => (0.0, 0.3, 0.5),
            RefLabel::Happy => (0.7, 0.6, 0.6),
            RefLabel::Sad => (-0.6, 0.2, 0.3),
            RefLabel::Angry => (-0.7, 0.85, 0.8),
            RefLabel::Fearful => (-0.6, 0.75, 0.25),
            RefLabel::Disgusted => (-0.6, 0.5, 0.6),
            RefLabel::Surprised => (0.3, 0.8, 0.5),
            RefLabel::Unknown => return None,
        })
    }
}

/// One entry of an offline reference track: label active from `t_ms` on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RefEntry {
    pub t_ms: u32,
    pub label: RefLabel,
    /// Max posterior of the reference run → confidence.
    pub posterior: f32,
}

/// Parse a reference track: one `t_ms LABEL posterior` per line, `#` comments,
/// `t_ms` strictly increasing, posterior in 0..=1.
pub fn parse_reference_track(src: &str) -> Result<Vec<RefEntry>, String> {
    let mut out: Vec<RefEntry> = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(t), Some(l), Some(p), None) = (it.next(), it.next(), it.next(), it.next()) else {
            return Err(format!("ref track line {}: want `t_ms LABEL posterior`", i + 1));
        };
        let t_ms: u32 = t.parse().map_err(|_| format!("ref track line {}: bad t_ms {t:?}", i + 1))?;
        let posterior: f32 = p.parse().map_err(|_| format!("ref track line {}: bad posterior {p:?}", i + 1))?;
        if !(0.0..=1.0).contains(&posterior) {
            return Err(format!("ref track line {}: posterior {posterior} outside 0..=1", i + 1));
        }
        if out.last().is_some_and(|e| e.t_ms >= t_ms) {
            return Err(format!("ref track line {}: t_ms must increase", i + 1));
        }
        out.push(RefEntry { t_ms, label: RefLabel::parse(l), posterior });
    }
    if out.is_empty() {
        return Err("ref track: no entries".into());
    }
    Ok(out)
}

/// Replays an offline reference track (e.g. a SenseVoice-Small dev-oracle run
/// on a licensed clip, produced **off-box**). Valence/dominance from the label
/// table, confidence = posterior; arousal is the label's, nudged by live
/// loudness so the band still moves with the audio. Before the first entry or
/// on `Unknown` → no valence opinion, confidence 0.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceEstimator {
    track: Vec<RefEntry>,
    loud: LoudnessEstimator,
}

impl ReferenceEstimator {
    pub fn new(track: Vec<RefEntry>) -> Self {
        Self { track, loud: LoudnessEstimator::default() }
    }
    pub fn from_str_track(src: &str) -> Result<Self, String> {
        parse_reference_track(src).map(Self::new)
    }
    pub fn entry_at(&self, t_ms: u32) -> Option<&RefEntry> {
        self.track.iter().rev().find(|e| e.t_ms <= t_ms)
    }
}

impl AffectEstimator for ReferenceEstimator {
    fn kind(&self) -> EstimatorKind {
        EstimatorKind::OfflineReference
    }
    fn name(&self) -> &'static str {
        "offline-reference"
    }
    fn estimate(&mut self, pcm: &[f32], sr: u32, t_ms: u32) -> Estimate {
        let loud = self.loud.estimate(pcm, sr, t_ms);
        match self.entry_at(t_ms).and_then(|e| e.label.vad().map(|v| (v, e.posterior))) {
            Some(((v, a, d), p)) => Estimate { valence: Some(v), arousal: 0.75 * a + 0.25 * loud.arousal, dominance: d, confidence: p },
            None => Estimate { valence: None, confidence: 0.0, ..loud },
        }
    }
}

// ---------------------------------------------------------------------------
// GatedDetector: consent + checks + smoothing around any estimator
// ---------------------------------------------------------------------------

/// [`AffectDetector`] around any [`AffectEstimator`]: consent gate first (deny →
/// reset, no PCM read), input checks, `seq`, clamp, EMA (`smoothing`, 0.5 default).
#[derive(Debug, Clone)]
pub struct GatedDetector<E> {
    pub estimator: E,
    /// EMA weight of the new frame (1.0 = no smoothing).
    pub smoothing: f32,
    seq: u32,
    prev: Option<EmotionFrame>,
    valence_known: bool,
}

impl<E: Default> Default for GatedDetector<E> {
    fn default() -> Self {
        Self::new(E::default())
    }
}

impl<E> GatedDetector<E> {
    pub fn new(estimator: E) -> Self {
        Self { estimator, smoothing: 0.5, seq: 0, prev: None, valence_known: false }
    }
    /// Did the last successful estimate carry a valence opinion?
    pub fn valence_known(&self) -> bool {
        self.valence_known
    }
}

impl<E: AffectEstimator> AffectDetector for GatedDetector<E> {
    fn detect(
        &mut self,
        pcm: &[f32],
        sample_rate: u32,
        t_ms: u32,
        gate: &dyn ConsentGate,
        purpose: Purpose,
    ) -> Result<EmotionFrame, DetectError> {
        let decision = gate.check(purpose);
        gate.audit(purpose, decision);
        if !decision.allowed() {
            self.reset();
            return Err(DetectError::NotPermitted(decision));
        }
        if pcm.is_empty() {
            return Err(DetectError::BadInput("empty window"));
        }
        if sample_rate == 0 {
            return Err(DetectError::BadInput("sample_rate = 0"));
        }
        let e = self.estimator.estimate(pcm, sample_rate, t_ms);
        self.valence_known = e.valence.is_some();
        let dur_ms = ((pcm.len() as u64 * 1000) / sample_rate as u64).min(u16::MAX as u64) as u16;
        let raw = EmotionFrame {
            seq: self.seq,
            t_ms,
            dur_ms,
            valence: e.valence.unwrap_or(0.0),
            arousal: e.arousal,
            dominance: e.dominance,
            confidence: e.confidence,
        }
        .clamped();
        self.seq = self.seq.wrapping_add(1);
        let out = match self.prev {
            Some(p) => p.ema(&raw, self.smoothing),
            None => raw,
        };
        self.prev = Some(out);
        Ok(out)
    }

    fn reset(&mut self) {
        self.prev = None;
        self.valence_known = false;
        self.estimator.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::DefaultConsentGate;
    use crate::detect::EnergyStubDetector;

    fn tone(amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (i as f32 * 0.1).sin()).collect()
    }
    fn granted() -> DefaultConsentGate {
        let mut g = DefaultConsentGate::default();
        g.grant(0, "en", Purpose::PersonalAssistant);
        g
    }
    fn run<E: AffectEstimator>(d: &mut GatedDetector<E>, frames: u32) -> Vec<EmotionFrame> {
        let g = granted();
        (0..frames)
            .map(|k| d.detect(&tone(0.02 + 0.01 * k as f32, 1920), 24_000, k * 80, &g, Purpose::PersonalAssistant).unwrap())
            .collect()
    }

    #[test]
    fn loudness_gated_detector_is_bit_identical_to_energy_stub() {
        let g = granted();
        let mut a = EnergyStubDetector::default();
        let mut b = GatedDetector::new(LoudnessEstimator::default());
        for k in 0..20u32 {
            let pcm = tone(0.01 + 0.02 * (k % 7) as f32, 1920);
            let x = a.detect(&pcm, 24_000, k * 80, &g, Purpose::PersonalAssistant).unwrap();
            let y = b.detect(&pcm, 24_000, k * 80, &g, Purpose::PersonalAssistant).unwrap();
            assert_eq!(x, y);
        }
        assert!(!b.valence_known(), "loudness has no valence opinion");
    }

    #[test]
    fn gate_checked_before_estimator_runs() {
        struct Panics;
        impl AffectEstimator for Panics {
            fn kind(&self) -> EstimatorKind {
                EstimatorKind::Learned
            }
            fn name(&self) -> &'static str {
                "panics"
            }
            fn estimate(&mut self, _: &[f32], _: u32, _: u32) -> Estimate {
                panic!("estimator touched PCM without consent")
            }
        }
        let mut d = GatedDetector::new(Panics);
        let deny = DefaultConsentGate::default();
        assert!(matches!(d.detect(&tone(0.1, 1920), 24_000, 0, &deny, Purpose::PersonalAssistant), Err(DetectError::NotPermitted(_))));
    }

    #[test]
    fn sweep_produces_signed_valence() {
        let mut d = GatedDetector::new(SyntheticEstimator::new(SyntheticMode::parse("sweep").unwrap()));
        d.smoothing = 1.0;
        let f = run(&mut d, 50);
        assert!(d.valence_known());
        assert!(f.iter().any(|x| x.valence > 0.5) && f.iter().any(|x| x.valence < -0.5));
        assert!(f.iter().all(|x| (x.confidence - 0.6).abs() < 1e-6));
        assert!(f[10].arousal > f[0].arousal, "arousal still follows loudness");
    }

    #[test]
    fn synthetic_modes_parse_and_label() {
        use crate::frame::Affect;
        for (name, want) in [("joy", Affect::Joy), ("angry", Affect::Angry), ("sad", Affect::Sad), ("tired", Affect::Tired), ("calm", Affect::Calm), ("neutral", Affect::Neutral)] {
            let mut d = GatedDetector::new(SyntheticEstimator::new(SyntheticMode::parse(name).unwrap()));
            let f = run(&mut d, 1)[0];
            assert_eq!(f.label(), want, "{name}: {f:?}");
        }
        assert_eq!(SyntheticMode::parse("valence:-0.4"), Some(SyntheticMode::LoudnessWithValence { valence: -0.4 }));
        assert_eq!(SyntheticMode::parse("valence:3"), None);
        assert_eq!(SyntheticMode::parse("bogus"), None);
        let mut s = GatedDetector::new(SyntheticEstimator::new(SyntheticMode::Script(vec![(0.5, 0.5, 0.5), (-0.5, 0.2, 0.5)])));
        s.smoothing = 1.0;
        let f = run(&mut s, 3);
        assert_eq!((f[0].valence, f[1].valence, f[2].valence), (0.5, -0.5, 0.5));
    }

    #[test]
    fn reset_restarts_synthetic_sequence() {
        let mut d = GatedDetector::new(SyntheticEstimator::new(SyntheticMode::Script(vec![(0.1, 0.3, 0.5), (0.9, 0.3, 0.5)])));
        d.smoothing = 1.0;
        run(&mut d, 1);
        d.reset();
        assert_eq!(run(&mut d, 1)[0].valence, 0.1);
    }

    #[test]
    fn reference_track_parse_and_replay() {
        let src = "# synthetic track, NOT model output\n0 <|NEUTRAL|> 0.8\n160 HAPPY 0.7  # joy\n320 EMO_UNKNOWN 0.4\n";
        let mut d = GatedDetector::new(ReferenceEstimator::from_str_track(src).unwrap());
        d.smoothing = 1.0;
        let f = run(&mut d, 5);
        assert_eq!(f[0].valence, 0.0);
        assert!((f[0].confidence - 0.8).abs() < 1e-6);
        assert_eq!(f[2].valence, 0.7);
        assert_eq!(f[3].label(), crate::frame::Affect::Joy);
        assert_eq!(f[4].confidence, 0.0, "unknown label → no confidence");
        assert!(!d.valence_known());
        assert_eq!(d.estimator.kind(), EstimatorKind::OfflineReference);
    }

    #[test]
    fn acoustic_features_match_rms_zcr_and_carry_no_valence() {
        let est = LoudnessEstimator::default();
        let mut live = est;
        for amp in [0.0, 0.02, 0.2, 0.9] {
            let pcm = tone(amp, 1920);
            let features = AcousticFeatures::from_pcm(&pcm);
            let (rms, zcr) = rms_zcr(&pcm);
            assert_eq!(features.rms, rms);
            assert_eq!(features.zcr, zcr);
            let from_features = est.estimate_features(features);
            let from_pcm = live.estimate(&pcm, 24_000, 0);
            assert_eq!(from_features, from_pcm);
            assert!(from_features.valence.is_none());
        }
        let quiet = est.estimate_features(AcousticFeatures { rms: 0.2, zcr: 0.01 });
        let harsh = est.estimate_features(AcousticFeatures { rms: 0.2, zcr: 0.9 });
        assert!(harsh.dominance < quiet.dominance);
        assert_eq!(harsh.arousal, quiet.arousal);
        assert_eq!(est.estimate_features(AcousticFeatures { rms: 0.0, zcr: 0.0 }).confidence, 0.0);
    }

    #[test]
    fn reference_track_errors() {
        assert!(parse_reference_track("").is_err());
        assert!(parse_reference_track("0 HAPPY").is_err());
        assert!(parse_reference_track("0 HAPPY 1.5").is_err());
        assert!(parse_reference_track("80 HAPPY 0.5\n80 SAD 0.5").unwrap_err().contains("increase"));
        assert_eq!(RefLabel::parse("<|ANGRY|>"), RefLabel::Angry);
        assert_eq!(RefLabel::Unknown.vad(), None);
    }
}
