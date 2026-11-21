// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The seam in front of HORBM, and the two engines this sector can run
//! without it.
//!
//! [`ReplyEngine::turn`] is the shape of `Brain::turn`. [`FakeReplyEngine`]
//! does not read audio and does not invent a transcript. [`MockEngine`]
//! walks the dot through idle, listening, thinking, and speaking so the
//! page can be tried with no brain. [`PassiveEngine`] keeps the clock and
//! the mic level and never writes a reply.

use std::time::Instant;

use crate::protocol::{MoodSource, Pad, PcmFrame, ServerMsg, TurnOut, VoiceState, TICK_MS};

/// Quiet after speech, same window as the phone page (`VAD_END_MS`).
pub const QUIET_MS: u32 = 640;
/// How long thinking lasts in the preview before the fake reply.
pub const THINK_MS: u32 = 640;
/// How long the preview holds speaking. Not a measured mouth.
pub const SPEAK_MS: u32 = 2_400;
/// Phone page `MAX_UTT_MS`. A stuck tone cannot hold the preview open.
pub const MAX_UTTERANCE_MS: u32 = 15_000;
/// Level at or above this counts as voice. The page uses the same floor.
pub const VOICE_RMS: f32 = 0.02;
/// No speech yet: advance the preview after this much quiet.
pub const LISTEN_CAP_MS: u32 = 1_600;

pub trait ReplyEngine {
    fn turn(&mut self, heard: &str) -> TurnOut;
}

/// A stand-in so tests and the preview have a `TurnOut`.
/// The real `horbm::Brain` is not linked. Empty `heard` is the greeting
/// from `docs/galion-voice-v2/personas/SAMANTHA.md`. Any other text is
/// named as a preview, not answered as if it had been understood.
pub struct FakeReplyEngine;

impl ReplyEngine for FakeReplyEngine {
    fn turn(&mut self, heard: &str) -> TurnOut {
        let started = Instant::now();
        let (reply, intent) = if heard.trim().is_empty() {
            ("Hi. I'm here.", "greet")
        } else {
            ("This preview has no brain yet.", "preview")
        };
        let took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        TurnOut {
            reply: reply.to_string(),
            intent: intent.to_string(),
            arm: "fake".to_string(),
            propensity: 0.0,
            evidence: Vec::new(),
            sources: vec!["fake:reply-engine".to_string()],
            violations: 0,
            lookup_us: 0,
            took_us,
            mood: Pad::RESTING,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Listening,
    Thinking,
    Speaking { left_ms: u32 },
}

/// Shared by the mock (a resting preview reply) and the passive wire
/// (resting mood, no reply). Mood on this clock is `source: "resting"`.
/// It does not drift.
pub struct SessionEngine<E> {
    brain: E,
    mock: bool,
    t_ms: u32,
    phase: Phase,
    level: f32,
    quiet_ms: u32,
    listen_ms: u32,
    think_ms: u32,
    voice: bool,
    pcm_since_tick: bool,
    pending_reply: Option<TurnOut>,
}

pub type MockEngine = SessionEngine<FakeReplyEngine>;
pub type PassiveEngine = SessionEngine<FakeReplyEngine>;

impl SessionEngine<FakeReplyEngine> {
    pub fn mock() -> Self {
        Self::with_brain(FakeReplyEngine, true)
    }

    pub fn passive() -> Self {
        Self::with_brain(FakeReplyEngine, false)
    }
}

impl<E: ReplyEngine> SessionEngine<E> {
    /// `mock_replies` asks the brain for a line when thinking ends.
    /// The passive wire passes `false` and stays in thinking.
    pub fn with_brain(brain: E, mock_replies: bool) -> Self {
        Self {
            brain,
            mock: mock_replies,
            t_ms: 0,
            phase: Phase::Idle,
            level: 0.0,
            quiet_ms: 0,
            listen_ms: 0,
            think_ms: 0,
            voice: false,
            pcm_since_tick: false,
            pending_reply: None,
        }
    }

    pub fn on_hello(&mut self) -> Vec<ServerMsg> {
        vec![ServerMsg::Welcome]
    }

    pub fn on_stop(&mut self) -> Vec<ServerMsg> {
        self.phase = Phase::Idle;
        self.level = 0.0;
        self.voice = false;
        self.pending_reply = None;
        vec![ServerMsg::Dot {
            state: VoiceState::Idle,
            level: 0.0,
        }]
    }

    /// A typed line. The architecture calls this `text`, not recognition.
    pub fn on_text(&mut self) -> Vec<ServerMsg> {
        self.begin_think();
        self.outbound(false)
    }

    pub fn on_pcm(&mut self, frame: &PcmFrame) -> Vec<ServerMsg> {
        if matches!(self.phase, Phase::Idle) {
            self.phase = Phase::Listening;
            self.quiet_ms = 0;
            self.listen_ms = 0;
            self.voice = false;
        }
        self.note_level(frame.rms());
        if matches!(self.phase, Phase::Listening) {
            vec![ServerMsg::Dot {
                state: VoiceState::Listening,
                level: self.level,
            }]
        } else {
            Vec::new()
        }
    }

    pub fn note_level(&mut self, rms: f32) {
        self.level = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.pcm_since_tick = true;
        if self.level >= VOICE_RMS {
            self.voice = true;
        }
    }

    pub fn on_tick(&mut self) -> Vec<ServerMsg> {
        self.t_ms = self.t_ms.saturating_add(TICK_MS);
        if !self.pcm_since_tick && matches!(self.phase, Phase::Listening) {
            self.level = 0.0;
        }
        self.pcm_since_tick = false;
        self.advance();
        let dot = !matches!(self.phase, Phase::Thinking);
        self.outbound(dot)
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Listening => {
                self.listen_ms = self.listen_ms.saturating_add(TICK_MS);
                if self.listen_ms >= MAX_UTTERANCE_MS {
                    self.begin_think();
                    return;
                }
                if self.voice {
                    if self.level >= VOICE_RMS {
                        self.quiet_ms = 0;
                    } else {
                        self.quiet_ms = self.quiet_ms.saturating_add(TICK_MS);
                        if self.quiet_ms >= QUIET_MS || self.listen_ms >= MAX_UTTERANCE_MS {
                            self.begin_think();
                        }
                    }
                } else if self.level == 0.0 && self.listen_ms >= LISTEN_CAP_MS {
                    self.phase = Phase::Idle;
                    self.level = 0.0;
                }
            }
            Phase::Thinking => {
                self.think_ms = self.think_ms.saturating_add(TICK_MS);
                if self.think_ms >= THINK_MS {
                    self.begin_speak();
                }
            }
            Phase::Speaking { left_ms } => {
                if left_ms <= TICK_MS {
                    self.phase = Phase::Idle;
                    self.level = 0.0;
                } else {
                    let left = left_ms - TICK_MS;
                    let elapsed = SPEAK_MS - left;
                    self.level = speak_env(elapsed);
                    self.phase = Phase::Speaking { left_ms: left };
                }
            }
            Phase::Idle => self.level = 0.0,
        }
    }

    fn begin_think(&mut self) {
        if matches!(self.phase, Phase::Thinking | Phase::Speaking { .. }) {
            return;
        }
        self.phase = Phase::Thinking;
        self.think_ms = 0;
        self.level = 0.0;
    }

    fn begin_speak(&mut self) {
        if !self.mock {
            // The brain is absent. Stay in thinking. Do not invent a line.
            self.level = 0.0;
            return;
        }
        let out = self.brain.turn("");
        self.pending_reply = Some(out);
        self.level = speak_env(0);
        self.phase = Phase::Speaking { left_ms: SPEAK_MS };
    }

    fn state(&self) -> VoiceState {
        match self.phase {
            Phase::Idle => VoiceState::Idle,
            Phase::Listening => VoiceState::Listening,
            Phase::Thinking => VoiceState::Thinking,
            Phase::Speaking { .. } => VoiceState::Speaking,
        }
    }

    fn outbound(&mut self, include_dot: bool) -> Vec<ServerMsg> {
        let mut out = vec![ServerMsg::Mood {
            t_ms: self.t_ms,
            source: MoodSource::Resting,
            pad: Pad::RESTING,
        }];
        if include_dot {
            out.push(ServerMsg::Dot {
                state: self.state(),
                level: self.level,
            });
        }
        if let Some(turn) = self.pending_reply.take() {
            out.push(ServerMsg::Turn {
                reply: turn.reply,
                sources: turn.sources,
                pad: Pad::RESTING,
            });
        }
        out
    }
}

fn speak_env(elapsed_ms: u32) -> f32 {
    let k = (elapsed_ms as f32 / SPEAK_MS as f32).clamp(0.0, 1.0);
    (k * std::f32::consts::PI).sin()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AudioDir, RateCode};

    #[test]
    fn fake_greeting_is_short_and_timed() {
        let mut brain = FakeReplyEngine;
        let out = brain.turn("");
        assert_eq!(out.reply, "Hi. I'm here.");
        assert_eq!(out.sources, vec!["fake:reply-engine".to_string()]);
        assert_eq!(out.mood, Pad::RESTING);
        assert_eq!(out.violations, 0);
        assert!(out.took_us < 1_000_000, "{}", out.took_us);
        let named = brain.turn("hello");
        assert_eq!(named.reply, "This preview has no brain yet.");
        assert!(named.reply.len() < 80);
    }

    fn loud() -> PcmFrame {
        PcmFrame {
            dir: AudioDir::Mic,
            rate: RateCode::Hz16k,
            seq: 0,
            samples: vec![16_000; 16],
        }
    }

    fn has_turn(msgs: &[ServerMsg]) -> Option<&str> {
        msgs.iter().find_map(|m| match m {
            ServerMsg::Turn { reply, .. } => Some(reply.as_str()),
            _ => None,
        })
    }

    #[test]
    fn text_reaches_the_greeting_after_the_think_window() {
        let mut eng = MockEngine::mock();
        assert!(matches!(eng.on_hello()[0], ServerMsg::Welcome));
        eng.on_text();
        assert!(matches!(eng.phase, Phase::Thinking));
        for i in 1..=7 {
            let msgs = eng.on_tick();
            assert!(
                matches!(eng.phase, Phase::Thinking),
                "tick {i} {:?}",
                eng.phase
            );
            assert!(has_turn(&msgs).is_none(), "tick {i}");
        }
        let spoke = eng.on_tick();
        assert!(matches!(eng.phase, Phase::Speaking { .. }));
        assert_eq!(has_turn(&spoke), Some("Hi. I'm here."));
    }

    #[test]
    fn voice_then_quiet_uses_the_640_ms_window() {
        let mut eng = MockEngine::mock();
        for _ in 0..30 {
            eng.on_pcm(&loud());
            eng.on_tick();
            assert!(matches!(eng.phase, Phase::Listening));
        }
        for _ in 0..7 {
            eng.note_level(0.0);
            eng.on_tick();
            assert!(matches!(eng.phase, Phase::Listening));
        }
        eng.on_tick();
        assert!(matches!(eng.phase, Phase::Thinking));
    }

    #[test]
    fn text_enters_thinking_without_waiting() {
        let mut eng = MockEngine::mock();
        eng.on_text();
        assert!(matches!(eng.phase, Phase::Thinking));
    }

    #[test]
    fn passive_engine_never_writes_a_reply() {
        let mut eng = PassiveEngine::passive();
        eng.on_text();
        assert!(matches!(eng.phase, Phase::Thinking));
        for _ in 0..40 {
            let msgs = eng.on_tick();
            assert!(has_turn(&msgs).is_none());
            assert!(!matches!(eng.phase, Phase::Speaking { .. }));
            assert!(msgs.iter().any(|m| matches!(
                m,
                ServerMsg::Mood {
                    source: MoodSource::Resting,
                    pad: Pad::RESTING,
                    ..
                }
            )));
        }
    }

    #[test]
    fn stop_returns_to_idle() {
        let mut eng = MockEngine::mock();
        eng.on_pcm(&loud());
        let ev = eng.on_stop();
        assert!(matches!(eng.phase, Phase::Idle));
        assert!(ev.iter().any(|m| matches!(
            m,
            ServerMsg::Dot {
                state: VoiceState::Idle,
                ..
            }
        )));
    }
}
