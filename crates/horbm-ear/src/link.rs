// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One capture frame, from the ear into the emotion core and the conductor.
//!
//! The conductor's ear is text. This layer has no words, so a voiced frame
//! is the fixed mark [`ACTIVITY_MARK`] and a quiet frame is `Quiet`. The
//! mark is not a transcript and it does not carry pitch or samples.
//! [`EmotionCore::step`] runs only for a heard frame. A denied frame ticks
//! the conductor as quiet and does not step the core.

use galion_emotion_core::{CoreError, EmotionCore, MomentState, VoiceBaseline};

use crate::{Ear, FrameOut};

/// Fixed activity mark passed to a conductor when the frame is voiced.
/// One character, no digits, not a word from the audio.
pub const ACTIVITY_MARK: &str = ".";

/// What the conductor is told. `Voiced` carries [`ACTIVITY_MARK`], not PCM.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FloorCue {
    Quiet,
    Voiced,
}

/// A conductor tick. The real `galion-conductor` is its own workspace, so
/// this crate cannot name it. [`crate::link::push_frame`] calls this instead.
pub trait FloorTick {
    fn tick_floor(&mut self, cue: FloorCue);
}

impl FloorTick for () {
    fn tick_floor(&mut self, _cue: FloorCue) {}
}

/// What one linked frame did.
#[derive(Debug)]
pub struct LinkOut {
    pub frame: FrameOut,
    pub moment: Option<MomentState>,
    pub cue: Option<FloorCue>,
}

/// Hear `pcm`, offer the point to the core, and cue the conductor.
///
/// A denied frame cues quiet and does not step the core. A rejected buffer
/// does neither.
pub fn push_frame(
    ear: &mut Ear,
    core: &mut EmotionCore,
    floor: &mut dyn FloorTick,
    baseline: VoiceBaseline,
    pcm: &[f32],
) -> Result<LinkOut, CoreError> {
    let frame = ear.push(pcm);
    match frame {
        FrameOut::Denied => {
            floor.tick_floor(FloorCue::Quiet);
            Ok(LinkOut {
                frame,
                moment: None,
                cue: Some(FloorCue::Quiet),
            })
        }
        FrameOut::Rejected => Ok(LinkOut {
            frame,
            moment: None,
            cue: None,
        }),
        FrameOut::Heard(heard) => {
            let cue = if heard.voiced {
                FloorCue::Voiced
            } else {
                FloorCue::Quiet
            };
            let msg = heard.offer.inbound_intent(baseline)?;
            let moment = core.step(&msg)?;
            floor.tick_floor(cue);
            Ok(LinkOut {
                frame: FrameOut::Heard(heard),
                moment: Some(moment),
                cue: Some(cue),
            })
        }
    }
}
