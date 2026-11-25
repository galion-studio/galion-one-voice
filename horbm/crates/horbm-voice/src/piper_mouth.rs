// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Optional Piper mouth. Compiled only with `--features piper`.
//!
//! The adapter speaks through [`galion_tts_piper::Tts`]. `FakeTts` stays in
//! process. `PiperTts` is the out-of-process CPU fallback and is not
//! constructed here. This module does not select a GPU.

use std::collections::VecDeque;

use galion_tts_piper::{Pad as PiperPad, Tts};

use crate::backchannel::render;
use crate::brain::SpokenLine;
use crate::clock::FRAME_MS;
use crate::mouth::{AudioFrame, BackchannelPlay, Mouth, MouthPull};

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

    fn listen_backchannel(&mut self, play: &BackchannelPlay) -> AudioFrame {
        // The reply queue stays put. A nod is not a turn.
        let pad = PiperPad {
            arousal: 0.3,
            valence: 0.25,
            dominance: 0.5,
        };
        let mut pcm = Vec::new();
        let spoke = self
            .tts
            .speak(play.text, pad, &mut |chunk| {
                pcm.extend_from_slice(chunk.samples);
            })
            .is_ok();
        let gain = if play.energy.is_finite() {
            play.energy.clamp(0.22, 0.55)
        } else {
            0.30
        };
        if !spoke || pcm.is_empty() {
            return AudioFrame {
                samples: render(play.kind, gain, frame_width(self.rate).max(1), self.rate),
                sample_rate: self.rate,
                first: true,
            };
        }
        let samples: Vec<i16> = pcm
            .into_iter()
            .take(frame_width(self.rate).max(1))
            .map(|sample| (f32::from(sample) * gain).round() as i16)
            .collect();
        AudioFrame {
            samples,
            sample_rate: self.rate,
            first: true,
        }
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
    use crate::mouth::{Mouth, MouthPull};
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

    #[test]
    fn a_nod_uses_piper_and_stays_quieter_than_the_reply() {
        use crate::backchannel::{BackchannelKind, ListenCue, ListenMood};
        use crate::mouth::BackchannelPlay;
        use crate::turn::{TickEvent, TurnMsg};

        let mut mouth = PiperMouth::new(FakeTts::new());
        let nod = mouth.listen_backchannel(&BackchannelPlay {
            text: "mm-hm",
            kind: BackchannelKind::MmHm,
            energy: 0.40,
        });
        assert!(!mouth.speaking(), "a nod does not open a turn");
        let nod_peak = peak(&nod.samples);
        assert!(nod_peak > 0);

        let mut reply_mouth = PiperMouth::new(FakeTts::new());
        reply_mouth.start(&crate::brain::SpokenLine {
            text: "The kettle is on the hob.".into(),
            mood: horbm::Pad::RESTING,
            rate: 1.0,
            energy: 1.0,
            pitch_st: 0.0,
            pause_ms: 0.0,
        });
        let MouthPull::Frame(reply) = reply_mouth.pull() else {
            panic!("reply frame");
        };
        let reply_peak = peak(&reply.samples);
        assert!(nod_peak < reply_peak, "nod {nod_peak} reply {reply_peak}");

        let mut voice = VoiceLoop::new(
            FakeEar::script([
                EarEvent::partial("I walked down to the market"),
                EarEvent::quiet(),
                EarEvent::partial("and the stall was still open"),
            ]),
            PiperMouth::new(FakeTts::new()),
            kettle_turn(),
        );
        let _ = voice.tick_listen(&[], TurnMsg::Hold, ListenCue::None, ListenMood::resting());
        let heard = voice.tick_listen(
            &[],
            TurnMsg::Hold,
            ListenCue::ClauseFall,
            ListenMood::resting(),
        );
        assert_eq!(heard.event, TickEvent::ListenBackchannel);
        assert!(!heard.cancel);
        assert!(!heard.speaking);
        assert!(heard.reply.is_none());
        assert!(!voice.mouth().speaking());
        let next = voice.tick_listen(&[], TurnMsg::Hold, ListenCue::None, ListenMood::resting());
        assert_ne!(next.event, TickEvent::BargeIn);
        assert!(!next.cancel);
        assert_eq!(next.epoch, heard.epoch);
    }

    fn peak(samples: &[i16]) -> i16 {
        samples
            .iter()
            .fold(0i16, |acc, sample| acc.max(sample.abs()))
    }
}
