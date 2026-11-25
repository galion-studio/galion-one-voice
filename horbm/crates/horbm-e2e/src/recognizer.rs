// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear text for one duplex pass.
//!
//! The scripted list is the stand-in and is not a recognition result. The
//! recognised list is the Vosk final for audio generated in this process.
//! That final is what the brain reads. The script is not substituted back in.

use std::collections::VecDeque;

use horbm_duplex::{Hypothesis, HypothesisKind, RecogError, Recognizer, FRAME_MS, FRAME_SAMPLES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Script,
    Recognised,
}

pub struct EarRec {
    script: VecDeque<Vec<String>>,
    words: Vec<String>,
    shown: usize,
    frames: u32,
    open: bool,
    source: Source,
}

impl EarRec {
    pub fn scripted(lines: &[&str]) -> Self {
        Self::from_lines(lines, Source::Script)
    }

    /// `lines` are recognizer finals (and the tone's backchannel cue).
    pub fn recognised(lines: &[&str]) -> Self {
        Self::from_lines(lines, Source::Recognised)
    }

    pub fn is_script(&self) -> bool {
        self.source == Source::Script
    }

    fn from_lines(lines: &[&str], source: Source) -> Self {
        Self {
            script: lines
                .iter()
                .map(|line| line.split_whitespace().map(str::to_string).collect())
                .collect(),
            words: Vec::new(),
            shown: 0,
            frames: 0,
            open: false,
            source,
        }
    }

    fn hypothesis(&self, kind: HypothesisKind) -> Hypothesis {
        Hypothesis {
            kind,
            text: self.words[..self.shown].join(" "),
            start_ms: 0,
            end_ms: self.frames.saturating_mul(FRAME_MS),
        }
    }
}

impl Recognizer for EarRec {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Hypothesis>, RecogError> {
        if pcm.len() != FRAME_SAMPLES {
            return Err(RecogError::Frame {
                got: pcm.len(),
                expected: FRAME_SAMPLES,
            });
        }
        self.frames = self.frames.saturating_add(1);
        if !self.open {
            self.words = self.script.pop_front().unwrap_or_default();
            self.shown = 0;
            self.open = true;
        }
        if self.source == Source::Recognised {
            if self.words.is_empty() {
                return Ok(Vec::new());
            }
            self.shown = self.words.len();
            return Ok(vec![self.hypothesis(HypothesisKind::Partial)]);
        }
        if self.shown < self.words.len() {
            self.shown += 1;
            return Ok(vec![self.hypothesis(HypothesisKind::Partial)]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<Hypothesis>, RecogError> {
        if !self.open || self.shown == 0 {
            self.open = false;
            self.shown = 0;
            return Ok(Vec::new());
        }
        let done = self.hypothesis(HypothesisKind::Final);
        self.open = false;
        self.shown = 0;
        Ok(vec![done])
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.open = false;
        self.shown = 0;
        self.words.clear();
        Ok(())
    }

    fn audio_ms(&self) -> u32 {
        self.frames.saturating_mul(FRAME_MS)
    }

    fn name(&self) -> &str {
        match self.source {
            Source::Recognised => "vosk",
            Source::Script => "stub",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Vec<i16> {
        vec![1000; FRAME_SAMPLES]
    }

    #[test]
    fn recognised_text_is_the_whole_line_and_not_a_script() {
        let mut ear = EarRec::recognised(&["the lamp is on"]);
        let first = ear.push_frame(&frame()).unwrap();
        assert_eq!(first[0].text, "the lamp is on");
        let second = ear.push_frame(&frame()).unwrap();
        assert_eq!(second[0].text, "the lamp is on");
        assert!(!ear.is_script());
        assert_eq!(ear.name(), "vosk");
        let done = ear.finish().unwrap();
        assert_eq!(done[0].text, "the lamp is on");
        assert_eq!(done[0].kind, HypothesisKind::Final);
    }

    #[test]
    fn scripted_text_grows_one_word_at_a_time() {
        let mut ear = EarRec::scripted(&["where is the kettle"]);
        let first = ear.push_frame(&frame()).unwrap();
        assert_eq!(first[0].text, "where");
        assert!(ear.is_script());
        assert_eq!(ear.name(), "stub");
        let _ = ear.push_frame(&frame()).unwrap();
        let third = ear.push_frame(&frame()).unwrap();
        assert_eq!(third[0].text, "where is the");
    }
}
