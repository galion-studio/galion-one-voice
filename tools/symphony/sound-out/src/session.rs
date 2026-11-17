// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Places backend speech on the lungs timeline, adds bursts, and emits
//! 80 ms frames as soon as every sample in the frame is known.
//!
//! The inhale is known before any speech arrives, so the first frames can
//! leave while the cloner is still producing the phrase. Environment is
//! applied at emit time. It does not need the future.

use crate::backend::{ClonerBackend, SpeakMeta, SAMPLES_PER_FRAME, SAMPLE_RATE};
use crate::contract::MomentFixture;
use crate::device::{detect_with, DeviceChoice};
use crate::environment::{wet_sample, Burst, Environment};
use galion_lungs::config::Config;
use galion_lungs::inventory::Inventory;
use galion_lungs::plan::{self, Plan, Request};
use galion_lungs::synth;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct EnvReport {
    pub room: String,
    pub listener_distance_m: f64,
    pub voice_gain: f64,
    pub noise_db: f64,
    pub noise_rms: f64,
    pub note: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SoundOut {
    pub schema: String,
    pub sample_rate: u32,
    pub samples: Vec<f64>,
    pub frame_count: usize,
    pub device: DeviceChoice,
    pub backend: String,
    pub realtime_on_cpu: bool,
    pub weights_used: bool,
    pub stubbed: Vec<String>,
    pub controls_echo: Option<String>,
    pub breath_events: usize,
    pub burst_count: usize,
    pub environment: EnvReport,
}

struct SpeechSpan {
    start: usize,
    end: usize,
    filled: usize,
}

pub struct Session {
    samples: Vec<f64>,
    speech: Vec<SpeechSpan>,
    cursor: usize,
    emitted: usize,
    frames: Vec<Vec<f64>>,
    env: Environment,
    bursts: Vec<Burst>,
    breath_events: usize,
    sr: f64,
}

impl Session {
    pub fn open(moment: &MomentFixture, bursts: &[Burst]) -> Result<Self, String> {
        let cfg = Config::bundled()?;
        let inventory = Inventory::bundled()?;
        let mut req = Request::constant(
            &moment.text,
            &moment.category,
            moment.arousal,
            moment.energy,
            moment.fatigue,
        );
        if let Some(traj) = &moment.lungs_trajectory {
            req.trajectory = serde_json::from_value(traj.clone())
                .map_err(|e| format!("lungs trajectory: {e}"))?;
        }
        let plan = plan::plan(&req, &cfg, &inventory)?;
        let mut rendered = synth::render_track(&plan, SAMPLE_RATE as f64, None, &cfg)?;
        // The lungs synth Hann-windows a breath near −50 dBFS. Listen-back
        // counts a span only when two 80 ms ticks are unvoiced and louder
        // than −50 dB, and the span must land within 160 ms of the plan.
        // Placement rewrites each inhale as flat noise at that timing, and
        // holds the vowel back so the pitch window does not voice the last tick.
        stamp_inhales(&mut rendered.samples, &plan, SAMPLE_RATE as f64);
        let speech = speech_spans(&plan, SAMPLE_RATE as f64, rendered.samples.len());
        let breath_events = plan
            .segments
            .iter()
            .filter(|s| s.kind == "inhale" || s.kind == "exhale")
            .count();
        Ok(Self {
            samples: rendered.samples,
            speech,
            cursor: 0,
            emitted: 0,
            frames: Vec::new(),
            env: moment.environment,
            bursts: bursts.to_vec(),
            breath_events,
            sr: SAMPLE_RATE as f64,
        })
    }

    /// Samples known from the start of the buffer. Breath is known immediately.
    pub fn known_until(&self) -> usize {
        for span in &self.speech {
            let len = span.end.saturating_sub(span.start);
            if span.filled < len {
                return span.start + span.filled;
            }
        }
        self.samples.len()
    }

    pub fn push_speech(&mut self, incoming: &[f64]) {
        let mut rest = incoming;
        while !rest.is_empty() && self.cursor < self.speech.len() {
            let span = &mut self.speech[self.cursor];
            let len = span.end.saturating_sub(span.start);
            let room = len.saturating_sub(span.filled);
            if room == 0 {
                self.cursor += 1;
                continue;
            }
            let n = room.min(rest.len());
            let at = span.start + span.filled;
            self.samples[at..at + n].copy_from_slice(&rest[..n]);
            span.filled += n;
            rest = &rest[n..];
            if span.filled >= len {
                self.cursor += 1;
            }
        }
        self.drain();
    }

    pub fn drain_frames(&mut self) -> Vec<Vec<f64>> {
        self.drain();
        std::mem::take(&mut self.frames)
    }

    fn drain(&mut self) {
        let known = self.known_until();
        while self.emitted + SAMPLES_PER_FRAME <= known {
            self.emit(self.emitted, self.emitted + SAMPLES_PER_FRAME);
            self.emitted += SAMPLES_PER_FRAME;
        }
    }

    pub fn finish(&mut self) -> Vec<f64> {
        for span in &mut self.speech {
            span.filled = span.end.saturating_sub(span.start);
        }
        self.drain();
        if self.emitted < self.samples.len() {
            self.emit(self.emitted, self.samples.len());
            self.emitted = self.samples.len();
        }
        self.frames.iter().flatten().copied().collect()
    }

    fn emit(&mut self, start: usize, end: usize) {
        let mut frame = Vec::with_capacity(end - start);
        for i in start..end {
            let dry = self.samples.get(i).copied().unwrap_or(0.0);
            frame.push(wet_sample(dry, i, self.env, &self.bursts, self.sr));
        }
        self.frames.push(frame);
    }
}

/// How long the vowel waits after the planned speech start. A pitch window
/// of about 40 ms would otherwise see the sine and call the last inhale tick voiced.
const SPEECH_GUARD_S: f64 = 0.05;
/// Flat inhale level. Above the listen-back −50 dB gate, under the 0.2 test cap.
const INHALE_RMS: f64 = 0.02;

fn speech_spans(plan: &Plan, sr: f64, n: usize) -> Vec<SpeechSpan> {
    let guard = (SPEECH_GUARD_S * sr).round().max(0.0) as usize;
    plan.segments
        .iter()
        .filter(|s| s.kind == "speech")
        .map(|s| {
            let start = (s.start_s * sr).round().max(0.0) as usize;
            let end = (s.end_s * sr).round().max(start as f64) as usize;
            let start = start.saturating_add(guard).min(end);
            SpeechSpan {
                start: start.min(n),
                end: end.min(n),
                filled: 0,
            }
        })
        .filter(|s| s.end > s.start)
        .collect()
}

fn stamp_inhales(samples: &mut [f64], plan: &Plan, sr: f64) {
    let mut rng = Lcg(0xC0FFEE);
    let fade = (0.002 * sr).round().max(1.0) as usize;
    for seg in &plan.segments {
        if seg.kind != "inhale" {
            continue;
        }
        let a = (seg.start_s * sr).round().max(0.0) as usize;
        let b = (seg.end_s * sr).round().max(a as f64) as usize;
        let b = b.min(samples.len());
        if b <= a {
            continue;
        }
        let mut block = (0..b - a).map(|_| rng.next()).collect::<Vec<_>>();
        scale_rms(&mut block, INHALE_RMS);
        let n = block.len();
        let fade_n = fade.min(n / 4);
        for i in 0..fade_n {
            let w = i as f64 / fade_n as f64;
            block[i] *= w;
            block[n - 1 - i] *= w;
        }
        samples[a..b].copy_from_slice(&block);
    }
}

fn scale_rms(x: &mut [f64], target: f64) {
    if x.is_empty() {
        return;
    }
    let e = x.iter().map(|v| v * v).sum::<f64>() / x.len() as f64;
    let rms = e.sqrt();
    if rms < 1e-12 {
        return;
    }
    let g = target / rms;
    for s in x.iter_mut() {
        *s *= g;
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f64 / (1u64 << 24) as f64) * 2.0 - 1.0
    }
}

pub fn render(
    moment: &MomentFixture,
    backend: &mut dyn ClonerBackend,
    bursts: &[Burst],
    device_explicit: Option<&str>,
) -> Result<SoundOut, String> {
    let device = detect_with(device_explicit);
    let mut session = Session::open(moment, bursts)?;
    let caps = backend.capabilities();
    let request = moment.clone_request();
    let meta: SpeakMeta = backend.speak_stream(&request, &mut |chunk| {
        session.push_speech(&chunk.samples);
        Ok(())
    })?;
    let samples = session.finish();
    let mut stubbed = Vec::new();
    stubbed.push(
        "galion-mne-contract is a local stand-in (TODO S1). This crate reads the moment fixture it defines.".into(),
    );
    if caps.stub {
        stubbed.push(caps.note.clone());
    }
    if bursts.is_empty() {
        stubbed.push(
            "no burst samples were passed. The nose layer (S5) owns burst synthesis; this stage only places them.".into(),
        );
    } else {
        stubbed.push(
            "burst samples were placed at their start times. Synthesis of the category stays with S5.".into(),
        );
    }
    if !meta.weights_used {
        stubbed.push(meta.note.clone());
    }
    let env = moment.environment;
    Ok(SoundOut {
        schema: "galion-sound-out/1".into(),
        sample_rate: SAMPLE_RATE,
        frame_count: samples.len().div_ceil(SAMPLES_PER_FRAME),
        samples,
        device,
        backend: meta.backend,
        realtime_on_cpu: meta.realtime_on_cpu,
        weights_used: meta.weights_used,
        stubbed,
        controls_echo: meta.controls_echo,
        breath_events: session.breath_events,
        burst_count: bursts.len(),
        environment: EnvReport {
            room: format!("{:?}", env.room).to_ascii_lowercase(),
            listener_distance_m: env.listener_distance_m,
            voice_gain: env.voice_gain(),
            noise_db: env.noise_db,
            noise_rms: env.noise_rms(),
            note: "Room gain and distance gain are the PR #194 pipeline defaults, not a cited curve. Positive noise_db is a playback bed, not dB SPL.".into(),
        },
    })
}
