// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! When a turn starts, when it ends, and when Mate is talking over Samantha.
//!
//! The numbers are the ones the live lane already runs:
//!
//! * onset 60 ms — `Vad::min_voiced_ms` in `archive/2026-11-24/crates/galion-v2-live/src/vad.rs`
//! * barge-in at 80 ms of consecutive overlap — the duck point in `archive/2026-11-24/crates/galion-v2-live/src/bin/call.rs`
//! * speculate at 60 ms, commit a finished sentence at 160 ms, hold an unfinished
//!   word for 500 ms — `EndpointCfg` in `archive/2026-11-24/crates/galion-v2-live/src/endpoint.rs`
//! * with no transcript yet, the hangover is the 500 ms hold, not the 160 ms
//!   commit. 160 ms is only safe once the words say he finished. The live call
//!   uses a 300 ms VAD hang (`Vad::new(300.0)`) because ASR is in that loop.
//!   Set `GALION_VAD_HANG_MS=300` to use that number here.
//!
//! A pause shorter than the hangover is not a turn end. That is the whole
//! point of the hangover: Samantha does not take the floor while Mate is
//! still in the sentence. This module emits events. It does not speak.

use crate::HEARTBEAT_MS;

/// Words after which a pause is more likely a breath than a turn end.
/// The list is the one in `archive/2026-11-24/crates/galion-v2-live/src/endpoint.rs`.
const UNFINISHED: &[&str] = &[
    "and", "but", "or", "so", "because", "cause", "the", "a", "an", "to", "of", "in", "on", "at",
    "with", "for", "from", "my", "your", "his", "her", "their", "our", "um", "uh", "er", "like",
    "that", "which", "who", "if", "when", "while", "then", "is", "are", "was", "were", "i", "i'm",
    "we", "you", "it's", "this", "about", "into", "than", "as", "by", "not", "very",
];

pub fn sounds_unfinished(transcript: &str) -> bool {
    match transcript.split_whitespace().last() {
        Some(w) => UNFINISHED.contains(
            &w.to_lowercase()
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '\''),
        ),
        None => false,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TurnCfg {
    pub frame_ms: u32,
    /// Sustained voice before [`Event::SpeechStart`].
    pub onset_ms: u32,
    /// Silence that ends a turn when the transcript is empty.
    pub hangover_ms: u32,
    /// Silence before [`Event::Speculate`], and only when a transcript is present.
    pub spec_ms: u32,
    /// Silence that ends a turn whose last word looks finished.
    pub commit_ms: u32,
    /// Silence that ends a turn whose last word looks unfinished.
    pub hold_ms: u32,
    /// Probability that starts a turn. Silero's published operating point is 0.5.
    /// The energy backend emits 0 or 1, so this threshold does not change it.
    pub start_threshold: f32,
    /// While a turn is open, stay voiced down to this probability so a dip
    /// does not start the hangover. Silero's own margin below 0.5 is 0.15.
    pub keep_threshold: f32,
    /// Consecutive voiced milliseconds, while she is speaking, before [`FrameReport`]
    /// style barge-in. 80 matches the live call's duck.
    pub barge_ms: u32,
}

impl Default for TurnCfg {
    fn default() -> Self {
        Self {
            frame_ms: 20,
            onset_ms: 60,
            hangover_ms: 500,
            spec_ms: 60,
            commit_ms: 160,
            hold_ms: 500,
            start_threshold: 0.5,
            keep_threshold: 0.35,
            barge_ms: 80,
        }
    }
}

impl TurnCfg {
    pub fn from_env() -> Self {
        let d = Self::default();
        let u = |k: &str, d: u32| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        let f = |k: &str, d: f32| {
            std::env::var(k)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(d)
        };
        let mut c = Self {
            frame_ms: d.frame_ms,
            onset_ms: u("GALION_VAD_ONSET_MS", d.onset_ms),
            hangover_ms: u("GALION_VAD_HANG_MS", d.hangover_ms),
            spec_ms: u("GALION_VAD_SPEC_MS", d.spec_ms),
            commit_ms: u("GALION_VAD_COMMIT_MS", d.commit_ms),
            hold_ms: u("GALION_VAD_HOLD_MS", d.hold_ms),
            start_threshold: f("GALION_VAD_THRESHOLD", d.start_threshold),
            keep_threshold: f("GALION_VAD_KEEP", d.keep_threshold),
            barge_ms: u("GALION_VAD_BARGE_MS", d.barge_ms),
        };
        if c.frame_ms == 0 {
            c.frame_ms = d.frame_ms;
        }
        if c.keep_threshold > c.start_threshold {
            c.keep_threshold = c.start_threshold;
        }
        c
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    None,
    /// His voice has held for `onset_ms`.
    SpeechStart,
    /// Silence has held for the hangover that applies to this transcript.
    SpeechEnd,
    /// Early hint that he might be done. Not a turn end. Cancel with [`Event::Resume`].
    Speculate,
    /// Voice came back after [`Event::Speculate`]. Drop the early guess.
    Resume,
    /// The short commit window passed and the last word still looks unfinished.
    /// The caller can refresh the ASR final. The turn stays open.
    Hold,
}

impl Event {
    pub fn name(self) -> &'static str {
        match self {
            Event::None => "none",
            Event::SpeechStart => "speech_start",
            Event::SpeechEnd => "speech_end",
            Event::Speculate => "speculate",
            Event::Resume => "resume",
            Event::Hold => "hold",
        }
    }

    /// The `EndpointEvent` name in `galion-sam-contract` (`voice-stack/contract`).
    /// `SpeechEnd` is that contract's `Commit`: the hangover closed and the turn may be spoken.
    pub fn endpoint_name(self) -> &'static str {
        match self {
            Event::None => "none",
            Event::SpeechStart => "start",
            Event::SpeechEnd => "commit",
            Event::Speculate => "speculate",
            Event::Resume => "resume",
            Event::Hold => "hold",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnOut {
    /// Index of this 20 ms frame, starting at 0.
    pub frame: u64,
    /// Start of this frame on the stream clock, milliseconds.
    pub t_ms: u32,
    /// Which 80 ms heartbeat this frame belongs to (`t_ms / 80`).
    pub heartbeat: u64,
    pub event: Event,
    /// He is speaking over her. Fires once per turn, at `barge_ms` of
    /// consecutive voiced overlap, and only if the turn started while
    /// `me_speaking` was already true.
    pub barge_in: bool,
    pub speaking: bool,
    /// Consecutive voiced milliseconds while `me_speaking` is true. Resets
    /// on a silent frame, the same way `voiced_in_ms` resets in `call.rs`.
    pub overlap_ms: u32,
    pub silent_ms: u32,
    pub voiced_ms: u32,
}

#[derive(Debug, Clone)]
pub struct Turn {
    cfg: TurnCfg,
    frame: u64,
    speaking: bool,
    voiced_ms: u32,
    silent_ms: u32,
    overlap_ms: u32,
    speculated: bool,
    held: bool,
    /// SpeechStart happened on a frame where she was already speaking.
    armed_barge: bool,
    barged: bool,
}

impl Turn {
    pub fn new(cfg: TurnCfg) -> Self {
        Self {
            cfg,
            frame: 0,
            speaking: false,
            voiced_ms: 0,
            silent_ms: 0,
            overlap_ms: 0,
            speculated: false,
            held: false,
            armed_barge: false,
            barged: false,
        }
    }

    pub fn cfg(&self) -> TurnCfg {
        self.cfg
    }

    pub fn reset(&mut self) {
        let cfg = self.cfg;
        *self = Self::new(cfg);
    }

    pub fn speaking(&self) -> bool {
        self.speaking
    }

    fn end_window(&self, transcript: &str) -> u32 {
        if transcript.trim().is_empty() {
            self.cfg.hangover_ms
        } else if sounds_unfinished(transcript) {
            self.cfg.hold_ms
        } else {
            self.cfg.commit_ms
        }
    }

    /// One frame. `prob` is 0..1 from Silero, or 0/1 from the energy gate.
    /// `me_speaking` is the me-band flag the 80 ms heartbeat already carries
    /// (`FrameIn::me_speaking`): true while Samantha's audio is in the speaker.
    /// `transcript` is the ASR text so far. Empty means "no words yet".
    pub fn push(&mut self, prob: f32, me_speaking: bool, transcript: &str) -> TurnOut {
        let frame = self.frame;
        self.frame = self.frame.saturating_add(1);
        let t_ms = (frame as u128)
            .saturating_mul(self.cfg.frame_ms as u128)
            .min(u32::MAX as u128) as u32;
        let heartbeat = (t_ms as u64) / (HEARTBEAT_MS as u64);

        let threshold = if self.speaking {
            self.cfg.keep_threshold
        } else {
            self.cfg.start_threshold
        };
        let candidate = prob >= threshold;

        if candidate {
            self.voiced_ms = self.voiced_ms.saturating_add(self.cfg.frame_ms);
            self.silent_ms = 0;
        } else {
            self.silent_ms = self.silent_ms.saturating_add(self.cfg.frame_ms);
            if !self.speaking {
                self.voiced_ms = 0;
            }
        }

        if me_speaking && candidate {
            self.overlap_ms = self.overlap_ms.saturating_add(self.cfg.frame_ms);
        } else {
            self.overlap_ms = 0;
        }

        let mut event = Event::None;
        if !self.speaking && candidate && self.voiced_ms >= self.cfg.onset_ms {
            self.speaking = true;
            self.speculated = false;
            self.held = false;
            self.armed_barge = me_speaking;
            event = Event::SpeechStart;
        } else if self.speaking && candidate {
            self.held = false;
            if self.speculated {
                self.speculated = false;
                event = Event::Resume;
            }
        } else if self.speaking && !candidate {
            let window = self.end_window(transcript);
            if self.silent_ms >= window {
                self.speaking = false;
                self.voiced_ms = 0;
                self.speculated = false;
                self.held = false;
                self.armed_barge = false;
                self.barged = false;
                self.overlap_ms = 0;
                event = Event::SpeechEnd;
            } else if !transcript.trim().is_empty()
                && sounds_unfinished(transcript)
                && !self.held
                && self.silent_ms >= self.cfg.commit_ms
            {
                self.held = true;
                event = Event::Hold;
            } else if !transcript.trim().is_empty()
                && !self.speculated
                && self.silent_ms >= self.cfg.spec_ms
            {
                self.speculated = true;
                event = Event::Speculate;
            }
        }

        let mut barge_in = false;
        if self.speaking
            && self.armed_barge
            && !self.barged
            && me_speaking
            && self.overlap_ms >= self.cfg.barge_ms
        {
            self.barged = true;
            barge_in = true;
        }
        if !me_speaking && !self.barged {
            self.armed_barge = false;
        }

        TurnOut {
            frame,
            t_ms,
            heartbeat,
            event,
            barge_in,
            speaking: self.speaking,
            overlap_ms: self.overlap_ms,
            silent_ms: self.silent_ms,
            voiced_ms: self.voiced_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(t: &mut Turn, n: usize, prob: f32, me: bool, text: &str) -> Vec<TurnOut> {
        (0..n).map(|_| t.push(prob, me, text)).collect()
    }

    #[test]
    fn hangover_keeps_a_breath_inside_the_turn() {
        let mut t = Turn::new(TurnCfg::default());
        let start = feed(&mut t, 5, 1.0, false, "");
        assert_eq!(start[2].event, Event::SpeechStart);
        assert_eq!(start[2].t_ms, 40);
        // 24 silent frames = 480 ms, under the 500 ms hangover.
        let gap = feed(&mut t, 24, 0.0, false, "");
        assert!(gap.iter().all(|o| o.event != Event::SpeechEnd));
        assert!(t.speaking());
        let end = feed(&mut t, 1, 0.0, false, "");
        assert_eq!(end[0].event, Event::SpeechEnd);
        assert_eq!(end[0].silent_ms, 500);
        assert!(!t.speaking());
    }

    #[test]
    fn a_click_does_not_start_a_turn() {
        let mut t = Turn::new(TurnCfg::default());
        let ev = feed(&mut t, 2, 1.0, false, "");
        assert!(ev.iter().all(|o| o.event == Event::None));
        let rest = feed(&mut t, 5, 0.0, false, "");
        assert!(rest.iter().all(|o| o.event != Event::SpeechStart));
    }

    #[test]
    fn finished_sentence_commits_at_160_unfinished_holds_to_500() {
        let mut done = Turn::new(TurnCfg::default());
        feed(&mut done, 5, 1.0, false, "hello there");
        let ev = feed(&mut done, 12, 0.0, false, "hello there");
        assert_eq!(ev.iter().position(|o| o.event == Event::Speculate), Some(2));
        assert_eq!(ev.iter().position(|o| o.event == Event::SpeechEnd), Some(7));
        assert!(!ev.iter().any(|o| o.event == Event::Hold));

        let mut held = Turn::new(TurnCfg::default());
        feed(&mut held, 5, 1.0, false, "i went to the");
        let early = feed(&mut held, 12, 0.0, false, "i went to the");
        assert!(early.iter().any(|o| o.event == Event::Speculate));
        assert_eq!(early.iter().filter(|o| o.event == Event::Hold).count(), 1);
        assert!(!early.iter().any(|o| o.event == Event::SpeechEnd));
        assert!(held.speaking());
        // 12 silent frames already pushed; the 25th (500 ms) ends it.
        let rest = feed(&mut held, 13, 0.0, false, "i went to the");
        assert_eq!(
            rest.iter().position(|o| o.event == Event::SpeechEnd),
            Some(12)
        );
    }

    #[test]
    fn voice_after_speculate_is_a_resume_not_a_cut() {
        let mut t = Turn::new(TurnCfg::default());
        feed(&mut t, 5, 1.0, false, "hello there");
        let gap = feed(&mut t, 4, 0.0, false, "hello there");
        assert!(gap.iter().any(|o| o.event == Event::Speculate));
        let back = t.push(1.0, false, "hello there");
        assert_eq!(back.event, Event::Resume);
        assert!(back.speaking);
        let later = feed(&mut t, 7, 0.0, false, "hello there");
        assert!(later.iter().all(|o| o.event != Event::SpeechEnd));
        assert_eq!(t.push(0.0, false, "hello there").event, Event::SpeechEnd);
    }

    #[test]
    fn barge_in_at_80_ms_only_when_he_starts_over_her() {
        let mut over = Turn::new(TurnCfg::default());
        let ev = feed(&mut over, 6, 1.0, true, "");
        assert_eq!(ev[2].event, Event::SpeechStart);
        assert!(!ev[2].barge_in);
        assert!(ev[3].barge_in);
        assert_eq!(ev[3].overlap_ms, 80);
        assert!(ev[4..].iter().all(|o| !o.barge_in));

        let mut quiet = Turn::new(TurnCfg::default());
        let ev = feed(&mut quiet, 10, 1.0, false, "");
        assert!(ev.iter().any(|o| o.event == Event::SpeechStart));
        assert!(ev.iter().all(|o| !o.barge_in));

        // He already has the floor. Her starting to speak is not him barging in.
        let mut his = Turn::new(TurnCfg::default());
        feed(&mut his, 5, 1.0, false, "");
        let later = feed(&mut his, 10, 1.0, true, "");
        assert!(later.iter().all(|o| !o.barge_in));
    }

    #[test]
    fn a_one_frame_dip_restarts_the_barge_counter() {
        let mut t = Turn::new(TurnCfg::default());
        let head = feed(&mut t, 3, 1.0, true, "");
        assert_eq!(head[2].event, Event::SpeechStart);
        assert!(!head.iter().any(|o| o.barge_in));
        let dip = t.push(0.0, true, "");
        assert_eq!(dip.overlap_ms, 0);
        let back = feed(&mut t, 4, 1.0, true, "");
        assert!(!back[0].barge_in && !back[1].barge_in && !back[2].barge_in);
        assert!(back[3].barge_in);
    }

    #[test]
    fn she_stopping_before_80_ms_cancels_the_pending_barge() {
        let mut t = Turn::new(TurnCfg::default());
        feed(&mut t, 3, 1.0, true, "");
        // She stops. He keeps going. That is not a barge-in.
        let ev = feed(&mut t, 8, 1.0, false, "");
        assert!(ev.iter().all(|o| !o.barge_in));
    }

    #[test]
    fn keep_threshold_rides_through_a_dip_that_would_not_start_a_turn() {
        let mut t = Turn::new(TurnCfg::default());
        assert_eq!(feed(&mut t, 3, 0.5, false, "")[2].event, Event::SpeechStart);
        let dip = feed(&mut t, 5, 0.4, false, "");
        assert!(dip.iter().all(|o| o.speaking && o.event == Event::None));
        assert_eq!(t.push(0.3, false, "").silent_ms, 20);
        let mut idle = Turn::new(TurnCfg::default());
        let ev = feed(&mut idle, 10, 0.4, false, "");
        assert!(ev.iter().all(|o| o.event != Event::SpeechStart));
    }

    #[test]
    fn heartbeat_index_steps_every_four_frames() {
        let mut t = Turn::new(TurnCfg::default());
        let ev = feed(&mut t, 8, 0.0, false, "");
        assert_eq!(ev[0].heartbeat, 0);
        assert_eq!(ev[3].heartbeat, 0);
        assert_eq!(ev[4].t_ms, 80);
        assert_eq!(ev[4].heartbeat, 1);
    }

    #[test]
    fn unfinished_word_list_matches_the_live_endpointer() {
        assert!(sounds_unfinished("so I said um"));
        assert!(sounds_unfinished("i went to the"));
        assert!(!sounds_unfinished("that was great"));
        assert!(!sounds_unfinished(""));
    }

    #[test]
    fn endpoint_names_match_the_contract() {
        assert_eq!(Event::None.endpoint_name(), "none");
        assert_eq!(Event::SpeechStart.endpoint_name(), "start");
        assert_eq!(Event::SpeechEnd.endpoint_name(), "commit");
        assert_eq!(Event::Speculate.endpoint_name(), "speculate");
        assert_eq!(Event::Resume.endpoint_name(), "resume");
        assert_eq!(Event::Hold.endpoint_name(), "hold");
    }
}
