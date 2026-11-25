// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Samantha's moment, on the controls the mouth already has.
//!
//! [`VoiceBaseline`] is who she is (persona, voice id, rest point, design
//! pitch centre). [`MomentState`] is how she feels on this 80 ms frame.
//! The map uses only the difference. A frame that sits on the baseline is
//! the Samantha e0 line from [`galion_sam_prosody::map`]. Aria is parked:
//! her packet stays the unsteered line no matter where the moment is.
//!
//! The warm Piper process reads `length_scale`, `noise_scale`, and `noise_w`
//! once, at spawn. This module still emits those three every frame. Pace is
//! applied as a sample stretch, which is the change a later rate can make
//! without killing the process. Pitch, energy, pause, and breath are the
//! sentence-level knobs the mouth already shapes onto PCM.
//!
//! A new target is fully on the voice two frames later (160 ms,
//! `LOCKS.toml` `gates.affect_to_voice_max_frames`). The frame in between
//! is the midpoint, so the controls do not jump. No model, no audio file.

use galion_emotion_core::{
    Breath, BreathPhase, MomentState, VoiceBaseline, APPROACH, AROUSAL, DOMINANCE, VALENCE,
};
use galion_sam_prosody::{
    map, Pad, BASE_ENERGY, BASE_LENGTH, BASE_PITCH_ST, BASE_PRE_PAUSE_MS, ENERGY_MAX, ENERGY_MIN,
    LENGTH_MAX, LENGTH_MIN, NOISE_MAX, NOISE_MIN, NOISE_W_MAX, NOISE_W_MIN, PAUSE_CEILING_MS,
    PAUSE_MIN_MS, PITCH_BOUND_ST, SILENCE_MAX_S, SILENCE_MIN_S, WORKER_RATE_MAX, WORKER_RATE_MIN,
};

use crate::clock::FRAME_MS;

/// `LOCKS.toml` `[gates] affect_to_voice_max_frames`.
pub const AFFECT_TO_VOICE_FRAMES: u32 = 2;
/// `LOCKS.toml` `[gates] affect_to_voice_max_ms`. Two frames of [`FRAME_MS`].
pub const AFFECT_TO_VOICE_MS: u32 = FRAME_MS * AFFECT_TO_VOICE_FRAMES;

/// Aria has no moment map. The flag is the park, not a fifth consent bit.
pub const ARIA_PARKED: bool = true;

/// Approach lean, length units per axis unit. Higher approach is slightly faster.
const APPROACH_LENGTH: f32 = 0.05;
/// Approach lean, energy units per axis unit.
const APPROACH_ENERGY: f32 = 0.05;

const _: () = assert!(AFFECT_TO_VOICE_FRAMES == 2);
const _: () = assert!(AFFECT_TO_VOICE_MS == 160);
const _: () = assert!(FRAME_MS == 80);

/// Where a breath sits on the sentence. Not a bracket token. Piper would
/// speak a token, and `samantha.toml` leaves inhale off that list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathPlace {
    None,
    /// Inhale. Silence immediately before the sentence body.
    Before,
    /// Exhale. Silence immediately after the sentence body.
    After,
}

/// One frame of mouth controls. Identity fields are copies of the baseline.
#[derive(Clone, Copy, Debug)]
pub struct VoiceControls {
    pub persona: [u8; 16],
    pub voice: [u8; 16],
    /// Design centre in hertz. The map does not add [`Self::pitch_st`] into it.
    pub pitch_centre_hz: f32,
    pub length_scale: f32,
    pub noise_scale: f32,
    pub noise_w: f32,
    pub sentence_silence_s: f32,
    /// Sentence-level offset from [`Self::pitch_centre_hz`], in semitones.
    pub pitch_st: f32,
    pub energy: f32,
    pub pre_pause_ms: f32,
    /// `1 / length_scale`, clamped to the worker rate band.
    pub rate: f32,
    pub breath: BreathPlace,
    pub breath_ms: u32,
    /// True for Aria and for every persona other than Samantha.
    pub parked: bool,
}

impl VoiceControls {
    /// Unsteered packet. Pace and energy are unity. Noise stays off so a
    /// parked seat does not inherit Samantha's timbre.
    pub const fn unsteered() -> Self {
        Self {
            persona: [0; 16],
            voice: [0; 16],
            pitch_centre_hz: 0.0,
            length_scale: 1.0,
            noise_scale: 0.0,
            noise_w: 0.0,
            sentence_silence_s: 0.0,
            pitch_st: 0.0,
            energy: 1.0,
            pre_pause_ms: 0.0,
            rate: 1.0,
            breath: BreathPlace::None,
            breath_ms: 0,
            parked: true,
        }
    }

    /// Pace, pitch, energy, pause, and breath leave the samples alone.
    pub fn is_neutral_shape(self) -> bool {
        (self.length_scale - 1.0).abs() < 1.0e-3
            && self.pitch_st.abs() < 1.0e-3
            && (self.energy - 1.0).abs() < 1.0e-3
            && self.pre_pause_ms < 0.5
            && self.breath == BreathPlace::None
            && self.breath_ms == 0
    }

    pub fn in_bounds(self) -> bool {
        self.pitch_centre_hz.is_finite()
            && self.rate.is_finite()
            && self.rate > 0.0
            && finite_between(self.length_scale, LENGTH_MIN, LENGTH_MAX)
            && finite_between(self.pitch_st, -PITCH_BOUND_ST, PITCH_BOUND_ST)
            && finite_between(self.energy, ENERGY_MIN, ENERGY_MAX)
            && finite_between(self.pre_pause_ms, PAUSE_MIN_MS, PAUSE_CEILING_MS)
            && finite_between(self.rate, WORKER_RATE_MIN, WORKER_RATE_MAX)
            && self.breath_ms <= AFFECT_TO_VOICE_MS
            && if self.parked {
                self.noise_scale == 0.0 && self.noise_w == 0.0 && self.is_neutral_shape()
            } else {
                finite_between(self.noise_scale, NOISE_MIN, NOISE_MAX)
                    && finite_between(self.noise_w, NOISE_W_MIN, NOISE_W_MAX)
                    && finite_between(self.sentence_silence_s, SILENCE_MIN_S, SILENCE_MAX_S)
            }
    }

    fn clamped(self) -> Self {
        if self.parked {
            return parked_packet(self.persona, self.voice, self.pitch_centre_hz);
        }
        let length = finite_or(self.length_scale, BASE_LENGTH).clamp(LENGTH_MIN, LENGTH_MAX);
        let rate = (1.0 / length).clamp(WORKER_RATE_MIN, WORKER_RATE_MAX);
        Self {
            persona: self.persona,
            voice: self.voice,
            pitch_centre_hz: finite_pitch(self.pitch_centre_hz),
            length_scale: length,
            noise_scale: finite_or(self.noise_scale, NOISE_MIN).clamp(NOISE_MIN, NOISE_MAX),
            noise_w: finite_or(self.noise_w, NOISE_MIN).clamp(NOISE_W_MIN, NOISE_W_MAX),
            sentence_silence_s: finite_or(self.sentence_silence_s, SILENCE_MIN_S)
                .clamp(SILENCE_MIN_S, SILENCE_MAX_S),
            pitch_st: finite_or(self.pitch_st, BASE_PITCH_ST)
                .clamp(-PITCH_BOUND_ST, PITCH_BOUND_ST),
            energy: finite_or(self.energy, BASE_ENERGY).clamp(ENERGY_MIN, ENERGY_MAX),
            pre_pause_ms: finite_or(self.pre_pause_ms, BASE_PRE_PAUSE_MS)
                .clamp(PAUSE_MIN_MS, PAUSE_CEILING_MS),
            rate,
            breath: self.breath,
            breath_ms: self.breath_ms.min(AFFECT_TO_VOICE_MS),
            parked: false,
        }
    }
}

/// What the voice loop reads off the brain for one frame.
#[derive(Clone, Copy, Debug)]
pub struct MomentView {
    pub baseline: VoiceBaseline,
    pub moment: MomentState,
    pub breath: Breath,
}

/// The voice turn's speaker. Persona id `samantha`, e0 rest point, the
/// design pitch centre already on [`VoiceBaseline::standin`]. Not a recording.
pub fn samantha_baseline() -> VoiceBaseline {
    VoiceBaseline {
        persona: id_slot("samantha"),
        voice: id_slot("samantha-e0"),
        // `SAMANTHA.dynamics.rest` (valence, arousal, dominance). Approach 0
        // is neither lean nor withdrawal; that axis is not in the persona rest.
        pos: [0.25, 0.35, 0.45, 0.0],
        pitch_centre_hz: VoiceBaseline::standin().pitch_centre_hz,
    }
}

/// Parked seat. The numbers fill the baseline struct. They are not a voice.
pub fn aria_baseline() -> VoiceBaseline {
    VoiceBaseline {
        persona: id_slot("aria"),
        voice: id_slot("aria-parked"),
        pos: [0.0, 0.30, 0.50, 0.0],
        pitch_centre_hz: VoiceBaseline::standin().pitch_centre_hz,
    }
}

pub fn is_samantha(baseline: &VoiceBaseline) -> bool {
    baseline.persona_id().eq_ignore_ascii_case("samantha")
}

pub fn is_aria(baseline: &VoiceBaseline) -> bool {
    baseline.persona_id().eq_ignore_ascii_case("aria")
}

/// Instant map. Does not smooth and does not write `baseline`.
pub fn instant(baseline: VoiceBaseline, moment: MomentState, breath: Breath) -> VoiceControls {
    if !is_samantha(&baseline) {
        return parked_packet(baseline.persona, baseline.voice, baseline.pitch_centre_hz);
    }
    map_samantha(&baseline, moment, breath)
}

/// Two-frame ramp from the baseline voice onto the moment.
#[derive(Clone, Copy, Debug)]
pub struct MomentVoice {
    baseline: VoiceBaseline,
    identity: galion_emotion_core::BaselineBits,
    applied: VoiceControls,
    goal: VoiceControls,
    origin: VoiceControls,
    step: u32,
}

impl MomentVoice {
    pub fn new(baseline: VoiceBaseline) -> Self {
        let rest = instant(baseline, MomentState::at(baseline.pos), Breath::rest());
        Self {
            identity: baseline.identity_bits(),
            baseline,
            applied: rest,
            goal: rest,
            origin: rest,
            step: AFFECT_TO_VOICE_FRAMES,
        }
    }

    pub fn baseline(&self) -> VoiceBaseline {
        self.baseline
    }

    pub fn identity(&self) -> galion_emotion_core::BaselineBits {
        self.identity
    }

    pub fn applied(&self) -> VoiceControls {
        self.applied
    }

    /// One 80 ms frame. A held target is fully applied on the second call.
    /// The baseline this was built with is not assigned.
    pub fn tick(&mut self, moment: MomentState, breath: Breath) -> VoiceControls {
        debug_assert_eq!(self.baseline.identity_bits(), self.identity);
        if !is_samantha(&self.baseline) {
            let frozen = parked_packet(
                self.baseline.persona,
                self.baseline.voice,
                self.baseline.pitch_centre_hz,
            );
            self.applied = frozen;
            self.goal = frozen;
            self.origin = frozen;
            self.step = AFFECT_TO_VOICE_FRAMES;
            return frozen;
        }
        let goal = map_samantha(&self.baseline, moment, breath);
        if !same_packet(&goal, &self.goal) {
            self.origin = self.applied;
            self.goal = goal;
            self.step = 0;
        }
        self.step = self.step.saturating_add(1).min(AFFECT_TO_VOICE_FRAMES);
        let t = self.step as f32 / AFFECT_TO_VOICE_FRAMES as f32;
        self.applied = stamp_identity(self.baseline, lerp(self.origin, self.goal, t).clamped());
        debug_assert_eq!(self.baseline.identity_bits(), self.identity);
        self.applied
    }
}

/// Tone shaped by `controls`. In memory. Not a voice and not written out.
pub struct Rendered {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    pub duration_ms: u32,
    pub rms: f32,
}

/// 200 ms of a 180 Hz tone at `sample_rate`, then [`shape_sentence`].
pub fn render_tone(controls: VoiceControls, sample_rate: u32) -> Rendered {
    let raw = tone(sample_rate, 200, 180.0, 6_000);
    let samples = shape_sentence(&raw, sample_rate, controls);
    let duration_ms = duration_ms(samples.len(), sample_rate);
    let rms = rms(&samples);
    Rendered {
        samples,
        sample_rate,
        duration_ms,
        rms,
    }
}

/// Length, pitch, energy, pause, and breath on one sentence buffer.
///
/// `length_scale` above 1 lengthens the buffer (Piper: larger length is
/// slower). Pitch is a separate resample, not folded into that scale.
/// `noise_scale` and `noise_w` stay on the control packet: they are spawn
/// arguments, and adding dither here would pretend to be the VITS noise.
pub fn shape_sentence(input: &[i16], sample_rate: u32, controls: VoiceControls) -> Vec<i16> {
    if input.is_empty() || controls.is_neutral_shape() {
        return input.to_vec();
    }
    let gained = apply_gain(input, controls.energy);
    let pitched = scale_duration(&gained, pitch_ratio(controls.pitch_st).recip());
    let paced = scale_duration(&pitched, controls.length_scale);
    place_silence(&paced, sample_rate, controls)
}

fn map_samantha(baseline: &VoiceBaseline, moment: MomentState, breath: Breath) -> VoiceControls {
    let da = axis_delta(moment.pos[AROUSAL], baseline.pos[AROUSAL]);
    let dv = axis_delta(moment.pos[VALENCE], baseline.pos[VALENCE]);
    let dd = axis_delta(moment.pos[DOMINANCE], baseline.pos[DOMINANCE]);
    let dapp = axis_delta(moment.pos[APPROACH], baseline.pos[APPROACH]);
    let pad = Pad {
        arousal: (Pad::RESTING.arousal + da).clamp(0.0, 1.0),
        valence: (Pad::RESTING.valence + dv).clamp(-1.0, 1.0),
        dominance: (Pad::RESTING.dominance + dd).clamp(0.0, 1.0),
    };
    let speaking = map(pad);
    let length = (speaking.length_scale - APPROACH_LENGTH * dapp).clamp(LENGTH_MIN, LENGTH_MAX);
    let energy = (speaking.energy + APPROACH_ENERGY * dapp).clamp(ENERGY_MIN, ENERGY_MAX);
    let (breath_place, breath_ms) = place_breath(breath.phase);
    stamp_identity(
        *baseline,
        VoiceControls {
            persona: baseline.persona,
            voice: baseline.voice,
            pitch_centre_hz: finite_pitch(baseline.pitch_centre_hz),
            length_scale: length,
            noise_scale: speaking.noise_scale,
            noise_w: speaking.noise_w,
            sentence_silence_s: speaking.sentence_silence_s,
            pitch_st: speaking.pitch_shift_st,
            energy,
            pre_pause_ms: speaking.pre_pause_ms,
            rate: (1.0 / length).clamp(WORKER_RATE_MIN, WORKER_RATE_MAX),
            breath: breath_place,
            breath_ms,
            parked: false,
        }
        .clamped(),
    )
}

fn place_breath(phase: BreathPhase) -> (BreathPlace, u32) {
    match phase {
        BreathPhase::Inhale => (BreathPlace::Before, FRAME_MS),
        BreathPhase::Exhale => (BreathPlace::After, FRAME_MS),
        BreathPhase::Hold | BreathPhase::Speech | BreathPhase::Rest => (BreathPlace::None, 0),
    }
}

fn parked_packet(persona: [u8; 16], voice: [u8; 16], pitch_centre_hz: f32) -> VoiceControls {
    let mut packet = VoiceControls::unsteered();
    packet.persona = persona;
    packet.voice = voice;
    packet.pitch_centre_hz = finite_pitch(pitch_centre_hz);
    packet
}

fn stamp_identity(baseline: VoiceBaseline, mut controls: VoiceControls) -> VoiceControls {
    controls.persona = baseline.persona;
    controls.voice = baseline.voice;
    controls.pitch_centre_hz = finite_pitch(baseline.pitch_centre_hz);
    controls
}

fn lerp(origin: VoiceControls, goal: VoiceControls, t: f32) -> VoiceControls {
    if t >= 1.0 {
        return goal;
    }
    if t <= 0.0 {
        return origin;
    }
    VoiceControls {
        persona: goal.persona,
        voice: goal.voice,
        pitch_centre_hz: goal.pitch_centre_hz,
        length_scale: mix(origin.length_scale, goal.length_scale, t),
        noise_scale: mix(origin.noise_scale, goal.noise_scale, t),
        noise_w: mix(origin.noise_w, goal.noise_w, t),
        sentence_silence_s: mix(origin.sentence_silence_s, goal.sentence_silence_s, t),
        pitch_st: mix(origin.pitch_st, goal.pitch_st, t),
        energy: mix(origin.energy, goal.energy, t),
        pre_pause_ms: mix(origin.pre_pause_ms, goal.pre_pause_ms, t),
        rate: mix(origin.rate, goal.rate, t),
        breath: origin.breath,
        breath_ms: origin.breath_ms,
        parked: goal.parked,
    }
}

fn same_packet(a: &VoiceControls, b: &VoiceControls) -> bool {
    a.parked == b.parked
        && a.breath == b.breath
        && a.breath_ms == b.breath_ms
        && a.length_scale.to_bits() == b.length_scale.to_bits()
        && a.noise_scale.to_bits() == b.noise_scale.to_bits()
        && a.noise_w.to_bits() == b.noise_w.to_bits()
        && a.sentence_silence_s.to_bits() == b.sentence_silence_s.to_bits()
        && a.pitch_st.to_bits() == b.pitch_st.to_bits()
        && a.energy.to_bits() == b.energy.to_bits()
        && a.pre_pause_ms.to_bits() == b.pre_pause_ms.to_bits()
        && a.rate.to_bits() == b.rate.to_bits()
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn axis_delta(moment: f32, baseline: f32) -> f32 {
    if moment.is_finite() && baseline.is_finite() {
        moment - baseline
    } else {
        0.0
    }
}

fn finite_pitch(hz: f32) -> f32 {
    if hz.is_finite() && hz > 0.0 {
        hz
    } else {
        VoiceBaseline::standin().pitch_centre_hz
    }
}

fn finite_or(x: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        fallback
    }
}

fn finite_between(x: f32, lo: f32, hi: f32) -> bool {
    x.is_finite() && x >= lo && x <= hi
}

fn id_slot(id: &str) -> [u8; 16] {
    let mut out = [0u8; 16];
    let bytes = id.as_bytes();
    let n = bytes.len().min(15);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

fn pitch_ratio(st: f32) -> f32 {
    let st = if st.is_finite() { st } else { 0.0 };
    (st / 12.0).exp2()
}

fn apply_gain(input: &[i16], energy: f32) -> Vec<i16> {
    let energy = if energy.is_finite() { energy } else { 1.0 };
    if (energy - 1.0).abs() < 1.0e-4 {
        return input.to_vec();
    }
    input
        .iter()
        .map(|sample| to_i16(*sample as f32 * energy))
        .collect()
}

fn scale_duration(input: &[i16], scale: f32) -> Vec<i16> {
    if input.len() < 2 || !scale.is_finite() || (scale - 1.0).abs() < 1.0e-4 {
        return input.to_vec();
    }
    let n = ((input.len() as f32) * scale).round().max(1.0) as usize;
    if n == input.len() {
        return input.to_vec();
    }
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(n);
    let denom = (n - 1).max(1) as f32;
    for i in 0..n {
        let pos = (i as f32) * (last as f32) / denom;
        let idx = pos.floor() as usize;
        let frac = pos - idx as f32;
        let a = input[idx.min(last)] as f32;
        let b = input[(idx + 1).min(last)] as f32;
        out.push(to_i16(a + (b - a) * frac));
    }
    out
}

fn place_silence(body: &[i16], sample_rate: u32, controls: VoiceControls) -> Vec<i16> {
    let lead_ms = controls.pre_pause_ms.round().max(0.0) as u32
        + if controls.breath == BreathPlace::Before {
            controls.breath_ms
        } else {
            0
        };
    let tail_ms = if controls.breath == BreathPlace::After {
        controls.breath_ms
    } else {
        0
    };
    let lead = samples_for_ms(lead_ms, sample_rate);
    let tail = samples_for_ms(tail_ms, sample_rate);
    if lead == 0 && tail == 0 {
        return body.to_vec();
    }
    let mut out = vec![0i16; lead];
    out.extend_from_slice(body);
    out.extend(std::iter::repeat(0i16).take(tail));
    out
}

fn samples_for_ms(ms: u32, sample_rate: u32) -> usize {
    if sample_rate == 0 || ms == 0 {
        0
    } else {
        (u64::from(ms) * u64::from(sample_rate) / 1000) as usize
    }
}

fn tone(sample_rate: u32, ms: u32, hz: f32, amp: i16) -> Vec<i16> {
    if sample_rate == 0 || ms == 0 {
        return Vec::new();
    }
    let n = samples_for_ms(ms, sample_rate);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let phase = (i as f32) * hz / sample_rate as f32;
        let s = (phase * std::f32::consts::TAU).sin();
        out.push(to_i16(s * f32::from(amp)));
    }
    out
}

fn duration_ms(samples: usize, sample_rate: u32) -> u32 {
    if sample_rate == 0 {
        0
    } else {
        ((samples as u64) * 1000 / u64::from(sample_rate)) as u32
    }
}

fn rms(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for sample in samples {
        let x = f64::from(*sample);
        acc += x * x;
    }
    (acc / samples.len() as f64).sqrt() as f32
}

fn to_i16(x: f32) -> i16 {
    let y = if x.is_finite() { x.round() } else { 0.0 };
    if y >= 32767.0 {
        32767
    } else if y <= -32768.0 {
        -32768
    } else {
        y as i16
    }
}
