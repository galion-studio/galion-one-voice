// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mouth. One pull is one conductor frame. The core does not link Piper.

use crate::backchannel::{render, BackchannelKind};
use crate::brain::SpokenLine;
use crate::clock::{FRAME_MS, FRAME_SAMPLES, SAMPLE_RATE};
use crate::moment_voice::{BreathPlace, VoiceControls};
use std::fmt;

/// What the mouth plays for one listening nod. Not a brain line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackchannelPlay {
    pub text: &'static str,
    pub kind: BackchannelKind,
    pub energy: f32,
}

/// One frame of mono PCM. Generated in memory. Not a recording.
pub struct AudioFrame {
    pub samples: Vec<i16>,
    pub sample_rate: u32,
    /// First playable frame of this utterance.
    pub first: bool,
}

impl fmt::Debug for AudioFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioFrame")
            .field("samples", &self.samples.len())
            .field("sample_rate", &self.sample_rate)
            .field("first", &self.first)
            .field("audible", &self.samples.iter().any(|sample| *sample != 0))
            .finish()
    }
}

/// What [`Mouth::pull`] returned.
#[derive(Debug)]
pub enum MouthPull {
    Idle,
    Frame(AudioFrame),
    Done,
}

/// Speaks a brain line as frames. `cancel` drops anything not yet pulled.
///
/// [`Mouth::feel`] is the latest moment packet. The default ignores it.
/// A mouth that can shape PCM uses it at sentence start.
pub trait Mouth {
    fn id(&self) -> &str;
    fn start(&mut self, line: &SpokenLine);
    fn pull(&mut self) -> MouthPull;
    fn cancel(&mut self);
    fn speaking(&self) -> bool;

    /// Latest moment packet. The default ignores it.
    fn feel(&mut self, _controls: VoiceControls) {}

    /// One short nod. Does not start a turn and does not clear a reply.
    /// The default is a generated tone. Piper overrides this behind its feature.
    fn listen_backchannel(&mut self, play: &BackchannelPlay) -> AudioFrame {
        AudioFrame {
            samples: render(play.kind, play.energy, FRAME_SAMPLES, SAMPLE_RATE),
            sample_rate: SAMPLE_RATE,
            first: true,
        }
    }
}

const SPEAK_FRAMES: u32 = 8;

/// In-process mouth. A short generated tone, eight frames, no file, no process.
#[derive(Debug)]
pub struct FakeMouth {
    left: u32,
    lead: u32,
    tail: u32,
    tone: i16,
    pending_first: bool,
    spoken: Vec<String>,
    cancels: u32,
    cues: Vec<String>,
    controls: VoiceControls,
}

impl Default for FakeMouth {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeMouth {
    pub fn new() -> Self {
        Self {
            left: 0,
            lead: 0,
            tail: 0,
            tone: 0,
            pending_first: false,
            spoken: Vec::new(),
            cancels: 0,
            cues: Vec::new(),
            controls: VoiceControls::unsteered(),
        }
    }

    pub fn spoken(&self) -> &[String] {
        &self.spoken
    }

    /// Listening nods this mouth has rendered. Not replies.
    pub fn cues(&self) -> &[String] {
        &self.cues
    }

    pub fn cancels(&self) -> u32 {
        self.cancels
    }

    pub fn controls(&self) -> VoiceControls {
        self.controls
    }
}

impl Mouth for FakeMouth {
    fn id(&self) -> &str {
        "fake"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.spoken.push(line.text.clone());
        if line.text.trim().is_empty() {
            self.left = 0;
            self.lead = 0;
            self.tail = 0;
            self.pending_first = false;
            return;
        }
        let tone = 200 + (line.text.len() as i16 % 100);
        self.tone = tone;
        self.pending_first = true;
        if self.controls.parked || self.controls.is_neutral_shape() {
            self.left = SPEAK_FRAMES;
            self.lead = 0;
            self.tail = 0;
            return;
        }
        let paced = ((SPEAK_FRAMES as f32) * self.controls.length_scale)
            .round()
            .clamp(1.0, 16.0);
        let body = paced as u32;
        self.lead = silence_frames(self.controls.pre_pause_ms)
            + if self.controls.breath == BreathPlace::Before {
                silence_frames(self.controls.breath_ms as f32)
            } else {
                0
            };
        self.tail = if self.controls.breath == BreathPlace::After {
            silence_frames(self.controls.breath_ms as f32)
        } else {
            0
        };
        self.left = self.lead.saturating_add(body).saturating_add(self.tail);
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        let silence = self.left <= self.tail || self.lead > 0;
        if self.lead > 0 {
            self.lead -= 1;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let mut samples = vec![0i16; FRAME_SAMPLES];
        if !silence {
            let amp = if self.controls.parked || self.controls.is_neutral_shape() {
                self.tone
            } else {
                gain_i16(self.tone, self.controls.energy)
            };
            for (index, sample) in samples.iter_mut().enumerate() {
                *sample = if index % 2 == 0 { amp } else { -amp };
            }
        }
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: SAMPLE_RATE,
            first,
        })
    }

    fn cancel(&mut self) {
        if self.left > 0 {
            self.cancels = self.cancels.saturating_add(1);
        }
        self.left = 0;
        self.lead = 0;
        self.tail = 0;
        self.pending_first = false;
    }

    fn speaking(&self) -> bool {
        self.left > 0
    }

    fn feel(&mut self, controls: VoiceControls) {
        self.controls = controls;
    }

    fn listen_backchannel(&mut self, play: &BackchannelPlay) -> AudioFrame {
        self.cues.push(play.text.to_string());
        AudioFrame {
            samples: render(play.kind, play.energy, FRAME_SAMPLES, SAMPLE_RATE),
            sample_rate: SAMPLE_RATE,
            first: true,
        }
    }
}

fn silence_frames(ms: f32) -> u32 {
    if !ms.is_finite() || ms <= 0.0 {
        0
    } else {
        (ms / (FRAME_MS as f32)).round() as u32
    }
}

fn gain_i16(sample: i16, energy: f32) -> i16 {
    let energy = if energy.is_finite() { energy } else { 1.0 };
    let y = f32::from(sample) * energy;
    let y = y.round();
    if y >= 32767.0 {
        32767
    } else if y <= -32768.0 {
        -32768
    } else {
        y as i16
    }
}
