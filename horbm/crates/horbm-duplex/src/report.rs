// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Frame latencies. One frame is 80 ms. The budgets are the locks and the
//! R4 targets (PR 207, sections 4.3, 11, and 12). This clock does not read
//! a wall time, so the 60 ms compute budget inside a frame is not a gate here.

use crate::FRAME_MS;

/// Upper bounds the loop asserts. Times are milliseconds on the virtual clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// Ear classifies the frame it was given. No extra frame.
    pub ear_frames: u32,
    /// A partial on the same frame as the samples. R4 §4.3.
    pub recognizer_frames: u32,
    /// The eager frame reads a cell written while the person was speaking.
    /// R4 §4.3, prefix hit, no extra frame on the turn gap.
    pub brain_frames: u32,
    /// One frame of hang after the last voiced frame. R4 §4.3, 80 ms.
    pub conductor_frames: u32,
    /// Commit is inside the content budget. Two frames is 160 ms.
    pub commit_frames: u32,
    /// Wordless reaction starts within one frame of eager end of turn.
    pub reaction_frames: u32,
    /// First content audio from eager end of turn. R4 §4.3 p50 and §12.
    pub voice_p50_ms: u32,
    /// First content audio, p95. R4 §4.3 and §12.
    pub voice_p95_ms: u32,
    /// `LOCKS.toml` `[gates] onset_p50_max_ms`.
    pub onset_ms: u32,
    /// `LOCKS.toml` `[gates] affect_to_voice_max_frames`.
    pub affect_frames: u32,
    /// `LOCKS.toml` `[gates] affect_to_voice_max_ms`.
    pub affect_ms: u32,
    /// Output stops two frames after the barge onset. 160 ms.
    /// Inside `LOCKS.toml` `[gates] barge_in_max_ms` (200).
    pub barge_frames: u32,
    pub barge_ms: u32,
    /// The lock. The harness gate is the tighter [`Self::barge_ms`].
    pub barge_lock_ms: u32,
    /// R4 phase 4. The first spoken piece is at most two words.
    pub first_clause_words: usize,
}

impl Budgets {
    /// Locks and R4 targets this harness binds.
    #[must_use]
    pub const fn locked() -> Self {
        Self {
            ear_frames: 0,
            recognizer_frames: 0,
            brain_frames: 0,
            conductor_frames: 1,
            commit_frames: 2,
            reaction_frames: 1,
            voice_p50_ms: 200,
            voice_p95_ms: 240,
            onset_ms: 300,
            affect_frames: 2,
            affect_ms: 160,
            barge_frames: 2,
            barge_ms: 160,
            barge_lock_ms: 200,
            first_clause_words: 2,
        }
    }
}

/// What one closed run measured. `None` means that stage did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatencyReport {
    pub ear_frames: u32,
    pub recognizer_frames: Option<u32>,
    pub brain_frames: Option<u32>,
    pub brain_filled_before_eager: bool,
    pub brain_read_on_eager: bool,
    pub brain_line_matched: bool,
    pub conductor_frames: Option<u32>,
    pub commit_frames: Option<u32>,
    pub reaction_frames: Option<u32>,
    pub voice_ms: Option<u64>,
    pub affect_frames: Option<u32>,
    pub barge_ms: Option<u64>,
    pub barge_frames: Option<u32>,
    pub barge_while_you_live: bool,
    pub first_clause_words: Option<usize>,
    pub backchannel_frames: u32,
    pub backchannel_cancels: u32,
    pub overlap_live_frames: u64,
    pub you_frames: u64,
    pub me_audible_frames: u64,
    pub research_frames: u32,
}

impl LatencyReport {
    pub(crate) fn fresh() -> Self {
        Self {
            ear_frames: 0,
            recognizer_frames: None,
            brain_frames: None,
            brain_filled_before_eager: false,
            brain_read_on_eager: false,
            brain_line_matched: false,
            conductor_frames: None,
            commit_frames: None,
            reaction_frames: None,
            voice_ms: None,
            affect_frames: None,
            barge_ms: None,
            barge_frames: None,
            barge_while_you_live: false,
            first_clause_words: None,
            backchannel_frames: 0,
            backchannel_cancels: 0,
            overlap_live_frames: 0,
            you_frames: 0,
            me_audible_frames: 0,
            research_frames: 0,
        }
    }

    /// Stages that missed a budget. Empty means the run held.
    #[must_use]
    pub fn budget_misses(&self) -> Vec<String> {
        let cap = Budgets::locked();
        let mut misses = Vec::new();
        stage_u32(&mut misses, "ear", self.ear_frames, cap.ear_frames);
        require_u32(
            &mut misses,
            "recognizer",
            self.recognizer_frames,
            cap.recognizer_frames,
        );
        require_u32(&mut misses, "brain", self.brain_frames, cap.brain_frames);
        if !self.brain_filled_before_eager {
            misses.push("brain cell was not filled before eager end of turn".to_string());
        }
        if !self.brain_read_on_eager {
            misses.push("brain cell was not read on the eager frame".to_string());
        }
        if !self.brain_line_matched {
            misses.push("mouth line was not the cell the conductor committed".to_string());
        }
        require_u32(
            &mut misses,
            "conductor",
            self.conductor_frames,
            cap.conductor_frames,
        );
        require_u32(&mut misses, "commit", self.commit_frames, cap.commit_frames);
        require_u32(
            &mut misses,
            "reaction",
            self.reaction_frames,
            cap.reaction_frames,
        );
        match self.voice_ms {
            Some(ms)
                if ms <= u64::from(cap.voice_p50_ms)
                    && ms <= u64::from(cap.voice_p95_ms)
                    && ms <= u64::from(cap.onset_ms) => {}
            Some(ms) => misses.push(format!(
                "voice {ms} ms, budget p50 {} ms, p95 {} ms, onset {} ms",
                cap.voice_p50_ms, cap.voice_p95_ms, cap.onset_ms
            )),
            None => misses.push("voice did not start".to_string()),
        }
        require_u32(&mut misses, "affect", self.affect_frames, cap.affect_frames);
        if let Some(frames) = self.affect_frames {
            let ms = u64::from(frames) * u64::from(FRAME_MS);
            if ms > u64::from(cap.affect_ms) {
                misses.push(format!("affect {ms} ms, budget {} ms", cap.affect_ms));
            }
        }
        match self.barge_ms {
            Some(ms) if ms <= u64::from(cap.barge_ms) && ms <= u64::from(cap.barge_lock_ms) => {}
            Some(ms) => misses.push(format!(
                "barge {ms} ms, budget {} ms, lock {} ms",
                cap.barge_ms, cap.barge_lock_ms
            )),
            None => misses.push("barge-in did not silence the me band".to_string()),
        }
        require_u32(&mut misses, "barge", self.barge_frames, cap.barge_frames);
        if !self.barge_while_you_live {
            misses.push("me band went silent without a live you frame".to_string());
        }
        match self.first_clause_words {
            Some(words) if words > 0 && words <= cap.first_clause_words => {}
            Some(words) => misses.push(format!(
                "first clause {words} words, budget {}",
                cap.first_clause_words
            )),
            None => misses.push("first clause was not committed".to_string()),
        }
        if self.backchannel_cancels != 0 {
            misses.push(format!(
                "backchannel cancels {}, lock is 0",
                self.backchannel_cancels
            ));
        }
        if self.backchannel_frames == 0 {
            misses.push("no backchannel frame was heard".to_string());
        }
        if self.overlap_live_frames == 0 {
            misses.push("the bands were never live on the same frame".to_string());
        }
        if self.you_frames == 0 || self.me_audible_frames == 0 {
            misses.push("a band did not run".to_string());
        }
        if self.research_frames != 0 {
            misses.push(format!(
                "side band entered the tick {} times",
                self.research_frames
            ));
        }
        misses
    }

    /// One line of counts. No transcript and no samples.
    #[must_use]
    pub fn gate_line(&self) -> String {
        let show = |value: Option<u32>| match value {
            Some(n) => n.to_string(),
            None => "none".to_string(),
        };
        let result = if self.budget_misses().is_empty() {
            "pass"
        } else {
            "fail"
        };
        format!(
            "duplex s26 frame_ms={FRAME_MS} clock=virtual persona=samantha ear_frames={} recognizer_frames={} brain_frames={} conductor_frames={} commit_frames={} reaction_frames={} voice_ms={} affect_frames={} barge_ms={} barge_frames={} backchannel_frames={} backchannel_cancels={} overlap_live_frames={} you_frames={} me_audible_frames={} first_clause_words={} research_frames={} result={result}",
            self.ear_frames,
            show(self.recognizer_frames),
            show(self.brain_frames),
            show(self.conductor_frames),
            show(self.commit_frames),
            show(self.reaction_frames),
            self.voice_ms.map(|ms| ms.to_string()).unwrap_or_else(|| "none".to_string()),
            show(self.affect_frames),
            self.barge_ms.map(|ms| ms.to_string()).unwrap_or_else(|| "none".to_string()),
            show(self.barge_frames),
            self.backchannel_frames,
            self.backchannel_cancels,
            self.overlap_live_frames,
            self.you_frames,
            self.me_audible_frames,
            self.first_clause_words.unwrap_or(0),
            self.research_frames,
        )
    }
}

fn stage_u32(misses: &mut Vec<String>, name: &str, got: u32, cap: u32) {
    if got > cap {
        misses.push(format!("{name} {got} frames, budget {cap}"));
    }
}

fn require_u32(misses: &mut Vec<String>, name: &str, got: Option<u32>, cap: u32) {
    match got {
        Some(got) if got <= cap => {}
        Some(got) => misses.push(format!("{name} {got} frames, budget {cap}")),
        None => misses.push(format!("{name} did not run")),
    }
}
