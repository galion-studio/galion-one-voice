// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! The document both mirrors encode.
//!
//! Intent is Inner Life to MnE. Read-back is MnE to Inner Life.
//! A feeling is arousal, valence, and dominance, plus an optional pin.
//! Ranges match the galion-emotion wire: arousal 0..1, valence −1..1,
//! dominance 0..1. PR #191's TickState uses −1..1 for all three; this
//! crate does not convert that scale.

use crate::names::{CowenName, Track};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocKind {
    Intent,
    Readback,
}

/// Where a number came from. `StandIn` is a fixture knob, not a measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueOrigin {
    StandIn,
    Config,
    Measured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyOrigin {
    Unmeasured,
    StandIn,
    Measured,
}

/// Physiology stage. Not a Cowen name. `Unknown` means the slot was not filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreathSegment {
    #[default]
    Unknown,
    Rest,
    Inhale,
    Hold,
    Speech,
    Exhale,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaRef {
    pub persona_id: String,
    /// Pluggable voice profile id. Not a recording and not a person.
    pub voice_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Moment {
    pub moment_id: String,
    pub memory_id: String,
    /// Short stand-in text. Empty means the id is a ref only.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub memory_snippet: String,
    pub place: String,
    /// Open token: `whisper_close`, `conversational`, `across_room`, …
    /// No distance-to-decibel function is applied.
    pub distance: String,
    /// Open token: `quiet`, `some`, `loud`, …
    /// No dB SPL is stored here.
    pub noise: String,
    pub listener: String,
}

/// Speaker-relative origin this document calls neutral.
/// The coordinates are not a population norm.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselinePoint {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub name: CowenName,
    pub track: Track,
    /// 0 is not a pin. Mixture weights are unpublished; this is the caller's weight.
    pub weight: f64,
}

/// Continuous feeling. Absent `pin` is an unnamed point, including the baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feeling {
    pub arousal: f64,
    pub valence: f64,
    pub dominance: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pin>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Intent {
    /// Open verb (`comfort`, `tease`, …). Not required to be a Cowen name.
    pub verb: String,
    pub strength: f64,
}

/// Byte offsets into [`Document::text`], on UTF-8 char boundaries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpan {
    pub start_byte: u32,
    pub end_byte: u32,
}

/// Factors the body layers already take as inputs. Not categories.
/// Fatigue has no published magnitude (lungs layer: literature = none).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InnerFactors {
    pub energy: f64,
    pub fatigue: f64,
    /// 1 well, 0 unwell. Not a diagnosis.
    pub health: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentFrame {
    pub t_ms: u32,
    pub intent: Intent,
    pub feeling: Feeling,
    pub inner: InnerFactors,
    pub text_span: TextSpan,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LungsState {
    pub segment: BreathSegment,
    /// Relative volume on this contract's 0..1 scale. Not a litre.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lung_volume_rel: Option<f64>,
    /// Relative pressure. Not cm H₂O. Absent until a layer fills it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subglottal_pressure_rel: Option<f64>,
    /// Loudness offset in dB from that layer's own reference. Not dB SPL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loudness_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThroatState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f0_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hnr_db: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouthState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f1_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f2_hz: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub f3_hz: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoseState {
    /// Nasalance was not given a target in the articulation spec. Slot only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nasalance: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyState {
    pub origin: BodyOrigin,
    #[serde(default)]
    pub lungs: LungsState,
    #[serde(default)]
    pub throat: ThroatState,
    #[serde(default)]
    pub mouth: MouthState,
    #[serde(default)]
    pub nose: NoseState,
}

impl Default for BodyState {
    fn default() -> Self {
        Self {
            origin: BodyOrigin::Unmeasured,
            lungs: LungsState::default(),
            throat: ThroatState::default(),
            mouth: MouthState::default(),
            nose: NoseState::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Confidence {
    pub feeling: f64,
    pub body: f64,
    pub confounded: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadbackFrame {
    pub t_ms: u32,
    pub feeling: Feeling,
    pub body: BodyState,
    pub confidence: Confidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: String,
    pub schema_version: u16,
    pub kind: DocKind,
    pub frame_ms: u16,
    pub value_origin: ValueOrigin,
    /// Human line. Fixtures use it for the expected feeling shape.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    pub persona: PersonaRef,
    pub moment: Moment,
    pub text: String,
    pub baseline: BaselinePoint,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intent_frames: Vec<IntentFrame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub readback_frames: Vec<ReadbackFrame>,
}

impl Document {
    pub fn frame_count(&self) -> usize {
        match self.kind {
            DocKind::Intent => self.intent_frames.len(),
            DocKind::Readback => self.readback_frames.len(),
        }
    }

    pub fn times_ms(&self) -> Vec<u32> {
        match self.kind {
            DocKind::Intent => self.intent_frames.iter().map(|f| f.t_ms).collect(),
            DocKind::Readback => self.readback_frames.iter().map(|f| f.t_ms).collect(),
        }
    }
}
