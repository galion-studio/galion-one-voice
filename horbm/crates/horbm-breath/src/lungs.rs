// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Pass 2. One 80 ms step of volume, phase, pressure, and flow.
//! The state is plain fields. The tick does not allocate.

use galion_emotion_core::{BreathPhase, EmotionCore, VOLUME_REST};

use crate::glottis::controls;
use crate::noise;
use crate::plan::{plan_pauses, Plan};
use crate::{
    depth_fraction, flow_of, group_limit_s, inhale_frames, loudness_db, loudness_gain,
    subglottal_pressure, BreathError, Drive, Frame, Pause, EXHALE_FRAMES, FRAME_S, NOISE_SEED,
    OPEN_PRESSURE, PAUSE_CAP, VOLUME_FLOOR, VOLUME_SPAN,
};

const FLOOR_MARGIN: f32 = 0.02;
const QUIET_STEP: f32 = 0.05;

/// Lung volume and the open phrase. One body for every configured persona.
pub struct Lungs {
    volume: f32,
    phase: BreathPhase,
    inhale_left: u32,
    hold_left: u32,
    release_left: u32,
    peak: f32,
    phrase_s: f32,
    utterance_s: f32,
    for_speech: bool,
    breaks: [f32; PAUSE_CAP],
    break_n: usize,
    break_i: usize,
    planned_fp: u64,
    have_plan: bool,
    cached: Plan,
    rng: u64,
    frames: u32,
    /// Caller nasal target, 0..=1. Used while speaking.
    nasal: f32,
    /// Caller mouth target, 0..=1. Used while speaking.
    mouth: f32,
}

impl Lungs {
    /// Deprecated forward. Same body as [`Self::open`] for any configured id.
    /// There is no `deprecated` attribute this round: the end-to-end runner
    /// still calls this.
    #[must_use]
    pub fn samantha() -> Self {
        Self::with_body()
    }

    /// Any configured persona id. The body is the neutral constructor until a
    /// persona file carries breath values. This does not read that file.
    /// A blank or illegal token is [`BreathError::Persona`].
    pub fn open(persona_id: &str) -> Result<Self, BreathError> {
        if !configured_id(persona_id) {
            return Err(BreathError::Persona);
        }
        Ok(Self::with_body())
    }

    /// Neutral body. Same numbers for every persona this round.
    fn with_body() -> Self {
        Self {
            volume: VOLUME_REST,
            phase: BreathPhase::Rest,
            inhale_left: 0,
            hold_left: 0,
            release_left: 0,
            peak: VOLUME_REST,
            phrase_s: 0.0,
            utterance_s: 0.0,
            for_speech: false,
            breaks: [0.0; PAUSE_CAP],
            break_n: 0,
            break_i: 0,
            planned_fp: 0,
            have_plan: false,
            cached: Plan {
                count: 0,
                truncated: false,
                group_limit_s: group_limit_s(&Drive::rest()),
                group_ms: 0,
            },
            rng: NOISE_SEED,
            frames: 0,
            nasal: 0.0,
            mouth: 0.5,
        }
    }

    /// Nasal and mouth targets for later ticks. Non-finite values become 0.
    /// This does not move lung volume or pressure.
    pub fn set_articulation(&mut self, nasal: f32, mouth: f32) {
        self.nasal = finite01(nasal);
        self.mouth = finite01(mouth);
    }

    #[must_use]
    pub fn frames(&self) -> u32 {
        self.frames
    }

    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Pass 1 for `reply`, kept for the following ticks.
    /// The same text does not reset the phrase clock.
    pub fn plan_reply(&mut self, reply: &str, drive: Drive) -> Plan {
        let fp = fingerprint(reply);
        if self.have_plan && self.planned_fp == fp {
            return self.cached;
        }
        let mut slots = [Pause::marker(); PAUSE_CAP];
        let plan = plan_pauses(reply, drive, &mut slots);
        self.break_n = plan.count.min(PAUSE_CAP);
        for (i, slot) in slots.iter().enumerate().take(self.break_n) {
            self.breaks[i] = slot.at_s;
        }
        self.break_i = 0;
        self.planned_fp = fp;
        self.have_plan = true;
        self.cached = plan;
        plan
    }

    /// Pass 1 when `reply` is `Some`, then pass 2.
    ///
    /// `speaking` is the me band. `false` is the you band on this same
    /// 80 ms call: the lungs recover and do not spend speech air.
    #[must_use]
    pub fn frame(&mut self, core: &EmotionCore, speaking: bool, reply: Option<&str>) -> Frame {
        let drive = Drive::from_core(core);
        if let Some(reply) = reply {
            self.plan_reply(reply, drive);
        }
        self.tick(drive, speaking)
    }

    /// One 80 ms step. Allocates nothing.
    #[must_use]
    pub fn tick(&mut self, drive: Drive, speaking: bool) -> Frame {
        let drive = drive.clamped();
        self.frames = self.frames.wrapping_add(1);
        if self.inhale_left > 0 {
            return self.step_inhale(&drive, speaking);
        }
        if self.hold_left > 0 {
            return self.step_hold(&drive, speaking);
        }
        if speaking {
            self.release_left = 0;
            if self.phase != BreathPhase::Speech {
                self.begin_speech_inhale(&drive);
                return self.step_inhale(&drive, true);
            }
            if self.needs_break(&drive) {
                self.consume_breaks();
                self.phrase_s = 0.0;
                self.begin_speech_inhale(&drive);
                return self.step_inhale(&drive, true);
            }
            return self.step_speech(&drive);
        }
        self.step_quiet(&drive)
    }

    /// Soft inhale for the frame just stepped, when that frame was an inhale.
    ///
    /// Writes at most one duplex frame into `out`. Returns 0 and leaves
    /// `out` untouched when the phase is not inhale. Allocates nothing.
    /// The noise sequence advances only on a write.
    pub fn write_inhale(&mut self, out: &mut [f32]) -> usize {
        if self.phase != BreathPhase::Inhale {
            return 0;
        }
        noise::fill(&mut self.rng, out)
    }

    fn begin_speech_inhale(&mut self, drive: &Drive) {
        let depth = depth_fraction(drive) * VOLUME_SPAN;
        let peak = (self.volume + depth)
            .min(VOLUME_REST + VOLUME_SPAN)
            .min(1.0)
            .max(self.volume);
        self.peak = peak;
        self.inhale_left = inhale_frames(drive).max(1);
        self.for_speech = true;
        self.phase = BreathPhase::Inhale;
    }

    fn step_inhale(&mut self, drive: &Drive, speaking: bool) -> Frame {
        let left = self.inhale_left.max(1) as f32;
        let target = if self.for_speech {
            self.peak
        } else {
            VOLUME_REST
        };
        let before = self.volume;
        let step = (target - self.volume) / left;
        self.volume = clamp01(self.volume + step);
        self.inhale_left = self.inhale_left.saturating_sub(1);
        if self.inhale_left == 0 && self.for_speech && speaking {
            self.hold_left = 1;
        }
        self.phase = BreathPhase::Inhale;
        let flow = flow_of(self.volume - before);
        self.air_frame(
            drive,
            BreathPhase::Inhale,
            flow,
            speaking && self.for_speech,
        )
    }

    fn step_hold(&mut self, drive: &Drive, speaking: bool) -> Frame {
        self.hold_left = 0;
        if speaking {
            self.phase = BreathPhase::Speech;
        } else {
            self.phase = BreathPhase::Rest;
            self.for_speech = false;
        }
        self.air_frame(drive, BreathPhase::Hold, 0.0, speaking)
    }

    fn step_speech(&mut self, drive: &Drive) -> Frame {
        let limit = group_limit_s(drive).max(FRAME_S);
        let remain = (limit - self.phrase_s).max(FRAME_S);
        let floor = VOLUME_FLOOR + FLOOR_MARGIN;
        let drop = ((self.volume - floor).max(0.0) * (FRAME_S / remain)).min(self.volume);
        let before = self.volume;
        self.volume = clamp01(self.volume - drop);
        self.phrase_s += FRAME_S;
        self.utterance_s += FRAME_S;
        self.phase = BreathPhase::Speech;
        self.for_speech = false;
        let flow = flow_of(self.volume - before);
        let pressure = subglottal_pressure(self.volume, *drive);
        let shape = controls(drive, BreathPhase::Speech, self.nasal, self.mouth);
        Frame {
            phase: BreathPhase::Speech,
            volume: self.volume,
            flow,
            pressure,
            loudness_db: loudness_db(pressure),
            loudness_gain: loudness_gain(pressure),
            pause: false,
            open_quotient: shape.open_quotient,
            breathiness: shape.breathiness,
            nasal_coupling: shape.nasal_coupling,
            mouth_openness: shape.mouth_openness,
        }
    }

    fn step_quiet(&mut self, drive: &Drive) -> Frame {
        if self.phase == BreathPhase::Speech {
            self.release_left = EXHALE_FRAMES;
            self.phrase_s = 0.0;
            self.phase = BreathPhase::Exhale;
        }
        if self.release_left > 0 {
            return self.step_release(drive);
        }
        if self.volume + 0.01 < VOLUME_REST {
            self.begin_quiet_inhale();
            return self.step_inhale(drive, false);
        }
        self.phase = BreathPhase::Rest;
        self.for_speech = false;
        self.air_frame(drive, BreathPhase::Rest, 0.0, false)
    }

    fn step_release(&mut self, drive: &Drive) -> Frame {
        self.release_left = self.release_left.saturating_sub(1);
        let before = self.volume;
        let drop = QUIET_STEP.min((self.volume - VOLUME_FLOOR).max(0.0));
        self.volume = clamp01(self.volume - drop);
        let flow = flow_of(self.volume - before);
        if self.release_left == 0 {
            self.phase = BreathPhase::Rest;
        } else {
            self.phase = BreathPhase::Exhale;
        }
        self.air_frame(drive, BreathPhase::Exhale, flow, false)
    }

    fn begin_quiet_inhale(&mut self) {
        let gap = (VOLUME_REST - self.volume).max(0.0);
        let steps = (gap / QUIET_STEP) as u32;
        let covered = steps as f32 * QUIET_STEP;
        let extra = if gap > covered { 1 } else { 0 };
        self.inhale_left = (steps.saturating_add(extra)).clamp(1, 40);
        self.for_speech = false;
        self.peak = VOLUME_REST;
        self.phase = BreathPhase::Inhale;
    }

    fn needs_break(&self, drive: &Drive) -> bool {
        let live = self.phrase_s + FRAME_S > group_limit_s(drive) + 1.0e-4;
        let planned = self.break_i < self.break_n
            && self.utterance_s + FRAME_S > self.breaks[self.break_i] + 1.0e-4;
        let empty = self.volume <= VOLUME_FLOOR + FLOOR_MARGIN + 1.0e-4;
        live || planned || empty
    }

    fn consume_breaks(&mut self) {
        while self.break_i < self.break_n
            && self.breaks[self.break_i] <= self.utterance_s + FRAME_S + 1.0e-3
        {
            self.break_i += 1;
        }
    }

    fn air_frame(&self, drive: &Drive, phase: BreathPhase, flow: f32, pause: bool) -> Frame {
        let shape = controls(drive, phase, self.nasal, self.mouth);
        Frame {
            phase,
            volume: self.volume,
            flow,
            pressure: OPEN_PRESSURE,
            loudness_db: 0.0,
            loudness_gain: 0.0,
            pause,
            open_quotient: shape.open_quotient,
            breathiness: shape.breathiness,
            nasal_coupling: shape.nasal_coupling,
            mouth_openness: shape.mouth_openness,
        }
    }
}

fn finite01(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn clamp01(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(0.0, 1.0)
    } else {
        VOLUME_REST
    }
}

/// Short token, the shape a configured persona id has: 1 to 16 bytes, a
/// lowercase letter, then lowercase letters, digits, or hyphens.
fn configured_id(id: &str) -> bool {
    let id = id.trim();
    let bytes = id.as_bytes();
    (1..=16).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

fn fingerprint(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash ^ (text.len() as u64).wrapping_mul(0x9e37_79b9)
}
