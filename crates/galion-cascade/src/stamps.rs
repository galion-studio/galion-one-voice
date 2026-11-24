// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Domain 04 turn stamps (phase 1 of the HORBM CPU plan).
//!
//! One sheet per turn. Every time is microseconds from a [`StampClock`], or
//! absent when that stage did not run. The log line is lengths and timings
//! only: no PCM, no transcript, no reply words.
//!
//! [`Heartbeat::step`](galion_heartbeat::Heartbeat::step) is not called.
//! `frame_ms` is the locked 80 ms quantum so a reader can turn microseconds
//! into frames. The stamps themselves stay on the monotonic clock.
//!
//! `t_vad_silence_start` stays absent until a caller that saw the silence
//! calls [`Domain04Stamps::mark`]. The phone lane does not see the VAD hang.
//! `t_first_media_frame_sent` and `t_first_audio_heard` stay absent here.
//! The bus publishes PCM later, and the heard-audio probe is outside this
//! process.

#![forbid(unsafe_code)]

use galion_heartbeat::FRAME_MS;
use std::time::Instant;

const _: () = assert!(FRAME_MS == 80);

/// Monotonic origin for one process or one lane. `Instant` is the portable
/// clock (Windows and Unix). It is not wall time and it is not a frame index.
#[derive(Clone, Copy, Debug)]
pub struct StampClock {
    origin: Instant,
}

impl StampClock {
    pub fn start() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    /// Microseconds since [`Self::start`]. Saturates instead of wrapping.
    pub fn us(self) -> u64 {
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

impl Default for StampClock {
    fn default() -> Self {
        Self::start()
    }
}

/// The domain 04 points, in the order a turn records them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StampPoint {
    VadSilenceStart,
    EouDecided,
    AsrFinal,
    LlmReq,
    LlmFirstToken,
    FirstClauseFlushed,
    TtsFirstPcm,
    FirstMediaFrameSent,
    FirstAudioHeard,
}

/// Lengths and monotonic times for one `turn_id`.
///
/// Times are microseconds from the lane's [`StampClock`], or `None` when the
/// stage did not run. `None` prints as `absent`. A zero is a real sample of
/// the clock, not a missing stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain04Stamps {
    pub turn_id: u64,
    pub t_vad_silence_start: Option<u64>,
    pub t_eou_decided: Option<u64>,
    pub t_asr_final: Option<u64>,
    pub t_llm_req: Option<u64>,
    pub t_llm_first_token: Option<u64>,
    pub t_first_clause_flushed: Option<u64>,
    pub t_tts_first_pcm: Option<u64>,
    pub t_first_media_frame_sent: Option<u64>,
    pub t_first_audio_heard: Option<u64>,
    /// Input samples the cascade accepted. Not the samples.
    pub pcm_in_samples: u64,
    /// Output samples handed to the WAV wrapper. Not the samples.
    pub pcm_out_samples: u64,
    /// Characters accepted as the heard line. Not the characters.
    pub heard_chars: u32,
    /// Characters in the spoken reply. Not the characters.
    pub reply_chars: u32,
}

impl Domain04Stamps {
    /// No stage has run. Lengths are zero. `turn_id` is kept.
    pub fn absent(turn_id: u64) -> Self {
        Self {
            turn_id,
            t_vad_silence_start: None,
            t_eou_decided: None,
            t_asr_final: None,
            t_llm_req: None,
            t_llm_first_token: None,
            t_first_clause_flushed: None,
            t_tts_first_pcm: None,
            t_first_media_frame_sent: None,
            t_first_audio_heard: None,
            pcm_in_samples: 0,
            pcm_out_samples: 0,
            heard_chars: 0,
            reply_chars: 0,
        }
    }

    /// The phase-1 dry run. No audio, no model, no clock sample.
    pub fn dry() -> Self {
        Self::absent(0)
    }

    pub fn is_dry(&self) -> bool {
        self.turn_id == 0
            && self.t_vad_silence_start.is_none()
            && self.t_eou_decided.is_none()
            && self.t_asr_final.is_none()
            && self.t_llm_req.is_none()
            && self.t_llm_first_token.is_none()
            && self.t_first_clause_flushed.is_none()
            && self.t_tts_first_pcm.is_none()
            && self.t_first_media_frame_sent.is_none()
            && self.t_first_audio_heard.is_none()
            && self.pcm_in_samples == 0
            && self.pcm_out_samples == 0
            && self.heard_chars == 0
            && self.reply_chars == 0
    }

    /// Record `us` the first time this point is marked. A later mark is ignored.
    pub fn mark(&mut self, point: StampPoint, us: u64) {
        let slot = self.slot_mut(point);
        if slot.is_none() {
            *slot = Some(us);
        }
    }

    fn slot_mut(&mut self, point: StampPoint) -> &mut Option<u64> {
        match point {
            StampPoint::VadSilenceStart => &mut self.t_vad_silence_start,
            StampPoint::EouDecided => &mut self.t_eou_decided,
            StampPoint::AsrFinal => &mut self.t_asr_final,
            StampPoint::LlmReq => &mut self.t_llm_req,
            StampPoint::LlmFirstToken => &mut self.t_llm_first_token,
            StampPoint::FirstClauseFlushed => &mut self.t_first_clause_flushed,
            StampPoint::TtsFirstPcm => &mut self.t_tts_first_pcm,
            StampPoint::FirstMediaFrameSent => &mut self.t_first_media_frame_sent,
            StampPoint::FirstAudioHeard => &mut self.t_first_audio_heard,
        }
    }

    /// True when the present times are in domain 04 order. Absent points are
    /// skipped. Equal microseconds are in order.
    pub fn timings_non_decreasing(&self) -> bool {
        let seq = [
            self.t_vad_silence_start,
            self.t_eou_decided,
            self.t_asr_final,
            self.t_llm_req,
            self.t_llm_first_token,
            self.t_first_clause_flushed,
            self.t_tts_first_pcm,
            self.t_first_media_frame_sent,
            self.t_first_audio_heard,
        ];
        let mut prev: Option<u64> = None;
        for v in seq.into_iter().flatten() {
            if let Some(p) = prev {
                if v < p {
                    return false;
                }
            }
            prev = Some(v);
        }
        true
    }

    /// One line. Keys are fixed. Values are `absent` or an integer.
    pub fn log_line(&self) -> String {
        let mut s = String::with_capacity(360);
        s.push_str("domain04");
        push_num(&mut s, "turn_id", self.turn_id);
        push_num(&mut s, "frame_ms", u64::from(FRAME_MS));
        push_us(&mut s, "t_vad_silence_start", self.t_vad_silence_start);
        push_us(&mut s, "t_eou_decided", self.t_eou_decided);
        push_us(&mut s, "t_asr_final", self.t_asr_final);
        push_us(&mut s, "t_llm_req", self.t_llm_req);
        push_us(&mut s, "t_llm_first_token", self.t_llm_first_token);
        push_us(
            &mut s,
            "t_first_clause_flushed",
            self.t_first_clause_flushed,
        );
        push_us(&mut s, "t_tts_first_pcm", self.t_tts_first_pcm);
        push_us(
            &mut s,
            "t_first_media_frame_sent",
            self.t_first_media_frame_sent,
        );
        push_us(&mut s, "t_first_audio_heard", self.t_first_audio_heard);
        push_num(&mut s, "pcm_in_samples", self.pcm_in_samples);
        push_num(&mut s, "pcm_out_samples", self.pcm_out_samples);
        push_num(&mut s, "heard_chars", u64::from(self.heard_chars));
        push_num(&mut s, "reply_chars", u64::from(self.reply_chars));
        s
    }
}

impl Default for Domain04Stamps {
    fn default() -> Self {
        Self::dry()
    }
}

/// Dry-run line. Does not open a microphone, a model, a socket, or a file.
pub fn dry_run_log() -> String {
    Domain04Stamps::dry().log_line()
}

const TIME_KEYS: &[&str] = &[
    "t_vad_silence_start",
    "t_eou_decided",
    "t_asr_final",
    "t_llm_req",
    "t_llm_first_token",
    "t_first_clause_flushed",
    "t_tts_first_pcm",
    "t_first_media_frame_sent",
    "t_first_audio_heard",
];

const LEN_KEYS: &[&str] = &[
    "turn_id",
    "frame_ms",
    "pcm_in_samples",
    "pcm_out_samples",
    "heard_chars",
    "reply_chars",
];

/// True when `line` is one domain 04 stamp line: fixed keys, and every value
/// is `absent` or an unsigned integer. A transcript word, a `RIFF` header,
/// or a sample list fails.
pub fn stamp_log_clean(line: &str) -> bool {
    if line.is_empty() || line.contains('\n') || line.contains('\r') {
        return false;
    }
    if line.contains("RIFF") || line.contains("WAVE") {
        return false;
    }
    let mut parts = line.split_whitespace();
    if parts.next() != Some("domain04") {
        return false;
    }
    let mut seen_time = 0usize;
    let mut seen_len = 0usize;
    for part in parts {
        let Some((key, value)) = part.split_once('=') else {
            return false;
        };
        if key.is_empty() || value.is_empty() || part.matches('=').count() != 1 {
            return false;
        }
        if TIME_KEYS.contains(&key) {
            seen_time += 1;
            if value != "absent" && !digits(value) {
                return false;
            }
        } else if LEN_KEYS.contains(&key) {
            seen_len += 1;
            if !digits(value) {
                return false;
            }
        } else {
            return false;
        }
    }
    seen_time == TIME_KEYS.len() && seen_len == LEN_KEYS.len()
}

fn digits(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}

fn push_num(out: &mut String, key: &str, n: u64) {
    out.push(' ');
    out.push_str(key);
    out.push('=');
    out.push_str(&n.to_string());
}

fn push_us(out: &mut String, key: &str, us: Option<u64>) {
    out.push(' ');
    out.push_str(key);
    out.push('=');
    match us {
        Some(n) => out.push_str(&n.to_string()),
        None => out.push_str("absent"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain04_dry_run_prints_absent_or_zero_and_no_pcm_or_words() {
        let sheet = Domain04Stamps::dry();
        assert!(sheet.is_dry());
        let log = dry_run_log();
        assert_eq!(log, sheet.log_line());
        assert!(stamp_log_clean(&log), "{log}");
        for key in TIME_KEYS {
            assert!(log.contains(&format!("{key}=absent")), "{log}");
        }
        assert!(log.contains("turn_id=0"), "{log}");
        assert!(log.contains("frame_ms=80"), "{log}");
        assert!(log.contains("pcm_in_samples=0"), "{log}");
        assert!(log.contains("pcm_out_samples=0"), "{log}");
        assert!(log.contains("heard_chars=0"), "{log}");
        assert!(log.contains("reply_chars=0"), "{log}");
        assert!(!log.contains("RIFF"), "{log}");
        assert!(!log.contains("WAVE"), "{log}");
        assert!(!log.contains("hello"), "{log}");
        assert!(!log.contains("galion"), "{log}");
        assert!(!log.contains(','), "{log}");
        assert!(sheet.timings_non_decreasing());
    }

    #[test]
    fn stamp_log_rejects_words_and_pcm() {
        assert!(!stamp_log_clean("domain04 turn_id=0 reply=hello"));
        assert!(!stamp_log_clean(&format!("{} RIFF", dry_run_log())));
        assert!(!stamp_log_clean("hello galion"));
        assert!(!stamp_log_clean("domain04 turn_id=1 t_asr_final=hi"));
        let mut poisoned = dry_run_log();
        poisoned.push_str(" heard=hello galion");
        assert!(!stamp_log_clean(&poisoned));
    }

    #[test]
    fn scripted_times_are_lengths_only_and_ordered() {
        let mut sheet = Domain04Stamps::absent(9);
        let order = [
            StampPoint::VadSilenceStart,
            StampPoint::EouDecided,
            StampPoint::AsrFinal,
            StampPoint::LlmReq,
            StampPoint::LlmFirstToken,
            StampPoint::FirstClauseFlushed,
            StampPoint::TtsFirstPcm,
            StampPoint::FirstMediaFrameSent,
            StampPoint::FirstAudioHeard,
        ];
        let mut t = 1_000u64;
        for point in order {
            sheet.mark(point, t);
            t += 1_000;
        }
        sheet.mark(StampPoint::AsrFinal, 99_000);
        assert_eq!(sheet.t_asr_final, Some(3_000));
        sheet.pcm_in_samples = 16_000;
        sheet.pcm_out_samples = 24_000;
        sheet.heard_chars = 12;
        sheet.reply_chars = 28;
        let log = sheet.log_line();
        assert!(stamp_log_clean(&log), "{log}");
        assert!(sheet.timings_non_decreasing());
        assert!(log.contains("turn_id=9"), "{log}");
        assert!(log.contains("t_vad_silence_start=1000"), "{log}");
        assert!(log.contains("t_tts_first_pcm=7000"), "{log}");
        assert!(log.contains("t_first_audio_heard=9000"), "{log}");
        assert!(log.contains("pcm_in_samples=16000"), "{log}");
        assert!(log.contains("heard_chars=12"), "{log}");
        assert!(log.contains("reply_chars=28"), "{log}");
        assert!(!log.contains("hello"), "{log}");
        assert!(!log.contains("galion"), "{log}");
        assert!(!sheet.is_dry());
    }

    #[test]
    fn out_of_order_times_fail_the_order_check() {
        let mut sheet = Domain04Stamps::dry();
        sheet.mark(StampPoint::TtsFirstPcm, 1);
        sheet.mark(StampPoint::AsrFinal, 5);
        assert!(!sheet.timings_non_decreasing());
        assert!(stamp_log_clean(&sheet.log_line()));
    }

    #[test]
    fn stamps_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Domain04Stamps>();
        assert_send_sync::<StampClock>();
    }
}
