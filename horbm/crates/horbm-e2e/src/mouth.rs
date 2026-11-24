// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mouth for the smoke. Piper when a voice pack is on disk, otherwise flite
//! when libflite opens, otherwise a tone that is not timed.

use std::collections::VecDeque;
use std::time::Instant;

use galion_ml_voice::speech::flite::FliteTts;
use galion_ml_voice::{LanguageId, TtsEngine};
use galion_tts_piper::{Pad as PiperPad, PiperTts, Tts};
use horbm_voice::{AudioFrame, Mouth, MouthPull, SpokenLine, FRAME_SAMPLES, SAMPLE_RATE};

use crate::buffer::Loopback;
use crate::wav::{f32_to_i16, lift_quiet, resample_i16};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineKind {
    Piper,
    Flite,
    Stub,
}

impl EngineKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Piper => "piper",
            Self::Flite => "flite",
            Self::Stub => "stub",
        }
    }
}

pub fn detect() -> EngineKind {
    if galion_tts_piper::resolve().is_ok() && PiperTts::open_resolved().is_ok() {
        return EngineKind::Piper;
    }
    if open_flite().is_ok() {
        return EngineKind::Flite;
    }
    EngineKind::Stub
}

fn flite_voice() -> String {
    std::env::var("GALION_FLITE_VOICE").unwrap_or_else(|_| "slt".into())
}

fn open_flite() -> Result<FliteTts, String> {
    let voice = flite_voice();
    FliteTts::open(&voice).map_err(|err| one_line(&err))
}

/// `libflite` opened. The ear's generated sentences use this voice.
pub fn flite_ready() -> bool {
    open_flite().is_ok()
}

fn one_line(err: &impl std::fmt::Display) -> String {
    err.to_string().replace(['\n', '\r'], " ")
}

/// 16 kHz mono for the ear. `None` when the engine is the untuned stub.
pub fn speak_16k(kind: EngineKind, text: &str) -> Result<Option<Vec<i16>>, String> {
    match kind {
        EngineKind::Stub => Ok(None),
        EngineKind::Flite => {
            let mut tts = open_flite()?;
            let audio = tts
                .synthesize(text, LanguageId::En)
                .map_err(|err| one_line(&err))?;
            let mut pcm = resample_i16(&f32_to_i16(&audio.samples), audio.sample_rate, SAMPLE_RATE);
            lift_quiet(&mut pcm);
            if pcm.iter().all(|sample| *sample == 0) {
                return Err("flite produced silence".into());
            }
            Ok(Some(pcm))
        }
        EngineKind::Piper => {
            let mut tts = PiperTts::open_resolved().map_err(|err| one_line(&err))?;
            let mut pcm = Vec::new();
            let mut rate = tts.sample_rate();
            tts.speak(text, PiperPad::RESTING, &mut |chunk| {
                rate = chunk.sample_rate;
                pcm.extend_from_slice(chunk.samples);
            })
            .map_err(|err| one_line(&err))?;
            let mut pcm = resample_i16(&pcm, rate, SAMPLE_RATE);
            lift_quiet(&mut pcm);
            if pcm.iter().all(|sample| *sample == 0) {
                return Err("piper produced silence".into());
            }
            Ok(Some(pcm))
        }
    }
}

enum Kind {
    Piper(PiperTts),
    Flite(FliteTts),
    Stub,
}

enum Produced {
    Stub,
    Audio { pcm: Vec<i16>, wall: u128 },
    Failed(String),
}

fn synthesize_flite(tts: &mut FliteTts, text: &str) -> Produced {
    let started = Instant::now();
    match tts.synthesize(text, LanguageId::En) {
        Ok(audio) => {
            let wall = started.elapsed().as_nanos();
            let mut pcm = resample_i16(&f32_to_i16(&audio.samples), audio.sample_rate, SAMPLE_RATE);
            lift_quiet(&mut pcm);
            Produced::Audio { pcm, wall }
        }
        Err(err) => Produced::Failed(one_line(&err)),
    }
}

fn synthesize_piper(tts: &mut PiperTts, text: &str, pad: PiperPad) -> Produced {
    let started = Instant::now();
    let mut pcm = Vec::new();
    let mut rate = tts.sample_rate();
    match tts.speak(text, pad, &mut |chunk| {
        rate = chunk.sample_rate;
        pcm.extend_from_slice(chunk.samples);
    }) {
        Ok(_) => {
            let wall = started.elapsed().as_nanos();
            let mut pcm = resample_i16(&pcm, rate, SAMPLE_RATE);
            lift_quiet(&mut pcm);
            Produced::Audio { pcm, wall }
        }
        Err(err) => Produced::Failed(one_line(&err)),
    }
}

pub struct LiveMouth {
    kind: Kind,
    name: &'static str,
    frames: VecDeque<Vec<i16>>,
    pending_first: bool,
    spoken: Vec<String>,
    buffer: Loopback,
    buffered_at: Option<Instant>,
    synth_wall_ns: u128,
    audio_ns: u128,
    stop_ns: Option<u64>,
    queued_at_barge: usize,
    real: bool,
    error: Option<String>,
}

impl LiveMouth {
    pub fn open(kind: EngineKind) -> Result<Self, String> {
        let (kind, name) = match kind {
            EngineKind::Piper => (
                Kind::Piper(PiperTts::open_resolved().map_err(|err| one_line(&err))?),
                "piper",
            ),
            EngineKind::Flite => (Kind::Flite(open_flite()?), "flite"),
            EngineKind::Stub => (Kind::Stub, "stub"),
        };
        Ok(Self {
            kind,
            name,
            frames: VecDeque::new(),
            pending_first: false,
            spoken: Vec::new(),
            buffer: Loopback::default(),
            buffered_at: None,
            synth_wall_ns: 0,
            audio_ns: 0,
            stop_ns: None,
            queued_at_barge: 0,
            real: false,
            error: None,
        })
    }

    pub fn spoken(&self) -> &[String] {
        &self.spoken
    }

    pub fn buffered_at(&self) -> Option<Instant> {
        self.buffered_at
    }

    pub fn stop_ns(&self) -> Option<u64> {
        self.stop_ns
    }

    pub fn queued_at_barge(&self) -> usize {
        self.queued_at_barge
    }

    pub fn real(&self) -> bool {
        self.real
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Wall time of synthesis divided by the audio duration. Piper's own
    /// definition: above 1 is slower than realtime.
    pub fn rtf(&self) -> Option<f64> {
        if !self.real || self.audio_ns == 0 || self.synth_wall_ns == 0 {
            return None;
        }
        Some(self.synth_wall_ns as f64 / self.audio_ns as f64)
    }

    fn enqueue(&mut self, pcm: &[i16], wall_ns: u128, rate: u32) {
        if pcm.iter().all(|sample| *sample == 0) || rate == 0 {
            self.error = Some("mouth produced silence".into());
            return;
        }
        self.audio_ns = (pcm.len() as u128).saturating_mul(1_000_000_000) / u128::from(rate);
        self.synth_wall_ns = wall_ns;
        self.buffer.push(pcm);
        self.buffered_at = Some(Instant::now());
        self.real = true;
        for chunk in pcm.chunks(FRAME_SAMPLES) {
            let mut frame = vec![0i16; FRAME_SAMPLES];
            frame[..chunk.len()].copy_from_slice(chunk);
            self.frames.push_back(frame);
        }
        self.pending_first = !self.frames.is_empty();
    }

    fn enqueue_stub(&mut self) {
        for _ in 0..24 {
            let mut frame = vec![0i16; FRAME_SAMPLES];
            for (index, sample) in frame.iter_mut().enumerate() {
                *sample = if index % 2 == 0 { 400 } else { -400 };
            }
            self.frames.push_back(frame);
        }
        self.pending_first = true;
        self.real = false;
    }
}

impl Mouth for LiveMouth {
    fn id(&self) -> &str {
        self.name
    }

    fn start(&mut self, line: &SpokenLine) {
        self.spoken.push(line.text.clone());
        self.frames.clear();
        self.pending_first = false;
        self.error = None;
        self.real = false;
        self.buffered_at = None;
        if line.text.trim().is_empty() {
            return;
        }
        let text = line.text.clone();
        let pad = PiperPad {
            arousal: line.mood.arousal,
            valence: line.mood.valence,
            dominance: line.mood.dominance,
        };
        let produced = match &mut self.kind {
            Kind::Stub => Produced::Stub,
            Kind::Flite(tts) => synthesize_flite(tts, &text),
            Kind::Piper(tts) => synthesize_piper(tts, &text, pad),
        };
        match produced {
            Produced::Stub => self.enqueue_stub(),
            Produced::Failed(err) => self.error = Some(err),
            Produced::Audio { pcm, wall } => self.enqueue(&pcm, wall, SAMPLE_RATE),
        }
    }

    fn pull(&mut self) -> MouthPull {
        let Some(samples) = self.frames.pop_front() else {
            return MouthPull::Done;
        };
        let first = self.pending_first;
        self.pending_first = false;
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        self.frames.clear();
        self.pending_first = false;
        if !self.real || self.stop_ns.is_some() {
            return;
        }
        self.queued_at_barge = self.buffer.queued();
        self.stop_ns = self.buffer.barge();
    }

    fn speaking(&self) -> bool {
        !self.frames.is_empty()
    }
}
