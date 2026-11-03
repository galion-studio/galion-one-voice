// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Two-stage endpointer (latency-240 lane 04).
//!
//! The old loop waited a fixed 160 ms of silence before doing anything. Here
//! the silence after speech drives two separate decisions:
//!
//! * `Speculate` after `spec_ms` of silence (default 60): start the LLM on
//!   the transcript we have. Cheap to undo: if voice comes back first we
//!   emit `Resume` and the caller cancels the request (one wasted 8b call).
//! * `Commit` after `commit_ms` (default 160, same as the old fixed hang, so
//!   cut-offs never get worse than before), or `hold_ms` (default 500)
//!   when the transcript ends on a word that usually means "not finished"
//!   ("and", "the", "um", ...). Only a committed turn may play audio, so a
//!   mid-sentence pause shorter than the commit window never cuts the user
//!   off. The commit wait runs while the LLM is already thinking, so on the
//!   happy path it costs nothing.
use crate::vad::{Vad, VadEv};

#[derive(Clone, Copy, Debug)]
pub struct EndpointCfg { pub spec_ms: f64, pub commit_ms: f64, pub hold_ms: f64 }
impl Default for EndpointCfg { fn default() -> Self { Self { spec_ms: 60.0, commit_ms: 160.0, hold_ms: 500.0 } } }
impl EndpointCfg {
    pub fn from_env() -> Self {
        let g = |k: &str, d: f64| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let d = Self::default();
        Self { spec_ms: g("GALION_EP_SPEC_MS", d.spec_ms), commit_ms: g("GALION_EP_COMMIT_MS", d.commit_ms), hold_ms: g("GALION_EP_HOLD_MS", d.hold_ms) }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum EpEv { None, Start, Speculate, Resume, Commit, Hold }

/// Words after which a pause is far more likely a breath than a turn end.
const UNFINISHED: &[&str] = &["and", "but", "or", "so", "because", "cause", "the", "a", "an", "to", "of", "in", "on", "at", "with",
    "for", "from", "my", "your", "his", "her", "their", "our", "um", "uh", "er", "like", "that", "which", "who", "if", "when",
    "while", "then", "is", "are", "was", "were", "i", "i'm", "we", "you", "it's", "this", "about", "into", "than", "as", "by", "not", "very"];
pub fn sounds_unfinished(transcript: &str) -> bool {
    match transcript.split_whitespace().last() { Some(w) => UNFINISHED.contains(&w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')), None => false }
}

pub struct Endpointer { pub vad: Vad, pub cfg: EndpointCfg, speculated: bool, held: bool }
impl Endpointer {
    pub fn new(cfg: EndpointCfg) -> Self { let mut vad = Vad::new(f64::INFINITY); vad.hang_ms = f64::INFINITY; Self { vad, cfg, speculated: false, held: false } }
    /// The commit window that applies to this transcript right now.
    pub fn commit_window(&self, transcript: &str) -> f64 { if sounds_unfinished(transcript) { self.cfg.hold_ms } else { self.cfg.commit_ms } }
    /// One 20 ms frame plus the transcript as of this frame (finals + partial).
    pub fn push(&mut self, frame: &[i16], frame_ms: f64, transcript: &str) -> EpEv {
        let was = self.vad.speaking;
        let ev = self.vad.push(frame, frame_ms);
        if ev == VadEv::Start { self.speculated = false; self.held = false; return EpEv::Start; }
        if !was && !self.vad.speaking { return EpEv::None; }
        if self.vad.last_voiced {
            self.held = false;
            if self.speculated { self.speculated = false; return EpEv::Resume; }
            return EpEv::None;
        }
        let s = self.vad.silent_ms;
        if s >= self.commit_window(transcript) {
            self.vad.end(); self.speculated = false; self.held = false;
            return EpEv::Commit;
        }
        // The streaming partial lags the last word by ~180 ms (p50), so an "unfinished"
        // ending may be stale: tell the caller once, so it can force the ASR final and
        // re-check with the real text (commit then fires on the next frame if it's finished).
        if !self.held && s >= self.cfg.commit_ms { self.held = true; return EpEv::Hold; }
        if !self.speculated && s >= self.cfg.spec_ms && !transcript.trim().is_empty() { self.speculated = true; return EpEv::Speculate; }
        EpEv::None
    }
}

/// Normalise for "did the transcript change" checks.
pub fn norm(s: &str) -> String { s.split_whitespace().map(|w| w.to_lowercase()).collect::<Vec<_>>().join(" ") }

#[cfg(test)]
mod t {
    use super::*;
    fn tone(n: usize) -> Vec<i16> { (0..n).map(|i| ((i as f64 * 0.2).sin() * 8000.0) as i16).collect() }
    #[test]
    fn speculate_then_commit() {
        let mut e = Endpointer::new(EndpointCfg::default());
        let mut evs = vec![];
        for _ in 0..20 { evs.push(e.push(&tone(320), 20.0, "hello there")); }
        for _ in 0..15 { evs.push(e.push(&vec![0; 320], 20.0, "hello there")); }
        let sp = evs.iter().position(|e| *e == EpEv::Speculate).unwrap();
        let cm = evs.iter().position(|e| *e == EpEv::Commit).unwrap();
        assert_eq!(sp, 20 + 2); // 60 ms of silence = 3rd silent frame
        assert_eq!(cm, 20 + 7); // 160 ms
    }
    #[test]
    fn resume_cancels_and_hold_extends() {
        let mut e = Endpointer::new(EndpointCfg::default());
        for _ in 0..20 { e.push(&tone(320), 20.0, "i went to the"); }
        let mut evs = vec![];
        for _ in 0..12 { evs.push(e.push(&vec![0; 320], 20.0, "i went to the")); }
        assert!(evs.contains(&EpEv::Speculate));
        assert!(!evs.contains(&EpEv::Commit), "unfinished transcript must hold past 160 ms");
        assert_eq!(evs.iter().filter(|e| **e == EpEv::Hold).count(), 1, "one Hold so the caller can re-check with the final");
        // once the forced final says the turn is finished, commit on the next silent frame
        assert_eq!(e.push(&vec![0; 320], 20.0, "i went to the shop"), EpEv::Commit);
        for _ in 0..20 { e.push(&tone(320), 20.0, "i went to the"); }
        for _ in 0..12 { e.push(&vec![0; 320], 20.0, "i went to the"); }
        assert_eq!(e.push(&tone(320), 20.0, "i went to the"), EpEv::Resume);
        assert!(sounds_unfinished("so I said um") && !sounds_unfinished("that was great"));
    }
}
