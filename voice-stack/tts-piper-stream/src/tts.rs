// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Mouth contract shared with the Samantha clone backend (S06).
//!
//! S06 implements [`Tts`] for the clone. This crate implements it for Piper
//! and for [`FakeTts`]. Emotion is a live [`Pad`], not a line chosen for a mood.

use crate::clause::{prepare_text, split_clauses};
use crate::mood::{shape_pcm, Prosody};
use std::fmt;
use std::time::Instant;

/// Live mood. Same fields as horbm `Pad`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pad {
    pub arousal: f32,
    pub valence: f32,
    pub dominance: f32,
}

impl Pad {
    /// horbm resting point.
    pub const RESTING: Pad = Pad {
        arousal: 0.3,
        valence: 0.25,
        dominance: 0.5,
    };
}

/// One clause of mono little-endian s16, valid only for the callback.
/// Copy the samples if you keep them. Return quickly: a blocking callback
/// delays the next clause's handoff. The child keeps rendering either way
/// only as far as the OS pipe allows; enqueue, do not play to the end here.
pub struct PcmChunk<'a> {
    pub sentence_index: u32,
    pub sample_rate: u32,
    pub samples: &'a [i16],
    /// First playable audio of this `speak` call.
    pub first_of_utterance: bool,
}

/// One clause inside a [`SpeakReport`].
#[derive(Clone, Debug, PartialEq)]
pub struct SentenceStat {
    pub index: u32,
    pub chars: u32,
    /// JSON line written -> first PCM byte of this clause.
    pub submit_to_first_byte_ms: u32,
    /// JSON line written -> `Real-time factor` on stderr.
    pub submit_to_done_ms: u32,
    pub samples: u32,
    /// Piper's own `Real-time factor` (infer time / audio time), when the line parsed.
    pub piper_rtf: Option<f32>,
    pub piper_infer_s: Option<f32>,
    pub piper_audio_s: Option<f32>,
}

/// Clock marks for one `speak`, from the start of that call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamEvent {
    /// Clause `sentence` was written to the warm process.
    Submit { sentence: u32, ms: u32 },
    /// `on_pcm` is about to run for `sentence`.
    Callback { sentence: u32, ms: u32 },
}

/// What one [`Tts::speak`] did. Times are measured, not targets.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeakReport {
    pub backend: String,
    pub sentences: u32,
    pub sample_rate: u32,
    pub samples: u32,
    /// First successful speak on this process. Later calls are warm.
    pub cold: bool,
    pub pid: Option<u32>,
    /// Submit of clause 0 -> first PCM byte. Does not include a caller's playback.
    pub first_pcm_byte_ms: u32,
    /// Submit of clause 0 -> first `on_pcm`. Includes drain of that clause.
    pub first_callback_ms: u32,
    pub wall_ms: u32,
    /// Shaped PCM duration at [`Self::sample_rate`].
    pub audio_ms: u32,
    /// `wall_ms / audio_ms`. Above 1 is slower than realtime.
    pub wall_rtf: Option<f32>,
    /// Spawn of the process -> first PCM byte ever. Set on the cold speak.
    pub cold_spawn_to_first_pcm_ms: Option<u32>,
    pub events: Vec<StreamEvent>,
    pub detail: Vec<SentenceStat>,
}

impl SpeakReport {
    pub(crate) fn from_parts(
        backend: &str,
        sample_rate: u32,
        cold: bool,
        pid: Option<u32>,
        cold_spawn_to_first_pcm_ms: Option<u32>,
        samples: usize,
        wall_ms: u32,
        events: Vec<StreamEvent>,
        detail: Vec<SentenceStat>,
    ) -> Self {
        let audio_ms = if sample_rate == 0 {
            0
        } else {
            ((samples as u64 * 1000) / u64::from(sample_rate)) as u32
        };
        let wall_rtf = if audio_ms == 0 {
            None
        } else {
            Some(wall_ms as f32 / audio_ms as f32)
        };
        let first_pcm_byte_ms = detail
            .first()
            .map(|d| d.submit_to_first_byte_ms)
            .unwrap_or(0);
        let first_callback_ms = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::Callback { sentence: 0, ms } => Some(*ms),
                _ => None,
            })
            .unwrap_or(wall_ms);
        Self {
            backend: backend.to_string(),
            sentences: detail.len() as u32,
            sample_rate,
            samples: samples as u32,
            cold,
            pid,
            first_pcm_byte_ms,
            first_callback_ms,
            wall_ms,
            audio_ms,
            wall_rtf,
            cold_spawn_to_first_pcm_ms: if cold {
                cold_spawn_to_first_pcm_ms
            } else {
                None
            },
            events,
            detail,
        }
    }
}

/// Shared mouth. Piper and the S06 clone both implement this.
///
/// `speak` calls `on_pcm` once per clause, in order. When the text has more
/// than one clause, clause N+1 is submitted before `on_pcm` runs for clause N.
/// The implementation stays warm across calls: no new process per line.
pub trait Tts {
    fn id(&self) -> &str;
    fn sample_rate(&self) -> u32;
    fn speak(
        &mut self,
        text: &str,
        mood: Pad,
        on_pcm: &mut dyn FnMut(PcmChunk<'_>),
    ) -> Result<SpeakReport, TtsError>;
}

#[derive(Debug)]
pub enum TtsError {
    Empty,
    Missing(String),
    BadSha {
        path: String,
        expected: String,
        got: String,
    },
    Io(String),
    Protocol(String),
    Timeout(String),
    Died(String),
}

impl fmt::Display for TtsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TtsError::Empty => write!(f, "empty text"),
            TtsError::Missing(s) => write!(f, "missing: {s}"),
            TtsError::BadSha {
                path,
                expected,
                got,
            } => {
                write!(
                    f,
                    "sha256 mismatch for {path}: expected {expected} got {got}"
                )
            }
            TtsError::Io(s) => write!(f, "io: {s}"),
            TtsError::Protocol(s) => write!(f, "piper protocol: {s}"),
            TtsError::Timeout(s) => write!(f, "piper timed out: {s}"),
            TtsError::Died(s) => write!(f, "piper exited: {s}"),
        }
    }
}

impl std::error::Error for TtsError {}

/// PCM the `fake-piper` binary writes for `text`. Not a voice and not a recording.
#[doc(hidden)]
pub fn stand_in_pcm(text: &str) -> Vec<i16> {
    let n = text.chars().count().max(1) as i16;
    let tone = 1000 + (n % 50);
    let mut v = Vec::with_capacity(33);
    v.push(n);
    v.extend(std::iter::repeat(tone).take(32));
    v
}

/// In-process [`Tts`] for tests. No child process, no weights, no reference audio.
#[derive(Debug)]
pub struct FakeTts {
    pub spoken: Vec<String>,
    rate: u32,
    calls: u32,
}

impl Default for FakeTts {
    fn default() -> Self {
        Self {
            spoken: Vec::new(),
            rate: 22_050,
            calls: 0,
        }
    }
}

impl FakeTts {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Tts for FakeTts {
    fn id(&self) -> &str {
        "fake"
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn speak(
        &mut self,
        text: &str,
        mood: Pad,
        on_pcm: &mut dyn FnMut(PcmChunk<'_>),
    ) -> Result<SpeakReport, TtsError> {
        let clauses = split_clauses(&prepare_text(text));
        if clauses.is_empty() {
            return Err(TtsError::Empty);
        }
        let prosody = Prosody::from_pad(mood);
        let t0 = Instant::now();
        let cold = self.calls == 0;
        self.calls += 1;
        let mut events = Vec::new();
        let mut detail = Vec::new();
        let mut total = 0usize;
        events.push(StreamEvent::Submit { sentence: 0, ms: 0 });
        for (i, clause) in clauses.iter().enumerate() {
            self.spoken.push(clause.clone());
            let raw = stand_in_pcm(clause);
            let shaped = shape_pcm(&raw, prosody, self.rate, i == 0);
            if i + 1 < clauses.len() {
                events.push(StreamEvent::Submit {
                    sentence: (i as u32) + 1,
                    ms: ms_since(t0),
                });
            }
            events.push(StreamEvent::Callback {
                sentence: i as u32,
                ms: ms_since(t0),
            });
            on_pcm(PcmChunk {
                sentence_index: i as u32,
                sample_rate: self.rate,
                samples: &shaped,
                first_of_utterance: i == 0,
            });
            total += shaped.len();
            detail.push(SentenceStat {
                index: i as u32,
                chars: clause.chars().count() as u32,
                submit_to_first_byte_ms: 0,
                submit_to_done_ms: 0,
                samples: shaped.len() as u32,
                piper_rtf: None,
                piper_infer_s: None,
                piper_audio_s: None,
            });
        }
        let wall = ms_since(t0);
        Ok(SpeakReport::from_parts(
            "fake", self.rate, cold, None, None, total, wall, events, detail,
        ))
    }
}

pub(crate) fn ms_since(t0: Instant) -> u32 {
    ms(t0.elapsed())
}

pub(crate) fn ms(d: std::time::Duration) -> u32 {
    d.as_millis().min(u128::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_ms(events: &[StreamEvent], sentence: u32, submit: bool) -> u32 {
        events
            .iter()
            .find_map(|e| match e {
                StreamEvent::Submit { sentence: s, ms } if submit && *s == sentence => Some(*ms),
                StreamEvent::Callback { sentence: s, ms } if !submit && *s == sentence => Some(*ms),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn fake_splits_strips_tags_and_submits_the_next_clause_first() {
        let mut tts = FakeTts::new();
        let mut chunks = 0;
        let report = tts
            .speak("I am here. I can hear you.", Pad::RESTING, &mut |_c| {
                chunks += 1
            })
            .unwrap();
        assert_eq!(chunks, 2);
        assert_eq!(
            tts.spoken,
            vec!["I am here.".to_string(), "I can hear you.".to_string()]
        );
        assert!(event_ms(&report.events, 1, true) <= event_ms(&report.events, 0, false));
        assert!(report.cold);
        let again = tts.speak("Hello.", Pad::RESTING, &mut |_| {}).unwrap();
        assert!(!again.cold);

        tts.spoken.clear();
        let mut got = Vec::new();
        tts.speak("Hello [laugh] there.", Pad::RESTING, &mut |c| {
            got.extend_from_slice(c.samples)
        })
        .unwrap();
        assert_eq!(tts.spoken, vec!["Hello there.".to_string()]);
        assert_eq!(got, stand_in_pcm("Hello there."));
        assert!(tts
            .speak("  [chuckle]  ", Pad::RESTING, &mut |_| {})
            .is_err());
    }

    #[test]
    fn trait_object_speaks() {
        let mut inner = FakeTts::new();
        let tts: &mut dyn Tts = &mut inner;
        let report = tts.speak("Hello.", Pad::RESTING, &mut |_| {}).unwrap();
        assert_eq!(report.backend, "fake");
        assert_eq!(report.sentences, 1);
    }
}
