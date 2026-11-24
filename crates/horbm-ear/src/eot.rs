// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! End-of-turn from the ear's prosody and the ASR partial.
//!
//! Pitch is already [`crate::ProsodyFrame::f0_hz`] from `pitch-detection`'s
//! YIN detector. This module does not detect pitch again and it does not
//! load a model. `voice-stack/vad` already ends turns from a pause plus the
//! trailing word, but that crate links `ort` and a Silero weight, which this
//! tree does not take. The rules below are the glue: thresholds live in
//! `eot.toml`, compiled in with `include_str!`.
//!
//! A denied frame ([`crate::FrameOut::Denied`], which is what [`crate::Ear::deny`]
//! and `GALION_EAR_SENSE=0|off|deny` produce) returns [`TurnMsg::PauseOnly`].
//! The conductor and the voice loop then keep the pause-only path they
//! already had. While sensing is on, a filler or an unfinished word holds
//! the floor through a short pause, and a finished word yields on a falling
//! contour with an energy drop, or on a rising question contour.

use std::collections::VecDeque;

use crate::{FrameOut, ProsodyFrame};

/// What one frame tells the turn taker. No audio, no transcript.
///
/// `galion-conductor` and `horbm-voice` are separate workspaces, so each
/// keeps a copy of these three variants and the host maps one to the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnMsg {
    /// Sensing is off. The caller keeps its pause-only rule.
    PauseOnly,
    /// The user still holds the floor.
    Hold,
    /// The user has finished. `question` is a rising terminal contour.
    Yield { question: bool },
}

/// The numbers the scorer reads. One row of `eot.toml`.
#[derive(Clone, Debug, PartialEq)]
pub struct EotConfig {
    pub fall_semitones: f32,
    pub strong_fall_semitones: f32,
    pub rise_semitones: f32,
    pub energy_ratio: f32,
    pub yield_pause_ms: u32,
    pub mild_pause_ms: u32,
    pub hold_pause_ms: u32,
    pub min_voiced: usize,
    pub trail: usize,
    pub pace_min_hz: f32,
    pub pace_drop_ratio: f32,
    fillers: Vec<String>,
    unfinished: Vec<String>,
}

/// Prosody the scorer needs. Copied off a [`ProsodyFrame`]. No PCM.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProsodyCue {
    pub voiced: bool,
    pub f0_hz: f32,
    pub energy: f32,
    pub speaking_rate_hz: f32,
    pub pause_ms: u32,
}

impl From<&ProsodyFrame> for ProsodyCue {
    fn from(frame: &ProsodyFrame) -> Self {
        Self {
            voiced: frame.voiced,
            f0_hz: frame.f0_hz,
            energy: frame.energy,
            speaking_rate_hz: frame.speaking_rate_hz,
            pause_ms: frame.pause_ms,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Voiced {
    f0: f32,
    energy: f32,
    rate: f32,
}

/// Rolling contour over the last voiced frames. Not shared across threads.
#[derive(Clone, Debug)]
pub struct Scorer {
    cfg: EotConfig,
    tail: VecDeque<Voiced>,
    /// Pause length of the latest unvoiced frame. A new phrase clears the tail.
    gap_ms: u32,
}

impl EotConfig {
    /// The thresholds shipped beside this crate.
    pub fn bundled() -> Self {
        Self::parse(include_str!("../eot.toml")).expect("eot.toml parses")
    }

    /// Parse the flat TOML this crate ships. Unknown keys and missing keys fail.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut fall_semitones = None;
        let mut strong_fall_semitones = None;
        let mut rise_semitones = None;
        let mut energy_ratio = None;
        let mut yield_pause_ms = None;
        let mut mild_pause_ms = None;
        let mut hold_pause_ms = None;
        let mut min_voiced = None;
        let mut trail = None;
        let mut pace_min_hz = None;
        let mut pace_drop_ratio = None;
        let mut fillers = None;
        let mut unfinished = None;

        for (lineno, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            let (key, value) = split_eq(line, lineno)?;
            match key {
                "fall_semitones" => fall_semitones = Some(parse_f32(value, lineno, key)?),
                "strong_fall_semitones" => {
                    strong_fall_semitones = Some(parse_f32(value, lineno, key)?);
                }
                "rise_semitones" => rise_semitones = Some(parse_f32(value, lineno, key)?),
                "energy_ratio" => energy_ratio = Some(parse_f32(value, lineno, key)?),
                "yield_pause_ms" => yield_pause_ms = Some(parse_u32(value, lineno, key)?),
                "mild_pause_ms" => mild_pause_ms = Some(parse_u32(value, lineno, key)?),
                "hold_pause_ms" => hold_pause_ms = Some(parse_u32(value, lineno, key)?),
                "min_voiced" => min_voiced = Some(parse_u32(value, lineno, key)? as usize),
                "trail" => trail = Some(parse_u32(value, lineno, key)? as usize),
                "pace_min_hz" => pace_min_hz = Some(parse_f32(value, lineno, key)?),
                "pace_drop_ratio" => pace_drop_ratio = Some(parse_f32(value, lineno, key)?),
                "fillers" => fillers = Some(parse_words(value, lineno, key)?),
                "unfinished" => unfinished = Some(parse_words(value, lineno, key)?),
                other => {
                    return Err(format!("line {}: unknown key {other}", lineno + 1));
                }
            }
        }

        let cfg = Self {
            fall_semitones: need(fall_semitones, "fall_semitones")?,
            strong_fall_semitones: need(strong_fall_semitones, "strong_fall_semitones")?,
            rise_semitones: need(rise_semitones, "rise_semitones")?,
            energy_ratio: need(energy_ratio, "energy_ratio")?,
            yield_pause_ms: need(yield_pause_ms, "yield_pause_ms")?,
            mild_pause_ms: need(mild_pause_ms, "mild_pause_ms")?,
            hold_pause_ms: need(hold_pause_ms, "hold_pause_ms")?,
            min_voiced: need(min_voiced, "min_voiced")?,
            trail: need(trail, "trail")?,
            pace_min_hz: need(pace_min_hz, "pace_min_hz")?,
            pace_drop_ratio: need(pace_drop_ratio, "pace_drop_ratio")?,
            fillers: need(fillers, "fillers")?,
            unfinished: need(unfinished, "unfinished")?,
        };
        cfg.check()?;
        Ok(cfg)
    }

    fn check(&self) -> Result<(), String> {
        if !(self.fall_semitones > 0.0 && self.strong_fall_semitones > self.fall_semitones) {
            return Err("strong_fall_semitones must be greater than fall_semitones".into());
        }
        if self.rise_semitones <= 0.0 {
            return Err("rise_semitones must be positive".into());
        }
        if !(self.energy_ratio > 0.0 && self.energy_ratio <= 1.0) {
            return Err("energy_ratio must be in (0, 1]".into());
        }
        if !(self.yield_pause_ms > 0
            && self.mild_pause_ms >= self.yield_pause_ms
            && self.hold_pause_ms > self.mild_pause_ms)
        {
            return Err("pauses must rise from yield to mild to hold".into());
        }
        if self.min_voiced < 2 || self.trail < self.min_voiced {
            return Err("trail must be at least min_voiced, and min_voiced at least 2".into());
        }
        if self.pace_min_hz <= 0.0 || !(self.pace_drop_ratio > 0.0 && self.pace_drop_ratio < 1.0) {
            return Err("pace_min_hz must be positive and pace_drop_ratio in (0, 1)".into());
        }
        if self.fillers.is_empty() || self.unfinished.is_empty() {
            return Err("fillers and unfinished must name at least one word".into());
        }
        Ok(())
    }

    fn holds(&self, word: &str) -> bool {
        self.fillers.iter().any(|w| w == word) || self.unfinished.iter().any(|w| w == word)
    }
}

impl Scorer {
    pub fn new(cfg: EotConfig) -> Self {
        Self {
            cfg,
            tail: VecDeque::new(),
            gap_ms: 0,
        }
    }

    pub fn bundled() -> Self {
        Self::new(EotConfig::bundled())
    }

    pub fn config(&self) -> &EotConfig {
        &self.cfg
    }

    /// One ear result plus the latest ASR partial.
    ///
    /// [`FrameOut::Denied`] clears the contour and returns [`TurnMsg::PauseOnly`].
    /// A rejected buffer holds and leaves the contour as it was.
    pub fn push(&mut self, frame: FrameOut, partial: &str) -> TurnMsg {
        match frame {
            FrameOut::Denied => {
                self.tail.clear();
                self.gap_ms = 0;
                TurnMsg::PauseOnly
            }
            FrameOut::Rejected => TurnMsg::Hold,
            FrameOut::Heard(frame) => self.push_cue(ProsodyCue::from(&frame), partial),
        }
    }

    /// Same rules as [`Self::push`] for a cue the host already measured.
    /// Sensing is on. A denied ear never reaches here.
    pub fn push_cue(&mut self, cue: ProsodyCue, partial: &str) -> TurnMsg {
        if cue.voiced {
            if cue.f0_hz.is_finite() && cue.f0_hz > 0.0 && cue.energy.is_finite() {
                if self.gap_ms >= self.cfg.yield_pause_ms {
                    self.tail.clear();
                }
                self.tail.push_back(Voiced {
                    f0: cue.f0_hz,
                    energy: cue.energy.max(0.0),
                    rate: if cue.speaking_rate_hz.is_finite() {
                        cue.speaking_rate_hz.max(0.0)
                    } else {
                        0.0
                    },
                });
                while self.tail.len() > self.cfg.trail {
                    self.tail.pop_front();
                }
            }
            self.gap_ms = 0;
            // The word is still sounding. Yield waits for the pause.
            return TurnMsg::Hold;
        }
        self.gap_ms = cue.pause_ms;
        self.decide(partial, cue.pause_ms)
    }

    fn decide(&self, partial: &str, pause_ms: u32) -> TurnMsg {
        if partial.trim().is_empty() {
            return TurnMsg::Hold;
        }
        let word = last_word(partial);
        let holding = word.as_deref().is_some_and(|w| self.cfg.holds(w));
        if holding && pause_ms < self.cfg.hold_pause_ms {
            return TurnMsg::Hold;
        }
        if holding && pause_ms >= self.cfg.hold_pause_ms {
            return TurnMsg::Yield { question: false };
        }
        if word.is_none() || self.tail.len() < self.cfg.min_voiced {
            return TurnMsg::Hold;
        }

        let early = self.tail.front().map(|s| s.f0).unwrap_or(0.0);
        let late = self.tail.back().copied().unwrap_or(Voiced {
            f0: 0.0,
            energy: 0.0,
            rate: 0.0,
        });
        let semis = semitones(early, late.f0);
        let peak = self
            .tail
            .iter()
            .map(|s| s.energy)
            .fold(0.0f32, f32::max)
            .max(1.0e-6);
        let energy_drop = late.energy / peak <= self.cfg.energy_ratio;
        let pace_drop = pace_dropped(&self.tail, self.cfg.pace_min_hz, self.cfg.pace_drop_ratio);
        let strong = semis <= -self.cfg.strong_fall_semitones && energy_drop;
        let mild = semis <= -self.cfg.fall_semitones && energy_drop && !pace_drop;
        let question = semis >= self.cfg.rise_semitones;

        if strong && pause_ms >= self.cfg.yield_pause_ms {
            return TurnMsg::Yield { question: false };
        }
        if question && pause_ms >= self.cfg.yield_pause_ms {
            return TurnMsg::Yield { question: true };
        }
        if mild && pause_ms >= self.cfg.mild_pause_ms {
            return TurnMsg::Yield { question: false };
        }
        if pause_ms >= self.cfg.hold_pause_ms {
            return TurnMsg::Yield { question: false };
        }
        TurnMsg::Hold
    }
}

fn pace_dropped(tail: &VecDeque<Voiced>, min_hz: f32, ratio: f32) -> bool {
    let Some(early) = tail.front() else {
        return false;
    };
    let Some(late) = tail.back() else {
        return false;
    };
    early.rate >= min_hz && late.rate <= early.rate * ratio
}

fn semitones(early_hz: f32, late_hz: f32) -> f32 {
    if early_hz <= 1.0 || late_hz <= 1.0 {
        return 0.0;
    }
    12.0 * (late_hz / early_hz).ln() / std::f32::consts::LN_2
}

fn last_word(partial: &str) -> Option<String> {
    let raw = partial.split_whitespace().last()?;
    let trimmed = raw.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '\'');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_ascii_lowercase())
    }
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(at) => &line[..at],
        None => line,
    }
}

fn split_eq(line: &str, lineno: usize) -> Result<(&str, &str), String> {
    let Some((key, value)) = line.split_once('=') else {
        return Err(format!("line {}: expected key = value", lineno + 1));
    };
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(format!("line {}: empty key or value", lineno + 1));
    }
    Ok((key, value))
}

fn parse_f32(value: &str, lineno: usize, key: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .ok()
        .filter(|n| n.is_finite())
        .ok_or_else(|| format!("line {}: {key} is not a number", lineno + 1))
}

fn parse_u32(value: &str, lineno: usize, key: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("line {}: {key} is not a whole number", lineno + 1))
}

fn parse_words(value: &str, lineno: usize, key: &str) -> Result<Vec<String>, String> {
    let inner = value
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("line {}: {key} must be a one-line list", lineno + 1))?;
    let mut words = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let word = part
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or_else(|| format!("line {}: {key} entries must be quoted", lineno + 1))?;
        if word.is_empty() {
            return Err(format!("line {}: empty word in {key}", lineno + 1));
        }
        words.push(word.to_ascii_lowercase());
    }
    if words.is_empty() {
        return Err(format!("line {}: {key} is empty", lineno + 1));
    }
    Ok(words)
}

fn need<T>(value: Option<T>, key: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing {key}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(voiced: bool, f0: f32, energy: f32, rate: f32, pause_ms: u32) -> ProsodyCue {
        ProsodyCue {
            voiced,
            f0_hz: f0,
            energy,
            speaking_rate_hz: rate,
            pause_ms,
        }
    }

    fn quiet(pause_ms: u32) -> ProsodyCue {
        cue(false, 0.0, 0.0, 0.0, pause_ms)
    }

    #[test]
    fn bundled_thresholds_match_the_shipped_file() {
        let cfg = EotConfig::bundled();
        assert_eq!(cfg.yield_pause_ms, 80);
        assert_eq!(cfg.mild_pause_ms, 160);
        assert_eq!(cfg.hold_pause_ms, 720);
        assert!(cfg.holds("um"));
        assert!(cfg.holds("the"));
        assert!(!cfg.holds("ready"));
        assert!(EotConfig::parse("fall_semitones = 1.0\n").is_err());
    }

    #[test]
    fn a_filler_holds_through_a_short_pause_even_when_the_pitch_falls() {
        let mut scorer = Scorer::bundled();
        let partial = "I was going to um...";
        for (f0, energy) in [(180.0, 0.2), (160.0, 0.12), (140.0, 0.06), (120.0, 0.04)] {
            assert_eq!(
                scorer.push_cue(cue(true, f0, energy, 3.0, 0), partial),
                TurnMsg::Hold
            );
        }
        for pause in [80, 160, 240, 400, 640] {
            assert_eq!(
                scorer.push_cue(quiet(pause), partial),
                TurnMsg::Hold,
                "pause {pause}"
            );
        }
    }

    #[test]
    fn a_mild_fall_waits_and_a_pace_drop_holds_that_fall() {
        let partial = "the kettle is ready";
        let mut mild = Scorer::bundled();
        for (f0, energy) in [(200.0, 0.20), (193.0, 0.16), (186.0, 0.12), (180.0, 0.08)] {
            assert_eq!(
                mild.push_cue(cue(true, f0, energy, 3.0, 0), partial),
                TurnMsg::Hold
            );
        }
        assert_eq!(mild.push_cue(quiet(80), partial), TurnMsg::Hold);
        assert_eq!(
            mild.push_cue(quiet(160), partial),
            TurnMsg::Yield { question: false }
        );

        let mut slowed = Scorer::bundled();
        for (f0, energy, rate) in [
            (200.0, 0.20, 4.0),
            (193.0, 0.16, 4.0),
            (186.0, 0.12, 3.5),
            (180.0, 0.08, 1.2),
        ] {
            slowed.push_cue(cue(true, f0, energy, rate, 0), partial);
        }
        assert_eq!(slowed.push_cue(quiet(160), partial), TurnMsg::Hold);
        assert_eq!(slowed.push_cue(quiet(400), partial), TurnMsg::Hold);
    }

    #[test]
    fn a_denied_frame_is_pause_only_and_drops_the_contour() {
        let mut scorer = Scorer::bundled();
        scorer.push_cue(cue(true, 200.0, 0.2, 3.0, 0), "ready");
        assert_eq!(scorer.push(FrameOut::Denied, "ready"), TurnMsg::PauseOnly);
        assert!(scorer.tail.is_empty());
        assert_eq!(scorer.push(FrameOut::Rejected, "ready"), TurnMsg::Hold);
    }
}
