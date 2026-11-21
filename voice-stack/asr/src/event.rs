// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use crate::json::{to_line, Json};

/// Heartbeat period shared with `galion-heartbeat` (`LOCKS.toml` `[clock] frame_ms = 80`).
pub const HEARTBEAT_MS: u32 = 80;
/// Phone-path rate. Same constant as `galion-asr-vosk`.
pub const SAMPLE_RATE: u32 = 16_000;
/// 20 ms at 16 kHz. `galion-asr-vosk` feeds libvosk in frames of this size.
pub const VOSK_FRAME_SAMPLES: usize = 320;
/// One heartbeat of 16 kHz audio: four Vosk frames.
pub const HEARTBEAT_SAMPLES: usize = SAMPLE_RATE as usize * HEARTBEAT_MS as usize / 1000;

/// One decoded word when the backend has timings. Times are seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Word {
    pub word: String,
    pub start: f32,
    pub end: f32,
    pub conf: f32,
}

/// A partial may be replaced by a later partial or by the final.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsrKind {
    Partial,
    Final,
}

/// One hypothesis snapshot.
///
/// `text` is the whole utterance so far, not a delta. `audio_ms` is how much
/// audio this engine has accepted in the current clip when the snapshot was made.
#[derive(Debug, Clone, PartialEq)]
pub struct AsrEvent {
    pub kind: AsrKind,
    pub text: String,
    pub audio_ms: u32,
    pub words: Vec<Word>,
    /// `partial`, `endpoint`, `finish`, or `worker`.
    pub why: &'static str,
}

impl AsrEvent {
    pub fn to_json_line(&self) -> String {
        let kind = match self.kind {
            AsrKind::Partial => "partial",
            AsrKind::Final => "final",
        };
        let words = Json::Arr(
            self.words
                .iter()
                .map(|w| {
                    Json::Obj(vec![
                        ("word".into(), Json::Str(w.word.clone())),
                        ("start".into(), Json::Num(w.start as f64)),
                        ("end".into(), Json::Num(w.end as f64)),
                        ("conf".into(), Json::Num(w.conf as f64)),
                    ])
                })
                .collect(),
        );
        to_line(&Json::Obj(vec![
            ("kind".into(), Json::Str(kind.into())),
            ("text".into(), Json::Str(self.text.clone())),
            ("audio_ms".into(), Json::Num(self.audio_ms as f64)),
            ("why".into(), Json::Str(self.why.into())),
            ("words".into(), words),
        ]))
    }
}

pub fn samples_to_ms(samples: usize) -> u32 {
    ((samples as u64).saturating_mul(1000) / u64::from(SAMPLE_RATE)).min(u32::MAX as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heartbeat_is_four_vosk_frames() {
        assert_eq!(HEARTBEAT_MS, 80);
        assert_eq!(VOSK_FRAME_SAMPLES, 320);
        assert_eq!(HEARTBEAT_SAMPLES, 1280);
        assert_eq!(samples_to_ms(SAMPLE_RATE as usize), 1000);
        assert_eq!(samples_to_ms(HEARTBEAT_SAMPLES), 80);
    }
}
