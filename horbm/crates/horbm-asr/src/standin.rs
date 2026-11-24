// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Deterministic stand-in. Closed tests run this. It does not load weights
//! and it does not look at the samples, only at how many frames arrived.

use crate::{
    check_frame, RecogError, SpeechRecognizer, TimedWord, Transcript, TranscriptKind, FRAME_MS,
    FRAME_SAMPLES,
};

/// Scripted recognizer. One whitespace-separated word per accepted frame.
///
/// The same frames always produce the same transcripts. Sample values do
/// not. [`SpeechRecognizer::finish`] speaks only the words those frames
/// revealed. [`SpeechRecognizer::cancel`] drops them and emits nothing.
#[derive(Debug, Clone)]
pub struct StandIn {
    words: Vec<String>,
    shown: usize,
    frames: u32,
}

impl StandIn {
    pub fn new(script: &str) -> Self {
        Self {
            words: script.split_whitespace().map(str::to_string).collect(),
            shown: 0,
            frames: 0,
        }
    }

    fn reset(&mut self) {
        self.shown = 0;
        self.frames = 0;
    }

    fn snapshot(&self, kind: TranscriptKind) -> Transcript {
        let shown = &self.words[..self.shown];
        let words = shown
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let start_ms = (index as u32).saturating_mul(FRAME_MS);
                TimedWord {
                    text: text.clone(),
                    start_ms,
                    end_ms: start_ms.saturating_add(FRAME_MS),
                }
            })
            .collect();
        Transcript {
            kind,
            text: shown.join(" "),
            start_ms: 0,
            end_ms: self.audio_ms(),
            words,
        }
    }
}

impl SpeechRecognizer for StandIn {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Transcript>, RecogError> {
        check_frame(pcm)?;
        self.frames = self.frames.saturating_add(1);
        let want = (self.frames as usize).min(self.words.len());
        if want > self.shown {
            self.shown = want;
            return Ok(vec![self.snapshot(TranscriptKind::Partial)]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Transcript>, RecogError> {
        if self.frames == 0 || self.shown == 0 {
            self.reset();
            return Ok(Vec::new());
        }
        let final_line = self.snapshot(TranscriptKind::Final);
        self.reset();
        Ok(vec![final_line])
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.reset();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        "stand-in"
    }
}

const _: () = assert!(FRAME_SAMPLES == 1_280);
