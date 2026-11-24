// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Domain 04 stamp names from phase 1 (PR #250). That module is not on `main`.
//!
//! Times are microseconds on the virtual clock, or absent. A missing stage
//! stays absent. The line carries counts, not a transcript and not PCM.
//! `mark` keeps the first write. Piper is not in this crate, so
//! `t_tts_first_pcm` stays absent. `t_first_audio_heard` belongs to an
//! external probe and stays absent.

use crate::clock::FRAME_MS;

/// One turn's stamp sheet. The log line is the phase 1 shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain04Stamps {
    pub t_vad_silence_start: Option<u64>,
    pub t_eou_decided: Option<u64>,
    pub t_asr_final: Option<u64>,
    pub t_llm_req: Option<u64>,
    pub t_llm_first_token: Option<u64>,
    pub t_first_clause_flushed: Option<u64>,
    pub t_tts_first_pcm: Option<u64>,
    pub t_first_media_frame_sent: Option<u64>,
    pub t_first_audio_heard: Option<u64>,
    pub turn_id: u64,
    pub frame_ms: u32,
    pub pcm_in_samples: u64,
    pub pcm_out_samples: u64,
    pub heard_chars: u32,
    pub reply_chars: u32,
}

impl Default for Domain04Stamps {
    fn default() -> Self {
        Self {
            t_vad_silence_start: None,
            t_eou_decided: None,
            t_asr_final: None,
            t_llm_req: None,
            t_llm_first_token: None,
            t_first_clause_flushed: None,
            t_tts_first_pcm: None,
            t_first_media_frame_sent: None,
            t_first_audio_heard: None,
            turn_id: 1,
            frame_ms: FRAME_MS,
            pcm_in_samples: 0,
            pcm_out_samples: 0,
            heard_chars: 0,
            reply_chars: 0,
        }
    }
}

impl Domain04Stamps {
    fn mark(slot: &mut Option<u64>, us: u64) {
        if slot.is_none() {
            *slot = Some(us);
        }
    }

    pub fn mark_silence(&mut self, us: u64) {
        Self::mark(&mut self.t_vad_silence_start, us);
    }

    pub fn mark_eou(&mut self, us: u64) {
        Self::mark(&mut self.t_eou_decided, us);
    }

    pub fn mark_asr(&mut self, us: u64) {
        Self::mark(&mut self.t_asr_final, us);
    }

    pub fn mark_llm_req(&mut self, us: u64) {
        Self::mark(&mut self.t_llm_req, us);
    }

    pub fn mark_llm_token(&mut self, us: u64) {
        Self::mark(&mut self.t_llm_first_token, us);
    }

    pub fn mark_clause(&mut self, us: u64) {
        Self::mark(&mut self.t_first_clause_flushed, us);
    }

    pub fn mark_media(&mut self, us: u64) {
        Self::mark(&mut self.t_first_media_frame_sent, us);
    }

    /// Drop the speculative times. The cell they belonged to is gone.
    pub fn clear_speculation(&mut self) {
        self.t_eou_decided = None;
        self.t_asr_final = None;
        self.t_llm_req = None;
        self.t_llm_first_token = None;
        self.t_first_clause_flushed = None;
        self.t_first_media_frame_sent = None;
        self.pcm_out_samples = 0;
        self.reply_chars = 0;
    }

    pub fn log_line(&self) -> String {
        format!(
            "domain04 turn_id={} frame_ms={} t_vad_silence_start={} t_eou_decided={} t_asr_final={} t_llm_req={} t_llm_first_token={} t_first_clause_flushed={} t_tts_first_pcm={} t_first_media_frame_sent={} t_first_audio_heard={} pcm_in_samples={} pcm_out_samples={} heard_chars={} reply_chars={}",
            self.turn_id,
            self.frame_ms,
            us(self.t_vad_silence_start),
            us(self.t_eou_decided),
            us(self.t_asr_final),
            us(self.t_llm_req),
            us(self.t_llm_first_token),
            us(self.t_first_clause_flushed),
            us(self.t_tts_first_pcm),
            us(self.t_first_media_frame_sent),
            us(self.t_first_audio_heard),
            self.pcm_in_samples,
            self.pcm_out_samples,
            self.heard_chars,
            self.reply_chars,
        )
    }
}

fn us(value: Option<u64>) -> String {
    match value {
        Some(n) => n.to_string(),
        None => "absent".to_string(),
    }
}

/// True when the line has the fixed keys and no leftover words.
pub fn stamp_log_is_lengths_only(line: &str) -> bool {
    const KEYS: &[&str] = &[
        "domain04",
        "turn_id",
        "frame_ms",
        "t_vad_silence_start",
        "t_eou_decided",
        "t_asr_final",
        "t_llm_req",
        "t_llm_first_token",
        "t_first_clause_flushed",
        "t_tts_first_pcm",
        "t_first_media_frame_sent",
        "t_first_audio_heard",
        "pcm_in_samples",
        "pcm_out_samples",
        "heard_chars",
        "reply_chars",
        "absent",
    ];
    let mut rest = line.to_string();
    for key in KEYS {
        rest = rest.replace(key, "");
    }
    rest.chars()
        .all(|ch| ch.is_ascii_digit() || ch == '=' || ch == ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_line_is_absent_times_and_zero_lengths() {
        let line = Domain04Stamps::default().log_line();
        assert!(line.contains("t_vad_silence_start=absent"));
        assert!(line.contains("t_first_audio_heard=absent"));
        assert!(line.contains("t_tts_first_pcm=absent"));
        assert!(line.contains("pcm_out_samples=0"));
        assert!(stamp_log_is_lengths_only(&line));
        assert!(!line.contains("RIFF"));
    }
}
