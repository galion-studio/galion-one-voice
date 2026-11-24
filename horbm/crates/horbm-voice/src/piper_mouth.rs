// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Optional Piper mouth. Compiled only with `--features piper`.
//!
//! The adapter speaks through [`galion_tts_piper::Tts`]. `FakeTts` stays in
//! process. `PiperTts` is the out-of-process CPU fallback and is not
//! constructed here. This module does not select a GPU.

use std::collections::VecDeque;

use galion_tts_piper::{Pad as PiperPad, Tts};

use crate::brain::SpokenLine;
use crate::clock::FRAME_MS;
use crate::mouth::{AudioFrame, Mouth, MouthPull};

/// One [`Tts`] sliced into conductor frames.
pub struct PiperMouth<T> {
    tts: T,
    queue: VecDeque<Vec<i16>>,
    rate: u32,
    pending_first: bool,
    error: Option<String>,
}

impl<T: Tts> PiperMouth<T> {
    pub fn new(tts: T) -> Self {
        let rate = tts.sample_rate();
        Self {
            tts,
            queue: VecDeque::new(),
            rate,
            pending_first: false,
            error: None,
        }
    }

    pub fn last_error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

impl<T: Tts> Mouth for PiperMouth<T> {
    fn id(&self) -> &str {
        self.tts.id()
    }

    fn start(&mut self, line: &SpokenLine) {
        self.queue.clear();
        self.pending_first = false;
        self.error = None;
        self.rate = self.tts.sample_rate();
        if line.text.trim().is_empty() {
            return;
        }
        let pad = PiperPad {
            arousal: line.mood.arousal,
            valence: line.mood.valence,
            dominance: line.mood.dominance,
        };
        let mut pcm = Vec::new();
        if let Err(err) = self.tts.speak(&line.text, pad, &mut |chunk| {
            pcm.extend_from_slice(chunk.samples);
        }) {
            self.error = Some(err.to_string());
            return;
        }
        let width = frame_width(self.rate);
        if pcm.is_empty() || width == 0 {
            return;
        }
        let mut at = 0;
        while at < pcm.len() {
            let end = (at + width).min(pcm.len());
            self.queue.push_back(pcm[at..end].to_vec());
            at = end;
        }
        self.pending_first = true;
    }

    fn pull(&mut self) -> MouthPull {
        let Some(samples) = self.queue.pop_front() else {
            return MouthPull::Done;
        };
        let first = self.pending_first;
        self.pending_first = false;
        MouthPull::Frame(AudioFrame {
            samples,
            sample_rate: self.rate,
            first,
        })
    }

    fn cancel(&mut self) {
        self.queue.clear();
        self.pending_first = false;
    }

    fn speaking(&self) -> bool {
        !self.queue.is_empty()
    }
}

fn frame_width(rate: u32) -> usize {
    (usize::try_from(rate).unwrap_or(0) * usize::try_from(FRAME_MS).unwrap_or(0)) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brain::kettle_turn;
    use crate::ear::{EarEvent, FakeEar};
    use crate::mouth::Mouth;
    use crate::turn::VoiceLoop;
    use galion_tts_piper::FakeTts;

    #[test]
    fn fake_tts_speaks_the_brain_line_without_a_process() {
        let mut voice = VoiceLoop::new(
            FakeEar::script([EarEvent::end("where is the kettle"), EarEvent::quiet()]),
            PiperMouth::new(FakeTts::new()),
            kettle_turn(),
        );
        let eot = voice.tick(&[]);
        assert_eq!(eot.reply.as_deref(), Some("The kettle is on the hob."));
        assert!(voice.mouth().last_error().is_none());
        assert_eq!(voice.mouth().id(), "fake");
        let audio = voice.tick(&[]);
        let frame = audio.audio.expect("first frame");
        assert!(frame.first);
        assert!(!frame.samples.is_empty());
        assert!(frame.samples.iter().any(|sample| *sample != 0));
    }
}
