// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Domain-04 turn stamps (`t_vad_silence_start` through `t_tts_first_pcm`).
//!
//! The sheet stores heartbeat milliseconds and lengths. It does not store a
//! transcript, a reply, or PCM. [`TurnStamps::log`] prints `absent` or a
//! number on every line. [`StampLane`] is the dry-run queue: depth 8,
//! `try_send` never waits, same contract as the process bus stamp lane.
//!
//! Nothing here runs inside [`galion_heartbeat::Heartbeat::step`].

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::Instant;

/// Same depth as the process-bus reply lane.
pub const STAMP_DEPTH: usize = 8;

/// Heartbeat period. Locked to [`galion_heartbeat::FRAME_MS`].
pub const FRAME_MS: u64 = galion_heartbeat::FRAME_MS as u64;

const _: () = assert!(galion_heartbeat::FRAME_MS == 80);

/// One cascade turn. Missing stages stay [`None`] (`absent` in the log).
/// `Some(0)` is a real stamp at the clock origin, not a missing stage.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurnStamps {
    pub turn_id: u64,
    pub t_vad_silence_start: Option<u64>,
    pub t_eou_decided: Option<u64>,
    pub t_asr_final: Option<u64>,
    pub t_llm_req: Option<u64>,
    pub t_llm_first_token: Option<u64>,
    pub t_first_clause_flushed: Option<u64>,
    pub t_tts_first_pcm: Option<u64>,
    /// Input PCM16 sample count. Not the samples.
    pub pcm_samples: u32,
    /// Heard-text character count. Not the text.
    pub text_chars: u32,
    /// Reply character count. Not the reply.
    pub reply_chars: u32,
    /// Synthesized PCM16 sample count. Not the samples.
    pub audio_samples: u32,
}

impl TurnStamps {
    /// Every stage absent, every length zero.
    pub fn dry() -> Self {
        Self::default()
    }

    pub fn with_turn_id(mut self, turn_id: u64) -> Self {
        self.turn_id = turn_id;
        self
    }

    /// Ear hook. The cascade does not invent a VAD decision.
    pub fn note_vad_silence_start(&mut self, t_ms: u64) {
        self.t_vad_silence_start = Some(t_ms);
    }

    /// Ear hook. The cascade does not invent an end-of-utterance decision.
    pub fn note_eou_decided(&mut self, t_ms: u64) {
        self.t_eou_decided = Some(t_ms);
    }

    /// `t_tts_first_pcm - t_vad_silence_start` when both ran and the clock
    /// did not go backwards. Later phases read this. This crate does not
    /// claim a latency budget.
    pub fn turn_gap_ms(&self) -> Option<u64> {
        let start = self.t_vad_silence_start?;
        let pcm = self.t_tts_first_pcm?;
        pcm.checked_sub(start)
    }

    /// One `key=value` line per field. Values are `absent` or a decimal.
    pub fn log(&self) -> String {
        let mut out = String::new();
        push_line(&mut out, "turn_id", &self.turn_id.to_string());
        for (name, value) in self.times() {
            match value {
                None => push_line(&mut out, name, "absent"),
                Some(v) => push_line(&mut out, name, &v.to_string()),
            }
        }
        push_line(&mut out, "pcm_samples", &self.pcm_samples.to_string());
        push_line(&mut out, "text_chars", &self.text_chars.to_string());
        push_line(&mut out, "reply_chars", &self.reply_chars.to_string());
        push_line(&mut out, "audio_samples", &self.audio_samples.to_string());
        match self.turn_gap_ms() {
            None => push_line(&mut out, "turn_gap_ms", "absent"),
            Some(v) => push_line(&mut out, "turn_gap_ms", &v.to_string()),
        }
        out
    }

    fn times(&self) -> [(&'static str, Option<u64>); 7] {
        [
            ("t_vad_silence_start", self.t_vad_silence_start),
            ("t_eou_decided", self.t_eou_decided),
            ("t_asr_final", self.t_asr_final),
            ("t_llm_req", self.t_llm_req),
            ("t_llm_first_token", self.t_llm_first_token),
            ("t_first_clause_flushed", self.t_first_clause_flushed),
            ("t_tts_first_pcm", self.t_tts_first_pcm),
        ]
    }
}

fn push_line(out: &mut String, key: &str, value: &str) {
    out.push_str(key);
    out.push('=');
    out.push_str(value);
    out.push('\n');
}

/// `true` when every line is `key=absent` or `key=<digits>`.
/// A transcript word, a space, or a PCM dump fails the check.
pub fn log_is_lengths_only(log: &str) -> bool {
    if log.is_empty() || !log.ends_with('\n') {
        return false;
    }
    for line in log.lines() {
        if line.is_empty() {
            return false;
        }
        let Some((key, value)) = line.split_once('=') else {
            return false;
        };
        if key.is_empty()
            || !key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        {
            return false;
        }
        if value != "absent" && (value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit())) {
            return false;
        }
    }
    true
}

/// Character count as a length. The characters are not kept.
pub fn char_len(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// Sample count as a length. The samples are not copied.
pub fn sample_len(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Monotonic clock quantized to the heartbeat. Not read from `step`.
#[derive(Clone, Copy, Debug)]
pub struct HeartbeatClock {
    origin: Instant,
}

impl HeartbeatClock {
    pub fn start() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// Milliseconds since [`Self::start`], rounded down to [`FRAME_MS`].
    /// The first frame window is `0`.
    pub fn now_ms(&self) -> u64 {
        let ms = u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX);
        ms - (ms % FRAME_MS)
    }
}

/// Test clock. Each [`Self::stamp`] advances one heartbeat.
#[derive(Clone, Copy, Debug, Default)]
pub struct VirtualHeartbeat {
    frame: u64,
}

impl VirtualHeartbeat {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    /// Next frame boundary in milliseconds. The first stamp is [`FRAME_MS`].
    pub fn stamp(&mut self) -> u64 {
        self.frame = self.frame.saturating_add(1);
        self.frame.saturating_mul(FRAME_MS)
    }
}

/// In-process stamp queue. `try_send` never waits. A full lane drops the oldest.
#[derive(Debug)]
pub struct StampLane {
    q: Mutex<VecDeque<TurnStamps>>,
}

impl StampLane {
    pub fn new() -> Self {
        Self {
            q: Mutex::new(VecDeque::new()),
        }
    }

    pub fn try_send(&self, sheet: TurnStamps) -> Result<(), ()> {
        let mut q = self.q.lock().unwrap_or_else(|e| e.into_inner());
        while q.len() >= STAMP_DEPTH {
            q.pop_front();
        }
        q.push_back(sheet);
        Ok(())
    }

    pub fn try_recv(&self) -> Option<TurnStamps> {
        self.q.lock().unwrap_or_else(|e| e.into_inner()).pop_front()
    }
}

impl Default for StampLane {
    fn default() -> Self {
        Self::new()
    }
}

/// Publish a dry sheet on [`StampLane`] and read it back.
/// The string is the phase-1 gate: zeros or `absent`, no PCM, no words.
pub fn dry_run_report() -> String {
    let lane = StampLane::new();
    let sheet = TurnStamps::dry();
    let _ = lane.try_send(sheet);
    lane.try_recv().unwrap_or_else(TurnStamps::dry).log()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_prints_absent_or_zero_and_no_words() {
        let log = dry_run_report();
        assert!(log_is_lengths_only(&log));
        assert!(log.contains("t_vad_silence_start=absent\n"));
        assert!(log.contains("t_eou_decided=absent\n"));
        assert!(log.contains("t_asr_final=absent\n"));
        assert!(log.contains("t_llm_req=absent\n"));
        assert!(log.contains("t_llm_first_token=absent\n"));
        assert!(log.contains("t_first_clause_flushed=absent\n"));
        assert!(log.contains("t_tts_first_pcm=absent\n"));
        assert!(log.contains("pcm_samples=0\n"));
        assert!(log.contains("text_chars=0\n"));
        assert!(log.contains("reply_chars=0\n"));
        assert!(log.contains("audio_samples=0\n"));
        assert!(log.contains("turn_gap_ms=absent\n"));
        assert!(!log.contains(' '));
        assert!(!log.contains("hello"));
        assert!(!log.contains(','));
    }

    #[test]
    fn a_word_or_a_pcm_dump_is_rejected() {
        assert!(!log_is_lengths_only("t_asr_final=hello galion\n"));
        assert!(!log_is_lengths_only("audio_samples=1, 2, 3\n"));
        assert!(!log_is_lengths_only("pcm=00ff 10\n"));
        assert!(log_is_lengths_only("t_tts_first_pcm=0\n"));
        assert!(log_is_lengths_only("t_tts_first_pcm=absent\n"));
    }

    #[test]
    fn virtual_clock_lands_on_heartbeat_frames() {
        let mut clock = VirtualHeartbeat::new();
        assert_eq!(clock.stamp(), FRAME_MS);
        assert_eq!(clock.stamp(), FRAME_MS * 2);
        assert_eq!(FRAME_MS, 80);
    }

    #[test]
    fn heartbeat_clock_is_quantized() {
        let clock = HeartbeatClock::start();
        let now = clock.now_ms();
        assert_eq!(now % FRAME_MS, 0);
    }

    #[test]
    fn turn_gap_needs_both_ends() {
        let mut sheet = TurnStamps::dry();
        assert_eq!(sheet.turn_gap_ms(), None);
        sheet.note_vad_silence_start(80);
        assert_eq!(sheet.turn_gap_ms(), None);
        sheet.t_tts_first_pcm = Some(400);
        assert_eq!(sheet.turn_gap_ms(), Some(320));
        sheet.t_tts_first_pcm = Some(40);
        assert_eq!(sheet.turn_gap_ms(), None);
        let log = sheet.log();
        assert!(log_is_lengths_only(&log));
        assert!(!log.contains("hello"));
    }

    #[test]
    fn lane_reads_what_it_was_sent_and_drops_the_oldest() {
        let lane = StampLane::new();
        for i in 0..STAMP_DEPTH + 1 {
            lane.try_send(TurnStamps::dry().with_turn_id(i as u64))
                .unwrap();
        }
        let first = lane.try_recv().unwrap();
        assert_eq!(first.turn_id, 1);
        assert!(lane.try_recv().is_some());
    }
}
