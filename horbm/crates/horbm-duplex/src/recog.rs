// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Recognizer port.
//!
//! [`SilentRecognizer`] accepts an ear frame and emits no text. It is the
//! stand-in used when the `vosk` feature is off, so a missing model is not
//! turned into a sentence.
//!
//! `AsrRecognizer` is compiled with `--features vosk`. It wraps
//! `galion_sam_asr::Asr` (`push_pcm`, `finish`, `reset`). The duplex tick
//! calls `finish` when the ear frame is quiet and `cancel` on barge-in.
//! That is the same split as StreamingEar: the energy gate ends the turn,
//! and the engine closes it. This type does not dlopen by itself. Opening
//! `VoskAsr` does, and a missing model is [`crate::RecogError::Engine`].

use crate::ports::{Hypothesis, RecogError, Recognizer};
use crate::{FRAME_MS, FRAME_SAMPLES};

/// No transcript. Loud frames and quiet frames both return an empty list.
#[derive(Clone, Debug, Default)]
pub struct SilentRecognizer {
    frames: u32,
}

impl SilentRecognizer {
    #[must_use]
    pub fn new() -> Self {
        Self { frames: 0 }
    }
}

impl Recognizer for SilentRecognizer {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        check_frame(pcm)?;
        self.frames = self.frames.saturating_add(1);
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        self.frames = 0;
        Ok(Vec::new())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.frames = 0;
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "stand-in"
    }
}

fn check_frame(pcm: &[i16]) -> Result<(), RecogError> {
    if pcm.len() == FRAME_SAMPLES {
        Ok(())
    } else {
        Err(RecogError::Frame {
            got: pcm.len(),
            expected: FRAME_SAMPLES,
        })
    }
}

#[cfg(feature = "vosk")]
mod vosk {
    use galion_sam_asr::{Asr, AsrEvent, AsrKind, VoskAsr, VoskConfig};

    use super::check_frame;
    use crate::ports::{Hypothesis, HypothesisKind, RecogError, Recognizer};
    use crate::FRAME_MS;

    /// One [`Asr`] behind the duplex recognizer port.
    pub struct AsrRecognizer<A> {
        inner: A,
        frames: u32,
    }

    impl<A> AsrRecognizer<A> {
        pub fn new(inner: A) -> Self {
            Self { inner, frames: 0 }
        }
    }

    impl AsrRecognizer<VoskAsr> {
        /// Open the in-tree Vosk engine. A missing model is an error and
        /// is not spoken.
        pub fn open(config: VoskConfig) -> Result<Self, RecogError> {
            VoskAsr::open(config).map(Self::new).map_err(map_err)
        }

        /// `VoskConfig::from_env`. Paths stay in `galion-sam-asr`.
        pub fn from_env() -> Result<Self, RecogError> {
            Self::open(VoskConfig::from_env().map_err(map_err)?)
        }
    }

    impl<A: Asr> Recognizer for AsrRecognizer<A> {
        fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
            check_frame(pcm)?;
            let events = self.inner.push_pcm(pcm).map_err(map_err)?;
            self.frames = self.frames.saturating_add(1);
            let utterance_ms = self.audio_ms();
            let saw_final = events.iter().any(|event| event.kind == AsrKind::Final);
            if saw_final {
                self.frames = 0;
            }
            Ok(map_events(&events, utterance_ms))
        }

        fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
            if self.frames == 0 {
                return Ok(Vec::new());
            }
            let utterance_ms = self.audio_ms();
            let events = self.inner.finish().map_err(map_err)?;
            self.frames = 0;
            Ok(map_events(&events, utterance_ms))
        }

        fn cancel(&mut self) -> Result<(), RecogError> {
            self.inner.reset().map_err(map_err)?;
            self.frames = 0;
            Ok(())
        }

        fn audio_ms(&self) -> u32 {
            self.frames.saturating_mul(FRAME_MS)
        }

        fn name(&self) -> &str {
            self.inner.name()
        }
    }

    fn map_err(err: galion_sam_asr::AsrError) -> RecogError {
        RecogError::Engine(err.message().to_string())
    }

    fn map_events(events: &[AsrEvent], utterance_ms: u32) -> Vec<Hypothesis> {
        let mut out = Vec::new();
        for event in events {
            let text = event.text.trim();
            if text.is_empty() {
                continue;
            }
            let (start_ms, end_ms) = word_span(event, utterance_ms);
            out.push(Hypothesis {
                kind: match event.kind {
                    AsrKind::Partial => HypothesisKind::Partial,
                    AsrKind::Final => HypothesisKind::Final,
                },
                text: text.to_string(),
                start_ms,
                end_ms,
            });
        }
        out
    }

    fn word_span(event: &AsrEvent, utterance_ms: u32) -> (u32, u32) {
        let Some(first) = event.words.first() else {
            let end = utterance_ms.max(event.audio_ms);
            return (0, end);
        };
        let start_ms = secs_to_ms(first.start);
        let mut end_ms = start_ms;
        for word in &event.words {
            end_ms = end_ms.max(secs_to_ms(word.end));
        }
        (start_ms, end_ms.max(start_ms))
    }

    fn secs_to_ms(secs: f32) -> u32 {
        if !secs.is_finite() || secs <= 0.0 {
            return 0;
        }
        let ms = (f64::from(secs) * 1000.0).round();
        if ms >= f64::from(u32::MAX) {
            u32::MAX
        } else if ms <= 0.0 {
            0
        } else {
            ms as u32
        }
    }
}

#[cfg(feature = "vosk")]
pub use vosk::AsrRecognizer;
