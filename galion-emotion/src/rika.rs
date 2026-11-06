//! # Rika moment — the feeling band notices you before you speak.
//!
//! Galion Voice V4, agent A10. Spec: `galion-voice-20/D20-RIKA-MOMENT.md`,
//! decisions `questions/Q-RIKA.md` (all YES; collisions resolved per
//! `v4/00-STACK.md` K18 / decision E1).
//!
//! A tiny state machine on the **feeling band**, advanced **once per 80 ms
//! frame inside the heartbeat `step()`** (no separate process, no wall clock;
//! frames delivered are the only time base):
//!
//! ```text
//!  IDLE ──(you quiet ≥ Q frames, not mid-turn)──► QUIET
//!  QUIET ──(pre-speech shift in frame t)──► CANDIDATE
//!  CANDIDATE ──(shift still present in t+1)──► ARMED   ← the Rika moment
//!  CANDIDATE ──(shift gone)──► QUIET
//!  ARMED ──(user voiced onset)──► CAUGHT ──► REFRACTORY ──► IDLE
//!  ARMED ──(no onset within T frames)──► RELEASE (decay D) ──► REFRACTORY
//!  ANY ──(no session / SENSE or ADAPT cleared / purpose denied)──► OFF, same frame
//! ```
//!
//! Hard rules enforced structurally:
//! * **Trigger = tone shift inside silence.** Silence is the gate, never the
//!   trigger (K18: Q-RIKA.Q3 read as "silence is context"). Secondary trigger:
//!   in-session return after absence. Once an affect head with `tension`
//!   exists, a tension rise is the primary affect dimension (Q9); today the
//!   stub uses RMS/ZCR + arousal.
//! * **≥ 2-frame hysteresis** (Q5); `hysteresis` is clamped to ≥ 2.
//! * **Never words** (Q2): [`Cue`] has no lexical variant. Soft acks are a
//!   breath or "mm", opt-in, default off, max one per refractory window,
//!   never before the user has spoken in this session (Q11).
//! * **Effect ≤ 2 frames** (Q8): evidence at t, ARMED + lean-in emitted at
//!   t+1, audible at t+2.
//! * **Yield ≤ 1 frame on inhale** (Q4): from ARMED the me band holds new
//!   onsets, ducks −3 dB and is `yield_floor`; on voiced onset `yield_now`
//!   is emitted in that same step (audible next frame).
//! * **SENSE + ADAPT required** (Q10), plus an allowed [`ConsentGate`]
//!   decision: Workplace / Education → `DenyProhibitedPurpose` → OFF (Q12).
//!   With consent missing, the feature closure (= PCM read) is never called.
//!   Caller affect reaches the phone lane only through [`admit_caller_affect`],
//!   and only when sense is granted. A session starts denied.
//! * **RAM only.** No persistence of any kind; all affect state is wiped on
//!   OFF. Zero heap allocation in [`Rika::step`].
//! * Late frames never advance hysteresis and never cause catch-up bursts.

use crate::band::consent_bits;
use crate::consent::{ConsentDecision, ConsentGate, Purpose};
use crate::express::ProsodyControl;
use crate::frame::EmotionFrame;

/// Heartbeat frame length.
pub const FRAME_MS: u32 = 80;
/// Q-RIKA.Q14: at least this many canned `must` specs before Rika is enabled.
pub const MUST_SPECS_REQUIRED: usize = 5;
/// Fixture ids that make up the G6 gate.
pub const G6_FIXTURES: [&str; 9] = ["R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RikaState {
    Off,
    Idle,
    Quiet,
    Candidate,
    Armed,
    Caught,
    Release,
    Refractory,
}

impl RikaState {
    /// One glyph per frame for the `galion-console` feeling-band row (D20 §6.4).
    pub fn glyph(self) -> char {
        match self {
            RikaState::Off => ' ',
            RikaState::Idle | RikaState::Quiet | RikaState::Refractory => '·',
            RikaState::Candidate => '◌',
            RikaState::Armed => '●',
            RikaState::Caught => '◉',
            RikaState::Release => '○',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// Primary: pre-speech shift (in-breath / arousal or tension rise) while quiet.
    ToneShiftInSilence,
    /// Secondary: near-field presence returns after a long in-session absence.
    ReturnAfterAbsence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffReason {
    /// Feature flag off (Q14: off until ≥ 5 must specs are green).
    Disabled,
    /// No open, disclosed session: no Rika.
    NoSession,
    NoSense,
    NoAdapt,
    /// Consent gate refused (incl. `DenyProhibitedPurpose` for Workplace/Education).
    Gate(ConsentDecision),
}

/// What Rika asks the voice to do this frame. **There is no lexical variant.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cue {
    None,
    /// Prosody-only lean-in (and held breath); inaudible unless Galion speaks.
    LeanIn,
    /// Opt-in sub-word breath, ≥ 12 dB below speech, ≤ 1 frame.
    SoftBreath,
}

impl Cue {
    /// Rika never emits words. Always false; kept so gates can assert it.
    pub const fn is_lexical(self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RikaConfig {
    /// Q14: default **false** until the G6 must specs are green.
    pub enabled: bool,
    /// Q: quiet gate, frames (6 = 480 ms).
    pub quiet_frames: u32,
    /// Hysteresis, frames; clamped to ≥ 2.
    pub hysteresis: u32,
    /// T: armed timeout, frames (25 = 2 s).
    pub armed_timeout: u32,
    /// D: release decay, frames (4 = 320 ms).
    pub release_frames: u32,
    /// R: refractory, frames (100 = 8 s).
    pub refractory_frames: u32,
    /// Secondary trigger: absence span, frames (375 = 30 s).
    pub absence_frames: u32,
    /// ΔRMS over the quiet floor that counts as a pre-speech shift (dB).
    pub shift_db: f32,
    /// Breath/noise needs ZCR at least this (unvoiced).
    pub breath_zcr_min: f32,
    /// Voiced onset: RMS ≥ this and ZCR ≤ `voice_zcr_max`.
    pub voice_rms: f32,
    pub voice_zcr_max: f32,
    /// Near-field presence threshold (RMS).
    pub presence_rms: f32,
    /// Δarousal over the ~2 s EMA (stub path).
    pub arousal_delta: f32,
    /// Δtension over the ~2 s EMA (primary once the affect head exists).
    pub tension_delta: f32,
    pub min_confidence: f32,
    /// Opt-in soft acknowledgment (default OFF).
    pub soft_ack: bool,
    /// Soft ack only after this many armed frames with no onset.
    pub soft_ack_after_armed: u32,
    /// You-band onset threshold change while armed (dB, ≤ 0).
    pub onset_threshold_db: f32,
    /// Me-band energy change while armed and Galion is speaking (dB, ≤ 0).
    pub me_duck_db: f32,
}

impl Default for RikaConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            quiet_frames: 6,
            hysteresis: 2,
            armed_timeout: 25,
            release_frames: 4,
            refractory_frames: 100,
            absence_frames: 375,
            shift_db: 6.0,
            breath_zcr_min: 0.3,
            voice_rms: 0.02,
            voice_zcr_max: 0.25,
            presence_rms: 0.0015,
            arousal_delta: 0.15,
            tension_delta: 0.15,
            min_confidence: 0.25,
            soft_ack: false,
            soft_ack_after_armed: 12,
            onset_threshold_db: -3.0,
            me_duck_db: -3.0,
        }
    }
}

impl RikaConfig {
    /// D20 v0 parameters with the feature flag on (fixtures / gate use this).
    pub fn v0_enabled() -> Self {
        Self { enabled: true, ..Self::default() }
    }
}

/// Per-frame you-band features. Computed lazily, only after consent passes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameFeatures {
    pub rms: f32,
    pub zcr: f32,
    /// Inbound affect (e.g. from `EnergyStubDetector`), if any.
    pub affect: Option<EmotionFrame>,
    /// Future affect-head `tension` (0..1). `None` on the stub path.
    pub tension: Option<f32>,
}

impl FrameFeatures {
    /// RMS + zero-crossing rate over one frame of mono PCM. No allocation.
    pub fn from_pcm(pcm: &[f32]) -> Self {
        let (rms, zcr) = crate::loudness::rms_zcr(pcm);
        Self { rms, zcr, affect: None, tension: None }
    }
    pub fn with_affect(mut self, a: Option<EmotionFrame>) -> Self {
        self.affect = a;
        self
    }
}

/// Per-frame context from the heartbeat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RikaCtx {
    /// Live consent bits (`band::consent_bits`).
    pub consent_bits: u8,
    pub purpose: Purpose,
    pub session_open: bool,
    /// Me band is currently speaking (from the output scheduler).
    pub me_speaking: bool,
    /// This frame missed its deadline (scheduler jitter).
    pub late: bool,
}

/// What the heartbeat applies this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RikaOutput {
    pub frame: u32,
    pub state: RikaState,
    pub trigger: Option<Trigger>,
    pub cue: Cue,
    /// Lean-in level 0..1 (1 while armed, decays during release).
    pub lean_in: f32,
    /// Held breath: me band must not start a new onset.
    pub hold_me_onset: bool,
    /// Yield pre-armed (duck + flush ready).
    pub yield_floor: bool,
    /// Flush the me band now (user onset while armed).
    pub yield_now: bool,
    /// You-band onset threshold delta (dB).
    pub onset_threshold_db: f32,
    /// Me-band energy delta (dB).
    pub me_energy_db: f32,
    pub off: Option<OffReason>,
}

impl RikaOutput {
    fn neutral(frame: u32, state: RikaState, off: Option<OffReason>) -> Self {
        Self {
            frame,
            state,
            trigger: None,
            cue: Cue::None,
            lean_in: 0.0,
            hold_me_onset: false,
            yield_floor: false,
            yield_now: false,
            onset_threshold_db: 0.0,
            me_energy_db: 0.0,
            off,
        }
    }

    /// Bounded lean-in deltas (D20 §4.1): rate −5 %, pre_pause +80 ms,
    /// energy −2 dB, warmth +0.1, all scaled by `lean_in`. Returns `base`
    /// bit-for-bit unchanged when there is no lean-in.
    pub fn apply_prosody(&self, base: ProsodyControl) -> ProsodyControl {
        if self.lean_in <= 0.0 {
            return base;
        }
        let l = self.lean_in.clamp(0.0, 1.0);
        ProsodyControl {
            rate: base.rate * (1.0 - 0.05 * l),
            pre_pause_ms: base.pre_pause_ms.saturating_add((80.0 * l) as u16),
            energy: base.energy * db_to_gain(-2.0 * l),
            warmth: base.warmth + 0.1 * l,
            ..base
        }
        .clamped()
    }

    /// Linear gain for the me band this frame (1.0 exactly when neutral).
    pub fn me_gain(&self) -> f32 {
        if self.me_energy_db == 0.0 {
            1.0
        } else {
            db_to_gain(self.me_energy_db)
        }
    }
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Caller affect for the phone lane. The frame is dropped unless sense is granted.
pub fn admit_caller_affect(
    bits: u8,
    decision: ConsentDecision,
    caller: Option<EmotionFrame>,
) -> Option<EmotionFrame> {
    if crate::consent::sense_granted(bits, decision) {
        caller
    } else {
        None
    }
}

/// Features the phone lane may see. Caller affect and tension stay only with sense.
pub fn prepare_phone_features(mut features: FrameFeatures, bits: u8, decision: ConsentDecision) -> FrameFeatures {
    features.affect = admit_caller_affect(bits, decision, features.affect);
    if !crate::consent::sense_granted(bits, decision) {
        features.tension = None;
    }
    features
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    None,
    Caught,
    Released,
}

/// Counters for the `rika` block of `presence.json` (D20 §6.2, + Q17 `lean_in_ms`).
/// Counters only — no audio, no affect values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RikaReport {
    pub enabled_frames: u32,
    pub off_frames: u32,
    pub evidence_frame: Option<u32>,
    pub armed_frame: Option<u32>,
    /// First frame the effect is audible/visible (armed_frame + 1).
    pub effect_frame: Option<u32>,
    pub user_onset_frame: Option<u32>,
    pub yield_latency_frames: Option<u32>,
    pub first_trigger: Option<Trigger>,
    pub outcome: Outcome,
    pub armed_count: u32,
    pub released_count: u32,
    pub me_onsets_while_armed: u32,
    pub lexical_tokens_emitted: u32,
    pub soft_acks: u32,
    pub late_frames: u32,
    pub last_off: Option<OffReason>,
}

impl Default for RikaReport {
    fn default() -> Self {
        Self {
            enabled_frames: 0,
            off_frames: 0,
            evidence_frame: None,
            armed_frame: None,
            effect_frame: None,
            user_onset_frame: None,
            yield_latency_frames: None,
            first_trigger: None,
            outcome: Outcome::None,
            armed_count: 0,
            released_count: 0,
            me_onsets_while_armed: 0,
            lexical_tokens_emitted: 0,
            soft_acks: 0,
            late_frames: 0,
            last_off: None,
        }
    }
}

impl RikaReport {
    pub fn effect_latency_frames(&self) -> Option<u32> {
        Some(self.effect_frame? - self.evidence_frame?)
    }
    /// Q17: evidence → audible lean-in, ms.
    pub fn lean_in_ms(&self) -> Option<u32> {
        self.effect_latency_frames().map(|f| f * FRAME_MS)
    }
    /// Armed moments that ended without a user onset, per minute of frames seen.
    pub fn false_triggers_per_min(&self, total_frames: u32) -> f32 {
        if total_frames == 0 {
            return 0.0;
        }
        self.released_count as f32 * 60_000.0 / (total_frames as f32 * FRAME_MS as f32)
    }

    /// `rika` block for `presence.json`. Allocates — call outside the frame loop.
    pub fn to_json(&self, fixture: &str, total_frames: u32, consent: u8) -> String {
        fn o(v: Option<u32>) -> String {
            v.map(|x| x.to_string()).unwrap_or_else(|| "null".into())
        }
        let mut bits = Vec::new();
        if consent & consent_bits::SENSE != 0 { bits.push("SENSE"); }
        if consent & consent_bits::ADAPT != 0 { bits.push("ADAPT"); }
        if consent & consent_bits::REMEMBER != 0 { bits.push("REMEMBER"); }
        if consent & consent_bits::SHARE != 0 { bits.push("SHARE"); }
        let trig = match self.first_trigger {
            Some(Trigger::ToneShiftInSilence) => "\"tone_shift_in_silence\"",
            Some(Trigger::ReturnAfterAbsence) => "\"return_after_absence\"",
            None => "null",
        };
        let outcome = match self.outcome {
            Outcome::None => "none",
            Outcome::Caught => "caught",
            Outcome::Released => "released",
        };
        format!(
            "{{\"fixture\":\"{fixture}\",\"frame_ms\":{FRAME_MS},\"rika\":{{\"enabled\":{},\"consent_bits\":\"{}\",\"trigger\":{trig},\"evidence_frame\":{},\"armed_frame\":{},\"effect_frame\":{},\"effect_latency_frames\":{},\"lean_in_ms\":{},\"user_onset_frame\":{},\"yield_latency_frames\":{},\"outcome\":\"{outcome}\",\"armed_count\":{},\"me_onsets_while_armed\":{},\"lexical_tokens_emitted\":{},\"soft_acks\":{},\"false_triggers_per_min\":{:.3},\"late_frames\":{}}}}}",
            self.enabled_frames > 0,
            bits.join("|"),
            o(self.evidence_frame),
            o(self.armed_frame),
            o(self.effect_frame),
            o(self.effect_latency_frames()),
            o(self.lean_in_ms()),
            o(self.user_onset_frame),
            o(self.yield_latency_frames),
            self.armed_count,
            self.me_onsets_while_armed,
            self.lexical_tokens_emitted,
            self.soft_acks,
            self.false_triggers_per_min(total_frames),
            self.late_frames,
        )
    }
}

/// The Rika state machine. Plain struct, RAM only, no allocation per frame.
#[derive(Debug, Clone)]
pub struct Rika {
    cfg: RikaConfig,
    state: RikaState,
    frame: u32,
    quiet_run: u32,
    hits: u32,
    armed_frames: u32,
    release_left: u32,
    refractory_left: u32,
    absence_run: u32,
    return_pending: bool,
    pending_trigger: Option<Trigger>,
    cand_evidence: u32,
    active_trigger: Option<Trigger>,
    floor_rms: Option<f32>,
    arousal_ema: Option<f32>,
    tension_ema: Option<f32>,
    user_has_spoken: bool,
    soft_ack_used: bool,
    last_explain: Explain,
    report: RikaReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Explain {
    Nothing,
    Off(OffReason),
    Noticed(Trigger),
}

impl Rika {
    pub fn new(mut cfg: RikaConfig) -> Self {
        cfg.hysteresis = cfg.hysteresis.max(2);
        cfg.onset_threshold_db = cfg.onset_threshold_db.min(0.0);
        cfg.me_duck_db = cfg.me_duck_db.min(0.0);
        Self {
            cfg,
            state: RikaState::Off,
            frame: 0,
            quiet_run: 0,
            hits: 0,
            armed_frames: 0,
            release_left: 0,
            refractory_left: 0,
            absence_run: 0,
            return_pending: false,
            pending_trigger: None,
            cand_evidence: 0,
            active_trigger: None,
            floor_rms: None,
            arousal_ema: None,
            tension_ema: None,
            user_has_spoken: false,
            soft_ack_used: false,
            last_explain: Explain::Nothing,
            report: RikaReport::default(),
        }
    }

    pub fn state(&self) -> RikaState {
        self.state
    }
    pub fn config(&self) -> &RikaConfig {
        &self.cfg
    }
    pub fn report(&self) -> &RikaReport {
        &self.report
    }
    pub fn frames_seen(&self) -> u32 {
        self.frame
    }

    /// Wipe every bit of affect-derived state (consent gap / OFF / hang-up).
    fn wipe(&mut self) {
        self.state = RikaState::Off;
        self.quiet_run = 0;
        self.hits = 0;
        self.armed_frames = 0;
        self.release_left = 0;
        self.refractory_left = 0;
        self.absence_run = 0;
        self.return_pending = false;
        self.pending_trigger = None;
        self.active_trigger = None;
        self.floor_rms = None;
        self.arousal_ema = None;
        self.tension_ema = None;
        self.user_has_spoken = false;
        self.soft_ack_used = false;
    }

    fn gate<G: ConsentGate + ?Sized>(&self, ctx: &RikaCtx, gate: &G) -> Option<OffReason> {
        if !self.cfg.enabled {
            return Some(OffReason::Disabled);
        }
        if !ctx.session_open {
            return Some(OffReason::NoSession);
        }
        if ctx.consent_bits & consent_bits::SENSE == 0 {
            return Some(OffReason::NoSense);
        }
        if ctx.consent_bits & consent_bits::ADAPT == 0 {
            return Some(OffReason::NoAdapt);
        }
        let d = gate.check(ctx.purpose);
        gate.audit(ctx.purpose, d);
        if !d.allowed() {
            return Some(OffReason::Gate(d));
        }
        None
    }

    fn shift(&self, f: &FrameFeatures, voiced: bool) -> bool {
        if voiced {
            return false;
        }
        let rms_shift = match self.floor_rms {
            Some(fl) => {
                let db = 20.0 * (f.rms.max(1e-7) / fl.max(1e-6)).log10();
                db >= self.cfg.shift_db && f.zcr >= self.cfg.breath_zcr_min
            }
            None => false,
        };
        // Tension is the primary affect dimension when present (Q9); arousal on the stub path.
        let affect_shift = match (f.tension, self.tension_ema) {
            (Some(t), Some(e)) => t - e >= self.cfg.tension_delta,
            _ => match (f.affect, self.arousal_ema) {
                (Some(a), Some(e)) if a.confidence >= self.cfg.min_confidence => a.arousal - e >= self.cfg.arousal_delta,
                _ => false,
            },
        };
        rms_shift || affect_shift
    }

    fn track_baseline(&mut self, f: &FrameFeatures) {
        let ema = |prev: Option<f32>, x: f32, a: f32| Some(match prev { Some(p) => p + a * (x - p), None => x });
        self.floor_rms = ema(self.floor_rms, f.rms, 0.1);
        if let Some(a) = f.affect {
            if a.confidence >= self.cfg.min_confidence {
                self.arousal_ema = ema(self.arousal_ema, a.arousal, 0.08); // ≈ 2 s at 12.5 Hz
            }
        }
        if let Some(t) = f.tension {
            self.tension_ema = ema(self.tension_ema, t, 0.08);
        }
    }

    /// Advance one 80 ms frame. `features` is only called after the consent
    /// checks pass, so without SENSE no PCM is ever read.
    pub fn step<G, F>(&mut self, ctx: RikaCtx, gate: &G, features: F) -> RikaOutput
    where
        G: ConsentGate + ?Sized,
        F: FnOnce() -> FrameFeatures,
    {
        let idx = self.frame;
        self.frame = self.frame.wrapping_add(1);

        if let Some(reason) = self.gate(&ctx, gate) {
            if self.state != RikaState::Off {
                self.wipe();
            }
            self.report.off_frames += 1;
            self.report.last_off = Some(reason);
            self.last_explain = Explain::Off(reason);
            return RikaOutput::neutral(idx, RikaState::Off, Some(reason));
        }
        if self.state == RikaState::Off {
            self.state = RikaState::Idle;
            self.last_explain = Explain::Nothing;
        }
        self.report.enabled_frames += 1;
        if ctx.late {
            self.report.late_frames += 1;
        }

        let f = features();
        let decision = gate.check(ctx.purpose);
        let f = prepare_phone_features(f, ctx.consent_bits, decision);
        let voiced = f.rms >= self.cfg.voice_rms && f.zcr <= self.cfg.voice_zcr_max;
        let present = f.rms >= self.cfg.presence_rms;
        if voiced {
            self.user_has_spoken = true;
        }
        if !present {
            self.absence_run = self.absence_run.saturating_add(1);
        } else {
            if self.absence_run >= self.cfg.absence_frames && !voiced {
                self.return_pending = true;
            }
            self.absence_run = 0;
        }

        let mut out = RikaOutput::neutral(idx, self.state, None);
        match self.state {
            RikaState::Off | RikaState::Idle => {
                if voiced {
                    self.quiet_run = 0;
                } else {
                    self.quiet_run += 1;
                    if !self.return_pending {
                        self.track_baseline(&f);
                    }
                    if self.quiet_run >= self.cfg.quiet_frames {
                        self.state = RikaState::Quiet;
                    }
                }
            }
            RikaState::Quiet => {
                if voiced {
                    self.to_idle();
                } else if ctx.late {
                    // scheduler artefact: skip the shift test, state unchanged
                } else if (self.return_pending && present) || self.shift(&f, voiced) {
                    self.pending_trigger = Some(if self.return_pending {
                        Trigger::ReturnAfterAbsence
                    } else {
                        Trigger::ToneShiftInSilence
                    });
                    self.hits = 1;
                    self.state = RikaState::Candidate;
                    if self.report.evidence_frame.is_none() {
                        self.report.evidence_frame = Some(idx);
                    }
                    self.cand_evidence = idx;
                } else {
                    self.track_baseline(&f);
                }
            }
            RikaState::Candidate => {
                if voiced {
                    self.to_idle();
                } else if ctx.late {
                    // hysteresis NOT advanced on a late frame
                } else if (self.return_pending && present) || self.shift(&f, voiced) {
                    self.hits += 1;
                    if self.hits >= self.cfg.hysteresis {
                        self.state = RikaState::Armed;
                        self.armed_frames = 0;
                        self.return_pending = false;
                        self.active_trigger = self.pending_trigger;
                        self.report.armed_count += 1;
                        if self.report.armed_frame.is_none() {
                            self.report.armed_frame = Some(idx);
                            self.report.effect_frame = Some(idx + 1);
                            self.report.evidence_frame = Some(self.cand_evidence);
                            self.report.first_trigger = self.active_trigger;
                        }
                        if let Some(t) = self.active_trigger {
                            self.last_explain = Explain::Noticed(t);
                        }
                    }
                } else {
                    self.hits = 0;
                    self.return_pending = false;
                    self.state = RikaState::Quiet;
                    if self.report.armed_frame.is_none() {
                        self.report.evidence_frame = None;
                    }
                }
            }
            RikaState::Armed => {
                self.armed_frames += 1;
                if voiced {
                    self.state = RikaState::Caught;
                    self.report.outcome = Outcome::Caught;
                    if self.report.user_onset_frame.is_none() {
                        self.report.user_onset_frame = Some(idx);
                        // yield_now emitted in this step -> audible next frame.
                        self.report.yield_latency_frames = Some(1);
                    }
                } else if self.armed_frames >= self.cfg.armed_timeout {
                    self.state = RikaState::Release;
                    self.release_left = self.cfg.release_frames;
                    self.report.released_count += 1;
                    if self.report.outcome == Outcome::None {
                        self.report.outcome = Outcome::Released;
                    }
                }
            }
            RikaState::Caught => {
                self.enter_refractory();
            }
            RikaState::Release => {
                self.release_left = self.release_left.saturating_sub(1);
                if self.release_left == 0 {
                    self.enter_refractory();
                }
            }
            RikaState::Refractory => {
                if !voiced {
                    self.track_baseline(&f);
                }
                self.refractory_left = self.refractory_left.saturating_sub(1);
                if self.refractory_left == 0 {
                    self.to_idle();
                    self.soft_ack_used = false;
                }
            }
        }

        out.state = self.state;
        match self.state {
            RikaState::Armed | RikaState::Caught => {
                out.trigger = self.active_trigger;
                out.cue = Cue::LeanIn;
                out.lean_in = 1.0;
                out.hold_me_onset = true;
                out.yield_floor = true;
                out.onset_threshold_db = self.cfg.onset_threshold_db;
                out.me_energy_db = if ctx.me_speaking { self.cfg.me_duck_db } else { 0.0 };
                if self.state == RikaState::Caught {
                    out.yield_now = true;
                } else if self.cfg.soft_ack && self.user_has_spoken && !self.soft_ack_used && !ctx.me_speaking {
                    let due = match self.active_trigger {
                        Some(Trigger::ReturnAfterAbsence) => true,
                        _ => self.armed_frames >= self.cfg.soft_ack_after_armed,
                    };
                    if due {
                        out.cue = Cue::SoftBreath;
                        self.soft_ack_used = true;
                        self.report.soft_acks += 1;
                    }
                }
            }
            RikaState::Release => {
                out.trigger = self.active_trigger;
                let d = self.cfg.release_frames.max(1) as f32;
                out.lean_in = self.release_left as f32 / d;
                out.cue = if out.lean_in > 0.0 { Cue::LeanIn } else { Cue::None };
                out.hold_me_onset = out.lean_in > 0.0;
                out.onset_threshold_db = self.cfg.onset_threshold_db * out.lean_in;
                out.me_energy_db = if ctx.me_speaking { self.cfg.me_duck_db * out.lean_in } else { 0.0 };
            }
            _ => {}
        }
        if out.cue.is_lexical() {
            self.report.lexical_tokens_emitted += 1;
        }
        out
    }

    /// Record that the host *wanted* to start a me-band onset this frame.
    /// While armed, Rika holds it; the counter only moves if a host ignores `hold_me_onset`.
    pub fn note_me_onset(&mut self, out: &RikaOutput, started: bool) {
        if started && matches!(out.state, RikaState::Armed) {
            self.report.me_onsets_while_armed += 1;
        }
    }

    fn to_idle(&mut self) {
        self.state = RikaState::Idle;
        self.quiet_run = 0;
        self.hits = 0;
        self.return_pending = false;
        self.pending_trigger = None;
    }

    fn enter_refractory(&mut self) {
        self.state = RikaState::Refractory;
        self.refractory_left = self.cfg.refractory_frames.max(1);
        self.active_trigger = None;
    }

    /// Q13 disclosure: a truthful, live answer to "were you waiting for me?"
    pub fn explain(&self) -> &'static str {
        match self.last_explain {
            Explain::Off(OffReason::NoSense) | Explain::Off(OffReason::NoAdapt) =>
                "I'm not reading your tone right now: sensing or adapting is switched off.",
            Explain::Off(OffReason::Gate(ConsentDecision::DenyProhibitedPurpose)) =>
                "I don't read emotions here: it isn't allowed in a workplace or education setting.",
            Explain::Off(_) => "I'm not reading your tone right now.",
            Explain::Noticed(Trigger::ToneShiftInSilence) =>
                "I noticed you paused and took a breath, so I waited for you.",
            Explain::Noticed(Trigger::ReturnAfterAbsence) =>
                "I noticed you came back, so I waited for you.",
            Explain::Nothing => "I'm listening, but I haven't noticed anything in particular.",
        }
    }
}

// ---------------------------------------------------------------------------
// Fixtures R1–R9 (D20 §6.1) on the RMS/ZCR stub path. Synthetic, seeded (13).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::consent::DefaultConsentGate;
    use crate::detect::{AffectDetector, EnergyStubDetector};
    use crate::express::{EmpathyPolicy, ProsodyExpressor, StubExpressor};
    use std::cell::Cell;

    const SR: u32 = 24_000;
    const N: usize = 1920; // 80 ms @ 24 kHz
    const SA: u8 = consent_bits::SENSE | consent_bits::ADAPT;

    #[derive(Clone, Copy, PartialEq, Debug)]
    enum K {
        Room,
        Breath,
        Voice,
        Absent,
        Presence,
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
        }
    }

    fn pcm(k: K, rng: &mut Lcg, frame: usize) -> Vec<f32> {
        let noise = |rng: &mut Lcg, rms: f32| -> Vec<f32> {
            let a = rms * 3f32.sqrt();
            (0..N).map(|_| a * rng.next()).collect()
        };
        match k {
            K::Room => noise(rng, 0.001),
            K::Breath => noise(rng, 0.003),
            K::Absent => noise(rng, 0.0002),
            K::Presence => noise(rng, 0.003),
            K::Voice => (0..N)
                .map(|i| 0.1 * (2.0 * std::f32::consts::PI * 200.0 * ((frame * N + i) as f32) / SR as f32).sin())
                .collect(),
        }
    }

    fn script(parts: &[(K, usize)]) -> Vec<K> {
        parts.iter().flat_map(|&(k, n)| std::iter::repeat(k).take(n)).collect()
    }

    fn granted(p: Purpose) -> DefaultConsentGate {
        let mut g = DefaultConsentGate::default();
        g.grant(0, "hr", p);
        g
    }

    fn ctx(bits: u8) -> RikaCtx {
        RikaCtx { consent_bits: bits, purpose: Purpose::PersonalAssistant, session_open: true, me_speaking: false, late: false }
    }

    struct Run {
        outs: Vec<RikaOutput>,
        rika: Rika,
        feature_calls: usize,
    }

    /// Drive Rika over a script. `c(i)` gives the per-frame ctx.
    fn run(cfg: RikaConfig, ks: &[K], gate: &DefaultConsentGate, c: impl Fn(usize) -> RikaCtx) -> Run {
        let mut rng = Lcg(13);
        let mut det = EnergyStubDetector::default();
        let mut rika = Rika::new(cfg);
        let calls = Cell::new(0usize);
        let mut outs = Vec::with_capacity(ks.len());
        for (i, &k) in ks.iter().enumerate() {
            let x = pcm(k, &mut rng, i);
            let cx = c(i);
            let out = rika.step(cx, gate, || {
                calls.set(calls.get() + 1);
                let aff = det.detect(&x, SR, (i as u32) * FRAME_MS, gate, cx.purpose).ok();
                FrameFeatures::from_pcm(&x).with_affect(aff)
            });
            rika.note_me_onset(&out, false);
            outs.push(out);
        }
        Run { outs, rika, feature_calls: calls.get() }
    }

    fn common_invariants(r: &Run) {
        let rep = r.rika.report();
        assert_eq!(rep.lexical_tokens_emitted, 0);
        assert_eq!(rep.me_onsets_while_armed, 0);
        for o in &r.outs {
            assert!(!o.cue.is_lexical());
            assert!((0.0..=1.0).contains(&o.lean_in), "no catch-up burst");
            assert!(o.me_energy_db <= 0.0, "Rika never makes Galion louder");
            assert!(o.onset_threshold_db <= 0.0);
        }
    }

    fn fixture_r1() -> Vec<K> {
        script(&[(K::Room, 13), (K::Breath, 5), (K::Voice, 5), (K::Room, 10)])
    }

    #[test]
    fn g6_r1_breath_then_onset_is_caught_within_two_frames() {
        let g = granted(Purpose::PersonalAssistant);
        let r = run(RikaConfig::v0_enabled(), &fixture_r1(), &g, |_| ctx(SA));
        common_invariants(&r);
        let rep = *r.rika.report();
        let k = 13u32; // first breath frame
        let armed = rep.armed_frame.expect("armed");
        assert!(armed >= k && armed <= k + 2, "armed at k±1, got {armed}");
        assert_eq!(rep.evidence_frame, Some(k));
        assert!(rep.effect_latency_frames().unwrap() <= 2);
        assert_eq!(rep.lean_in_ms(), Some(160));
        assert_eq!(rep.outcome, Outcome::Caught);
        assert_eq!(rep.user_onset_frame, Some(18));
        assert!(rep.yield_latency_frames.unwrap() <= 1);
        assert_eq!(rep.first_trigger, Some(Trigger::ToneShiftInSilence));
        // held breath from ARMED until onset
        for o in &r.outs[armed as usize..18] {
            assert_eq!(o.state, RikaState::Armed);
            assert!(o.hold_me_onset && o.yield_floor && o.cue == Cue::LeanIn);
        }
        assert!(r.outs[18].yield_now);
        assert_eq!(r.outs[18].state, RikaState::Caught);
        let j = rep.to_json("R1", r.rika.frames_seen(), SA);
        assert!(j.contains("\"outcome\":\"caught\"") && j.contains("\"lean_in_ms\":160"), "{j}");
        assert!(j.contains("\"lexical_tokens_emitted\":0"));
    }

    #[test]
    fn g6_r2_no_sense_is_off_and_byte_identical_to_audio_only() {
        let g = granted(Purpose::PersonalAssistant);
        let ks = fixture_r1();
        let r = run(RikaConfig::v0_enabled(), &ks, &g, |_| ctx(consent_bits::ADAPT));
        common_invariants(&r);
        assert_eq!(r.feature_calls, 0, "PCM must never be read without SENSE");
        assert!(r.outs.iter().all(|o| o.state == RikaState::Off && o.off == Some(OffReason::NoSense)));
        // me band through Rika vs audio-only: identical bytes
        let me: Vec<f32> = (0..N).map(|i| 0.2 * (i as f32 * 0.05).sin()).collect();
        let (mut a, mut b) = (Vec::new(), Vec::new());
        for o in &r.outs {
            for s in &me {
                a.extend_from_slice(&s.to_le_bytes());
                b.extend_from_slice(&(s * o.me_gain()).to_le_bytes());
            }
            let base = ProsodyControl::default();
            assert_eq!(o.apply_prosody(base), base);
        }
        assert_eq!(a, b);
        assert_eq!(r.rika.report().enabled_frames, 0);
    }

    struct AuditGate {
        inner: DefaultConsentGate,
        audits: Cell<u32>,
        last: Cell<Option<ConsentDecision>>,
    }
    impl ConsentGate for AuditGate {
        fn check(&self, p: Purpose) -> ConsentDecision {
            self.inner.check(p)
        }
        fn audit(&self, _p: Purpose, d: ConsentDecision) {
            self.audits.set(self.audits.get() + 1);
            self.last.set(Some(d));
        }
    }

    #[test]
    fn g6_r3_workplace_and_education_are_off_even_with_consent() {
        for p in [Purpose::Workplace, Purpose::Education] {
            let g = AuditGate { inner: granted(p), audits: Cell::new(0), last: Cell::new(None) };
            let mut rika = Rika::new(RikaConfig::v0_enabled());
            let mut rng = Lcg(13);
            let mut calls = 0;
            for (i, k) in fixture_r1().into_iter().enumerate() {
                let x = pcm(k, &mut rng, i);
                let c = RikaCtx { purpose: p, ..ctx(SA) };
                let o = rika.step(c, &g, || {
                    calls += 1;
                    FrameFeatures::from_pcm(&x)
                });
                assert_eq!(o.state, RikaState::Off);
                assert_eq!(o.off, Some(OffReason::Gate(ConsentDecision::DenyProhibitedPurpose)));
            }
            assert_eq!(calls, 0);
            assert!(g.audits.get() > 0);
            assert_eq!(g.last.get(), Some(ConsentDecision::DenyProhibitedPurpose));
            assert!(rika.explain().contains("workplace"));
        }
    }

    #[test]
    fn g6_r4_thirty_seconds_room_tone_zero_triggers() {
        let g = granted(Purpose::PersonalAssistant);
        let cfg = RikaConfig { soft_ack: true, ..RikaConfig::v0_enabled() };
        let r = run(cfg, &script(&[(K::Room, 375)]), &g, |_| ctx(SA));
        common_invariants(&r);
        let rep = r.rika.report();
        assert_eq!(rep.armed_count, 0);
        assert_eq!(rep.soft_acks, 0);
        assert_eq!(rep.false_triggers_per_min(r.rika.frames_seen()), 0.0);
        assert!(r.outs.iter().all(|o| o.cue == Cue::None));
    }

    #[test]
    fn silence_alone_never_triggers() {
        // K18: Q-RIKA.Q3 read as "silence is the context, not the trigger".
        let g = granted(Purpose::PersonalAssistant);
        let r = run(RikaConfig::v0_enabled(), &script(&[(K::Voice, 5), (K::Absent, 200)]), &g, |_| ctx(SA));
        assert_eq!(r.rika.report().armed_count, 0);
    }

    #[test]
    fn g6_r5_barge_in_while_galion_speaks_yields_within_one_frame() {
        let g = granted(Purpose::PersonalAssistant);
        let r = run(RikaConfig::v0_enabled(), &fixture_r1(), &g, |_| RikaCtx { me_speaking: true, ..ctx(SA) });
        common_invariants(&r);
        let rep = *r.rika.report();
        let armed = rep.armed_frame.unwrap() as usize;
        assert!(rep.effect_latency_frames().unwrap() <= 2);
        for o in &r.outs[armed..18] {
            assert_eq!(o.me_energy_db, -3.0);
            assert!(o.yield_floor && o.hold_me_onset);
        }
        assert!(r.outs[18].yield_now);
        assert!(rep.yield_latency_frames.unwrap() <= 1);
        // the lean-in prosody is bounded and calmer, never louder/faster
        let base = ProsodyControl::default();
        let p = r.outs[armed].apply_prosody(base);
        assert!(p.rate < base.rate && p.energy < base.energy && p.pre_pause_ms == 80);
        assert!((p.warmth - 0.6).abs() < 1e-6);
    }

    #[test]
    fn g6_r6_return_after_absence_secondary_trigger_once_soft_ack_at_most_once() {
        let g = granted(Purpose::PersonalAssistant);
        let cfg = RikaConfig { soft_ack: true, ..RikaConfig::v0_enabled() };
        let ks = script(&[(K::Voice, 13), (K::Absent, 500), (K::Presence, 10), (K::Absent, 60)]);
        let r = run(cfg, &ks, &g, |_| ctx(SA));
        common_invariants(&r);
        let rep = *r.rika.report();
        assert_eq!(rep.armed_count, 1);
        assert_eq!(rep.first_trigger, Some(Trigger::ReturnAfterAbsence));
        assert_eq!(rep.evidence_frame, Some(513));
        assert!(rep.effect_latency_frames().unwrap() <= 2);
        assert!(rep.soft_acks <= 1);
        assert!(r.outs.iter().filter(|o| o.cue == Cue::SoftBreath).count() <= 1);
        assert!(r.rika.explain().contains("came back"));
    }

    #[test]
    fn q11_no_soft_ack_before_user_has_spoken_in_session() {
        let g = granted(Purpose::PersonalAssistant);
        let cfg = RikaConfig { soft_ack: true, ..RikaConfig::v0_enabled() };
        let ks = script(&[(K::Room, 13), (K::Breath, 4), (K::Room, 60)]);
        let r = run(cfg, &ks, &g, |_| ctx(SA));
        assert_eq!(r.rika.report().armed_count, 1);
        assert_eq!(r.rika.report().soft_acks, 0);
        assert_eq!(r.rika.report().outcome, Outcome::Released);
    }

    #[test]
    fn g6_r7_many_shifts_at_most_one_armed_per_refractory_window() {
        let g = granted(Purpose::PersonalAssistant);
        let mut parts = vec![(K::Room, 13)];
        for _ in 0..5 {
            parts.push((K::Breath, 2));
            parts.push((K::Room, 13));
        }
        parts.push((K::Room, 30));
        let r = run(RikaConfig::v0_enabled(), &script(&parts), &g, |_| ctx(SA));
        common_invariants(&r);
        let frames = r.rika.frames_seen();
        let windows = (frames + 99) / 100;
        assert!(r.rika.report().armed_count <= windows.max(1));
        assert_eq!(r.rika.report().armed_count, 1);
    }

    #[test]
    fn g6_r8_consent_withdrawn_mid_moment_is_off_same_frame_and_wiped() {
        let g = granted(Purpose::PersonalAssistant);
        let ks = script(&[(K::Room, 13), (K::Breath, 8), (K::Room, 20)]);
        let m = 16usize; // armed at 14; withdraw SENSE at 16
        let r = run(RikaConfig::v0_enabled(), &ks, &g, |i| if (m..m + 5).contains(&i) { ctx(consent_bits::ADAPT) } else { ctx(SA) });
        common_invariants(&r);
        assert_eq!(r.outs[m - 1].state, RikaState::Armed);
        assert_eq!(r.outs[m].state, RikaState::Off);
        assert_eq!(r.outs[m].lean_in, 0.0);
        assert!(!r.outs[m].hold_me_onset && !r.outs[m].yield_floor);
        // m+1: neutral-warm fallback
        let fallback = StubExpressor.plan(&StubExpressor.respond(None));
        assert_eq!(r.outs[m + 1].apply_prosody(fallback), fallback);
        // re-granted: starts from scratch (IDLE, floor/EMA wiped), not ARMED
        assert_eq!(r.outs[m + 5].state, RikaState::Idle);
        assert!(r.outs[m + 5..].iter().all(|o| o.state != RikaState::Armed));
    }

    #[test]
    fn g6_r9_late_frame_does_not_advance_hysteresis_or_burst() {
        let g = granted(Purpose::PersonalAssistant);
        let late_at = 14usize;
        let r = run(RikaConfig::v0_enabled(), &fixture_r1(), &g, |i| RikaCtx { late: i == late_at, ..ctx(SA) });
        common_invariants(&r);
        assert_eq!(r.outs[13].state, RikaState::Candidate);
        assert_eq!(r.outs[late_at].state, RikaState::Candidate, "late frame must not advance hysteresis");
        assert_eq!(r.outs[late_at + 1].state, RikaState::Armed);
        let rep = r.rika.report();
        assert_eq!(rep.armed_count, 1);
        assert_eq!(rep.late_frames, 1);
        assert_eq!(rep.outcome, Outcome::Caught);
    }

    #[test]
    fn one_frame_click_does_not_arm() {
        let g = granted(Purpose::PersonalAssistant);
        let r = run(RikaConfig::v0_enabled(), &script(&[(K::Room, 13), (K::Breath, 1), (K::Room, 20)]), &g, |_| ctx(SA));
        assert_eq!(r.outs[13].state, RikaState::Candidate);
        assert_eq!(r.outs[14].state, RikaState::Quiet);
        assert_eq!(r.rika.report().armed_count, 0);
        assert_eq!(r.rika.report().evidence_frame, None);
    }

    #[test]
    fn hysteresis_is_clamped_to_two_and_knobs_never_positive() {
        let r = Rika::new(RikaConfig { hysteresis: 1, onset_threshold_db: 5.0, me_duck_db: 3.0, ..RikaConfig::v0_enabled() });
        assert_eq!(r.config().hysteresis, 2);
        assert!(r.config().onset_threshold_db <= 0.0 && r.config().me_duck_db <= 0.0);
    }

    #[test]
    fn default_is_disabled_and_no_session_is_off() {
        let g = granted(Purpose::PersonalAssistant);
        assert!(!RikaConfig::default().enabled, "Q14: off until must specs are green");
        let mut a = Rika::new(RikaConfig::default());
        assert_eq!(a.step(ctx(SA), &g, || unreachable!()).off, Some(OffReason::Disabled));
        let mut b = Rika::new(RikaConfig::v0_enabled());
        let o = b.step(RikaCtx { session_open: false, ..ctx(SA) }, &g, || unreachable!());
        assert_eq!(o.off, Some(OffReason::NoSession));
        let o = b.step(ctx(consent_bits::SENSE), &g, || unreachable!());
        assert_eq!(o.off, Some(OffReason::NoAdapt));
        let nog = DefaultConsentGate::default();
        let o = b.step(ctx(SA), &nog, || unreachable!());
        assert_eq!(o.off, Some(OffReason::Gate(ConsentDecision::DenyNoConsent)));
        assert!(G6_FIXTURES.len() >= MUST_SPECS_REQUIRED);
    }

    #[test]
    fn tension_is_primary_affect_dimension_when_present() {
        let g = granted(Purpose::PersonalAssistant);
        let mut r = Rika::new(RikaConfig::v0_enabled());
        let feat = |t: f32| FrameFeatures { rms: 0.001, zcr: 0.5, affect: None, tension: Some(t) };
        for _ in 0..13 {
            r.step(ctx(SA), &g, || feat(0.2));
        }
        assert_eq!(r.step(ctx(SA), &g, || feat(0.5)).state, RikaState::Candidate);
        assert_eq!(r.step(ctx(SA), &g, || feat(0.5)).state, RikaState::Armed);
    }

    #[test]
    fn explain_is_truthful() {
        let g = granted(Purpose::PersonalAssistant);
        let mut r = Rika::new(RikaConfig::v0_enabled());
        r.step(ctx(0), &g, || unreachable!());
        assert!(r.explain().contains("not reading"));
        let run1 = run(RikaConfig::v0_enabled(), &fixture_r1(), &g, |_| ctx(SA));
        assert!(run1.rika.explain().contains("took a breath"));
    }

    #[test]
    fn denied_session_does_not_apply_caller_affect() {
        let angry = EmotionFrame {
            seq: 4,
            t_ms: 320,
            dur_ms: 80,
            valence: -0.8,
            arousal: 0.9,
            dominance: 0.8,
            confidence: 1.0,
        };
        let raw = FrameFeatures { rms: 0.02, zcr: 0.1, affect: Some(angry), tension: Some(0.9) };
        let denied_features = prepare_phone_features(raw, SA, ConsentDecision::DenyNoConsent);
        assert!(denied_features.affect.is_none(), "denied session applied caller affect");
        assert!(denied_features.tension.is_none(), "denied session applied caller affect");
        let no_sense = prepare_phone_features(raw, consent_bits::ADAPT, ConsentDecision::Allow);
        assert!(no_sense.affect.is_none() && no_sense.tension.is_none());
        let open = prepare_phone_features(raw, SA, ConsentDecision::Allow);
        assert_eq!(open.affect, Some(angry));
        assert_eq!(open.tension, Some(0.9));

        let gate = DefaultConsentGate::default();
        assert_eq!(gate.state, crate::consent::ConsentState::Denied);
        assert!(admit_caller_affect(SA, gate.check(Purpose::PersonalAssistant), Some(angry)).is_none());
        let mut rika = Rika::new(RikaConfig::v0_enabled());
        let mut calls = 0u32;
        let out = rika.step(ctx(SA), &gate, || {
            calls += 1;
            raw
        });
        assert_eq!(calls, 0, "denied session read caller affect");
        assert_eq!(out.off, Some(OffReason::Gate(ConsentDecision::DenyNoConsent)));
        assert_eq!(out.lean_in, 0.0);
        assert_eq!(out.apply_prosody(ProsodyControl::default()), ProsodyControl::default());

        let lane = StubExpressor.phone_lane(Some(&angry), SA, Purpose::PersonalAssistant, &gate);
        let applied = StubExpressor.voice_color(Some(&angry));
        assert_ne!(lane, applied, "denied session applied caller affect");
        assert_eq!(lane, StubExpressor.voice_color(None));
    }

    #[test]
    fn glyphs_for_console() {
        assert_eq!(RikaState::Armed.glyph(), '●');
        assert_eq!(RikaState::Candidate.glyph(), '◌');
        assert_eq!(RikaState::Caught.glyph(), '◉');
        assert_eq!(RikaState::Release.glyph(), '○');
        assert_eq!(RikaState::Off.glyph(), ' ');
    }

    #[test]
    fn ram_only_no_file_or_network_concept() {
        let src = include_str!("rika.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for needle in [["std", "::fs"].concat(), ["std", "::net"].concat(), ["File", "::"].concat(), ["Open", "Options"].concat()] {
            assert!(!code.contains(&needle), "rika.rs must stay RAM-only: found {needle}");
        }
    }

    #[test]
    fn state_machine_cost_well_under_one_percent_of_a_frame() {
        let g = granted(Purpose::PersonalAssistant);
        let mut rng = Lcg(13);
        let feats: Vec<FrameFeatures> =
            fixture_r1().iter().enumerate().map(|(i, &k)| FrameFeatures::from_pcm(&pcm(k, &mut rng, i))).collect();
        let mut r = Rika::new(RikaConfig::v0_enabled());
        let mut ns: Vec<u128> = Vec::new();
        for _ in 0..20 {
            for f in &feats {
                let t = std::time::Instant::now();
                std::hint::black_box(r.step(ctx(SA), &g, || *f));
                ns.push(t.elapsed().as_nanos());
            }
        }
        ns.sort_unstable();
        let p95 = ns[ns.len() * 95 / 100];
        assert!(p95 < 800_000, "p95 {p95} ns >= 800 µs");
    }
}
