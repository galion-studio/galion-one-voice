// Galion Voice — Perpetual Open MIT License
// SPDX-License-Identifier: MIT
//! FiLM affect conditioning (interface + stub), counterbalance policy check,
//! and the 3 % feeling-band budget helper (A9).
//!
//! ## FiLM
//! Feature-wise Linear Modulation (Perez et al., 2018): a target affect frame
//! is projected to per-channel `(gamma, beta)` and applied to a hidden state
//! `h` of the speech decoder as `h' = gamma ⊙ h + beta`. This is how the
//! *outbound* feeling band (Galion's target, from [`crate::express::EmpathyPolicy`])
//! steers the student's depth decoder without a separate TTS-prosody model.
//!
//! * `None` target (ADAPT off / no consent) -> **identity** (gamma = 1, beta = 0):
//!   Galion speaks neutral-warm from its own voice prior.
//! * Neutral-warm target -> near-identity by construction (inputs are
//!   centred on the neutral-warm point).
//! * [`apply_film`] never allocates: safe inside the 80 ms frame loop.
//! * [`StubFilm`] uses fixed deterministic weights (no model, no download).
//!   The real projection is a tiny trained linear layer shipped in `.gq`
//!   alongside the student; the trait does not change.
//!
//! ## Counterbalance, never mirror
//! Galion does **not** mirror the caller's affect. Anger is met with calm,
//! lower-dominance warmth; anxiety with slow steadiness; sadness/tiredness
//! with gentle, low-arousal warmth. **Joy is the only mirrored state** (shared,
//! slightly below the caller). Galion's target valence is never negative.
//! [`check_counterbalance`] encodes that rule so any policy (stub, learned,
//! demo) can be gated. The Python demo currently *mirrors* (pitch tracks
//! caller emotion) and fails this rule — see `A09-emotion-film.md`.
//!
//! ## 3 % budget
//! The whole feeling path (detect -> store -> policy -> prosody plan -> FiLM)
//! must cost ≤ 3 % of one 80 ms heartbeat = **2.4 ms** per frame on the
//! target CPU. [`measure_feeling`] / [`within_budget`] check it.

use std::time::{Duration, Instant};

use crate::frame::{Affect, EmotionFrame};

// ---------------------------------------------------------------- FiLM ----

/// Per-channel FiLM parameters, pre-allocated once for a decoder width.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmParams {
    pub gamma: Vec<f32>,
    pub beta: Vec<f32>,
}

impl FilmParams {
    /// Identity modulation for `dim` channels (allocates; do at setup).
    pub fn identity(dim: usize) -> Self {
        Self { gamma: vec![1.0; dim], beta: vec![0.0; dim] }
    }
    pub fn dim(&self) -> usize {
        self.gamma.len()
    }
    pub fn set_identity(&mut self) {
        self.gamma.iter_mut().for_each(|g| *g = 1.0);
        self.beta.iter_mut().for_each(|b| *b = 0.0);
    }
    pub fn is_identity(&self) -> bool {
        self.gamma.iter().all(|&g| g == 1.0) && self.beta.iter().all(|&b| b == 0.0)
    }
}

/// Affect -> FiLM parameters. Implementations must write into `out` without
/// allocating and must produce identity for `None`.
pub trait FilmConditioner {
    fn dim(&self) -> usize;
    fn condition(&self, target: Option<&EmotionFrame>, out: &mut FilmParams);
}

/// Apply FiLM in place. `h.len()` must be a multiple of `p.dim()` (frames × channels).
/// Zero allocation.
pub fn apply_film(h: &mut [f32], p: &FilmParams) {
    let d = p.dim();
    assert!(d > 0 && h.len() % d == 0, "hidden len must be a multiple of FiLM dim");
    for row in h.chunks_exact_mut(d) {
        for ((x, g), b) in row.iter_mut().zip(&p.gamma).zip(&p.beta) {
            *x = *g * *x + *b;
        }
    }
}

/// Neutral-warm point the stub centres on (matches `StubExpressor::respond(None)`).
pub const NEUTRAL_WARM: [f32; 3] = [0.3, 0.35, 0.5];

/// Deterministic stub conditioner: `gamma = 1 + s·tanh(W_g·x)`, `beta = s·tanh(W_b·x)`,
/// `x = confidence · (VAD − neutral_warm)`. `strength` bounds the modulation.
#[derive(Debug, Clone)]
pub struct StubFilm {
    dim: usize,
    strength: f32,
    w_gamma: Vec<[f32; 3]>,
    w_beta: Vec<[f32; 3]>,
}

impl StubFilm {
    pub fn new(dim: usize, strength: f32) -> Self {
        // Tiny LCG: reproducible, dependency-free "weights".
        let mut s: u32 = 0x9e37_79b9;
        let mut next = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        };
        let mut w = || [next(), next(), next()];
        let w_gamma = (0..dim).map(|_| w()).collect();
        let w_beta = (0..dim).map(|_| w()).collect();
        Self { dim, strength: strength.clamp(0.0, 1.0), w_gamma, w_beta }
    }
}

impl FilmConditioner for StubFilm {
    fn dim(&self) -> usize {
        self.dim
    }
    fn condition(&self, target: Option<&EmotionFrame>, out: &mut FilmParams) {
        debug_assert_eq!(out.dim(), self.dim);
        let Some(t) = target else {
            out.set_identity();
            return;
        };
        let t = t.clamped();
        let c = t.confidence;
        let x = [
            c * (t.valence - NEUTRAL_WARM[0]),
            c * (t.arousal - NEUTRAL_WARM[1]),
            c * (t.dominance - NEUTRAL_WARM[2]),
        ];
        let dot = |w: &[f32; 3]| w[0] * x[0] + w[1] * x[1] + w[2] * x[2];
        for i in 0..self.dim {
            out.gamma[i] = 1.0 + self.strength * dot(&self.w_gamma[i]).tanh();
            out.beta[i] = self.strength * dot(&self.w_beta[i]).tanh();
        }
    }
}

// ------------------------------------------------------- counterbalance ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PolicyViolation {
    /// Galion's target valence went negative (never allowed).
    NegativeValence { target: f32 },
    /// High-arousal negative caller (angry/anxious) met with ≥ arousal: mirroring.
    ArousalMirrored { caller: f32, target: f32 },
    /// Angry caller met with ≥ dominance: escalation.
    DominanceEscalated { caller: f32, target: f32 },
    /// Sad/tired caller pushed to much higher energy ("cheer up!").
    OverEnergised { caller: f32, target: f32 },
}

/// Max arousal Galion may add above a sad/tired caller (beyond the
/// neutral-warm floor, which is always allowed).
pub const LOW_AROUSAL_HEADROOM: f32 = 0.15;

/// Counterbalance-not-mirror rule. Joy/Excited may be mirrored; every other
/// state must be met with non-negative valence, and negative states with
/// counterbalancing arousal/dominance.
pub fn check_counterbalance(caller: &EmotionFrame, target: &EmotionFrame) -> Result<(), PolicyViolation> {
    let (c, t) = (caller.clamped(), target.clamped());
    if t.valence < 0.0 {
        return Err(PolicyViolation::NegativeValence { target: t.valence });
    }
    match c.label() {
        Affect::Joy | Affect::Excited | Affect::Neutral | Affect::Calm => Ok(()),
        Affect::Angry | Affect::Anxious => {
            if t.arousal >= c.arousal {
                return Err(PolicyViolation::ArousalMirrored { caller: c.arousal, target: t.arousal });
            }
            if c.label() == Affect::Angry && t.dominance >= c.dominance {
                return Err(PolicyViolation::DominanceEscalated { caller: c.dominance, target: t.dominance });
            }
            Ok(())
        }
        Affect::Sad | Affect::Tired => {
            // Gentle presence (up to neutral-warm arousal) is always fine.
            if t.arousal > (c.arousal + LOW_AROUSAL_HEADROOM).max(NEUTRAL_WARM[1]) {
                return Err(PolicyViolation::OverEnergised { caller: c.arousal, target: t.arousal });
            }
            Ok(())
        }
    }
}

// --------------------------------------------------------------- budget ----

/// Heartbeat length (LOCK: 80 ms).
pub const FRAME: Duration = Duration::from_millis(80);
/// Feeling band share of the frame budget (LOCK: ≤ 3 %).
pub const FEELING_BUDGET_PCT: u32 = 3;

/// 3 % of `frame` (80 ms -> 2.4 ms).
pub fn feeling_budget(frame: Duration) -> Duration {
    frame * FEELING_BUDGET_PCT / 100
}

pub fn within_budget(elapsed: Duration, frame: Duration) -> bool {
    elapsed <= feeling_budget(frame)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BudgetReport {
    pub iters: usize,
    pub median: Duration,
    pub p95: Duration,
    pub max: Duration,
    pub budget: Duration,
    /// Gate on p95 (not max: one OS hiccup must not fail CI; max is reported).
    pub ok: bool,
    /// p95 as a percentage of the frame.
    pub pct_of_frame: f32,
}

/// Time `iters` runs of one full feeling-path step `f` and compare p95 to 3 % of `frame`.
/// Allocates the sample buffer up front (measurement harness, not the frame loop).
pub fn measure_feeling<F: FnMut()>(iters: usize, frame: Duration, mut f: F) -> BudgetReport {
    let iters = iters.max(1);
    let mut s = Vec::with_capacity(iters);
    for _ in 0..iters {
        let t0 = Instant::now();
        f();
        s.push(t0.elapsed());
    }
    s.sort_unstable();
    let pick = |q: f32| s[((iters as f32 - 1.0) * q).round() as usize];
    let (median, p95, max) = (pick(0.5), pick(0.95), s[iters - 1]);
    let budget = feeling_budget(frame);
    BudgetReport {
        iters,
        median,
        p95,
        max,
        budget,
        ok: p95 <= budget,
        pct_of_frame: p95.as_secs_f32() / frame.as_secs_f32() * 100.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::express::{EmpathyPolicy, ProsodyExpressor, StubExpressor};

    fn f(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
        EmotionFrame { seq: 0, t_ms: 0, dur_ms: 80, valence: v, arousal: a, dominance: d, confidence: c }
    }

    #[test]
    fn none_target_is_identity() {
        let film = StubFilm::new(16, 0.2);
        let mut p = FilmParams::identity(16);
        p.gamma[0] = 7.0;
        film.condition(None, &mut p);
        assert!(p.is_identity());
        let mut h = vec![0.5f32; 32];
        apply_film(&mut h, &p);
        assert!(h.iter().all(|&x| x == 0.5));
    }

    #[test]
    fn neutral_warm_target_is_identity_and_strength_bounds() {
        let film = StubFilm::new(64, 0.2);
        let mut p = FilmParams::identity(64);
        film.condition(Some(&f(NEUTRAL_WARM[0], NEUTRAL_WARM[1], NEUTRAL_WARM[2], 1.0)), &mut p);
        assert!(p.is_identity());
        film.condition(Some(&f(1.0, 1.0, 1.0, 1.0)), &mut p);
        assert!(!p.is_identity());
        assert!(p.gamma.iter().all(|g| (0.8..=1.2).contains(g)));
        assert!(p.beta.iter().all(|b| (-0.2..=0.2).contains(b)));
    }

    #[test]
    fn film_is_deterministic_and_applies_per_row() {
        let a = StubFilm::new(4, 0.5);
        let b = StubFilm::new(4, 0.5);
        let (mut pa, mut pb) = (FilmParams::identity(4), FilmParams::identity(4));
        let t = f(-0.2, 0.9, 0.3, 0.8);
        a.condition(Some(&t), &mut pa);
        b.condition(Some(&t), &mut pb);
        assert_eq!(pa, pb);
        let mut h = vec![1.0f32, 2.0, 3.0, 4.0, 1.0, 2.0, 3.0, 4.0];
        apply_film(&mut h, &pa);
        assert_eq!(h[0..4], h[4..8]);
        assert!((h[1] - (pa.gamma[1] * 2.0 + pa.beta[1])).abs() < 1e-6);
    }

    #[test]
    #[should_panic]
    fn film_rejects_misaligned_hidden() {
        apply_film(&mut [0.0; 5], &FilmParams::identity(4));
    }

    #[test]
    fn naive_mirror_policy_is_rejected_for_negative_callers() {
        // W20-04: a "target = caller" policy (what the Python demo does) must fail.
        for c in [f(-0.7, 0.9, 0.8, 0.9), f(-0.5, 0.8, 0.3, 0.9), f(-0.6, 0.2, 0.3, 0.9)] {
            let lbl = c.clamped().label();
            if matches!(lbl, Affect::Angry | Affect::Anxious | Affect::Sad | Affect::Tired) {
                assert!(check_counterbalance(&c, &c).is_err(), "mirror passed for {lbl:?}");
            }
        }
        // Joy mirroring is allowed.
        let joy = f(0.8, 0.7, 0.5, 0.9);
        assert!(check_counterbalance(&joy, &joy).is_ok());
    }

    #[test]
    fn stub_policy_counterbalances_over_grid() {
        let p = StubExpressor;
        for v in [-1.0f32, -0.6, -0.3, 0.0, 0.3, 0.6, 1.0] {
            for a in [0.0f32, 0.2, 0.4, 0.6, 0.8, 1.0] {
                for d in [0.0f32, 0.5, 1.0] {
                    let c = f(v, a, d, 0.9);
                    let t = p.respond(Some(&c));
                    assert_eq!(check_counterbalance(&c, &t), Ok(()), "caller {c:?} -> {t:?}");
                }
            }
        }
    }

    #[test]
    fn mirror_policy_is_rejected() {
        // What galion-demo does today: target = caller.
        let angry = f(-0.8, 0.9, 0.8, 0.9);
        assert!(matches!(check_counterbalance(&angry, &angry), Err(PolicyViolation::NegativeValence { .. })));
        let mut warm_but_loud = angry;
        warm_but_loud.valence = 0.2;
        assert!(matches!(check_counterbalance(&angry, &warm_but_loud), Err(PolicyViolation::ArousalMirrored { .. })));
        let sad = f(-0.5, 0.1, 0.5, 0.9);
        let cheer = f(0.8, 0.9, 0.5, 1.0);
        assert!(matches!(check_counterbalance(&sad, &cheer), Err(PolicyViolation::OverEnergised { .. })));
        let joy = f(0.8, 0.8, 0.5, 0.9);
        assert_eq!(check_counterbalance(&joy, &joy), Ok(()), "joy is the one mirrored state");
    }

    #[test]
    fn budget_math() {
        assert_eq!(feeling_budget(FRAME), Duration::from_micros(2400));
        assert!(within_budget(Duration::from_micros(2400), FRAME));
        assert!(!within_budget(Duration::from_micros(2401), FRAME));
    }

    #[test]
    fn full_feeling_path_within_3pct_budget() {
        use crate::band::consent_bits::{ADAPT, SENSE};
        use crate::consent::{DefaultConsentGate, Purpose};
        use crate::detect::{AffectDetector, EnergyStubDetector};
        use crate::store::AffectStore;

        let mut gate = DefaultConsentGate::default();
        gate.grant(0, "hr", Purpose::PersonalAssistant);
        let mut store = AffectStore::new(64).grant(&gate, Purpose::PersonalAssistant, SENSE | ADAPT).unwrap();
        let mut det = EnergyStubDetector::default();
        let exp = StubExpressor;
        let film = StubFilm::new(512, 0.2);
        let mut params = FilmParams::identity(512);
        let mut hidden = vec![0.1f32; 512];
        let pcm: Vec<f32> = (0..1920).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect(); // 80 ms @ 24 kHz
        let mut t = 0u32;
        let r = measure_feeling(200, FRAME, || {
            if let Ok(fr) = det.detect(&pcm, 24_000, t, &gate, Purpose::PersonalAssistant) {
                store.record(&fr);
            }
            let target = exp.respond(store.adapt_target().as_ref());
            let ctrl = exp.plan(&target);
            film.condition(Some(&target), &mut params);
            apply_film(&mut hidden, &params);
            std::hint::black_box((&ctrl, &hidden));
            t += 80;
        });
        eprintln!("A9 feeling-path budget: median {:?} p95 {:?} max {:?} budget {:?} ({:.3}% of frame)", r.median, r.p95, r.max, r.budget, r.pct_of_frame);
        assert!(r.ok, "feeling path p95 {:?} > budget {:?} ({:.2}% of frame)", r.p95, r.budget, r.pct_of_frame);
    }
}
