// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Persona-conditioned voice (owner t641u / t644u): PersonaPlex-style
//! conditioning on the Piper path. Design note: `docs/galion-voice-v2/PERSONA-MOOD.md`.
//!
//! ```text
//!  user voice ─► GatedDetector (consent first) ─┐
//!  user text  ─► text_affect (lexicon)          ├─► UserObservation (fused VAD + confidence)
//!                                               ▼
//!                       per-persona appraisal (Samantha mirrors + softens,
//!                       Jarvis steadies + stays composed)
//!                                               ▼
//!                       PersonaMood (persistent VAD, inertia toward the appraisal,
//!                       exponential decay back to the persona's resting mood)
//!                                               ▼
//!          PersonaConditioning  ← one shared, versioned, fixed-size vector
//!            │                    (persona one-hot + mood VAD + user VAD/conf + e)
//!            ├─► PiperVoiceProfile::prosody() → PiperProsodyVector
//!            │     (length_scale, noise_scale, noise_w, sentence silence,
//!            │      pitch shift st, energy, pre-pause)       — today, Piper
//!            └─► ConditionedDecoder::condition()             — later, the in-house
//!                                                              dual-band model (PR #73)
//! ```
//!
//! V1 building blocks reused, not replaced: [`EmotionFrame`] (VAD + confidence),
//! [`empathy_weight`] (the `e` from EMPATHY.md), the consent-gated
//! [`crate::loudness::GatedDetector`] for the voice side, and the persona
//! register baselines of `personas/voice/{samantha,jarvis}.toml` (`e0` row) as
//! each persona's resting voice.
//!
//! Hot path ([`PersonaMood::observe`], [`PersonaMood::conditioning`],
//! [`PiperVoiceProfile::prosody`], [`text_affect`]) is stack-only: no heap
//! allocation (tested with a counting allocator in `tests/persona_mood_alloc.rs`).

use crate::express::empathy_weight;
use crate::frame::EmotionFrame;
use crate::loudness::Estimate;

/// The two V2 test personas. A future id is a new variant + profile, nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Persona {
    Samantha = 0,
    Jarvis = 1,
}

impl Persona {
    pub const ALL: [Persona; 2] = [Persona::Samantha, Persona::Jarvis];
    pub fn as_str(self) -> &'static str {
        match self {
            Persona::Samantha => "samantha",
            Persona::Jarvis => "jarvis",
        }
    }
    pub fn parse(s: &str) -> Option<Persona> {
        let s = s.trim();
        if s.eq_ignore_ascii_case("samantha") {
            Some(Persona::Samantha)
        } else if s.eq_ignore_ascii_case("jarvis") {
            Some(Persona::Jarvis)
        } else {
            None
        }
    }
    /// The persona's full profile (appraisal + mood dynamics + Piper voice).
    pub fn profile(self) -> &'static PersonaProfile {
        match self {
            Persona::Samantha => &SAMANTHA,
            Persona::Jarvis => &JARVIS,
        }
    }
}

// ---------------------------------------------------------------------------
// User emotion observation (voice and/or text)
// ---------------------------------------------------------------------------

/// Where the user-emotion reading came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObsSource {
    /// Consent-gated voice detector (galion-emotion `GatedDetector`).
    Voice,
    /// Lexical cue from the ASR transcript ([`text_affect`]).
    Text,
    /// Both, fused by confidence.
    Fused,
}

/// One reading of the user's emotion. VAD ranges as in [`EmotionFrame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserObservation {
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
    pub confidence: f32,
    pub source: ObsSource,
}

impl UserObservation {
    pub const NONE: UserObservation =
        UserObservation { valence: 0.0, arousal: 0.3, dominance: 0.5, confidence: 0.0, source: ObsSource::Text };

    pub fn from_frame(f: &EmotionFrame) -> Self {
        let f = f.clamped();
        Self { valence: f.valence, arousal: f.arousal, dominance: f.dominance, confidence: f.confidence, source: ObsSource::Voice }
    }

    pub fn from_text(e: Estimate) -> Self {
        Self {
            valence: e.valence.unwrap_or(0.0),
            arousal: e.arousal,
            dominance: e.dominance,
            confidence: if e.valence.is_some() { e.confidence } else { 0.0 },
            source: ObsSource::Text,
        }
    }

    /// Fuse voice and text. Voice carries arousal/dominance (loudness knows
    /// those); valence comes from whichever source has an opinion, weighted by
    /// confidence. `voice_valence_known = false` (the loudness stub) means the
    /// voice frame's 0.0 valence is "unknown", not "neutral".
    pub fn fuse(voice: Option<&EmotionFrame>, voice_valence_known: bool, text: Option<Estimate>) -> Self {
        let v = voice.map(Self::from_frame);
        let t = text.map(Self::from_text);
        match (v, t) {
            (None, None) => Self::NONE,
            (Some(v), None) => {
                if voice_valence_known { v } else { Self { valence: 0.0, ..v } }
            }
            (None, Some(t)) => t,
            (Some(v), Some(t)) => {
                let wv = if voice_valence_known { v.confidence } else { 0.0 };
                let wt = t.confidence;
                let valence = if wv + wt > 1e-6 { (v.valence * wv + t.valence * wt) / (wv + wt) } else { 0.0 };
                // Arousal: voice is the stronger evidence; text nudges it.
                let wa_v = v.confidence.max(1e-3);
                let wa_t = 0.5 * t.confidence;
                let arousal = (v.arousal * wa_v + t.arousal * wa_t) / (wa_v + wa_t);
                Self {
                    valence,
                    arousal,
                    dominance: v.dominance,
                    // Independent evidence: 1 − (1 − c_voice)(1 − c_text).
                    confidence: (1.0 - (1.0 - v.confidence) * (1.0 - t.confidence)).min(1.0),
                    source: ObsSource::Fused,
                }
            }
        }
        .clamped()
    }

    pub fn clamped(mut self) -> Self {
        let c = |x: f32, lo: f32, hi: f32, d: f32| if x.is_finite() { x.clamp(lo, hi) } else { d };
        self.valence = c(self.valence, -1.0, 1.0, 0.0);
        self.arousal = c(self.arousal, 0.0, 1.0, 0.3);
        self.dominance = c(self.dominance, 0.0, 1.0, 0.5);
        self.confidence = c(self.confidence, 0.0, 1.0, 0.0);
        self
    }

    /// As a V1 frame, so V1 rules ([`empathy_weight`], `label`) apply unchanged.
    pub fn as_frame(&self) -> EmotionFrame {
        EmotionFrame {
            seq: 0,
            t_ms: 0,
            dur_ms: 0,
            valence: self.valence,
            arousal: self.arousal,
            dominance: self.dominance,
            confidence: self.confidence,
        }
    }
}

/// (word, valence, arousal). Lower-case ASCII. Small on purpose: a cue, not a model.
const LEXICON: &[(&str, f32, f32)] = &[
    // positive, high arousal
    ("amazing", 0.8, 0.8), ("awesome", 0.8, 0.8), ("excited", 0.7, 0.9), ("wow", 0.6, 0.85),
    ("yay", 0.8, 0.85), ("fantastic", 0.8, 0.75), ("incredible", 0.7, 0.8), ("thrilled", 0.8, 0.9),
    ("won", 0.7, 0.75), ("love", 0.8, 0.55),
    // positive, lower arousal
    ("happy", 0.7, 0.55), ("glad", 0.6, 0.4), ("great", 0.6, 0.55), ("good", 0.4, 0.4),
    ("thanks", 0.5, 0.35), ("thank", 0.5, 0.35), ("nice", 0.5, 0.4), ("relaxed", 0.5, 0.15),
    ("calm", 0.3, 0.1), ("peaceful", 0.5, 0.1), ("fine", 0.2, 0.3),
    // negative, low arousal
    ("sad", -0.7, 0.25), ("lonely", -0.7, 0.2), ("tired", -0.4, 0.1), ("exhausted", -0.5, 0.1),
    ("miss", -0.5, 0.3), ("died", -0.8, 0.3), ("lost", -0.6, 0.35), ("hurt", -0.6, 0.4),
    ("cry", -0.7, 0.45), ("crying", -0.7, 0.45), ("depressed", -0.8, 0.15), ("sorry", -0.3, 0.3),
    // negative, high arousal
    ("angry", -0.7, 0.85), ("furious", -0.85, 0.95), ("hate", -0.8, 0.8), ("annoyed", -0.5, 0.65),
    ("worried", -0.5, 0.7), ("scared", -0.7, 0.8), ("afraid", -0.6, 0.75), ("anxious", -0.6, 0.75),
    ("panic", -0.8, 0.95), ("stressed", -0.6, 0.75), ("broken", -0.5, 0.6), ("terrible", -0.7, 0.6),
    ("awful", -0.7, 0.6), ("urgent", -0.2, 0.85), ("emergency", -0.5, 0.95),
];
const NEGATORS: &[&str] = &["not", "no", "never", "don't", "dont", "isn't", "isnt", "wasn't", "can't", "cant"];

/// Lexical affect cue from the user's transcript. Allocation-free byte scan.
/// Valence `None` when no cue word is present. Negation flips the next cue's
/// valence (and halves it). `!` raises arousal. Confidence grows with hits,
/// capped at 0.6: text is a cue, the voice detector is the primary sensor.
pub fn text_affect(text: &str) -> Estimate {
    let mut v_sum = 0.0f32;
    let mut a_sum = 0.0f32;
    let mut hits = 0u32;
    let mut negate = 0u8;
    let mut excl = 0u32;
    let b = text.as_bytes();
    let mut i = 0;
    let mut word = [0u8; 24];
    while i < b.len() {
        if b[i] == b'!' {
            excl += 1;
        }
        if !(b[i].is_ascii_alphabetic() || b[i] == b'\'') {
            i += 1;
            continue;
        }
        let mut n = 0;
        while i < b.len() && (b[i].is_ascii_alphabetic() || b[i] == b'\'') {
            if n < word.len() {
                word[n] = b[i].to_ascii_lowercase();
            }
            n += 1;
            i += 1;
        }
        if n > word.len() {
            negate = negate.saturating_sub(1);
            continue;
        }
        let w = &word[..n];
        if NEGATORS.iter().any(|x| x.as_bytes() == w) {
            negate = 3; // applies within the next 3 words
            continue;
        }
        if let Some(&(_, v, a)) = LEXICON.iter().find(|(k, _, _)| k.as_bytes() == w) {
            let v = if negate > 0 { -0.5 * v } else { v };
            v_sum += v;
            a_sum += a;
            hits += 1;
            negate = 0;
        } else {
            negate = negate.saturating_sub(1);
        }
    }
    if hits == 0 {
        let mut e = Estimate::UNKNOWN;
        if excl > 0 {
            e.arousal = (0.3 + 0.15 * excl as f32).min(0.8);
        }
        return e;
    }
    let n = hits as f32;
    Estimate {
        valence: Some((v_sum / n).clamp(-1.0, 1.0)),
        arousal: (a_sum / n + 0.08 * excl.min(3) as f32).clamp(0.0, 1.0),
        dominance: 0.5,
        confidence: (0.3 + 0.12 * (n - 1.0)).min(0.6),
    }
}

// ---------------------------------------------------------------------------
// Persona mood state + appraisal
// ---------------------------------------------------------------------------

/// A persona's own emotional state. VAD ranges as in [`EmotionFrame`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mood {
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
}

impl Mood {
    fn lerp(self, o: Mood, t: f32) -> Mood {
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        Mood {
            valence: self.valence + (o.valence - self.valence) * t,
            arousal: self.arousal + (o.arousal - self.arousal) * t,
            dominance: self.dominance + (o.dominance - self.dominance) * t,
        }
    }
    fn clamped(self) -> Mood {
        let c = |x: f32, lo: f32, hi: f32, d: f32| if x.is_finite() { x.clamp(lo, hi) } else { d };
        Mood { valence: c(self.valence, -1.0, 1.0, 0.0), arousal: c(self.arousal, 0.0, 1.0, 0.3), dominance: c(self.dominance, 0.0, 1.0, 0.5) }
    }
    /// Coarse name for logs / the listening set.
    pub fn label(&self) -> &'static str {
        let (v, a) = (self.valence, self.arousal);
        if v >= 0.35 && a >= 0.55 { "excited" }
        else if v >= 0.3 { "happy" }
        else if v <= -0.1 && a >= 0.5 { "concerned" }
        else if v <= -0.1 { "sad" }
        else if a <= 0.22 { "calm" }
        else if v >= 0.12 { "warm" }
        else { "steady" }
    }
}

/// How a persona reacts to what the user feels. All gains are 0..1 shares.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appraisal {
    /// Share of the user's positive valence the persona takes on.
    pub mirror_joy: f32,
    /// Share of the user's negative valence the persona takes on (concern, never despair).
    pub mirror_sorrow: f32,
    /// Floor on the appraised valence (the persona never goes below this).
    pub valence_floor: f32,
    /// Share of the user's arousal the persona follows.
    pub mirror_arousal: f32,
    /// Arousal ceiling of the appraisal.
    pub arousal_cap: f32,
    /// When the user is distressed (negative valence), pull arousal down by this share (soften/steady).
    pub distress_calming: f32,
    /// Dominance the persona takes when the user is distressed (Jarvis: composed, higher).
    pub distress_dominance: f32,
}

/// Mood dynamics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoodDynamics {
    /// Resting mood (where decay returns).
    pub rest: Mood,
    /// Per-observation step toward the appraisal at confidence 1 (inertia = 1 − this).
    pub reactivity: f32,
    /// Half-life of the decay back to `rest`, ms.
    pub half_life_ms: f32,
}

/// Persona voice on the Piper path. `base_*` is the resting voice; gains map
/// mood (and the V1 empathy weight `e`) to controls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PiperVoiceProfile {
    pub base_length_scale: f32,
    pub base_noise_scale: f32,
    pub base_noise_w: f32,
    /// Resting pitch offset, semitones (TOML `register.prosody.e0.pitch_st`).
    pub base_pitch_st: f32,
    /// Resting loudness (TOML `e0.loudness`).
    pub base_energy: f32,
    /// Resting pre-pause, ms (TOML `e0.pre_pause_ms`).
    pub base_pre_pause_ms: f32,
    pub base_sentence_silence_s: f32,
    /// d(length_scale)/d(arousal − 0.35): negative = faster when aroused.
    pub pace_per_arousal: f32,
    /// Pitch semitones per unit valence and per unit (arousal − 0.35).
    pub pitch_per_valence: f32,
    pub pitch_per_arousal: f32,
    /// Energy multiplier slope per (arousal − 0.35).
    pub energy_per_arousal: f32,
    /// noise_scale / noise_w slopes per (arousal − 0.35) (expressiveness).
    pub noise_per_arousal: f32,
    pub noise_w_per_arousal: f32,
    /// At e = 1 (V1 empathy weight): Δlength_scale, Δpitch st, energy ×, pre-pause ms.
    pub empathy_slow: f32,
    pub empathy_pitch_st: f32,
    pub empathy_energy: f32,
    pub empathy_pause_ms: f32,
    /// Hard bounds (persona-specific: Jarvis has a narrower range).
    pub pitch_bound_st: f32,
    pub length_bounds: (f32, f32),
    pub energy_bounds: (f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersonaProfile {
    pub persona: Persona,
    pub appraisal: Appraisal,
    pub dynamics: MoodDynamics,
    pub voice: PiperVoiceProfile,
}

/// Samantha: warm and mobile. Mirrors the user, then softens: shares joy
/// nearly fully, takes on sorrow as gentle concern, follows arousal but caps it.
pub const SAMANTHA: PersonaProfile = PersonaProfile {
    persona: Persona::Samantha,
    appraisal: Appraisal {
        mirror_joy: 0.9,
        mirror_sorrow: 0.45,
        valence_floor: -0.35,
        mirror_arousal: 0.7,
        arousal_cap: 0.8,
        distress_calming: 0.75,
        distress_dominance: 0.4,
    },
    dynamics: MoodDynamics {
        rest: Mood { valence: 0.25, arousal: 0.35, dominance: 0.45 },
        reactivity: 0.45,
        half_life_ms: 20_000.0,
    },
    voice: PiperVoiceProfile {
        base_length_scale: 1.0, // TOML e0.rate = 1.00
        base_noise_scale: 0.667,
        base_noise_w: 0.8,
        base_pitch_st: 0.0,     // TOML e0.pitch_st
        base_energy: 1.0,       // TOML e0.loudness
        base_pre_pause_ms: 0.0, // TOML e0.pre_pause_ms
        base_sentence_silence_s: 0.20,
        pace_per_arousal: -0.40,
        pitch_per_valence: 1.6,
        pitch_per_arousal: 2.2,
        energy_per_arousal: 0.45,
        noise_per_arousal: 0.30,
        noise_w_per_arousal: 0.30,
        empathy_slow: 0.10,
        empathy_pitch_st: -0.5, // TOML e1.pitch_st − e0
        empathy_energy: 0.80,   // TOML e1.loudness / e0
        empathy_pause_ms: 250.0,
        pitch_bound_st: 3.0,
        length_bounds: (0.80, 1.30),
        energy_bounds: (0.60, 1.20),
    },
};

/// Jarvis: composed. Steadies the user instead of following: small share of
/// their arousal, low arousal cap, takes more dominance when they are
/// distressed, never mirrors anger.
pub const JARVIS: PersonaProfile = PersonaProfile {
    persona: Persona::Jarvis,
    appraisal: Appraisal {
        mirror_joy: 0.45,
        mirror_sorrow: 0.2,
        valence_floor: -0.12,
        mirror_arousal: 0.2,
        arousal_cap: 0.5,
        distress_calming: 0.7,
        distress_dominance: 0.68,
    },
    dynamics: MoodDynamics {
        rest: Mood { valence: 0.1, arousal: 0.28, dominance: 0.62 },
        reactivity: 0.25,
        half_life_ms: 12_000.0,
    },
    voice: PiperVoiceProfile {
        base_length_scale: 1.0 / 0.96, // TOML e0.rate = 0.96
        base_noise_scale: 0.5,
        base_noise_w: 0.6,
        base_pitch_st: -0.3,     // TOML e0.pitch_st
        base_energy: 0.92,       // TOML e0.loudness
        base_pre_pause_ms: 80.0, // TOML e0.pre_pause_ms
        base_sentence_silence_s: 0.25,
        pace_per_arousal: -0.15,
        pitch_per_valence: 0.6,
        pitch_per_arousal: 0.8,
        energy_per_arousal: 0.20,
        noise_per_arousal: 0.10,
        noise_w_per_arousal: 0.10,
        empathy_slow: 0.04,
        empathy_pitch_st: -0.1,           // TOML e1 − e0 pitch
        empathy_energy: 0.88 / 0.92,      // TOML e1 / e0 loudness
        empathy_pause_ms: 40.0,           // TOML e1 − e0 pre-pause
        pitch_bound_st: 1.5,
        length_bounds: (0.92, 1.18),
        energy_bounds: (0.75, 1.05),
    },
};

/// Persona-specific appraisal: what the persona comes to feel given the user.
pub fn appraise(p: &PersonaProfile, obs: &UserObservation) -> Mood {
    let a = &p.appraisal;
    let rest = p.dynamics.rest;
    let o = obs.clamped();
    let mut valence = if o.valence >= 0.0 {
        rest.valence + a.mirror_joy * (o.valence - rest.valence).max(0.0)
    } else {
        rest.valence + a.mirror_sorrow * o.valence
    };
    valence = valence.max(a.valence_floor);
    let mut arousal = rest.arousal + a.mirror_arousal * (o.arousal - rest.arousal);
    let mut dominance = rest.dominance;
    if o.valence < -0.2 {
        // Distress: soften (Samantha) / steady (Jarvis). Anger is answered, not copied.
        let distress = ((-o.valence - 0.2) / 0.8).clamp(0.0, 1.0);
        arousal -= a.distress_calming * distress * (arousal - 0.15).max(0.0);
        dominance += distress * (a.distress_dominance - dominance);
    }
    arousal = arousal.min(a.arousal_cap);
    Mood { valence, arousal, dominance }.clamped()
}

/// The persistent per-persona mood. One per persona per session.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersonaMood {
    pub persona: Persona,
    pub mood: Mood,
    /// Last user observation (for conditioning + V1 empathy weight).
    pub last_user: UserObservation,
    last_t_ms: u64,
    pub observations: u32,
}

impl PersonaMood {
    pub fn new(persona: Persona) -> Self {
        Self { persona, mood: persona.profile().dynamics.rest, last_user: UserObservation::NONE, last_t_ms: 0, observations: 0 }
    }

    /// Exponential decay toward the resting mood over the elapsed time.
    pub fn tick(&mut self, now_ms: u64) {
        let d = &self.persona.profile().dynamics;
        let dt = now_ms.saturating_sub(self.last_t_ms) as f32;
        if dt > 0.0 && d.half_life_ms > 0.0 {
            let keep = 0.5f32.powf(dt / d.half_life_ms);
            self.mood = d.rest.lerp(self.mood, keep);
        }
        self.last_t_ms = self.last_t_ms.max(now_ms);
        // The user reading also fades: an old observation is no longer evidence.
        if dt > 0.0 {
            self.last_user.confidence *= 0.5f32.powf(dt / 8_000.0);
        }
    }

    /// One user-emotion observation: decay to `now`, appraise, step toward it.
    pub fn observe(&mut self, obs: &UserObservation, now_ms: u64) {
        self.tick(now_ms);
        let p = self.persona.profile();
        let o = obs.clamped();
        let target = appraise(p, &o);
        // √conf: a modest-confidence cue still moves the mood; conf 0 moves nothing.
        let step = p.dynamics.reactivity * o.confidence.sqrt();
        self.mood = self.mood.lerp(target, step).clamped();
        self.last_user = o;
        self.observations += 1;
    }

    /// The shared conditioning vector at `now` (decays first).
    pub fn conditioning(&mut self, now_ms: u64) -> PersonaConditioning {
        self.tick(now_ms);
        let u = self.last_user;
        PersonaConditioning {
            persona: self.persona,
            mood: self.mood,
            user: u,
            empathy_e: empathy_weight(Some(&u.as_frame())),
        }
    }
}

// ---------------------------------------------------------------------------
// Shared conditioning interface
// ---------------------------------------------------------------------------

/// Version of [`PersonaConditioning::to_array`]'s layout.
pub const CONDITIONING_VERSION: u8 = 1;
/// Length of [`PersonaConditioning::to_array`].
pub const CONDITIONING_DIM: usize = 10;

/// Persona + persona mood + user emotion, the one input every voice decoder
/// is conditioned on. Piper consumes it through [`PiperVoiceProfile::prosody`];
/// a future in-house dual-band decoder (PersonaPlex re-anchor, PR #73) gets
/// the same values as a fixed `[f32; CONDITIONING_DIM]` (FiLM input).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PersonaConditioning {
    pub persona: Persona,
    pub mood: Mood,
    pub user: UserObservation,
    /// V1 empathy weight from the user reading (EMPATHY.md).
    pub empathy_e: f32,
}

impl PersonaConditioning {
    /// Layout v1: `[is_samantha, is_jarvis, mood_v, mood_a, mood_d, user_v, user_a, user_d, user_conf, e]`.
    pub fn to_array(&self) -> [f32; CONDITIONING_DIM] {
        [
            (self.persona == Persona::Samantha) as u8 as f32,
            (self.persona == Persona::Jarvis) as u8 as f32,
            self.mood.valence,
            self.mood.arousal,
            self.mood.dominance,
            self.user.valence,
            self.user.arousal,
            self.user.dominance,
            self.user.confidence,
            self.empathy_e,
        ]
    }
}

/// Any decoder that takes persona+mood conditioning. Piper implements it in
/// galion-ml-voice; the in-house model implements it later. Must not allocate.
pub trait ConditionedDecoder {
    fn condition(&mut self, c: &PersonaConditioning);
}

/// Piper controls for one utterance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PiperProsodyVector {
    /// Piper `length_scale` (pace; >1 slower). Before pitch compensation.
    pub length_scale: f32,
    pub noise_scale: f32,
    pub noise_w: f32,
    pub sentence_silence_s: f32,
    /// Post-synthesis pitch shift, semitones.
    pub pitch_shift_st: f32,
    /// Post-synthesis loudness multiplier.
    pub energy: f32,
    pub pre_pause_ms: u16,
}

impl PiperProsodyVector {
    pub const NEUTRAL: PiperProsodyVector = PiperProsodyVector {
        length_scale: 1.0,
        noise_scale: 0.667,
        noise_w: 0.8,
        sentence_silence_s: 0.2,
        pitch_shift_st: 0.0,
        energy: 1.0,
        pre_pause_ms: 0,
    };
    /// Resample ratio for the pitch shift (`2^(st/12)`).
    pub fn pitch_ratio(&self) -> f32 {
        (self.pitch_shift_st / 12.0).exp2()
    }
    /// `length_scale` to send to Piper so that the pitch-shift resample
    /// (which shortens audio by `1/ratio`) leaves the planned pace intact.
    pub fn compensated_length_scale(&self) -> f32 {
        self.length_scale * self.pitch_ratio()
    }
}

impl PiperVoiceProfile {
    /// Persona-conditioned Piper prosody from mood + user emotion (via `e`).
    pub fn prosody(&self, c: &PersonaConditioning) -> PiperProsodyVector {
        let m = c.mood;
        let da = m.arousal - 0.35;
        let e = if c.empathy_e.is_finite() { c.empathy_e.clamp(0.0, 1.0) } else { 0.0 };
        let length = (self.base_length_scale * (1.0 + self.pace_per_arousal * da) + self.empathy_slow * e)
            .clamp(self.length_bounds.0, self.length_bounds.1);
        let pitch = (self.base_pitch_st + self.pitch_per_valence * m.valence + self.pitch_per_arousal * da + self.empathy_pitch_st * e)
            .clamp(-self.pitch_bound_st, self.pitch_bound_st);
        let energy = (self.base_energy * (1.0 + self.energy_per_arousal * da) * (1.0 + (self.empathy_energy - 1.0) * e))
            .clamp(self.energy_bounds.0, self.energy_bounds.1);
        let noise = (self.base_noise_scale + self.noise_per_arousal * da + 0.05 * m.valence).clamp(0.2, 0.9);
        let noise_w = (self.base_noise_w + self.noise_w_per_arousal * da).clamp(0.2, 1.0);
        let silence = (self.base_sentence_silence_s - 0.25 * da).clamp(0.08, 0.45);
        let pause = (self.base_pre_pause_ms + self.empathy_pause_ms * e).clamp(0.0, 400.0);
        PiperProsodyVector {
            length_scale: length,
            noise_scale: noise,
            noise_w,
            sentence_silence_s: silence,
            pitch_shift_st: pitch,
            energy,
            pre_pause_ms: pause.round() as u16,
        }
    }
}

impl PersonaConditioning {
    /// Shortcut: this persona's Piper prosody.
    pub fn piper(&self) -> PiperProsodyVector {
        self.persona.profile().voice.prosody(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(v: f32, a: f32, d: f32, c: f32) -> UserObservation {
        UserObservation { valence: v, arousal: a, dominance: d, confidence: c, source: ObsSource::Fused }
    }

    #[test]
    fn mood_evolves_with_inertia_and_decays() {
        let mut m = PersonaMood::new(Persona::Samantha);
        let rest = m.mood;
        let joy = obs(0.9, 0.85, 0.5, 0.9);
        m.observe(&joy, 1_000);
        let one = m.mood;
        assert!(one.valence > rest.valence && one.arousal > rest.arousal);
        let target = appraise(&SAMANTHA, &joy);
        assert!(one.valence < target.valence, "inertia: one observation does not jump to the target");
        m.observe(&joy, 1_500);
        m.observe(&joy, 2_000);
        assert!(m.mood.valence > one.valence, "repeated evidence accumulates");
        let peak = m.mood;
        m.tick(2_000 + 20_000); // one half-life
        let half = (peak.valence - rest.valence) * 0.5 + rest.valence;
        assert!((m.mood.valence - half).abs() < 0.02, "decay half-life {} vs {}", m.mood.valence, half);
        m.tick(2_000 + 400_000);
        assert!((m.mood.valence - rest.valence).abs() < 1e-3, "returns to rest");
    }

    #[test]
    fn low_confidence_moves_nothing() {
        let mut m = PersonaMood::new(Persona::Jarvis);
        let rest = m.mood;
        m.observe(&obs(-0.9, 0.9, 0.9, 0.0), 10);
        assert_eq!(m.mood, rest);
    }

    #[test]
    fn personas_differ_for_same_input() {
        for o in [obs(0.8, 0.85, 0.5, 0.9), obs(-0.7, 0.3, 0.4, 0.9), obs(-0.8, 0.9, 0.8, 0.9)] {
            let mut s = PersonaMood::new(Persona::Samantha);
            let mut j = PersonaMood::new(Persona::Jarvis);
            for t in 0..4 {
                s.observe(&o, t * 500);
                j.observe(&o, t * 500);
            }
            let ps = s.conditioning(2_000).piper();
            let pj = j.conditioning(2_000).piper();
            assert_ne!(ps, pj);
            // Jarvis stays steadier: his mood moved less from rest.
            let ds = (s.mood.arousal - SAMANTHA.dynamics.rest.arousal).abs() + (s.mood.valence - SAMANTHA.dynamics.rest.valence).abs();
            let dj = (j.mood.arousal - JARVIS.dynamics.rest.arousal).abs() + (j.mood.valence - JARVIS.dynamics.rest.valence).abs();
            assert!(dj < ds, "jarvis {dj} vs samantha {ds} for {o:?}");
            assert!(pj.pitch_shift_st < ps.pitch_shift_st + 0.01 || o.valence < 0.0);
        }
    }

    #[test]
    fn excited_user_speeds_samantha_up_and_raises_pitch() {
        let mut s = PersonaMood::new(Persona::Samantha);
        let base = s.conditioning(0).piper();
        for t in 1..6 {
            s.observe(&obs(0.9, 0.9, 0.5, 0.9), t * 300);
        }
        let p = s.conditioning(1_800).piper();
        assert!(p.length_scale < base.length_scale && p.pitch_shift_st > base.pitch_shift_st + 0.5 && p.energy > base.energy);
    }

    #[test]
    fn angry_user_is_not_mirrored() {
        for persona in Persona::ALL {
            let mut m = PersonaMood::new(persona);
            for t in 0..8 {
                m.observe(&obs(-0.9, 0.95, 0.9, 0.9), t * 300);
            }
            assert!(m.mood.arousal <= persona.profile().appraisal.arousal_cap + 1e-6);
            assert!(m.mood.arousal < 0.5, "{persona:?} {m:?}");
            let c = m.conditioning(2_400);
            assert!(c.empathy_e > 0.5, "V1 empathy weight engaged");
            let p = c.piper();
            assert!(p.pre_pause_ms > persona.profile().voice.base_pre_pause_ms as u16);
        }
        // Jarvis takes composure (dominance up) under distress.
        let mut j = PersonaMood::new(Persona::Jarvis);
        for t in 0..8 {
            j.observe(&obs(-0.9, 0.95, 0.9, 0.9), t * 300);
        }
        assert!(j.mood.dominance > JARVIS.dynamics.rest.dominance);
    }

    #[test]
    fn both_inputs_affect_output() {
        // Same user reading, different persona mood → different prosody.
        let user = obs(0.0, 0.3, 0.5, 0.0);
        let mut calm = PersonaMood::new(Persona::Samantha);
        let mut happy = PersonaMood::new(Persona::Samantha);
        for t in 0..5 {
            happy.observe(&obs(0.9, 0.85, 0.5, 0.9), t * 200);
        }
        calm.last_user = user;
        happy.last_user = user;
        let (pc, ph) = (calm.conditioning(1_000).piper(), happy.conditioning(1_000).piper());
        assert!(ph.pitch_shift_st > pc.pitch_shift_st && ph.length_scale < pc.length_scale, "mood drives voice");
        // Same persona mood, different user reading → different prosody (via e).
        let mut a = PersonaMood::new(Persona::Samantha);
        let b_mood = a;
        a.last_user = obs(-0.9, 0.3, 0.4, 0.9);
        let mut b = b_mood;
        b.last_user = obs(0.6, 0.3, 0.4, 0.9);
        assert_eq!(a.mood, b.mood);
        let (pa, pb) = (a.conditioning(0).piper(), b.conditioning(0).piper());
        assert!(pa.pre_pause_ms > pb.pre_pause_ms && pa.length_scale > pb.length_scale && pa.energy < pb.energy, "user emotion drives voice");
    }

    #[test]
    fn resting_voice_matches_persona_toml_e0() {
        const S: &str = include_str!("../../docs/galion-voice-v2/personas/voice/samantha.toml");
        const J: &str = include_str!("../../docs/galion-voice-v2/personas/voice/jarvis.toml");
        for (toml, p) in [(S, &SAMANTHA), (J, &JARVIS)] {
            let e0 = &toml[toml.find("[register.prosody.e0]").unwrap()..];
            let e0 = &e0[..e0[1..].find('[').unwrap()];
            let get = |k: &str| -> f32 {
                let l = e0.lines().find(|l| l.trim_start().starts_with(k)).unwrap();
                l.split('=').nth(1).unwrap().trim().parse().unwrap()
            };
            assert!((p.voice.base_pitch_st - get("pitch_st")).abs() < 1e-6);
            assert!((p.voice.base_energy - get("loudness")).abs() < 1e-6);
            assert!((p.voice.base_pre_pause_ms - get("pre_pause_ms")).abs() < 1e-6);
            assert!((p.voice.base_length_scale - 1.0 / get("rate")).abs() < 1e-5);
        }
    }

    #[test]
    fn text_affect_cues() {
        let sad = text_affect("My dog died yesterday and I feel so lonely.");
        assert!(sad.valence.unwrap() < -0.5 && sad.confidence > 0.3);
        let joy = text_affect("I got the job, this is amazing!");
        assert!(joy.valence.unwrap() > 0.5 && joy.arousal > 0.7);
        let neg = text_affect("I'm not happy about this");
        assert!(neg.valence.unwrap() < 0.0);
        assert_eq!(text_affect("Set a timer for ten minutes.").valence, None);
        assert_eq!(text_affect("").confidence, 0.0);
    }

    #[test]
    fn fusion_uses_text_valence_when_voice_cannot_know_it() {
        let voice = EmotionFrame { seq: 1, t_ms: 0, dur_ms: 80, valence: 0.0, arousal: 0.8, dominance: 0.6, confidence: 0.3 };
        let f = UserObservation::fuse(Some(&voice), false, Some(text_affect("I am so angry right now")));
        assert_eq!(f.source, ObsSource::Fused);
        assert!(f.valence < -0.5 && f.arousal > 0.7);
        let only_voice = UserObservation::fuse(Some(&voice), false, None);
        assert_eq!(only_voice.valence, 0.0);
        assert_eq!(UserObservation::fuse(None, false, None).confidence, 0.0);
    }

    #[test]
    fn conditioning_array_layout() {
        let mut m = PersonaMood::new(Persona::Jarvis);
        let a = m.conditioning(0).to_array();
        assert_eq!(a.len(), CONDITIONING_DIM);
        assert_eq!((a[0], a[1]), (0.0, 1.0));
        assert_eq!(CONDITIONING_VERSION, 1);
    }

    #[test]
    fn prosody_always_in_bounds() {
        for persona in Persona::ALL {
            let v = persona.profile().voice;
            for mv in [-1.0f32, 0.0, 1.0] {
                for ma in [0.0f32, 0.5, 1.0] {
                    for e in [0.0f32, 1.0, f32::NAN] {
                        let c = PersonaConditioning {
                            persona,
                            mood: Mood { valence: mv, arousal: ma, dominance: 0.5 },
                            user: UserObservation::NONE,
                            empathy_e: e,
                        };
                        let p = v.prosody(&c);
                        assert!(p.pitch_shift_st.abs() <= v.pitch_bound_st + 1e-6);
                        assert!(p.length_scale >= v.length_bounds.0 && p.length_scale <= v.length_bounds.1);
                        assert!(p.energy >= v.energy_bounds.0 && p.energy <= v.energy_bounds.1);
                        assert!(p.compensated_length_scale().is_finite());
                    }
                }
            }
        }
    }
}
