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
    /// Final transcript for this utterance. Empty text is an energy turn-end
    /// with no recognizer, and the loop does not invent a line.
    End(String),
    /// The recognizer failed. The loop does not invent a line.
    Fault(String),
    /// Speech crossed the onset on this frame.
    Onset,
    /// User energy while she speaks, still inside the short-backchannel window.
    User,
    /// Third consecutive user frame while she speaks. Barge-in.
    Barge,
    /// A short overlap ended. It does not cancel her turn.
    Backchannel,
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

/// Streaming ear for one frame of PCM.
///
/// [`Ear::note_playback`] is the mouth frame already in the air. The default
/// ignores it. The energy ear uses it to drop her own voice.
pub trait Ear {
    fn id(&self) -> &str;
    fn poll(&mut self, pcm: &[i16]) -> EarEvent;

    fn note_playback(&mut self, pcm: &[i16]) {
        let _ = pcm;
    }
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
            EarEvent::Onset => write!(f, "onset"),
            EarEvent::User => write!(f, "user"),
            EarEvent::Barge => write!(f, "barge"),
            EarEvent::Backchannel => write!(f, "backchannel"),
        }
    }
}
