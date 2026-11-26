// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The configured persona's lungs for MnE V2.
//!
//! Emotion stays an open point on the 80 ms clock. This crate does not
//! read a category list. The speaker's baseline (persona bytes, voice id,
//! pitch centre) is not an input. The moment's arousal and the inside
//! factors energy, fatigue, and health are.
//!
//! Two passes sit on the duplex's dual-band tick:
//!
//! 1. [`plan_pauses`] places phrase breaks in a reply when the air budget
//!    would run out, or at a sentence or clause boundary.
//! 2. [`Lungs::tick`] steps lung volume, phase, pressure, and flow for one
//!    80 ms frame. `speaking` is the me band. Quiet is the you band on that
//!    same call: recover, do not spend the speech budget. The same frame
//!    carries glottal open quotient, breathiness, nasal coupling, and mouth
//!    openness. Those do not change the lungs sample.
//!
//! The lungs sample is [`galion_emotion_core::Breath`]: phase, volume, and
//! flow. `galion-source-filter` (PR 305) reads that value in
//! `MouthControls::from_moment` and mixes breath noise only while the phase
//! is [`BreathPhase::Exhale`]. Pressure on the frame is the loudness
//! envelope. It is not a second lungs type. This crate does not link the
//! source-filter or the articulation crates.
//!
//! The hot path allocates nothing ([`TICK_ALLOC_BUDGET`]). Inhale noise is
//! the heartbeat's seeded LCG, written into a caller buffer. No recording
//! is stored here.
//!
//! # Safety
//!
//! No `unsafe`.

#![forbid(unsafe_code)]

mod glottis;
mod lungs;
mod noise;
mod plan;

use galion_emotion_core::{
    EmotionCore, IntentMsg, VoiceBaseline, AROUSAL, ENERGY_REST, FATIGUE_REST, HEALTH_REST,
    MASK_BREATH, VOLUME_REST,
};

pub use galion_emotion_core::{Breath, BreathPhase, FRAME_MS as EMOTION_FRAME_MS};
pub use glottis::{controls, open_quotient_from_rd, Articulation, GlottisError};
pub use lungs::Lungs;
pub use plan::{plan_pauses, Pause, PauseWhy, Plan};

/// Symphony clock. Same value as `LOCKS.toml` `clock.frame_ms`.
pub const FRAME_MS: u32 = 80;
pub const FRAME_S: f32 = 0.08;

/// 16 kHz, one duplex frame. The mouth and the ear use this rate.
pub const SAMPLE_RATE: u32 = 16_000;
pub const INHALE_SAMPLES: usize = 1_280;

/// Heap allocations one [`Lungs::tick`], [`plan_pauses`], or
/// [`Lungs::write_inhale`] is allowed to perform.
pub const TICK_ALLOC_BUDGET: usize = 0;

/// Pause slots in one reply. A longer reply sets [`Plan::truncated`].
pub const PAUSE_CAP: usize = 32;

/// Breath-group limit at rate 1. Knob from `configs/symphony/lungs.toml`
/// `[phrase]`. Not a measured breath-group length.
pub const GROUP_MAX_S: f32 = 4.0;
pub const GROUP_MIN_S: f32 = 0.48;
pub const CLAUSE_BREAK_S: f32 = 1.2;
pub const CHARS_PER_S: f32 = 12.0;

pub const INSP_BASE_S: f32 = 0.24;
pub const INSP_PER_PHRASE_S: f32 = 0.08;
pub const INSP_MIN_S: f32 = 0.16;
pub const INSP_MAX_S: f32 = 0.80;

pub const DEPTH_BASE: f32 = 0.35;
pub const DEPTH_PER_PHRASE_S: f32 = 0.08;
pub const DEPTH_MIN: f32 = 0.15;
pub const DEPTH_MAX: f32 = 1.0;
pub const VOLUME_SPAN: f32 = 0.45;
pub const VOLUME_FLOOR: f32 = 0.15;

pub const PS_REF: f32 = 1.0;
pub const PS_PER_VOLUME: f32 = 0.80;
pub const PS_PER_ENERGY: f32 = 0.25;
pub const PS_PER_AROUSAL: f32 = 0.15;
pub const LOUDNESS_DB_PER_PS: f32 = 6.0;
/// First-order map from a small dB offset to a linear gain.
/// `ln(10) / 20`. Not a sound-pressure calibration.
pub const DB_TO_LIN: f32 = 0.115_129_25;

/// Homma and Masaoka (2008) direction: higher arousal, faster breathing.
/// The coefficient is the lungs-toml knob, not a published breaths/min.
pub const AROUSAL_RATE_K: f32 = 0.50;
/// Same shape as [`AROUSAL_RATE_K`], zeroed at [`ENERGY_REST`].
/// Energy is not in the lungs-toml rate term. This knob is the owner's
/// "energy raises the breath rate" rule. It is not a published slope.
pub const ENERGY_RATE_K: f32 = 0.50;
/// Fatigue literature is none. Sign matches `lungs.toml` `[fatigue]`.
pub const FATIGUE_GROUP_PER: f32 = -0.20;
pub const FATIGUE_DEPTH_PER: f32 = -0.20;
/// Health 1 keeps depth. Health 0 multiplies depth by this floor.
/// A knob, not a diagnosis.
pub const HEALTH_DEPTH_FLOOR: f32 = 0.6;

/// Open-glottis pressure while air is moving and the folds are not driving speech.
/// Relative, not cm H₂O.
pub const OPEN_PRESSURE: f32 = 0.05;
pub const NEUTRAL_RMS: f32 = 0.003;
pub const NOISE_SEED: u64 = 13;
pub const EXHALE_FRAMES: u32 = 2;
pub const FLOW_FULL_PER_S: f32 = VOLUME_SPAN / 0.40;

pub const MEASURE_BATCHES: u32 = 200;
pub const MEASURE_BATCH: u32 = 100;

const _: () = assert!(FRAME_MS == 80);
const _: () = assert!(EMOTION_FRAME_MS == FRAME_MS);
const _: () = assert!(INHALE_SAMPLES == 80 * 16);

/// Why a speaker was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathError {
    /// The id is blank or not a short token.
    Persona,
}

impl std::fmt::Display for BreathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Persona => f.write_str("persona id is not a short token"),
        }
    }
}

impl std::error::Error for BreathError {}

/// What the lungs read from the emotion core. Not the baseline voice.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drive {
    /// Moment arousal, 0..1. Not the baseline pitch and not a category.
    pub arousal: f32,
    pub energy: f32,
    pub fatigue: f32,
    pub health: f32,
}

impl Drive {
    /// Rest point of [`EmotionCore::at_rest`]. Pitch and persona are not copied.
    #[must_use]
    pub fn rest() -> Self {
        Self::from_core(&EmotionCore::at_rest())
    }

    /// Moment arousal plus inside energy, fatigue, and health.
    ///
    /// Lung volume on the core is ignored. This crate owns that track.
    /// Baseline pitch, persona bytes, and voice id are ignored.
    #[must_use]
    pub fn from_core(core: &EmotionCore) -> Self {
        let moment = core.moment();
        let inside = core.inside();
        Self {
            arousal: unit(moment.pos[AROUSAL], arousal_rest()),
            energy: unit(inside.energy, ENERGY_REST),
            fatigue: unit(inside.fatigue, FATIGUE_REST),
            health: unit(inside.health, HEALTH_REST),
        }
    }

    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            arousal: unit(self.arousal, arousal_rest()),
            energy: unit(self.energy, ENERGY_REST),
            fatigue: unit(self.fatigue, FATIGUE_REST),
            health: unit(self.health, HEALTH_REST),
        }
    }
}

/// Arousal of the published stand-in rest. Not a population norm.
#[must_use]
pub fn arousal_rest() -> f32 {
    VoiceBaseline::standin().pos[AROUSAL]
}

/// Rate multiplier. 1 at arousal 0 and energy at or below rest.
/// Higher arousal or energy makes the multiplier larger, so groups shorten.
#[must_use]
pub fn rate_mul(drive: &Drive) -> f32 {
    let drive = drive.clamped();
    let mut rate = 1.0;
    if drive.arousal > 0.0 {
        rate += AROUSAL_RATE_K * drive.arousal;
    }
    if drive.energy > ENERGY_REST {
        rate += ENERGY_RATE_K * (drive.energy - ENERGY_REST);
    }
    rate
}

/// Fatigue scale divided by [`rate_mul`]. Smaller means a shorter group.
#[must_use]
pub fn group_scale(drive: &Drive) -> f32 {
    let drive = drive.clamped();
    let fatigue = 1.0 + FATIGUE_GROUP_PER * drive.fatigue;
    (fatigue / rate_mul(&drive)).max(0.05)
}

/// Spoken seconds one breath group may spend at this drive.
#[must_use]
pub fn group_limit_s(drive: &Drive) -> f32 {
    let raw = GROUP_MAX_S * group_scale(drive);
    snap_s(raw).clamp(GROUP_MIN_S, GROUP_MAX_S)
}

/// A comma, semicolon, or colon may close a group once it has reached this.
#[must_use]
pub fn clause_limit_s(drive: &Drive) -> f32 {
    let raw = CLAUSE_BREAK_S * group_scale(drive);
    snap_s(raw).max(FRAME_S)
}

/// Inhale length that supports the current group. Faster drive, shorter inhale.
#[must_use]
pub fn inhale_s(drive: &Drive) -> f32 {
    let speech_s = group_limit_s(drive);
    let raw = (INSP_BASE_S + INSP_PER_PHRASE_S * speech_s).clamp(INSP_MIN_S, INSP_MAX_S);
    snap_s(raw / rate_mul(drive)).max(FRAME_S)
}

#[must_use]
pub fn inhale_frames(drive: &Drive) -> u32 {
    let frames = round_pos(inhale_s(drive) / FRAME_S);
    if frames >= 1.0 {
        frames as u32
    } else {
        1
    }
}

/// Fraction of [`VOLUME_SPAN`] inhaled before the next phrase.
#[must_use]
pub fn depth_fraction(drive: &Drive) -> f32 {
    let speech_s = group_limit_s(drive);
    let drive = drive.clamped();
    let raw = (DEPTH_BASE + DEPTH_PER_PHRASE_S * speech_s).clamp(DEPTH_MIN, DEPTH_MAX);
    let fatigue = (1.0 + FATIGUE_DEPTH_PER * drive.fatigue).clamp(0.2, 2.0);
    let health = HEALTH_DEPTH_FLOOR + (1.0 - HEALTH_DEPTH_FLOOR) * drive.health;
    (raw * fatigue * health).clamp(DEPTH_MIN, DEPTH_MAX)
}

/// Relative subglottal pressure. Not cm H₂O.
///
/// Zero extra drive at rest volume, rest energy, and rest arousal.
/// Volume falling lowers it. Arousal above rest and energy above rest raise it.
#[must_use]
pub fn subglottal_pressure(volume: f32, drive: Drive) -> f32 {
    let volume = if volume.is_finite() {
        volume.clamp(0.0, 1.0)
    } else {
        VOLUME_REST
    };
    let drive = drive.clamped();
    let ps = PS_REF
        + PS_PER_ENERGY * (drive.energy - ENERGY_REST)
        + PS_PER_AROUSAL * (drive.arousal - arousal_rest()).max(0.0)
        + PS_PER_VOLUME * (volume - VOLUME_REST);
    if ps.is_finite() {
        ps
    } else {
        PS_REF
    }
}

/// Loudness offset in dB from [`PS_REF`]. Not dB SPL.
#[must_use]
pub fn loudness_db(pressure: f32) -> f32 {
    if pressure.is_finite() {
        LOUDNESS_DB_PER_PS * (pressure - PS_REF)
    } else {
        0.0
    }
}

/// Linear gain the mouth can multiply onto a speech frame.
/// First-order in the dB offset, clamped to 0..2.
#[must_use]
pub fn loudness_gain(pressure: f32) -> f32 {
    let gain = 1.0 + DB_TO_LIN * loudness_db(pressure);
    if gain.is_finite() {
        gain.clamp(0.0, 2.0)
    } else {
        1.0
    }
}

/// One 80 ms lungs frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub phase: BreathPhase,
    /// Relative volume, 0..1. Not a litre.
    pub volume: f32,
    /// +1 inhale through −1 exhale. Negative flow is outward air.
    /// `galion-source-filter` uses that sign for the exhale gain.
    pub flow: f32,
    /// Relative subglottal pressure during speech, or [`OPEN_PRESSURE`]
    /// otherwise. The loudness envelope. Not a second lungs sample, and not
    /// cm H₂O. The source-filter does not read it.
    pub pressure: f32,
    /// dB offset from [`PS_REF`] during speech. 0 otherwise. Not dB SPL.
    pub loudness_db: f32,
    /// Mouth amplitude multiplier. 0 when this frame is not speech.
    pub loudness_gain: f32,
    /// The me band should not phonate. Air is moving in, or the phrase ended.
    pub pause: bool,
    /// Fant open quotient while speaking. Abduction otherwise.
    pub open_quotient: f32,
    /// 1 when this frame is air. Rd mix while speaking.
    pub breathiness: f32,
    /// 0 oral, 1 nasal. Caller target, reduced when health is low.
    pub nasal_coupling: f32,
    /// 0 closed, 1 open. Caller target, reduced by fatigue while speaking.
    pub mouth_openness: f32,
}

impl Frame {
    /// The lungs sample `MouthControls::from_moment` and
    /// `SpeechStack::set_mouth_moment` take.
    ///
    /// Noise is mixed only when `phase` is [`BreathPhase::Exhale`]. Outward
    /// air is a negative flow. Volume is relative.
    #[must_use]
    pub fn lungs(self) -> Breath {
        Breath {
            phase: self.phase,
            volume: self.volume,
            flow: self.flow,
        }
    }
}

/// Write the lungs frame into an intent so the core can stop using the
/// stand-in breath cycle. Does not touch the pull, the verb, or the baseline.
pub fn stamp_intent(frame: &Frame, msg: &mut IntentMsg) {
    let lungs = frame.lungs();
    msg.breath_phase = lungs.phase;
    msg.lung_volume = lungs.volume.clamp(0.0, 1.0);
    msg.air_flow = lungs.flow.clamp(-1.0, 1.0);
    msg.factor_mask |= MASK_BREATH;
}

/// Isolated tick times. Nanoseconds are floored per batch.
/// The collector allocates. [`Lungs::tick`] does not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickTimes {
    pub batches: u32,
    pub batch: u32,
    pub p50_ns: u64,
    pub p99_ns: u64,
}

impl TickTimes {
    #[must_use]
    pub fn p50_us(self) -> f64 {
        self.p50_ns as f64 / 1_000.0
    }

    #[must_use]
    pub fn p99_us(self) -> f64 {
        self.p99_ns as f64 / 1_000.0
    }
}

/// Time [`Lungs::tick`] at the rest drive, me band.
///
/// [`MEASURE_BATCHES`] batches of [`MEASURE_BATCH`] ticks. One warmup batch
/// is not recorded. Nearest-rank p50 and p99 of the per-tick nanoseconds.
#[must_use]
pub fn measure_ticks() -> TickTimes {
    measure_loop(MEASURE_BATCHES, MEASURE_BATCH, false)
}

/// Time [`Lungs::write_inhale`] on a full frame, same batch shape as [`measure_ticks`].
#[must_use]
pub fn measure_inhale() -> TickTimes {
    measure_loop(MEASURE_BATCHES, MEASURE_BATCH, true)
}

fn measure_loop(batches: u32, batch: u32, inhale: bool) -> TickTimes {
    let mut lungs = Lungs::open("configured").expect("bench id");
    let drive = Drive::rest();
    let mut buf = [0.0f32; INHALE_SAMPLES];
    if inhale {
        prime_inhale(&mut lungs, drive);
    }
    for _ in 0..batch.max(1) {
        step_measured(&mut lungs, drive, inhale, &mut buf);
    }
    let mut samples = Vec::with_capacity(batches as usize);
    for _ in 0..batches {
        let start = std::time::Instant::now();
        for _ in 0..batch {
            step_measured(&mut lungs, drive, inhale, &mut buf);
        }
        let elapsed = start.elapsed().as_nanos();
        let per = elapsed / u128::from(batch.max(1));
        samples.push(u64::try_from(per).unwrap_or(u64::MAX));
    }
    samples.sort_unstable();
    TickTimes {
        batches,
        batch,
        p50_ns: nearest(&samples, 0.50).unwrap_or(0),
        p99_ns: nearest(&samples, 0.99).unwrap_or(0),
    }
}

fn step_measured(lungs: &mut Lungs, drive: Drive, inhale: bool, buf: &mut [f32; INHALE_SAMPLES]) {
    if inhale {
        let n = lungs.write_inhale(buf);
        let _ = std::hint::black_box(n);
        let _ = std::hint::black_box(buf[0]);
    } else {
        let frame = lungs.tick(drive, true);
        let _ = std::hint::black_box(frame);
    }
}

fn prime_inhale(lungs: &mut Lungs, drive: Drive) {
    for _ in 0..8 {
        let frame = lungs.tick(drive, true);
        if frame.phase == BreathPhase::Inhale {
            return;
        }
    }
}

fn nearest(sorted: &[u64], quantile: f64) -> Option<u64> {
    if sorted.is_empty() || !(0.0..=1.0).contains(&quantile) {
        return None;
    }
    let n = sorted.len();
    let rank = (quantile * n as f64).ceil() as usize;
    let index = rank.clamp(1, n) - 1;
    Some(sorted[index])
}

pub(crate) fn snap_s(seconds: f32) -> f32 {
    if seconds <= 0.0 || !seconds.is_finite() {
        return 0.0;
    }
    round_pos(seconds / FRAME_S) * FRAME_S
}

/// Positive half-up rounding without libm.
pub(crate) fn round_pos(x: f32) -> f32 {
    if !x.is_finite() || x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0e6 {
        return 1.0e6;
    }
    let whole = x as i32;
    let frac = x - whole as f32;
    let rounded = if frac >= 0.5 { whole + 1 } else { whole };
    rounded as f32
}

pub(crate) fn flow_of(delta_volume: f32) -> f32 {
    if !delta_volume.is_finite() || FLOW_FULL_PER_S <= 0.0 {
        return 0.0;
    }
    (delta_volume / FRAME_S / FLOW_FULL_PER_S).clamp(-1.0, 1.0)
}

fn unit(x: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_pressure_sits_on_the_reference() {
        let ps = subglottal_pressure(VOLUME_REST, Drive::rest());
        assert!((ps - PS_REF).abs() < 1.0e-5, "ps {ps}");
        assert!((loudness_db(ps)).abs() < 1.0e-5);
    }

    #[test]
    fn arousal_and_energy_raise_rate_and_pressure() {
        let rest = Drive::rest();
        let hot = Drive {
            arousal: 1.0,
            ..rest
        };
        let charged = Drive {
            energy: 1.0,
            ..rest
        };
        assert!(rate_mul(&hot) > rate_mul(&rest));
        assert!(rate_mul(&charged) > rate_mul(&rest));
        assert!(group_limit_s(&hot) < group_limit_s(&rest));
        assert!(group_limit_s(&charged) < group_limit_s(&rest));
        assert!(inhale_frames(&hot) <= inhale_frames(&rest));
        let ps_rest = subglottal_pressure(VOLUME_REST, rest);
        assert!(subglottal_pressure(VOLUME_REST, hot) > ps_rest);
        assert!(subglottal_pressure(VOLUME_REST, charged) > ps_rest);
        assert!(subglottal_pressure(VOLUME_REST - 0.1, rest) < ps_rest);
    }

    #[test]
    fn low_energy_does_not_slow_the_rate() {
        let rest = Drive::rest();
        let tired = Drive {
            energy: 0.1,
            ..rest
        };
        assert_eq!(rate_mul(&tired).to_bits(), rate_mul(&rest).to_bits());
    }
}
