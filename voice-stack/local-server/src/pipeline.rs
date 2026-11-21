// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Energy on the mic moves `vad` and `dot`. Words come from a typed line or
//! from a scripted test ear. The unwired ear does not invent a transcript.
//! A turn is spoken only when [`crate::reply::may_speak_piper`] is true.
//! The mouth audio here is a hold tone at 24 kHz, not Piper.

use std::sync::Arc;
use std::time::Instant;

use crate::audio::{self, hold_tone, rms, MIC_RATE, SPEAKER_RATE};
use crate::protocol::{dot_json, error_json, turn_json, vad_json};
use crate::reply::{may_speak_piper, Pad, ReplyEngine, UnlinkedBrain};

const SPEECH_RMS: f32 = 0.02;
const START_FRAMES: u32 = 2;
/// 32 × 20 ms = 640 ms. Eight frames (160 ms) cut one question into two turns.
const END_FRAMES: u32 = 32;
const MAX_CARRY: usize = 48_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeInfo {
    pub ear: &'static str,
    pub mouth: &'static str,
    pub mind: String,
    pub vad_backend: &'static str,
    pub vad_ready: bool,
}

#[derive(Debug, Clone)]
pub struct TurnProduct {
    pub events: Vec<String>,
    pub pcm: Vec<i16>,
    pub rate: u32,
    pub reply: String,
}

impl TurnProduct {
    pub(crate) fn events(events: Vec<String>) -> Self {
        Self {
            events,
            pcm: Vec::new(),
            rate: SPEAKER_RATE,
            reply: String::new(),
        }
    }
}

pub trait Pipeline: Send {
    fn push_mic(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct;
    fn push_text(&mut self, text: &str, t_ms: u64) -> TurnProduct;
    /// One finished take. The default ear still needs a quiet tail to close.
    fn commit_pcm(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        let mut product = self.push_mic(pcm, rate, t_ms);
        if !product.pcm.is_empty()
            || product
                .events
                .iter()
                .any(|e| e.contains("\"type\":\"error\""))
        {
            return product;
        }
        let quiet = vec![0i16; audio::samples_per_frame(rate, 20).max(1)];
        for i in 0..END_FRAMES {
            let next = self.push_mic(&quiet, rate, t_ms.saturating_add(u64::from(i) * 20));
            if product.reply.is_empty() {
                product.reply = next.reply.clone();
            }
            product.events.extend(next.events);
            if !next.pcm.is_empty() {
                product.pcm = next.pcm;
                product.rate = next.rate;
                product.reply = next.reply;
            }
        }
        product
    }
    fn stop(&mut self);
    fn tick_mood(&mut self) -> (Pad, &'static str);
    fn phase(&self) -> &'static str;
    fn level(&self) -> f32;
    fn info(&self) -> PipeInfo;
}

#[derive(Debug, Clone)]
enum Ear {
    Unwired,
    Scripted(String),
}

struct Vad {
    carry: Vec<i16>,
    rate: u32,
    speech_run: u32,
    silence_run: u32,
    in_speech: bool,
    level: f32,
}

impl Vad {
    fn new() -> Self {
        Self {
            carry: Vec::new(),
            rate: MIC_RATE,
            speech_run: 0,
            silence_run: 0,
            in_speech: false,
            level: 0.0,
        }
    }

    fn reset(&mut self) {
        self.carry.clear();
        self.speech_run = 0;
        self.silence_run = 0;
        self.in_speech = false;
        self.level = 0.0;
    }
}

enum VadEvent {
    None,
    Started,
    Ended,
}

pub struct FakePipeline<R> {
    reply: R,
    ear: Ear,
    vad: Vad,
    phase: &'static str,
}

impl FakePipeline<UnlinkedBrain> {
    pub fn unlinked() -> Self {
        Self::new(UnlinkedBrain, Ear::Unwired)
    }
}

impl<R: ReplyEngine> FakePipeline<R> {
    pub fn scripted(line: impl Into<String>, reply: R) -> Self {
        Self::new(reply, Ear::Scripted(line.into()))
    }

    fn new(reply: R, ear: Ear) -> Self {
        Self {
            reply,
            ear,
            vad: Vad::new(),
            phase: "idle",
        }
    }

    fn on_frame(&mut self, frame: &[i16]) -> VadEvent {
        let level = rms(frame);
        self.vad.level = level.clamp(0.0, 1.0);
        let hot = level >= SPEECH_RMS;
        if hot {
            self.vad.silence_run = 0;
            self.vad.speech_run = self.vad.speech_run.saturating_add(1);
            if !self.vad.in_speech && self.vad.speech_run >= START_FRAMES {
                self.vad.in_speech = true;
                return VadEvent::Started;
            }
        } else {
            self.vad.speech_run = 0;
            if self.vad.in_speech {
                self.vad.silence_run = self.vad.silence_run.saturating_add(1);
                if self.vad.silence_run >= END_FRAMES {
                    self.vad.in_speech = false;
                    self.vad.silence_run = 0;
                    return VadEvent::Ended;
                }
            }
        }
        VadEvent::None
    }

    fn speak(&mut self, text: &str) -> TurnProduct {
        let text = text.trim();
        if text.is_empty() {
            self.phase = "idle";
            return TurnProduct::events(vec![
                error_json("empty-text"),
                dot_json("idle", self.vad.level),
            ]);
        }
        let started = Instant::now();
        match self.reply.turn(text) {
            Err(code) => {
                self.phase = "idle";
                TurnProduct::events(vec![error_json(&code), dot_json("idle", self.vad.level)])
            }
            Ok(mut out) => {
                if out.took_us == 0 {
                    out.took_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
                }
                out.mood = out.mood.finite_or_resting();
                let reply = out.reply.clone();
                let mut events = vec![turn_json(&out)];
                let pcm = if may_speak_piper(&out.reply, out.violations) {
                    self.phase = "speaking";
                    events.push(dot_json("speaking", self.vad.level));
                    hold_tone(out.mood.valence)
                } else {
                    self.phase = "idle";
                    events.push(dot_json("idle", self.vad.level));
                    Vec::new()
                };
                TurnProduct {
                    events,
                    pcm,
                    rate: SPEAKER_RATE,
                    reply,
                }
            }
        }
    }
}

impl<R: ReplyEngine> Pipeline for FakePipeline<R> {
    fn push_mic(&mut self, pcm: &[i16], rate: u32, t_ms: u64) -> TurnProduct {
        if rate != 16_000 && rate != 24_000 {
            return TurnProduct::events(vec![error_json("bad-rate")]);
        }
        if self.vad.rate != rate {
            self.vad.carry.clear();
            self.vad.rate = rate;
        }
        if pcm.len() > MAX_CARRY {
            self.vad.reset();
            self.phase = "idle";
            return TurnProduct::events(vec![error_json("bad-frame")]);
        }
        self.vad.carry.extend_from_slice(pcm);
        if self.vad.carry.len() > MAX_CARRY {
            self.vad.reset();
            self.phase = "idle";
            return TurnProduct::events(vec![error_json("bad-frame")]);
        }
        let frame_n = audio::samples_per_frame(rate, 20);
        let mut events = Vec::new();
        let mut ended = false;
        while frame_n > 0 && self.vad.carry.len() >= frame_n {
            let frame: Vec<i16> = self.vad.carry.drain(..frame_n).collect();
            match self.on_frame(&frame) {
                VadEvent::Started => {
                    self.phase = "listening";
                    events.push(vad_json("start", t_ms));
                    events.push(dot_json("listening", self.vad.level));
                }
                VadEvent::Ended => ended = true,
                VadEvent::None => {}
            }
        }
        if !ended {
            return TurnProduct::events(events);
        }
        events.push(vad_json("end", t_ms));
        let text = match &self.ear {
            Ear::Unwired => String::new(),
            Ear::Scripted(line) => line.clone(),
        };
        if text.is_empty() {
            self.phase = "idle";
            events.push(dot_json("idle", self.vad.level));
            return TurnProduct::events(events);
        }
        let mut produced = self.speak(&text);
        events.append(&mut produced.events);
        produced.events = events;
        produced
    }

    fn push_text(&mut self, text: &str, _t_ms: u64) -> TurnProduct {
        self.speak(text)
    }

    fn stop(&mut self) {
        self.vad.reset();
        self.phase = "idle";
    }

    fn tick_mood(&mut self) -> (Pad, &'static str) {
        match self.reply.tick() {
            Some(pad) => (pad.finite_or_resting(), "brain"),
            None => (Pad::RESTING, "resting"),
        }
    }

    fn phase(&self) -> &'static str {
        self.phase
    }

    fn level(&self) -> f32 {
        self.vad.level
    }

    fn info(&self) -> PipeInfo {
        let ear = match self.ear {
            Ear::Unwired => "unwired",
            Ear::Scripted(_) => "scripted",
        };
        PipeInfo {
            ear,
            mouth: "hold",
            mind: self.reply.name().to_string(),
            vad_backend: "energy",
            vad_ready: true,
        }
    }
}

pub fn unlinked_factory() -> Arc<dyn Fn() -> Box<dyn Pipeline> + Send + Sync> {
    Arc::new(|| Box::new(FakePipeline::unlinked()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reply::{Pad, ReplyEngine, TurnOut};

    struct Fixed;

    impl ReplyEngine for Fixed {
        fn name(&self) -> &str {
            "fixed"
        }
        fn turn(&mut self, _heard: &str) -> Result<TurnOut, String> {
            Ok(TurnOut {
                reply: "The fence is loose.".into(),
                intent_json: "null".into(),
                arm_json: "null".into(),
                propensity: 0.4,
                evidence: vec!["card:fixture".into()],
                sources: vec!["fixture".into()],
                violations: 0,
                lookup_us: 7,
                took_us: 3,
                mood: Pad::RESTING,
            })
        }
    }

    fn loud() -> Vec<i16> {
        vec![8000; 320]
    }
    fn quiet() -> Vec<i16> {
        vec![0; 320]
    }

    #[test]
    fn scripted_ear_speaks_without_claiming_recognition() {
        let mut pipe = FakePipeline::scripted("hello from the mic", Fixed);
        assert!(pipe.push_mic(&loud(), 16_000, 0).pcm.is_empty());
        let started = pipe.push_mic(&loud(), 16_000, 20);
        assert!(started
            .events
            .iter()
            .any(|e| e.contains("\"type\":\"vad\"") && e.contains("start")));
        let mut last = TurnProduct::events(vec![]);
        for _ in 0..END_FRAMES {
            last = pipe.push_mic(&quiet(), 16_000, 40);
        }
        assert!(!last.pcm.is_empty());
        assert_eq!(last.rate, SPEAKER_RATE);
        assert!(last.events.iter().any(|e| e.contains("\"type\":\"turn\"")));
        assert!(last
            .events
            .iter()
            .any(|e| e.contains("The fence is loose.")));
        assert!(last
            .events
            .iter()
            .all(|e| !e.contains("\"type\":\"final\"")));
        let (pad, source) = pipe.tick_mood();
        assert_eq!(source, "resting");
        assert_eq!(pad, Pad::RESTING);
    }

    #[test]
    fn a_160_ms_pause_does_not_end_the_turn() {
        let mut pipe = FakePipeline::scripted("hello from the mic", Fixed);
        pipe.push_mic(&loud(), 16_000, 0);
        pipe.push_mic(&loud(), 16_000, 20);
        for _ in 0..8 {
            let mid = pipe.push_mic(&quiet(), 16_000, 40);
            assert!(mid.pcm.is_empty());
            assert!(mid.events.iter().all(|e| !e.contains("\"event\":\"end\"")));
        }
        assert_eq!(pipe.phase(), "listening");
    }

    #[test]
    fn unwired_endpoint_does_not_call_the_brain() {
        let mut pipe = FakePipeline::unlinked();
        pipe.push_mic(&loud(), 16_000, 0);
        pipe.push_mic(&loud(), 16_000, 20);
        let mut last = TurnProduct::events(vec![]);
        for _ in 0..END_FRAMES {
            last = pipe.push_mic(&quiet(), 16_000, 40);
        }
        assert!(last.pcm.is_empty());
        assert!(last.events.iter().all(|e| !e.contains("brain-unlinked")));
        assert!(last.events.iter().any(|e| e.contains("\"event\":\"end\"")));
    }

    #[test]
    fn unlinked_text_is_an_error_code() {
        let mut pipe = FakePipeline::unlinked();
        let out = pipe.push_text("hello", 0);
        assert!(out.pcm.is_empty());
        assert!(out.events.iter().any(|e| e.contains("brain-unlinked")));
        assert!(out.events.iter().all(|e| !e.contains("hello")));
    }
}
