// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! In-process stages. Every clock is a wall time around real work: a scan,
//! a string copy, one sine frame, a sink copy. There is no sleep standing
//! in for a model. `product_claim` stays false.

use crate::clock::{
    advance_toward_rest, input_samples_for_one_frame, FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE_HZ,
};
use crate::pipeline::{measure, MeasureIn};
use crate::report::Report;
use crate::traits::{
    AsrStage, AudioOut, Pad, PcmChunk, ReplyEngine, SpeechEnd, StageError, TtsStage, TurnOut,
    HEARD, PHRASE,
};
use crate::HarnessError;
use std::time::{Duration, Instant};

/// Energy endpoint on 80 ms frames. Speech is RMS at or above `threshold`.
/// The end sample is the end of the last full frame that was speech.
pub struct EnergyVad {
    pub threshold: f32,
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self { threshold: 500.0 }
    }
}

impl SpeechEnd for EnergyVad {
    fn name(&self) -> &str {
        "energy-rms-80ms"
    }

    fn end_sample(&mut self, pcm: &[i16], sample_rate: u32) -> Result<usize, StageError> {
        if pcm.is_empty() || sample_rate == 0 {
            return Err(StageError::new("vad", "empty audio"));
        }
        let frame = (sample_rate as usize) * (FRAME_MS as usize) / 1000;
        if frame == 0 {
            return Err(StageError::new("vad", "frame length is 0"));
        }
        let mut last = None;
        for (i, chunk) in pcm.chunks(frame).enumerate() {
            if chunk.len() < frame {
                break;
            }
            if rms(chunk) >= self.threshold {
                last = Some((i + 1) * frame);
            }
        }
        last.ok_or_else(|| StageError::new("vad", "no speech frame"))
    }
}

fn rms(chunk: &[i16]) -> f32 {
    let mut acc = 0.0f64;
    for s in chunk {
        let v = *s as f64;
        acc += v * v;
    }
    (acc / chunk.len() as f64).sqrt() as f32
}

/// Scans every sample, then returns [`HEARD`]. The scan is the work.
/// Silence is an error, so an empty result is not timed as a transcript.
pub struct FakeAsr;

impl AsrStage for FakeAsr {
    fn name(&self) -> &str {
        "fake-rms-scan"
    }

    fn finalize(&mut self, pcm: &[i16], _sample_rate: u32) -> Result<String, StageError> {
        if pcm.is_empty() {
            return Err(StageError::new("asr", "empty audio"));
        }
        let mut acc = 0.0f64;
        for s in pcm {
            let v = *s as f64;
            acc += v * v;
        }
        let mean = acc / pcm.len() as f64;
        if mean < 1.0 {
            return Err(StageError::new("asr", "silence"));
        }
        let _ = mean.sqrt();
        Ok(HEARD.to_string())
    }
}

/// HORBM-shaped stand-in. `on_heartbeat` steps mood toward rest once per
/// 80 ms of elapsed time. `turn` returns [`PHRASE`] plus that mood.
/// The sentence does not change with mood.
pub struct FakeReply {
    pad: Pad,
    ticks: u32,
}

impl FakeReply {
    pub fn resting() -> Self {
        Self {
            pad: Pad::RESTING,
            ticks: 0,
        }
    }

    pub fn at(pad: Pad) -> Self {
        Self { pad, ticks: 0 }
    }

    pub fn ticks(&self) -> u32 {
        self.ticks
    }
}

impl ReplyEngine for FakeReply {
    fn name(&self) -> &str {
        "fake-horbm-slot"
    }

    fn on_heartbeat(&mut self, elapsed_us: u64) {
        let tick_us = u64::from(FRAME_MS) * 1000;
        let due = (elapsed_us / tick_us) as u32;
        while self.ticks < due {
            self.pad = advance_toward_rest(self.pad);
            self.ticks += 1;
        }
    }

    fn turn(&mut self, utterance: &str) -> Result<TurnOut, StageError> {
        if utterance.trim().is_empty() {
            return Err(StageError::new("reply", "empty utterance"));
        }
        let t0 = Instant::now();
        let _chars = utterance.chars().count();
        let took = u32::try_from(t0.elapsed().as_micros()).unwrap_or(u32::MAX);
        Ok(TurnOut {
            reply: PHRASE.to_string(),
            intent: "companion".into(),
            arm: "none".into(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: Vec::new(),
            violations: 0,
            lookup_us: 0,
            took_us: took,
            mood: self.pad,
        })
    }
}

/// One 80 ms sine frame at 24 kHz. The stage time is the render, not the
/// 80 ms of audio the frame represents.
pub struct FakeTts {
    pending: Option<PcmChunk>,
}

impl FakeTts {
    pub fn new() -> Self {
        Self { pending: None }
    }

    fn render(&self) -> Vec<i16> {
        let n = input_samples_for_one_frame(SAMPLE_RATE_HZ);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let t = i as f32 / SAMPLE_RATE_HZ as f32;
            let s = (t * 220.0 * std::f32::consts::TAU).sin();
            out.push((s * 6000.0) as i16);
        }
        out
    }
}

impl Default for FakeTts {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsStage for FakeTts {
    fn name(&self) -> &str {
        "fake-sine-frame"
    }

    fn begin(&mut self, text: &str, _mood: &Pad) -> Result<(), StageError> {
        if text.trim().is_empty() {
            return Err(StageError::new("tts", "empty text"));
        }
        let samples = self.render();
        self.pending = Some(PcmChunk {
            at: Instant::now(),
            samples,
            sample_rate: SAMPLE_RATE_HZ,
        });
        Ok(())
    }

    fn pull(&mut self, _timeout: Duration) -> Result<Option<PcmChunk>, StageError> {
        Ok(self.pending.take())
    }

    fn finish(&mut self) -> Result<(), StageError> {
        self.pending = None;
        Ok(())
    }
}

/// In-memory sink. Keeps a peak, not the samples, so a report can show the
/// frame had energy without storing audio.
#[derive(Clone, Debug, Default)]
pub struct FrameSink {
    pub frames: u32,
    pub peak_max: f32,
}

impl AudioOut for FrameSink {
    fn name(&self) -> &str {
        "heartbeat-frame-sink"
    }

    fn emit_frame(&mut self, frame: &[f32]) -> Result<(), StageError> {
        if frame.len() != FRAME_SAMPLES {
            return Err(StageError::new(
                "audio_out",
                format!("frame length {} != {FRAME_SAMPLES}", frame.len()),
            ));
        }
        if frame.iter().any(|s| !s.is_finite()) {
            return Err(StageError::new("audio_out", "non-finite sample"));
        }
        let peak = frame.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        self.frames = self.frames.saturating_add(1);
        self.peak_max = self.peak_max.max(peak);
        Ok(())
    }
}

/// 320 ms of a 220 Hz tone, then 160 ms of silence. Not a voice.
pub fn fixture_pcm(sample_rate: u32) -> Vec<i16> {
    let speech = (sample_rate as usize) * 320 / 1000;
    let silence = (sample_rate as usize) * 160 / 1000;
    let mut pcm = Vec::with_capacity(speech + silence);
    for n in 0..speech {
        let t = n as f32 / sample_rate as f32;
        let s = (t * 220.0 * std::f32::consts::TAU).sin();
        pcm.push((s * 8000.0) as i16);
    }
    pcm.extend(std::iter::repeat(0).take(silence));
    pcm
}

pub fn run_fake(reps: usize, warmup: usize) -> Result<Report, HarnessError> {
    let pcm = fixture_pcm(16_000);
    let mut vad = EnergyVad::default();
    let mut asr = FakeAsr;
    let mut reply = FakeReply::resting();
    let mut tts = FakeTts::new();
    let mut sink = FrameSink::default();
    let mut notes = Vec::new();
    notes.push(
        "Fake backend. VAD, ASR, reply, and TTS are in-process and labeled fake. \
No sleep stands in for a model. product_claim is false. The sine is not a voice. \
The reply-to-first-slice clock is the first speakable piece. This phrase is one sentence, so first_slice_grain is sentence. It is not a Piper time."
            .to_string(),
    );
    measure(
        &mut vad,
        &mut asr,
        &mut reply,
        &mut tts,
        &mut sink,
        MeasureIn {
            pcm: &pcm,
            pcm_rate: 16_000,
            reps,
            warmup,
            backend: "fake",
            claims_tts_measured: false,
            voice: None,
            cold: None,
            piper: None,
            notes,
        },
    )
}
