// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Glue over `galion-sam-asr`. Compiled only with `--features vosk`.
//!
//! The engine is used as-is: 16 kHz PCM in, partials and finals out.
//! This module does not bind `vosk_gpu_init` and does not load a weight file
//! of its own. A missing model is an error. The error is not spoken.

use galion_sam_asr::{Asr, AsrEvent, AsrKind, VoskAsr, VoskConfig};

use crate::{
    check_frame, RecogError, SpeechRecognizer, TimedWord, Transcript, TranscriptKind, FRAME_MS,
};

/// One [`Asr`] behind the HORBM port. `A` is [`VoskAsr`] or a test double.
pub struct EngineSpeech<A> {
    inner: A,
    frames: u32,
}

impl<A> EngineSpeech<A> {
    pub fn new(inner: A) -> Self {
        Self { inner, frames: 0 }
    }
}

impl<A: Asr> SpeechRecognizer for EngineSpeech<A> {
    fn push_frame(&mut self, pcm: &[i16]) -> Result<Vec<Transcript>, RecogError> {
        check_frame(pcm)?;
        let events = self.inner.push_pcm(pcm).map_err(RecogError::engine)?;
        self.frames = self.frames.saturating_add(1);
        let utterance_ms = self.audio_ms();
        let mut out = Vec::new();
        let mut saw_final = false;
        for event in &events {
            if event.kind == AsrKind::Final {
                saw_final = true;
            }
            if let Some(transcript) = map_event(event, utterance_ms) {
                out.push(transcript);
            }
        }
        if saw_final {
            self.frames = 0;
        }
        Ok(out)
    }

    fn finish(&mut self) -> Result<Vec<Transcript>, RecogError> {
        if self.frames == 0 {
            return Ok(Vec::new());
        }
        let utterance_ms = self.audio_ms();
        let events = self.inner.finish().map_err(RecogError::engine)?;
        self.frames = 0;
        Ok(events
            .iter()
            .filter_map(|event| map_event(event, utterance_ms))
            .collect())
    }

    fn cancel(&mut self) -> Result<(), RecogError> {
        self.inner.reset().map_err(RecogError::engine)?;
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

/// Open the in-tree Vosk engine. CPU threads come from `config`.
/// A missing model returns [`RecogError`] and does not invent a line.
pub fn open_vosk(config: VoskConfig) -> Result<EngineSpeech<VoskAsr>, RecogError> {
    VoskAsr::open(config)
        .map(EngineSpeech::new)
        .map_err(RecogError::engine)
}

/// [`VoskConfig::from_env`]. Paths stay inside `galion-sam-asr`.
pub fn open_vosk_from_env() -> Result<EngineSpeech<VoskAsr>, RecogError> {
    open_vosk(VoskConfig::from_env().map_err(RecogError::engine)?)
}

fn map_event(event: &AsrEvent, utterance_ms: u32) -> Option<Transcript> {
    let text = event.text.trim();
    if text.is_empty() {
        return None;
    }
    let words: Vec<TimedWord> = event
        .words
        .iter()
        .filter_map(|word| {
            let token = word.word.trim();
            if token.is_empty() {
                return None;
            }
            let start_ms = secs_to_ms(word.start);
            let end_ms = secs_to_ms(word.end).max(start_ms);
            Some(TimedWord {
                text: token.to_string(),
                start_ms,
                end_ms,
            })
        })
        .collect();
    let start_ms = words.first().map(|word| word.start_ms).unwrap_or(0);
    let word_end = words.last().map(|word| word.end_ms).unwrap_or(0);
    Some(Transcript {
        kind: match event.kind {
            AsrKind::Partial => TranscriptKind::Partial,
            AsrKind::Final => TranscriptKind::Final,
        },
        text: text.to_string(),
        start_ms,
        end_ms: utterance_ms.max(word_end).max(start_ms),
        words,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use galion_sam_asr::{AsrError, FakeAsr, Word, HEARTBEAT_SAMPLES as EAR_SAMPLES};

    use crate::{within_barge_window, SpeechRecognizer, FRAME_SAMPLES};

    #[test]
    fn ear_frame_matches_the_existing_engine() {
        assert_eq!(FRAME_SAMPLES, EAR_SAMPLES);
    }

    #[test]
    fn seconds_round_to_milliseconds() {
        assert_eq!(secs_to_ms(0.0), 0);
        assert_eq!(secs_to_ms(-1.0), 0);
        assert_eq!(secs_to_ms(f32::NAN), 0);
        assert_eq!(secs_to_ms(0.08), 80);
        assert_eq!(secs_to_ms(0.12), 120);
        assert_eq!(secs_to_ms(0.45), 450);
    }

    #[test]
    fn fake_engine_partials_and_a_passed_through_final() {
        let mut rec = EngineSpeech::new(FakeAsr::new("where is the kettle"));
        let frame = vec![0i16; FRAME_SAMPLES];
        let first = rec.push_frame(&frame).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, TranscriptKind::Partial);
        assert_eq!(first[0].text, "where");
        assert_eq!(first[0].start_ms, 0);
        assert_eq!(first[0].end_ms, 80);
        assert_eq!(rec.audio_ms(), 80);
        assert_eq!(rec.name(), "fake");
        for _ in 0..3 {
            rec.push_frame(&frame).unwrap();
        }
        let done = rec.finish().unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].kind, TranscriptKind::Final);
        assert_eq!(done[0].text, "where is the kettle");
        assert_eq!(done[0].end_ms, 320);
        assert_eq!(rec.audio_ms(), 0);
        assert!(rec.finish().unwrap().is_empty());
    }

    #[test]
    fn cancel_drops_the_open_hypothesis() {
        let mut rec = EngineSpeech::new(FakeAsr::new("where is the kettle"));
        let frame = vec![0i16; FRAME_SAMPLES];
        rec.push_frame(&frame).unwrap();
        rec.push_frame(&frame).unwrap();
        assert!(within_barge_window(rec.audio_ms()));
        rec.cancel().unwrap();
        assert_eq!(rec.audio_ms(), 0);
        assert!(rec.finish().unwrap().is_empty());
        let again = rec.push_frame(&frame).unwrap();
        assert_eq!(again[0].text, "where");
        assert_eq!(again[0].end_ms, 80);
        assert_eq!(again[0].kind, TranscriptKind::Partial);
    }

    #[test]
    fn an_empty_final_resets_the_clock_and_is_not_a_line() {
        let mut rec = EngineSpeech::new(EmptyFinal);
        let out = rec.push_frame(&vec![0i16; FRAME_SAMPLES]).unwrap();
        assert!(out.is_empty());
        assert_eq!(rec.audio_ms(), 0);
    }

    #[test]
    fn a_short_frame_does_not_reach_the_engine() {
        let mut rec = EngineSpeech::new(PanicOnPush);
        let err = rec.push_frame(&[0i16; 320]).unwrap_err();
        assert_eq!(
            err,
            RecogError::Frame {
                got: 320,
                expected: FRAME_SAMPLES
            }
        );
        assert_eq!(rec.audio_ms(), 0);
    }

    #[test]
    fn engine_word_times_are_kept() {
        let mut rec = EngineSpeech::new(WordsOnce::default());
        let done = rec.push_frame(&vec![0i16; FRAME_SAMPLES]).unwrap();
        assert_eq!(done.len(), 1);
        assert_eq!(done[0].kind, TranscriptKind::Final);
        assert_eq!(done[0].text, "hello");
        assert_eq!(done[0].words.len(), 1);
        assert_eq!(done[0].words[0].text, "hello");
        assert_eq!(done[0].words[0].start_ms, 120);
        assert_eq!(done[0].words[0].end_ms, 450);
        assert_eq!(done[0].start_ms, 120);
        assert_eq!(done[0].end_ms, 450);
        assert_eq!(rec.audio_ms(), 0);
    }

    struct EmptyFinal;

    impl Asr for EmptyFinal {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(vec![AsrEvent {
                kind: AsrKind::Final,
                text: "   ".into(),
                audio_ms: 80,
                words: Vec::new(),
                why: "endpoint",
            }])
        }
        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }
        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "empty"
        }
    }

    struct PanicOnPush;

    impl Asr for PanicOnPush {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            panic!("engine saw a frame the port should have rejected");
        }
        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }
        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "panic"
        }
    }

    #[derive(Default)]
    struct WordsOnce {
        done: bool,
    }

    impl Asr for WordsOnce {
        fn push_pcm(&mut self, _pcm: &[i16]) -> Result<Vec<AsrEvent>, AsrError> {
            if self.done {
                return Ok(Vec::new());
            }
            self.done = true;
            Ok(vec![AsrEvent {
                kind: AsrKind::Final,
                text: "hello".into(),
                audio_ms: 80,
                words: vec![Word {
                    word: "hello".into(),
                    start: 0.12,
                    end: 0.45,
                    conf: 0.9,
                }],
                why: "endpoint",
            }])
        }
        fn finish(&mut self) -> Result<Vec<AsrEvent>, AsrError> {
            Ok(Vec::new())
        }
        fn reset(&mut self) -> Result<(), AsrError> {
            Ok(())
        }
        fn name(&self) -> &str {
            "words"
        }
    }
}
