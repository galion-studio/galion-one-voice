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
    EndOfSpeech,
    FirstAudio,
    Backchannel,
    BargeIn,
}

/// End-of-turn message from the ear prosody scorer.
///
/// The same three variants live on `horbm_ear::TurnMsg` and on
/// `galion_conductor::TurnMsg`. This crate does not link the ear.
/// [`TurnMsg::PauseOnly`] is [`VoiceLoop::tick`]: only an ASR final starts
/// the reply. [`TurnMsg::Hold`] does not start it, even on a final, so a
/// filler pause is not a turn. [`TurnMsg::Yield`] starts it from the latest
/// partial on this frame, before a late final arrives.
///
/// Barge-in does not read this message. Overlap while the mouth is speaking
/// still yields on the 200 ms lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnMsg {
    PauseOnly,
    Hold,
    Yield { question: bool },
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
    /// True when this frame's yield was a rising terminal contour.
    pub end_question: bool,
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
    last_partial: String,
    frame_question: bool,
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
            last_partial: String::new(),
            frame_question: false,
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

    /// Advance one virtual frame. Pause-only: an ASR final ends the turn.
    /// Does not read a wall clock.
    pub fn tick(&mut self, pcm: &[i16]) -> TickOut {
        self.tick_turn(pcm, TurnMsg::PauseOnly)
    }

    /// [`Self::tick`] with an end-of-turn message.
    ///
    /// A yield starts the brain from the latest partial. The mouth still
    /// waits one frame before the first samples, the same gap a final uses.
    /// Hold leaves the mouth idle. Overlap after the mouth has started does
    /// not consult the message.
    pub fn tick_turn(&mut self, pcm: &[i16], turn: TurnMsg) -> TickOut {
        let t_ms = self.clock.t_ms();
        let heard = self.ear.poll(pcm);
        if self.silence_next {
            let out = self.yield_floor(t_ms);
            self.clock.advance();
            return out;
        }

        let mut event = TickEvent::None;
        let mut reply = None;
        self.frame_question = false;
        self.on_ear(heard, t_ms, turn, &mut event, &mut reply);

        let mut audio = None;
        if self.speaking && self.await_audio {
            self.await_audio = false;
        } else if self.speaking {
            audio = self.pull_audio(t_ms, &mut event);
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
            end_question: self.frame_question,
        };
        self.clock.advance();
        out
    }

    fn on_ear(
        &mut self,
        heard: EarEvent,
        t_ms: u64,
        turn: TurnMsg,
        event: &mut TickEvent,
        reply: &mut Option<String>,
    ) {
        if let EarEvent::Partial(ref text) | EarEvent::End(ref text) = heard {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                self.last_partial = trimmed.to_string();
            }
        }
        if self.speaking {
            let text = match &heard {
                EarEvent::Quiet | EarEvent::Fault(_) => return,
                EarEvent::Partial(text) | EarEvent::End(text) => text.trim(),
            };
            if text.is_empty() {
                return;
            }
            if is_listed_backchannel(text) {
                if *event == TickEvent::None {
                    *event = TickEvent::Backchannel;
                }
            } else {
                self.arm_overlap(t_ms);
            }
            return;
        }
        match turn {
            TurnMsg::Hold => {}
            TurnMsg::PauseOnly => {
                if let EarEvent::End(text) = &heard {
                    let text = text.trim();
                    if !text.is_empty() {
                        self.begin_turn(text, t_ms, event, reply);
                    }
                }
            }
            TurnMsg::Yield { question } => {
                let owned = match &heard {
                    EarEvent::Fault(_) => return,
                    EarEvent::Partial(text) | EarEvent::End(text) => text.trim().to_string(),
                    EarEvent::Quiet => self.last_partial.clone(),
                };
                if owned.is_empty() {
                    return;
                }
                self.frame_question = question;
                self.begin_turn(&owned, t_ms, event, reply);
            }
        }
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
            end_question: false,
        }
    }
}
