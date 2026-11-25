// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! The note between inner life and the MnE stack.
//!
//! [`InnerMsg`] is the in-process note (plain struct, serde). The bytes the
//! emotion core already steps are [`galion_emotion_core::IntentMsg`]: 72
//! bytes, magic `GEMI`, version 1. [`InnerMsg::to_gemi`] writes that record.
//! [`InnerMsg::from_gemi`] reads one. Goal levels ride the serde note. They
//! are not packed into the GEMI factor slots, so a step cannot write the baseline.

use galion_emotion::consent_bits::{ADAPT, REMEMBER, SENSE, SHARE};
use galion_emotion_core::{
    CoreError, IntentMsg, IntentSource, APPROACH, AROUSAL, DOMINANCE, VALENCE,
};
use serde::{Deserialize, Serialize};

/// Which way the note is travelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MsgDir {
    /// Inner life to MnE. This layer's own intent.
    Out,
    /// MnE or the ear to inner life. What was perceived.
    In,
}

/// Verbs on the note. Not emotion names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verb {
    /// Sad listener. Warmer and softer on the moment.
    Comfort,
    /// Ask one thing.
    Ask,
    /// Silence. Keep company. Do not open a topic.
    Wait,
    /// Stay with them. The own-intent verb the ear also uses.
    With,
    /// They asked. An answer is expected.
    Answer,
    /// One gentle reach after a long quiet. Rate-limited.
    CheckIn,
    /// No pull.
    Hold,
    /// Inbound. The listener point is in the valence and arousal fields.
    Heard,
}

impl Verb {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Comfort => "comfort",
            Self::Ask => "ask",
            Self::Wait => "wait",
            Self::With => "with",
            Self::Answer => "answer",
            Self::CheckIn => "check-in",
            Self::Hold => "hold",
            Self::Heard => "heard",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "comfort" => Some(Self::Comfort),
            "ask" => Some(Self::Ask),
            "wait" => Some(Self::Wait),
            "with" => Some(Self::With),
            "answer" => Some(Self::Answer),
            "check-in" => Some(Self::CheckIn),
            "hold" => Some(Self::Hold),
            "heard" => Some(Self::Heard),
            _ => None,
        }
    }
}

/// One note. Copy. No transcript and no user id.
///
/// On the way out, `valence`..`approach` are pulls off the voice baseline.
/// On the way in (`verb` = `heard`), `valence` and `arousal` are the
/// listener's point and `strength` is how sure that reading is.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InnerMsg {
    pub dir: MsgDir,
    pub verb: Verb,
    pub strength: f32,
    pub t_ms: u32,
    pub care: f32,
    pub curiosity: f32,
    pub help: f32,
    pub expect: f32,
    pub valence: f32,
    pub arousal: f32,
    pub dominance: f32,
    pub approach: f32,
    /// Ear switch. Off means a listener point on this note is ignored.
    pub sense: bool,
}

impl InnerMsg {
    pub const fn rest(care: f32, curiosity: f32, help: f32, expect: f32) -> Self {
        Self {
            dir: MsgDir::Out,
            verb: Verb::Hold,
            strength: 0.0,
            t_ms: 0,
            care,
            curiosity,
            help,
            expect,
            valence: 0.0,
            arousal: 0.0,
            dominance: 0.0,
            approach: 0.0,
            sense: false,
        }
    }

    /// JSON object. Allocates. Not the tick path.
    pub fn to_json(self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self)
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// The 72-byte GEMI record, stamped at `t_ms`.
    ///
    /// Consent is the sense bit or nothing. Adapt, remember, and share are
    /// cleared. Factor mask stays 0.
    pub fn to_gemi_at(self, t_ms: u32) -> Result<IntentMsg, CoreError> {
        let mut msg = IntentMsg::hold(t_ms);
        msg.source = match self.dir {
            MsgDir::Out => IntentSource::Own,
            MsgDir::In => IntentSource::Inbound,
        };
        msg.consent = if self.sense { SENSE } else { 0 };
        debug_assert_eq!(msg.consent & (ADAPT | REMEMBER | SHARE), 0);
        msg.set_verb(self.verb.as_str())?;
        msg.strength = unit(self.strength);
        if self.dir == MsgDir::Out {
            set_pull(&mut msg, VALENCE, self.valence)?;
            set_pull(&mut msg, AROUSAL, self.arousal)?;
            set_pull(&mut msg, DOMINANCE, self.dominance)?;
            set_pull(&mut msg, APPROACH, self.approach)?;
        } else {
            msg.set_pull(VALENCE, clamp_pull(self.valence))?;
            msg.set_pull(AROUSAL, clamp_pull(self.arousal))?;
        }
        msg.factor_mask = 0;
        Ok(msg)
    }

    pub fn to_gemi(self) -> Result<IntentMsg, CoreError> {
        self.to_gemi_at(self.t_ms)
    }

    /// Read a GEMI record. Goal levels are not on that record; they come back as 0.
    pub fn from_gemi(msg: &IntentMsg) -> Self {
        let verb = Verb::parse(msg.verb_str()).unwrap_or(match msg.source {
            IntentSource::Own => Verb::Hold,
            IntentSource::Inbound => Verb::Heard,
        });
        let masked = |axis: usize| {
            if msg.pull_mask & (1 << axis) != 0 {
                msg.pull[axis]
            } else {
                0.0
            }
        };
        Self {
            dir: match msg.source {
                IntentSource::Own => MsgDir::Out,
                IntentSource::Inbound => MsgDir::In,
            },
            verb,
            strength: msg.strength,
            t_ms: msg.t_ms,
            care: 0.0,
            curiosity: 0.0,
            help: 0.0,
            expect: 0.0,
            valence: masked(VALENCE),
            arousal: masked(AROUSAL),
            dominance: masked(DOMINANCE),
            approach: masked(APPROACH),
            sense: msg.consent & SENSE != 0,
        }
    }
}

fn set_pull(msg: &mut IntentMsg, axis: usize, value: f32) -> Result<(), CoreError> {
    let value = clamp_pull(value);
    if value == 0.0 {
        return Ok(());
    }
    msg.set_pull(axis, value)
}

fn clamp_pull(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(-1.5, 1.5)
    } else {
        0.0
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}
