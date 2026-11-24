// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear. One poll per conductor frame. The core does not link Vosk.

use std::collections::VecDeque;
use std::fmt;

/// What the ear heard on this frame. Text is a transcript, not audio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EarEvent {
    Quiet,
    Partial(String),
    /// Final transcript for this utterance.
    End(String),
    /// The recognizer failed. The loop does not invent a line.
    Fault(String),
}

impl EarEvent {
    pub fn partial(text: impl Into<String>) -> Self {
        Self::Partial(text.into())
    }

    pub fn end(text: impl Into<String>) -> Self {
        Self::End(text.into())
    }

    pub fn quiet() -> Self {
        Self::Quiet
    }
}

/// Streaming speech-to-text for one frame of PCM.
pub trait Ear {
    fn id(&self) -> &str;
    fn poll(&mut self, pcm: &[i16]) -> EarEvent;
}

/// Scripted ear for tests. It does not read the samples.
#[derive(Debug)]
pub struct FakeEar {
    script: VecDeque<EarEvent>,
}

impl FakeEar {
    pub fn script(events: impl IntoIterator<Item = EarEvent>) -> Self {
        Self {
            script: events.into_iter().collect(),
        }
    }
}

impl Ear for FakeEar {
    fn id(&self) -> &str {
        "fake"
    }

    fn poll(&mut self, _pcm: &[i16]) -> EarEvent {
        self.script.pop_front().unwrap_or(EarEvent::Quiet)
    }
}

impl fmt::Display for EarEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EarEvent::Quiet => write!(f, "quiet"),
            EarEvent::Partial(text) => write!(f, "partial:{text}"),
            EarEvent::End(text) => write!(f, "end:{text}"),
            EarEvent::Fault(text) => write!(f, "fault:{text}"),
        }
    }
}
