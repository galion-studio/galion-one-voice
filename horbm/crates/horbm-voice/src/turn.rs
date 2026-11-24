// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear, then the HORBM turn, then the mouth. One call is one conductor frame.

use crate::brain::TurnBrain;
use crate::clock::{VirtualClock, BARGE_IN_MAX_MS, FRAME_MS, PAUSE_MS};
use crate::device::{place, Place};
use crate::ear::{Ear, EarEvent};
use crate::lexicon::is_listed_backchannel;
use crate::mouth::{AudioFrame, Mouth, MouthPull};

/// What this frame decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickEvent {
    None,
    /// The ear heard speech start on this frame.
    Onset,
    EndOfSpeech,
    FirstAudio,
    Backchannel,
    BargeIn,
}

/// One frame of the voice path. `t_ms` is the virtual frame start.
#[derive(Debug)]
pub struct TickOut {
    pub t_ms: u64,
    pub event: TickEvent,
    pub cancel: bool,
    pub epoch: u64,
    pub me_silent: bool,
    pub speaking: bool,
    pub audio: Option<AudioFrame>,
    pub reply: Option<String>,
    pub place: Place,
}

/// The turn loop. `epoch` starts at 1, matching the conductor.
pub struct VoiceLoop<E, M, B> {
    ear: E,
    mouth: M,
    brain: B,
    clock: VirtualClock,
    place: Place,
    epoch: u64,
    speaking: bool,
    await_audio: bool,
    overlap_ms: u32,
    overlap_origin_ms: Option<u64>,
    silence_next: bool,
    eot_ms: Option<u64>,
    first_audio_ms: Option<u64>,
    barge_ms: Option<u64>,
    /// Mouth samples from the previous frame. The ear treats a copy as echo.
    played: Vec<i16>,
}

impl<E, M, B> VoiceLoop<E, M, B>
where
    E: Ear,
    M: Mouth,
    B: TurnBrain,
{
    pub fn new(ear: E, mouth: M, brain: B) -> Self {
        Self::with_place(ear, mouth, brain, place())
    }

    pub fn with_place(ear: E, mouth: M, brain: B, place: Place) -> Self {
        Self {
            ear,
            mouth,
            brain,
            clock: VirtualClock::new(),
            place,
            epoch: 1,
            speaking: false,
            await_audio: false,
            overlap_ms: 0,
            overlap_origin_ms: None,
            silence_next: false,
            eot_ms: None,
            first_audio_ms: None,
            barge_ms: None,
            played: Vec::new(),
        }
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    pub fn place(&self) -> Place {
        self.place
    }

    pub fn mouth(&self) -> &M {
        &self.mouth
    }

    pub fn ear_mut(&mut self) -> &mut E {
        &mut self.ear
    }

    /// Milliseconds from end of speech to the first mouth frame, on this clock.
    pub fn first_audio_after_eot_ms(&self) -> Option<u64> {
        Some(self.first_audio_ms?.saturating_sub(self.eot_ms?))
    }

    /// Milliseconds from the first overlapping frame to the silent frame.
    pub fn barge_in_ms(&self) -> Option<u64> {
        self.barge_ms
    }

    /// Advance one virtual frame. Does not read a wall clock.
    ///
    /// The previous mouth frame is handed to the ear first, so a copy of her
    /// own voice is echo. Then the ear, then the brain on a transcript, then
    /// one mouth frame.
    pub fn tick(&mut self, pcm: &[i16]) -> TickOut {
        let t_ms = self.clock.t_ms();
        if self.speaking {
            self.ear.note_playback(&self.played);
        } else {
            self.ear.note_playback(&[]);
        }
        let heard = self.ear.poll(pcm);
        if self.silence_next {
            let out = self.yield_floor(t_ms);
            self.clock.advance();
            return out;
        }

        let mut event = TickEvent::None;
        let mut reply = None;
        if self.on_ear(heard, t_ms, &mut event, &mut reply) {
            let out = self.yield_floor(t_ms);
            self.clock.advance();
            return out;
        }

        let mut audio = None;
        if self.speaking && self.await_audio {
            self.await_audio = false;
        } else if self.speaking {
            audio = self.pull_audio(t_ms, &mut event);
        }
        if let Some(frame) = &audio {
            self.played = frame.samples.clone();
        } else if !self.speaking {
            self.played.clear();
        }

        let out = TickOut {
            t_ms,
            event,
            cancel: false,
            epoch: self.epoch,
            me_silent: false,
            speaking: self.speaking,
            audio,
            reply,
            place: self.place,
        };
        self.clock.advance();
        out
    }

    /// `true` means yield the floor on this frame. The ear has already decided
    /// the overlap is a barge-in, so the silent frame is this one.
    fn on_ear(
        &mut self,
        heard: EarEvent,
        t_ms: u64,
        event: &mut TickEvent,
        reply: &mut Option<String>,
    ) -> bool {
        let (text, end) = match heard {
            EarEvent::Quiet | EarEvent::Fault(_) | EarEvent::User => return false,
            EarEvent::Onset => {
                if *event == TickEvent::None {
                    *event = TickEvent::Onset;
                }
                return false;
            }
            EarEvent::Backchannel => {
                self.overlap_ms = 0;
                self.overlap_origin_ms = None;
                if self.speaking && *event == TickEvent::None {
                    *event = TickEvent::Backchannel;
                }
                return false;
            }
            EarEvent::Barge => {
                if !self.speaking {
                    return false;
                }
                let back = u64::from(FRAME_MS) * 2;
                self.overlap_origin_ms = Some(t_ms.saturating_sub(back));
                self.overlap_ms = FRAME_MS.saturating_mul(3);
                return true;
            }
            EarEvent::Partial(text) => (text, false),
            EarEvent::End(text) => (text, true),
        };
        let text = text.trim();
        if text.is_empty() {
            if end && !self.speaking && *event == TickEvent::None {
                *event = TickEvent::EndOfSpeech;
                self.eot_ms = Some(t_ms);
            }
            return false;
        }
        if self.speaking {
            if is_listed_backchannel(text) {
                if *event == TickEvent::None {
                    *event = TickEvent::Backchannel;
                }
            } else {
                self.arm_overlap(t_ms);
            }
            return false;
        }
        if end {
            self.begin_turn(text, t_ms, event, reply);
        }
        false
    }

    fn begin_turn(
        &mut self,
        text: &str,
        t_ms: u64,
        event: &mut TickEvent,
        reply: &mut Option<String>,
    ) {
        let line = self.brain.turn(text);
        self.mouth.start(&line);
        self.eot_ms = Some(t_ms);
        self.first_audio_ms = None;
        self.overlap_ms = 0;
        self.overlap_origin_ms = None;
        self.silence_next = false;
        *reply = Some(line.text);
        *event = TickEvent::EndOfSpeech;
        if self.mouth.speaking() {
            self.speaking = true;
            self.await_audio = true;
        }
    }

    fn arm_overlap(&mut self, t_ms: u64) {
        if self.overlap_ms == 0 {
            self.overlap_origin_ms = Some(t_ms);
        }
        self.overlap_ms = self.overlap_ms.saturating_add(FRAME_MS);
        if self.overlap_ms.saturating_add(FRAME_MS) > PAUSE_MS {
            self.silence_next = true;
        }
    }

    fn pull_audio(&mut self, t_ms: u64, event: &mut TickEvent) -> Option<AudioFrame> {
        match self.mouth.pull() {
            MouthPull::Frame(frame) => {
                if self.first_audio_ms.is_none() {
                    self.first_audio_ms = Some(t_ms);
                    if *event == TickEvent::None {
                        *event = TickEvent::FirstAudio;
                    }
                }
                self.speaking = self.mouth.speaking();
                Some(frame)
            }
            MouthPull::Done | MouthPull::Idle => {
                if !self.mouth.speaking() {
                    self.speaking = false;
                }
                None
            }
        }
    }

    fn yield_floor(&mut self, t_ms: u64) -> TickOut {
        let origin = self.overlap_origin_ms.unwrap_or(t_ms);
        let barge_ms = t_ms.saturating_sub(origin);
        debug_assert!(barge_ms <= u64::from(BARGE_IN_MAX_MS));
        self.barge_ms = Some(barge_ms);
        self.mouth.cancel();
        self.speaking = false;
        self.await_audio = false;
        self.silence_next = false;
        self.overlap_ms = 0;
        self.overlap_origin_ms = None;
        self.played.clear();
        self.epoch = self.epoch.wrapping_add(1);
        TickOut {
            t_ms,
            event: TickEvent::BargeIn,
            cancel: true,
            epoch: self.epoch,
            me_silent: true,
            speaking: false,
            audio: None,
            reply: None,
            place: self.place,
        }
    }
}
