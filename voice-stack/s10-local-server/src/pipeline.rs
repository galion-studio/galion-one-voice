// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! The server-side pipeline. Energy on the mic moves the phase. Words come
//! from a typed line, or from a scripted test ear. The unwired ear does not
//! invent a transcript. The mouth here is a hold tone, not Piper.

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;

use crate::audio::{hold_tone, rms, MIC_FRAME_SAMPLES, MIC_RATE, TTS_RATE};
use crate::mood::MoodClock;
use crate::protocol::{error_json, heard_json, reply_json, state_json};
use crate::reply::{FakeReply, ReplyEngine};

const SPEECH_RMS: f32 = 0.02;
const START_FRAMES: u32 = 2;
const END_FRAMES: u32 = 8;
/// One second of mic audio. Past this the carry is dropped.
const MAX_CARRY: usize = MIC_RATE as usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeInfo {
    pub ear: &'static str,
    pub mouth: &'static str,
    pub mind: String,
}

#[derive(Debug, Clone)]
pub struct TurnProduct {
    pub events: Vec<Value>,
    pub pcm: Vec<i16>,
    pub rate: u32,
}

impl TurnProduct {
    fn events(events: Vec<Value>) -> Self {
        Self {
            events,
            pcm: Vec::new(),
            rate: TTS_RATE,
        }
    }
}

pub trait Pipeline: Send {
    fn push_mic(&mut self, pcm: &[i16]) -> TurnProduct;
    fn push_text(&mut self, text: &str) -> TurnProduct;
    fn barge(&mut self);
    fn tick_mood(&mut self) -> (crate::reply::Pad, u64);
    fn phase(&self) -> &'static str;
    fn info(&self) -> PipeInfo;
}

#[derive(Debug, Clone)]
enum Ear {
    Unwired,
    /// Repeats `line` at each endpoint. It does not recognise speech.
    Scripted(String),
}

#[derive(Debug)]
struct Vad {
    carry: Vec<i16>,
    speech_run: u32,
    silence_run: u32,
    in_speech: bool,
}

impl Vad {
    fn new() -> Self {
        Self {
            carry: Vec::new(),
            speech_run: 0,
            silence_run: 0,
            in_speech: false,
        }
    }

    fn reset(&mut self) {
        self.carry.clear();
        self.speech_run = 0;
        self.silence_run = 0;
        self.in_speech = false;
    }
}

enum VadEvent {
    None,
    Started,
    Ended,
}

/// Fake pipeline used by the binary (unwired ear) and by the integration test
/// (scripted ear). `R` is the [`ReplyEngine`].
pub struct FakePipeline<R> {
    reply: R,
    ear: Ear,
    vad: Vad,
    mood: MoodClock,
    phase: &'static str,
}

impl FakePipeline<FakeReply> {
    pub fn unwired(reply: FakeReply) -> Self {
        Self::new(reply, Ear::Unwired)
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
            mood: MoodClock::resting(),
            phase: "idle",
        }
    }

    fn on_frame(&mut self, frame: &[i16]) -> VadEvent {
        let hot = rms(frame) >= SPEECH_RMS;
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

    fn speak(&mut self, text: &str, ear_name: &str) -> TurnProduct {
        let text = text.trim();
        self.phase = "thinking";
        let mut events = vec![state_json("thinking", None), heard_json(text, ear_name)];
        if text.is_empty() {
            self.phase = "idle";
            events.push(state_json("idle", Some("empty")));
            return TurnProduct::events(events);
        }
        let started = Instant::now();
        let mut out = self.reply.turn(text);
        let wall_us = u32::try_from(started.elapsed().as_micros()).unwrap_or(u32::MAX);
        if out.took_us == 0 {
            out.took_us = wall_us;
        }
        out.mood = out.mood.sanitised();
        self.mood.set_target(out.mood);
        let tone_at = Instant::now();
        let pcm = if out.reply.is_empty() {
            Vec::new()
        } else {
            hold_tone(out.mood.valence)
        };
        let mouth_us = u32::try_from(tone_at.elapsed().as_micros()).unwrap_or(u32::MAX);
        events.push(reply_json(&out, "hold", mouth_us));
        self.phase = "idle";
        TurnProduct {
            events,
            pcm,
            rate: TTS_RATE,
        }
    }
}

impl<R: ReplyEngine> Pipeline for FakePipeline<R> {
    fn push_mic(&mut self, pcm: &[i16]) -> TurnProduct {
        if pcm.len() > MAX_CARRY {
            self.vad.reset();
            self.phase = "idle";
            return TurnProduct::events(vec![error_json("mic frame too large")]);
        }
        self.vad.carry.extend_from_slice(pcm);
        if self.vad.carry.len() > MAX_CARRY {
            self.vad.reset();
            self.phase = "idle";
            return TurnProduct::events(vec![error_json("mic buffer overflow")]);
        }
        let mut events = Vec::new();
        let mut ended = false;
        while self.vad.carry.len() >= MIC_FRAME_SAMPLES {
            let frame: Vec<i16> = self.vad.carry.drain(..MIC_FRAME_SAMPLES).collect();
            match self.on_frame(&frame) {
                VadEvent::Started => {
                    self.phase = "listening";
                    events.push(state_json("listening", None));
                }
                VadEvent::Ended => ended = true,
                VadEvent::None => {}
            }
        }
        if !ended {
            return TurnProduct::events(events);
        }
        let (text, ear_name) = match &self.ear {
            Ear::Unwired => (String::new(), "unwired"),
            Ear::Scripted(line) => (line.clone(), "scripted"),
        };
        let mut produced = self.speak(&text, ear_name);
        events.append(&mut produced.events);
        produced.events = events;
        produced
    }

    fn push_text(&mut self, text: &str) -> TurnProduct {
        let text = text.trim();
        if text.is_empty() {
            return TurnProduct::events(vec![error_json("empty text")]);
        }
        self.speak(text, "text")
    }

    fn barge(&mut self) {
        self.vad.reset();
        self.phase = "idle";
    }

    fn tick_mood(&mut self) -> (crate::reply::Pad, u64) {
        let pad = self.mood.step();
        (pad, self.mood.tick)
    }

    fn phase(&self) -> &'static str {
        self.phase
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
        }
    }
}

pub fn unwired_factory() -> Arc<dyn Fn() -> Box<dyn Pipeline> + Send + Sync> {
    Arc::new(|| Box::new(FakePipeline::unwired(FakeReply)))
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
        fn turn(&mut self, heard: &str) -> TurnOut {
            TurnOut {
                reply: format!("Okay. {heard}"),
                intent: "chat".into(),
                arm: "fake".into(),
                propensity: 0.4,
                evidence: vec!["card:fixture".into()],
                sources: vec!["fixture".into()],
                violations: 0,
                lookup_us: 7,
                took_us: 3,
                mood: Pad {
                    arousal: 0.8,
                    valence: 0.7,
                    dominance: 0.6,
                },
            }
        }
    }

    fn loud() -> Vec<i16> {
        vec![8000; MIC_FRAME_SAMPLES]
    }

    fn quiet() -> Vec<i16> {
        vec![0; MIC_FRAME_SAMPLES]
    }

    #[test]
    fn scripted_ear_endpoints_into_a_reply() {
        let mut pipe = FakePipeline::scripted("hello from the mic", Fixed);
        let first = pipe.push_mic(&loud());
        assert!(first.pcm.is_empty());
        let second = pipe.push_mic(&loud());
        assert_eq!(second.events[0]["state"], "listening");
        assert!(second.pcm.is_empty());
        let mut last = TurnProduct::events(vec![]);
        for _ in 0..END_FRAMES {
            last = pipe.push_mic(&quiet());
        }
        assert!(!last.pcm.is_empty());
        assert_eq!(
            last.pcm.len(),
            crate::audio::TTS_FRAME_SAMPLES * crate::audio::HOLD_SLICES
        );
        let heard = last
            .events
            .iter()
            .find(|e| e["type"] == "heard")
            .expect("heard");
        assert_eq!(heard["text"], "hello from the mic");
        assert_eq!(heard["recognized"], false);
        let reply = last.events.iter().find(|e| e["type"] == "reply").unwrap();
        assert_eq!(reply["text"], "Okay. hello from the mic");
        assert_eq!(reply["mouth"], "hold");
        assert_eq!(reply["sources"][0], "fixture");
        assert_eq!(pipe.mood.target.arousal, 0.8);
    }

    #[test]
    fn unwired_ear_does_not_invent_words() {
        let mut pipe = FakePipeline::unwired(FakeReply);
        pipe.push_mic(&loud());
        pipe.push_mic(&loud());
        let mut last = TurnProduct::events(vec![]);
        for _ in 0..END_FRAMES {
            last = pipe.push_mic(&quiet());
        }
        assert!(last.pcm.is_empty());
        let heard = last.events.iter().find(|e| e["type"] == "heard").unwrap();
        assert_eq!(heard["text"], "");
        assert_eq!(heard["recognized"], false);
        assert!(last.events.iter().all(|e| e["type"] != "reply"));
    }

    #[test]
    fn barge_resets_vad() {
        let mut pipe = FakePipeline::scripted("x", Fixed);
        pipe.push_mic(&loud());
        pipe.push_mic(&loud());
        assert_eq!(pipe.phase(), "listening");
        pipe.barge();
        assert_eq!(pipe.phase(), "idle");
        let quiet_only = pipe.push_mic(&quiet());
        assert!(quiet_only.pcm.is_empty());
    }
}
