//! Outbound: one caller frame → Galion's voice and colour.
//!
//! [`StubExpressor::voice_color`] is the decision used on the feeling path.
//! It reads the caller once and returns the target, an empathy weight, the
//! prosody and an OKLCH colour. Hearing and seeing take that value. There is
//! no second estimator and no reply-text table.
//!
//! 1. [`EmpathyPolicy`] decides *what Galion should feel*. It does not mirror:
//!    anger is met with lower-dominance warmth, sadness with gentle warmth,
//!    joy is shared.
//! 2. [`ProsodyExpressor::plan`] is the VAD rule table (pitch, rate, energy,
//!    warmth, pause).
//! 3. Empathy is a continuous weight `e` in 0..=1 from the caller's consented
//!    frame, not a ninth [`crate::frame::Affect`] label. The 3-bit wire field
//!    is full. `voice_color` blends `plan` toward [`EMPATHY_PROSODY`] by `e`
//!    and blends the neutral mark toward [`EMPATHY_OKLCH`] by `e`.
//!
//! The locked circle stays `#F5F5F7`. `e` may pull the amber↔violet background
//! wash toward a mid mix. Spectrum does not set hue.
//!
//! The phone lane uses [`StubExpressor::phone_lane`]. Caller affect is read
//! there only when [`crate::consent::sense_granted`] is true. A session starts
//! denied, so the lane stays on the neutral-warm decision.

use crate::consent::{sense_granted, ConsentGate, Purpose};
use crate::frame::{Affect, EmotionFrame};

/// Decoder-agnostic prosody knobs. Neutral = all defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProsodyControl {
    /// Semitones relative to the voice's base pitch, clamped to ±4.
    pub pitch_shift_st: f32,
    /// Pitch range multiplier (expressiveness), 0.5..1.5.
    pub pitch_range: f32,
    /// Speaking-rate multiplier, 0.8..1.2.
    pub rate: f32,
    /// Loudness multiplier, 0.6..1.2.
    pub energy: f32,
    /// 0..1 spectral warmth / breathiness hint.
    pub warmth: f32,
    /// Extra pause before speaking, ms (0..400) — gives room to an upset caller.
    pub pre_pause_ms: u16,
}

impl Default for ProsodyControl {
    fn default() -> Self {
        Self { pitch_shift_st: 0.0, pitch_range: 1.0, rate: 1.0, energy: 1.0, warmth: 0.5, pre_pause_ms: 0 }
    }
}

impl ProsodyControl {
    pub fn clamped(mut self) -> Self {
        self.pitch_shift_st = self.pitch_shift_st.clamp(-4.0, 4.0);
        self.pitch_range = self.pitch_range.clamp(0.5, 1.5);
        self.rate = self.rate.clamp(0.8, 1.2);
        self.energy = self.energy.clamp(0.6, 1.2);
        self.warmth = self.warmth.clamp(0.0, 1.0);
        self.pre_pause_ms = self.pre_pause_ms.min(400);
        self
    }
}

pub trait EmpathyPolicy {
    /// Given the caller's (consented) frame, return Galion's target frame.
    /// `None` caller frame (no consent / unknown) -> neutral-warm default.
    fn respond(&self, caller: Option<&EmotionFrame>) -> EmotionFrame;
}

pub trait ProsodyExpressor {
    /// VAD rule table only. The consoling voice is [`StubExpressor::voice_color`]:
    /// `plan` by itself still raises pitch on the de-escalation targets.
    fn plan(&self, target: &EmotionFrame) -> ProsodyControl;
    /// Stub hook for applying controls directly to PCM (energy only here).
    fn apply_energy(&self, ctrl: &ProsodyControl, pcm: &mut [f32]) {
        for s in pcm.iter_mut() {
            *s = (*s * ctrl.energy).clamp(-1.0, 1.0);
        }
    }
}

/// Reference rule-based implementation of both traits.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubExpressor;

impl EmpathyPolicy for StubExpressor {
    fn respond(&self, caller: Option<&EmotionFrame>) -> EmotionFrame {
        let base = |seq, t, d| EmotionFrame {
            seq, t_ms: t, dur_ms: d, valence: 0.3, arousal: 0.35, dominance: 0.5, confidence: 1.0,
        };
        let Some(c) = caller else { return base(0, 0, 0) };
        let mut out = base(c.seq, c.t_ms, c.dur_ms);
        match c.label() {
            // De-escalate: calmer, lower dominance, still warm.
            Affect::Angry => { out.arousal = 0.2; out.dominance = 0.35; out.valence = 0.2; }
            Affect::Anxious => { out.arousal = 0.15; out.dominance = 0.55; out.valence = 0.3; }
            Affect::Sad | Affect::Tired => { out.arousal = 0.2; out.valence = 0.25; out.dominance = 0.45; }
            // Share positive energy, slightly below the caller.
            Affect::Joy | Affect::Excited => {
                out.valence = (c.valence * 0.9).max(0.3);
                out.arousal = (c.arousal * 0.85).max(0.35);
            }
            Affect::Calm | Affect::Neutral => {}
        }
        out.clamped()
    }
}

impl ProsodyExpressor for StubExpressor {
    fn plan(&self, t: &EmotionFrame) -> ProsodyControl {
        let t = t.clamped();
        let low_arousal_negative_context = t.arousal < 0.25;
        ProsodyControl {
            pitch_shift_st: 2.0 * t.valence + 1.5 * (t.arousal - 0.35),
            pitch_range: 0.8 + 0.6 * t.arousal,
            rate: 0.9 + 0.25 * t.arousal,
            energy: 0.75 + 0.4 * t.arousal,
            warmth: 0.5 + 0.4 * t.valence.max(0.0) + 0.1 * (1.0 - t.dominance),
            pre_pause_ms: if low_arousal_negative_context { 250 } else { 0 },
        }
        .clamped()
    }
}

/// Neutral mark `#F5F5F7` in OKLCH. The circle stays this colour under the lock.
pub const NEUTRAL_OKLCH: Oklch = Oklch { l: 0.971, c: 0.003, h: 286.0 };
/// Empathy anchor at `e = 1`. Heather. Hue is a design choice; see the sector note.
pub const EMPATHY_OKLCH: Oklch = Oklch { l: 0.72, c: 0.09, h: 310.0 };
/// Amber share of the locked background wash that empathy eases toward.
/// `0` is violet `#7C3AED`, `1` is amber `#F5A524`. The circle is not in this.
pub const EMPATHY_WASH_AMBER: f32 = 0.5;

/// Prosody at `e = 1`, then clamped by [`ProsodyControl::clamped`].
/// Directions: lower pitch, narrower range, slower, quieter, warmer.
/// Magnitudes are a design choice on top of those directions.
pub const EMPATHY_PROSODY: ProsodyControl = ProsodyControl {
    pitch_shift_st: -0.5,
    pitch_range: 0.88,
    rate: 0.92,
    energy: 0.80,
    warmth: 0.75,
    pre_pause_ms: 250,
};

/// OKLCH (L 0..=1, C chroma, h degrees). Hue is never taken from the spectrum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

impl Oklch {
    fn to_lab(self) -> [f32; 3] {
        let h = self.h.to_radians();
        [self.l, self.c * h.cos(), self.c * h.sin()]
    }

    fn from_lab(lab: [f32; 3]) -> Self {
        let c = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
        let mut h = lab[2].atan2(lab[1]).to_degrees();
        if h < 0.0 {
            h += 360.0;
        }
        Self { l: lab[0], c, h }
    }

    fn lerp_lab(self, other: Self, t: f32) -> Self {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        if t == 0.0 {
            return self;
        }
        if t == 1.0 {
            return other;
        }
        let a = self.to_lab();
        let b = other.to_lab();
        Self::from_lab([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t])
    }
}

/// One outbound decision for a single 80 ms frame.
/// Prosody and colour both come from `e`. No text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceColor {
    /// 0 = no empathy blend. 1 = [`EMPATHY_PROSODY`] and [`EMPATHY_OKLCH`].
    pub e: f32,
    /// Policy target. Valence stays non-negative.
    pub target: EmotionFrame,
    pub prosody: ProsodyControl,
    /// `lerp(neutral, heather, e)` in OKLab. Not painted on the locked circle.
    pub oklch: Oklch,
}

/// Empathy weight from the caller's frame.
///
/// `e = confidence · clamp(−valence / 0.6, 0, 1)` when the label is Sad, Tired,
/// Anxious or Angry. Otherwise 0, including no frame (no consent → nothing to
/// answer) and confidence below the label floor. Anger is in the set so it is
/// met with warmth; it is not copied.
pub fn empathy_weight(caller: Option<&EmotionFrame>) -> f32 {
    let Some(c) = caller else { return 0.0 };
    let c = c.clamped();
    match c.label() {
        Affect::Sad | Affect::Tired | Affect::Anxious | Affect::Angry => {
            let mag = (-c.valence / 0.6).clamp(0.0, 1.0);
            (c.confidence * mag).clamp(0.0, 1.0)
        }
        _ => 0.0,
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn blend_prosody(base: ProsodyControl, e: f32) -> ProsodyControl {
    if e <= 0.0 {
        return base;
    }
    if e >= 1.0 {
        return EMPATHY_PROSODY;
    }
    let e = e.clamp(0.0, 1.0);
    ProsodyControl {
        pitch_shift_st: lerp(base.pitch_shift_st, EMPATHY_PROSODY.pitch_shift_st, e),
        pitch_range: lerp(base.pitch_range, EMPATHY_PROSODY.pitch_range, e),
        rate: lerp(base.rate, EMPATHY_PROSODY.rate, e),
        energy: lerp(base.energy, EMPATHY_PROSODY.energy, e),
        warmth: lerp(base.warmth, EMPATHY_PROSODY.warmth, e),
        pre_pause_ms: lerp(base.pre_pause_ms as f32, EMPATHY_PROSODY.pre_pause_ms as f32, e).round() as u16,
    }
    .clamped()
}

impl StubExpressor {
    /// Shared outbound decision for this frame. Stack only.
    pub fn voice_color(&self, caller: Option<&EmotionFrame>) -> VoiceColor {
        let e = empathy_weight(caller);
        let target = self.respond(caller);
        let base = self.plan(&target);
        VoiceColor {
            e,
            target,
            prosody: blend_prosody(base, e),
            oklch: NEUTRAL_OKLCH.lerp_lab(EMPATHY_OKLCH, e),
        }
    }

    /// Phone-lane decision. Caller affect is applied only when sense is granted
    /// (in-band `SENSE` bit and an [`crate::consent::ConsentDecision::Allow`]).
    /// A denied session does not read `caller`; the result matches `voice_color(None)`.
    pub fn phone_lane<G: ConsentGate + ?Sized>(
        &self,
        caller: Option<&EmotionFrame>,
        bits: u8,
        purpose: Purpose,
        gate: &G,
    ) -> VoiceColor {
        let decision = gate.check(purpose);
        gate.audit(purpose, decision);
        let caller = if sense_granted(bits, decision) { caller } else { None };
        self.voice_color(caller)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f32, a: f32, d: f32) -> EmotionFrame {
        EmotionFrame { seq: 3, t_ms: 240, dur_ms: 80, valence: v, arousal: a, dominance: d, confidence: 0.9 }
    }

    #[test]
    fn angry_caller_gets_calm_galion() {
        let p = StubExpressor;
        let caller = f(-0.8, 0.9, 0.8);
        let voiced = p.voice_color(Some(&caller));
        assert!(voiced.target.arousal < caller.arousal);
        assert!(voiced.target.valence > 0.0);
        assert!(voiced.prosody.rate < 1.0 && voiced.prosody.energy < 1.0);
        assert_eq!(voiced.prosody.pre_pause_ms, 250);
        // The VAD table alone raises pitch here. The shared decision must not.
        let raw = p.plan(&voiced.target);
        assert!(raw.pitch_shift_st > 0.0, "unblended consoling pitch {raw:?}");
        assert!(voiced.prosody.pitch_shift_st < 0.0);
        assert!(voiced.prosody.pitch_shift_st < raw.pitch_shift_st);
        assert!(voiced.e > 0.8);
    }

    #[test]
    fn joyful_caller_is_shared() {
        let p = StubExpressor;
        let tgt = p.respond(Some(&f(0.8, 0.8, 0.5)));
        assert!(tgt.valence >= 0.7 && tgt.arousal >= 0.6);
        let ctrl = p.plan(&tgt);
        assert!(ctrl.pitch_shift_st > 0.0 && ctrl.rate > 1.0);
    }

    #[test]
    fn no_consent_yields_neutral_warm() {
        let p = StubExpressor;
        let tgt = p.respond(None);
        assert!(tgt.valence > 0.0);
        let c = p.plan(&tgt);
        assert!(c.warmth > 0.5);
    }

    #[test]
    fn controls_always_in_range() {
        let p = StubExpressor;
        for v in [-1.0f32, 0.0, 1.0] {
            for a in [0.0f32, 0.5, 1.0] {
                let c = p.plan(&f(v, a, 0.5));
                assert!((-4.0..=4.0).contains(&c.pitch_shift_st));
                assert!((0.8..=1.2).contains(&c.rate));
                assert!((0.6..=1.2).contains(&c.energy));
                assert!((0.0..=1.0).contains(&c.warmth));
            }
        }
    }

    #[test]
    fn apply_energy_scales_and_clips() {
        let p = StubExpressor;
        let mut pcm = vec![0.5, -0.9, 1.0];
        let c = ProsodyControl { energy: 1.2, ..Default::default() };
        p.apply_energy(&c, &mut pcm);
        assert!((pcm[0] - 0.6).abs() < 1e-6);
        assert_eq!(pcm[1], -1.0);
        assert_eq!(pcm[2], 1.0);
    }

    #[test]
    fn empathy_at_one_is_the_profile_and_heather() {
        let p = StubExpressor;
        // Tired: low arousal, negative valence. e = 1 · clamp(0.6/0.6, 0, 1).
        let caller = EmotionFrame { seq: 1, t_ms: 80, dur_ms: 80, valence: -0.6, arousal: 0.2, dominance: 0.4, confidence: 1.0 };
        let v = p.voice_color(Some(&caller));
        assert!((v.e - 1.0).abs() < 1e-6);
        assert_eq!(v.prosody.pitch_shift_st, EMPATHY_PROSODY.pitch_shift_st);
        assert_eq!(v.prosody.pitch_range, EMPATHY_PROSODY.pitch_range);
        assert_eq!(v.prosody.rate, EMPATHY_PROSODY.rate);
        assert_eq!(v.prosody.energy, EMPATHY_PROSODY.energy);
        assert_eq!(v.prosody.warmth, EMPATHY_PROSODY.warmth);
        assert_eq!(v.prosody.pre_pause_ms, 250);
        assert_eq!(v.oklch, EMPATHY_OKLCH);
        assert_eq!(oklch_hex(EMPATHY_OKLCH), "#B695CF");
        assert!(v.target.valence > 0.0, "empathy does not become the caller's sadness");
    }

    #[test]
    fn empathy_is_not_calm_or_grief_and_does_not_answer_joy() {
        let p = StubExpressor;
        let calm = p.plan(&f(0.15, 0.15, 0.5));
        let grief = p.plan(&f(-0.6, 0.35, 0.5));
        let emp = EMPATHY_PROSODY;
        assert!(emp.pitch_shift_st < calm.pitch_shift_st && emp.pitch_shift_st > grief.pitch_shift_st);
        assert!(emp.warmth > calm.warmth && emp.warmth > grief.warmth);
        assert!((EMPATHY_OKLCH.l - 0.86).abs() > 0.1 && (EMPATHY_OKLCH.l - 0.45).abs() > 0.1);
        assert!((EMPATHY_OKLCH.h - 195.0).abs() > 40.0 && (EMPATHY_OKLCH.h - 262.0).abs() > 20.0);

        let joy = p.voice_color(Some(&f(0.8, 0.8, 0.5)));
        assert_eq!(joy.e, 0.0);
        assert_eq!(joy.prosody, p.plan(&joy.target));
        assert_eq!(joy.oklch, NEUTRAL_OKLCH);
        assert_eq!(p.voice_color(None).e, 0.0);
        let unsure = EmotionFrame { confidence: 0.1, ..f(-0.9, 0.9, 0.9) };
        assert_eq!(p.voice_color(Some(&unsure)).e, 0.0, "low confidence is not a guess");
    }

    #[test]
    fn denied_session_does_not_apply_caller_affect_on_the_phone_lane() {
        use crate::band::consent_bits;
        use crate::consent::{ConsentDecision, DefaultConsentGate, Purpose};

        let p = StubExpressor;
        let caller = f(-0.8, 0.9, 0.8);
        let applied = p.voice_color(Some(&caller));
        let neutral = p.voice_color(None);
        assert_ne!(applied, neutral, "the fixture must be one the lane could leak");
        assert!(applied.e > 0.8);

        let denied = DefaultConsentGate::default();
        assert_eq!(denied.state, crate::consent::ConsentState::Denied);
        let bits = consent_bits::SENSE | consent_bits::ADAPT;
        let lane = p.phone_lane(Some(&caller), bits, Purpose::PersonalAssistant, &denied);
        assert_eq!(lane, neutral, "denied session applied caller affect");
        assert_ne!(lane.prosody, applied.prosody);
        assert_ne!(lane.target, applied.target);
        assert_eq!(lane.e, 0.0);

        let mut granted = DefaultConsentGate::denied();
        granted.grant(1, "en", Purpose::PersonalAssistant);
        let open = p.phone_lane(Some(&caller), bits, Purpose::PersonalAssistant, &granted);
        assert_eq!(open, applied, "sense granted admits the caller frame");
        let no_sense = p.phone_lane(Some(&caller), consent_bits::ADAPT, Purpose::PersonalAssistant, &granted);
        assert_eq!(no_sense, neutral, "adapt without sense does not admit caller affect");

        let mut workplace = DefaultConsentGate::denied();
        workplace.grant(1, "en", Purpose::Workplace);
        let blocked = p.phone_lane(Some(&caller), bits, Purpose::Workplace, &workplace);
        assert_eq!(blocked, neutral, "a prohibited purpose is still a denied session");
        assert_eq!(workplace.check(Purpose::Workplace), ConsentDecision::DenyProhibitedPurpose);
    }

    #[test]
    fn circle_stays_locked_while_empathy_only_names_a_wash() {
        let locks = include_str!("../../LOCKS.toml");
        assert!(locks.contains("dot_color = \"#F5F5F7\""), "lock moved; do not recolour the circle from this crate");
        assert!(locks.contains("dot_varies = [\"size\", \"brightness\"]"));
        assert!(locks.contains("gradient_scope = \"background\""));
        assert!(locks.contains("gradient_from = \"#F5A524\""));
        assert!(locks.contains("gradient_to = \"#7C3AED\""));
        assert_eq!(oklch_hex(NEUTRAL_OKLCH), "#F5F5F7");
        assert_eq!(EMPATHY_WASH_AMBER, 0.5);
        // Heather is the colour decision. It is not a dot fill.
        assert_ne!(oklch_hex(EMPATHY_OKLCH), "#F5F5F7");
    }

    /// Ottosson OKLab → sRGB. Test-only check that the anchor is the named hex.
    fn oklch_hex(c: Oklch) -> String {
        let [l, a, b] = c.to_lab();
        let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
        let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
        let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
        let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
        let r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        let g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        let b = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;
        let gamma = |x: f32| {
            if x >= 0.0031308 { 1.055 * x.powf(1.0 / 2.4) - 0.055 } else { 12.92 * x }
        };
        let ch = |x: f32| (gamma(x).clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("#{:02X}{:02X}{:02X}", ch(r), ch(g), ch(b))
    }
}
