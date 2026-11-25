// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One Linux smoke of Samantha's live loop.
//!
//! Ear, brain, emotion, mouth, and an in-process output buffer. There is no
//! microphone and no speaker. The input WAV is written under the system temp
//! directory at run time and removed before the function returns. Nothing
//! here dials a socket.
//!
//! Stages that are still open pull requests are not given a timing. When
//! their source is on the tree, the smoke calls it. Until then the report
//! says `stub`.
//!
//! # Safety
//!
//! No `unsafe`.

#![forbid(unsafe_code)]

mod buffer;
mod corpus;
mod model;
mod mouth;
mod probe;
mod recognizer;
mod report;
mod run;
mod stages;
mod vad;
mod wav;

pub use corpus::SENTENCES;
pub use galion_emotion_core::{EmotionCore, IntentMsg, FRAME_MS as EMOTION_FRAME_MS};
pub use report::Report;
pub use run::{refuse_other_persona, run, Outcome, Sense, SCRIPT, TRIALS};
pub use vad::{count_frames, Energy, FRAME_SAMPLES as VAD_FRAME_SAMPLES};

/// Isolated emotion ticks. The caller arms its allocator around
/// [`EmotionClaim::steps`] after [`EmotionClaim::warmed`] has returned.
pub struct EmotionClaim {
    core: EmotionCore,
    msg: IntentMsg,
    next: u32,
}

impl EmotionClaim {
    /// One unarmed step so the following loop is the steady state the
    /// existing zero-allocation test measures.
    pub fn warmed() -> Result<Self, galion_emotion_core::CoreError> {
        let mut core = EmotionCore::at_rest();
        let msg = IntentMsg::steady(0);
        core.step(&msg)?;
        Ok(Self { core, msg, next: 1 })
    }

    /// `steps` more frames. This is the body of
    /// `galion-emotion-core` `tests/zero_alloc.rs`.
    pub fn steps(&mut self, steps: u32) -> Result<(), galion_emotion_core::CoreError> {
        for _ in 0..steps {
            self.msg.t_ms = self.next.saturating_mul(EMOTION_FRAME_MS);
            let moment = self.core.step(&self.msg)?;
            let cue = self.core.conductor_cue();
            let seq = u16::try_from(moment.tick).unwrap_or(u16::MAX);
            let wire = self.core.local_wire(seq);
            std::hint::black_box((moment, cue, wire));
            self.next = self.next.saturating_add(1);
        }
        Ok(())
    }

    pub fn next_index(&self) -> u32 {
        self.next
    }
}

/// `0`, `off`, and `deny` close ear prosody. Unset, empty, and anything else
/// leave it on. Same words as the ear-prosody switch.
pub fn env_closes(value: Option<&str>) -> bool {
    matches!(
        value
            .map(|item| item.trim().to_ascii_lowercase())
            .as_deref(),
        Some("0" | "off" | "deny")
    )
}

pub fn sense_from_env(value: Option<&str>) -> Sense {
    if env_closes(value) {
        Sense::Off
    } else {
        Sense::On
    }
}

/// Nearest-rank percentile. `quantile` is 0.0 to 1.0. The slice must already
/// be sorted ascending.
pub fn percentile_nearest(sorted: &[u64], quantile: f64) -> Option<u64> {
    if sorted.is_empty() || !(0.0..=1.0).contains(&quantile) {
        return None;
    }
    let n = sorted.len();
    let rank = (quantile * n as f64).ceil() as usize;
    let index = rank.clamp(1, n) - 1;
    Some(sorted[index])
}

pub fn percentile_nearest_f64(mut samples: Vec<f64>, quantile: f64) -> Option<f64> {
    if samples.is_empty() || !(0.0..=1.0).contains(&quantile) {
        return None;
    }
    samples.sort_by(|a, b| a.total_cmp(b));
    let n = samples.len();
    let rank = (quantile * n as f64).ceil() as usize;
    let index = rank.clamp(1, n) - 1;
    Some(samples[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ear_sense_is_on_unless_the_switch_is_closed() {
        assert!(!env_closes(None));
        assert!(!env_closes(Some("")));
        assert!(!env_closes(Some("1")));
        assert!(!env_closes(Some("on")));
        assert!(!env_closes(Some("allow")));
        assert!(env_closes(Some("0")));
        assert!(env_closes(Some("off")));
        assert!(env_closes(Some(" deny ")));
        assert_eq!(sense_from_env(None), Sense::On);
        assert_eq!(sense_from_env(Some("off")), Sense::Off);
    }

    #[test]
    fn nearest_rank_picks_the_upper_sample() {
        let samples = [1, 2, 3, 4, 5];
        assert_eq!(percentile_nearest(&samples, 0.50), Some(3));
        assert_eq!(percentile_nearest(&samples, 0.95), Some(5));
        assert_eq!(percentile_nearest(&[], 0.50), None);
    }

    #[test]
    fn the_generated_set_starts_with_the_kettle_line() {
        assert_eq!(SENTENCES[0], SCRIPT);
        assert_eq!(SENTENCES.len(), 11);
        assert!(SENTENCES.iter().all(|line| !line.is_empty()));
    }

    #[test]
    fn a_missing_hypothesis_is_not_a_zero_wer() {
        let stub = report::format_wer(SCRIPT, None);
        assert_eq!(stub, "stub");
        let same = report::format_wer(SCRIPT, Some(SCRIPT));
        assert_eq!(same, "0.0000");
        let dropped = report::format_wer(SCRIPT, Some("where is kettle"));
        assert_ne!(dropped, "stub");
        assert_ne!(dropped, "0.0000");
    }
}
