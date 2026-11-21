// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use crate::engine::Asr;
use crate::error::AsrError;
use crate::event::{samples_to_ms, AsrEvent, AsrKind, HEARTBEAT_SAMPLES};

/// Test double. It reveals a script you already gave it as audio arrives.
/// It does not look at the samples and it is not a measurement of recognition.
pub struct FakeAsr {
    words: Vec<String>,
    samples_per_word: usize,
    fed: usize,
    shown: usize,
}

impl FakeAsr {
    /// One scripted word per 80 ms heartbeat.
    pub fn new(transcript: &str) -> Self {
        Self::with_pace(transcript, HEARTBEAT_SAMPLES)
    }

    pub fn with_pace(transcript: &str, samples_per_word: usize) -> Self {
        let words = if transcript.trim().is_empty() {
            Vec::new()
        } else {
            transcript.split_whitespace().map(str::to_string).collect()
        };
        Self {
            words,
            samples_per_word: samples_per_word.max(1),
            fed: 0,
            shown: 0,
        }
    }

    fn audio_ms(&self) -> u32 {
        samples_to_ms(self.fed)
    }

    fn partial(&self) -> AsrEvent {
        AsrEvent {
            kind: AsrKind::Partial,
            text: self.words[..self.shown].join(" "),
            audio_ms: self.audio_ms(),
            words: Vec::new(),
            why: "partial",
        }
    }
}

impl Asr for FakeAsr {
    fn push_pcm(&mut self, pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
        if pcm.is_empty() {
            return Ok(Vec::new());
        }
        self.fed = self.fed.saturating_add(pcm.len());
        let want = (self.fed / self.samples_per_word).min(self.words.len());
        if want > self.shown {
            self.shown = want;
            return Ok(vec![self.partial()]);
        }
        Ok(Vec::new())
    }

    fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
        if self.fed == 0 {
            return Ok(Vec::new());
        }
        let text = self.words.join(" ");
        let event = AsrEvent {
            kind: AsrKind::Final,
            text,
            audio_ms: self.audio_ms(),
            words: Vec::new(),
            why: "finish",
        };
        self.fed = 0;
        self.shown = 0;
        Ok(vec![event])
    }

    fn reset(&mut self) -> Result<(), AsrError> {
        self.fed = 0;
        self.shown = 0;
        Ok(())
    }

    fn name(&self) -> &str {
        "fake"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::AsrKind;

    #[test]
    fn partials_grow_on_the_heartbeat_and_finish_is_the_script() {
        let mut asr = FakeAsr::new("one two");
        assert!(asr.push_pcm(&[]).unwrap().is_empty());
        assert!(asr.finish().unwrap().is_empty());
        let first = asr.push_pcm(&vec![0; HEARTBEAT_SAMPLES]).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, AsrKind::Partial);
        assert_eq!(first[0].text, "one");
        assert_eq!(first[0].audio_ms, 80);
        assert!(asr
            .push_pcm(&vec![0; HEARTBEAT_SAMPLES - 1])
            .unwrap()
            .is_empty());
        let second = asr.push_pcm(&[0]).unwrap();
        assert_eq!(second[0].text, "one two");
        let done = asr.finish().unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].kind, AsrKind::Final);
        assert_eq!(done[0].text, "one two");
        assert!(asr.finish().unwrap().is_empty());
    }
}
