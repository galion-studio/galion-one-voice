// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mouth. One pull is one conductor frame. The core does not link Piper.

use crate::backchannel::{render, BackchannelKind};
use crate::brain::SpokenLine;
use crate::clock::{FRAME_SAMPLES, SAMPLE_RATE};
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
pub trait Mouth {
    fn id(&self) -> &str;
    fn start(&mut self, line: &SpokenLine);
    fn pull(&mut self) -> MouthPull;
    fn cancel(&mut self);
    fn speaking(&self) -> bool;

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
    tone: i16,
    pending_first: bool,
    spoken: Vec<String>,
    cancels: u32,
    cues: Vec<String>,
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
            tone: 0,
            pending_first: false,
            spoken: Vec::new(),
            cancels: 0,
            cues: Vec::new(),
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
}

impl Mouth for FakeMouth {
    fn id(&self) -> &str {
        "fake"
    }

    fn start(&mut self, line: &SpokenLine) {
        self.spoken.push(line.text.clone());
        if line.text.trim().is_empty() {
            self.left = 0;
            self.pending_first = false;
            return;
        }
        let tone = 200 + (line.text.len() as i16 % 100);
        self.tone = tone;
        self.left = SPEAK_FRAMES;
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        if self.left == 0 {
            return MouthPull::Done;
        }
        self.left -= 1;
        let first = self.pending_first;
        self.pending_first = false;
        let mut samples = vec![0i16; FRAME_SAMPLES];
        for (index, sample) in samples.iter_mut().enumerate() {
            *sample = if index % 2 == 0 {
                self.tone
            } else {
                -self.tone
            };
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
        self.pending_first = false;
    }

    fn speaking(&self) -> bool {
        self.left > 0
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
