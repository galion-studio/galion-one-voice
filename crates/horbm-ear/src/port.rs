// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The listener point the emotion core can take later.
//!
//! Axis order and limits are the core's: valence and approach on −1..=1,
//! arousal and dominance on 0..=1. [`ListenerOffer::heard_axes`] is the
//! three floats a later inner layer reads: valence, arousal, and strength.
//! It is `None` when any of those three is non-finite. A denied frame never
//! builds an offer, so it has no axes. [`ListenerOffer::inbound_intent`]
//! copies the point into an inbound [`IntentMsg`] as a pull relative to a
//! speaker baseline. Neither function calls [`EmotionCore::step`]. The core
//! still admits an inbound pull only when both sense and adapt are set.

pub use galion_emotion_core::{
    CoreError, EmotionCore, IntentMsg, IntentSource, VoiceBaseline, APPROACH, AROUSAL, AXES,
    AXIS_NAME, DOMINANCE, VALENCE,
};

/// Pull clamp. The core's `PULL_LIMIT` is 2 and is not re-exported. A delta
/// of two axis-units is the widest gap between two legal points, so this
/// stays inside `IntentMsg::encode`.
const PULL_CLAMP: f32 = 2.0;

/// One granted frame, as numbers. Not a category and not a PCM buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListenerOffer {
    pub t_ms: u32,
    /// EmotionWire byte 7, low four bits.
    pub consent: u8,
    /// `[valence, arousal, dominance, approach]`.
    pub point: [f32; AXES],
    /// 0..=1. How firmly this frame supports `point`. Silence is near 0.
    pub strength: f32,
}

impl ListenerOffer {
    pub fn valence(self) -> f32 {
        self.point[VALENCE]
    }
    pub fn arousal(self) -> f32 {
        self.point[AROUSAL]
    }
    pub fn dominance(self) -> f32 {
        self.point[DOMINANCE]
    }
    pub fn approach(self) -> f32 {
        self.point[APPROACH]
    }

    /// `(valence, arousal, strength)` for a later inner-life point.
    ///
    /// `None` when valence, arousal, or strength is non-finite (`NaN` or
    /// infinity). Dominance and approach are not part of this triple. A
    /// denied frame does not produce a [`ListenerOffer`], so it has nothing
    /// to read. This function does not call [`EmotionCore::step`].
    pub fn heard_axes(self) -> Option<(f32, f32, f32)> {
        let axes = (self.valence(), self.arousal(), self.strength);
        (axes.0.is_finite() && axes.1.is_finite() && axes.2.is_finite()).then_some(axes)
    }

    /// Inbound intent for a later `step`. Factor mask stays 0: the ear does
    /// not write breath, energy, fatigue, or health. The verb is `hear`.
    pub fn inbound_intent(self, baseline: VoiceBaseline) -> Result<IntentMsg, CoreError> {
        let mut msg = IntentMsg::hold(self.t_ms);
        msg.source = IntentSource::Inbound;
        msg.consent = self.consent & 0x0f;
        msg.set_verb("hear")?;
        msg.strength = self.strength.clamp(0.0, 1.0);
        let mut i = 0;
        while i < AXES {
            let delta = (self.point[i] - baseline.pos[i]).clamp(-PULL_CLAMP, PULL_CLAMP);
            msg.set_pull(i, delta)?;
            i += 1;
        }
        let _ = msg.encode()?;
        Ok(msg)
    }
}

/// A later consumer. The ear calls this only for [`crate::FrameOut::Heard`].
pub trait ListenerPort {
    fn take_listener(&mut self, offer: &ListenerOffer);
}

impl ListenerPort for () {
    fn take_listener(&mut self, _offer: &ListenerOffer) {}
}

/// Keeps the latest offer in RAM. Dropping it drops the offer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RamPort {
    pub takes: u32,
    pub last: Option<ListenerOffer>,
}

impl ListenerPort for RamPort {
    fn take_listener(&mut self, offer: &ListenerOffer) {
        self.takes = self.takes.wrapping_add(1);
        self.last = Some(*offer);
    }
}
