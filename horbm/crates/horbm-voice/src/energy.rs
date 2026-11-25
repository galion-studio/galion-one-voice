// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Energy voice activity and the 80 ms endpointer.
//!
//! The gate is the one in `galion_sam_orch::EnergyVad`: floor −60 dBFS, +12 dB
//! margin, −48 dB clamp, 60 ms onset, adaptive floor on unvoiced frames. The
//! same numbers are the energy backend in `galion-sam-vad`. This module does
//! not link that crate, so `ort` stays out of the default tree.
//!
//! [`EnergyEar`] runs the gate on 20 ms slices inside one conductor frame and
//! reports onset, turn-end, barge-in, and a short backchannel. While she is
//! speaking it keeps listening. A mic frame that is her own playback, or a
//! scaled copy of it, is echo and is not a barge-in.

use crate::clock::{FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};
use crate::ear::{Ear, EarEvent};

/// 20 ms capture quantum. Same width as `galion-sam-orch` and the Vosk frame.
pub const CAPTURE_MS: f64 = 20.0;
/// Samples in one capture quantum at 16 kHz.
pub const CAPTURE_SAMPLES: usize = (SAMPLE_RATE as usize * CAPTURE_MS as usize) / 1000;

const ON_DB: f64 = 12.0;
const FLOOR_DB: f64 = -60.0;
const CLAMP_DB: f64 = -48.0;
/// Onset. `EnergyVad::min_voiced_ms`.
const MIN_VOICED_MS: f64 = 60.0;
/// One conductor frame of silence closes a turn, so the offset lands on that frame.
const FRAME_HANG_MS: f64 = FRAME_MS as f64;
/// Consecutive user frames, while she speaks, before barge-in.
/// Frame 3 is 160 ms from the first overlapping frame, inside the 200 ms lock.
/// One or two frames is a short backchannel and does not fire.
const BARGE_FRAMES: u32 = 3;

const _: () = assert!(CAPTURE_SAMPLES == 320);
const _: () = assert!(FRAME_SAMPLES == CAPTURE_SAMPLES * 4);

/// What one capture quantum did. Same three outcomes as `VadEvent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateEvent {
    None,
    Start,
    End,
}

/// Energy gate. `hang_ms` is the silence that closes a turn.
#[derive(Clone, Debug)]
pub struct EnergyGate {
    floor_db: f64,
    speaking: bool,
    silent_ms: f64,
    voiced_ms: f64,
    hang_ms: f64,
}

impl EnergyGate {
    /// `hang_ms` matches `EnergyVad::new`. Onset stays 60 ms.
    pub fn new(hang_ms: f64) -> Self {
        Self {
            floor_db: FLOOR_DB,
            speaking: false,
            silent_ms: 0.0,
            voiced_ms: 0.0,
            hang_ms,
        }
    }

    pub fn speaking(&self) -> bool {
        self.speaking
    }

    pub fn reset_turn(&mut self) {
        self.speaking = false;
        self.voiced_ms = 0.0;
        self.silent_ms = 0.0;
    }

    /// One capture buffer. `frame_ms` is that buffer's duration.
    pub fn push(&mut self, pcm: &[i16], frame_ms: f64) -> GateEvent {
        let voiced = self.observe(pcm);
        if voiced {
            self.voiced_ms += frame_ms;
            self.silent_ms = 0.0;
        } else {
            self.silent_ms += frame_ms;
        }
        if !self.speaking && voiced && self.voiced_ms >= MIN_VOICED_MS {
            self.speaking = true;
            return GateEvent::Start;
        }
        if !voiced && !self.speaking {
            self.voiced_ms = 0.0;
        }
        if self.speaking && self.silent_ms >= self.hang_ms {
            self.speaking = false;
            self.voiced_ms = 0.0;
            return GateEvent::End;
        }
        GateEvent::None
    }

    /// RMS decision and the noise-floor update. Does not move the turn.
    fn observe(&mut self, pcm: &[i16]) -> bool {
        let db = rms_db(pcm);
        let voiced = db > (self.floor_db + ON_DB).max(CLAMP_DB);
        if !voiced {
            self.floor_db = 0.95 * self.floor_db + 0.05 * db.max(-90.0);
        }
        voiced
    }
}

/// RMS in dB. Same formula as `galion_sam_orch::rms_db`.
pub fn rms_db(frame: &[i16]) -> f64 {
    if frame.is_empty() {
        return -120.0;
    }
    let energy = frame
        .iter()
        .map(|&sample| (sample as f64 / 32768.0).powi(2))
        .sum::<f64>()
        / frame.len() as f64;
    10.0 * energy.max(1e-12).log10()
}

/// Default ear. No model. Energy activity, endpointing, duplex.
#[derive(Clone, Debug)]
pub struct EnergyEar {
    gate: EnergyGate,
    reference: Vec<i16>,
    previous: Vec<i16>,
    me_speaking: bool,
    user_run: u32,
    user_open: bool,
}

impl Default for EnergyEar {
    fn default() -> Self {
        Self::new()
    }
}

impl EnergyEar {
    pub fn new() -> Self {
        Self {
            gate: EnergyGate::new(FRAME_HANG_MS),
            reference: Vec::new(),
            previous: Vec::new(),
            me_speaking: false,
            user_run: 0,
            user_open: false,
        }
    }

    /// The user turn is open. Streaming ASR uses this to keep feeding audio.
    pub fn user_turn_open(&self) -> bool {
        self.gate.speaking()
    }

    fn classify_duplex(&mut self, pcm: &[i16]) -> bool {
        let residual = cancel_best(pcm, &self.reference, &self.previous);
        let audio = residual.as_deref().unwrap_or(pcm);
        self.gate.observe(audio)
    }

    fn while_speaking(&mut self, voiced: bool) -> EarEvent {
        if voiced {
            self.user_run = self.user_run.saturating_add(1);
            self.user_open = true;
            if self.user_run >= BARGE_FRAMES {
                EarEvent::Barge
            } else {
                EarEvent::User
            }
        } else if self.user_open {
            let run = self.user_run;
            self.user_open = false;
            self.user_run = 0;
            if run > 0 && run < BARGE_FRAMES {
                EarEvent::Backchannel
            } else {
                EarEvent::Quiet
            }
        } else {
            EarEvent::Quiet
        }
    }

    fn while_idle(&mut self, pcm: &[i16]) -> EarEvent {
        let mut saw_start = false;
        let mut saw_end = false;
        for chunk in capture_chunks(pcm) {
            match self.gate.push(&chunk, CAPTURE_MS) {
                GateEvent::Start => saw_start = true,
                GateEvent::End => saw_end = true,
                GateEvent::None => {}
            }
        }
        if saw_end {
            EarEvent::end("")
        } else if saw_start {
            EarEvent::Onset
        } else {
            EarEvent::Quiet
        }
    }
}

impl Ear for EnergyEar {
    fn id(&self) -> &str {
        "energy"
    }

    fn note_playback(&mut self, pcm: &[i16]) {
        self.previous = std::mem::take(&mut self.reference);
        self.reference = pcm.to_vec();
        self.me_speaking = pcm.iter().any(|sample| *sample != 0);
    }

    fn poll(&mut self, pcm: &[i16]) -> EarEvent {
        if self.me_speaking {
            let voiced = self.classify_duplex(pcm);
            return self.while_speaking(voiced);
        }
        self.user_run = 0;
        self.user_open = false;
        self.while_idle(pcm)
    }
}

fn capture_chunks(pcm: &[i16]) -> Vec<Vec<i16>> {
    if pcm.is_empty() {
        return vec![vec![0; CAPTURE_SAMPLES]];
    }
    let mut out = Vec::new();
    let mut offset = 0;
    while offset < pcm.len() {
        let mut frame = vec![0i16; CAPTURE_SAMPLES];
        let n = (pcm.len() - offset).min(CAPTURE_SAMPLES);
        frame[..n].copy_from_slice(&pcm[offset..offset + n]);
        out.push(frame);
        offset += CAPTURE_SAMPLES;
    }
    out
}

/// Subtract a scaled copy of `reference` when the mic is that signal.
/// `None` means the mic is not her voice.
fn cancel_echo(mic: &[i16], reference: &[i16]) -> Option<Vec<i16>> {
    let n = mic.len().min(reference.len());
    if n == 0 {
        return None;
    }
    let mut dot_mr = 0.0;
    let mut dot_rr = 0.0;
    let mut dot_mm = 0.0;
    for index in 0..n {
        let sample = f64::from(mic[index]);
        let played = f64::from(reference[index]);
        dot_mr += sample * played;
        dot_rr += played * played;
        dot_mm += sample * sample;
    }
    if dot_rr < 1.0 || dot_mm < 1.0 {
        return None;
    }
    let gain = dot_mr / dot_rr;
    let corr = dot_mr / (dot_rr.sqrt() * dot_mm.sqrt());
    if gain < 0.25 || corr < 0.75 {
        return None;
    }
    let mut residual = mic.to_vec();
    for index in 0..n {
        let cleaned = f64::from(mic[index]) - gain * f64::from(reference[index]);
        residual[index] = cleaned.round().clamp(-32768.0, 32767.0) as i16;
    }
    Some(residual)
}

fn cancel_best(mic: &[i16], current: &[i16], previous: &[i16]) -> Option<Vec<i16>> {
    match (cancel_echo(mic, current), cancel_echo(mic, previous)) {
        (Some(current), Some(previous)) => {
            if energy_sum(&current) <= energy_sum(&previous) {
                Some(current)
            } else {
                Some(previous)
            }
        }
        (Some(current), None) => Some(current),
        (None, Some(previous)) => Some(previous),
        (None, None) => None,
    }
}

fn energy_sum(pcm: &[i16]) -> f64 {
    pcm.iter()
        .map(|sample| f64::from(*sample) * f64::from(*sample))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: i16, step: f64) -> Vec<i16> {
        (0..n)
            .map(|index| ((index as f64 * step).sin() * f64::from(amp)) as i16)
            .collect()
    }

    #[test]
    fn gate_starts_at_60_ms_and_ends_after_the_hang() {
        let mut gate = EnergyGate::new(160.0);
        let loud = vec![12_000i16; CAPTURE_SAMPLES];
        let quiet = vec![0i16; CAPTURE_SAMPLES];
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::None);
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::None);
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::Start);
        for _ in 0..7 {
            assert_eq!(gate.push(&quiet, CAPTURE_MS), GateEvent::None);
        }
        assert_eq!(gate.push(&quiet, CAPTURE_MS), GateEvent::End);
    }

    #[test]
    fn silence_never_starts_and_quiet_noise_stays_under_the_clamp() {
        let mut gate = EnergyGate::new(640.0);
        for _ in 0..12 {
            assert_eq!(
                gate.push(&vec![0i16; CAPTURE_SAMPLES], CAPTURE_MS),
                GateEvent::None
            );
        }
        assert!(!gate.speaking());
        let mut ear = EnergyEar::new();
        let noise = tone(FRAME_SAMPLES, 40, 0.4);
        for _ in 0..6 {
            assert_eq!(ear.poll(&noise), EarEvent::Quiet);
        }
    }

    #[test]
    fn socket_hang_is_640_ms_and_does_not_end_early() {
        let mut gate = EnergyGate::new(640.0);
        let loud = vec![8_000i16; CAPTURE_SAMPLES];
        let quiet = vec![0i16; CAPTURE_SAMPLES];
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::None);
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::None);
        assert_eq!(gate.push(&loud, CAPTURE_MS), GateEvent::Start);
        for _ in 0..31 {
            assert_eq!(gate.push(&quiet, CAPTURE_MS), GateEvent::None);
        }
        assert_eq!(gate.push(&quiet, CAPTURE_MS), GateEvent::End);
    }

    #[test]
    fn a_scaled_copy_of_playback_is_not_voice() {
        let played = tone(FRAME_SAMPLES, 8_000, 0.2);
        let echo: Vec<i16> = played.iter().map(|sample| sample / 2).collect();
        let residual = cancel_echo(&echo, &played).expect("echo");
        assert!(rms_db(&residual) < CLAMP_DB);
        let other = tone(FRAME_SAMPLES, 8_000, 0.07);
        assert!(cancel_echo(&other, &played).is_none());
    }
}
