// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Breath and articulation controls for one voice.
//!
//! [`VoiceBaseline`] is the steady speaker. [`Inside`] is energy, fatigue,
//! and health. [`EmotionMoment`] is a handful of plain numbers for this
//! moment. None of the three is stored inside another. There is no
//! dependency on an emotion-core crate.
//!
//! [`plan_speech`] and [`plan_quiet`] emit one control frame per 80 ms.
//! The bytes of a plan are fixed for a fixed input. No sample is written.
//!
//! ```
//! use horbm_breath::{plan_speech, EmotionMoment, Inside, Span, VoiceBaseline};
//!
//! let track = plan_speech(
//!     VoiceBaseline::ADULT,
//!     Inside::REST,
//!     EmotionMoment::STILL,
//!     &[
//!         Span::talk(20, 0.5, 0.0),
//!         Span::pause(8),
//!         Span::talk(20, 0.5, 0.0),
//!     ],
//! )?;
//! assert!(track.frames.iter().any(|frame| frame.pause));
//! assert_eq!(track.frames[0].t_ms, 0);
//! assert_eq!(track.frames[1].t_ms, 80);
//! # Ok::<(), horbm_breath::Error>(())
//! ```

#![forbid(unsafe_code)]

mod glottis;
mod plan;
mod wire;

pub use glottis::open_quotient_from_rd;
pub use plan::{plan_quiet, plan_speech, quiet_cycle, QuietCycle};

/// Heartbeat. `LOCKS.toml` `clock.frame_ms`.
pub const FRAME_MS: u32 = 80;

/// One frame in seconds. Timing itself is counted in [`FRAME_MS`] ticks.
pub const FRAME_S: f32 = 0.08;

/// Ten minutes of frames. Longer plans are refused.
pub const MAX_FRAMES: u32 = 7_500;

/// Wire schema. Version 1 is the little-endian layout of [`Track::to_bytes`].
pub const SCHEMA: &str = "horbm-breath/1";

/// Directions used for the sign of a change. A coefficient in this crate is a
/// model scale sitting inside the envelope below, not a cell copied from a table.
pub const DIRECTIONS: &[&str] = &[
    "Hixon, Goldman and Mead (1973), JSHR 16:78-115: speech is exhaled.",
    "Winkworth, Davis, Adams and Ellis (1994, 1995): inhalations sit at boundaries the caller marks.",
    "Whalen and Kinsella-Shaw (1997), Phonetica 54:138-152: a longer phrase has a longer inspiration.",
    "Fant and Kruckenberg (1996), TMH-QPSR 37(4): Rd sets the open quotient. Modal Rd 1 is about 65%.",
    "Boiten (1998), Biological Psychology 49:29-51: amusement shortens inspiration; disgust holds after it.",
    "Homma and Masaoka (2008), Experimental Physiology 93:1011-1021: arousal and anxiety speed the breath.",
    "Ladefoged and McKinney (1963): louder speech goes with higher subglottal pressure. Energy uses that sign.",
    "Lieberman (1967): subglottal pressure falls across a breath group.",
    "Stevens (1998), Acoustic Phonetics: an open velum couples the nose. The caller sets the target.",
];

/// Crates that were read and not linked. None of them is an 80 ms control stepper.
pub const NOT_LINKED: &[&str] = &[
    "organpool 0.2 (MIT OR Apache-2.0) runs lungs on wall-clock threads. It is not an 80 ms clock and it is not deterministic.",
    "phoneme-synth 0.3 (MIT OR Apache-2.0) renders PCM. Open quotient is not a Voice field. This crate emits no samples.",
    "svara 2 is GPL-3.0-only. synthie-trombone 0.2 is EUPL-1.2 and wants rustc 1.88.",
    "galion-lungs also writes a breath waveform. The waveform stays out of this crate.",
];

/// Speech subglottal pressure clamp, cm H2O above atmosphere.
/// A model envelope for comfortable phonation, not a copied Titze cell.
pub const SPEECH_PS_MIN: f32 = 3.0;
pub const SPEECH_PS_MAX: f32 = 16.0;

/// Quiet expiratory pressure, and the magnitude of a quiet inspiratory dip.
pub const QUIET_PS_MIN: f32 = 0.2;
pub const QUIET_PS_MAX: f32 = 2.0;

/// Fant open quotient while the folds are phonating.
pub const SPEECH_OQ_MIN: f32 = 0.35;
pub const SPEECH_OQ_MAX: f32 = 0.85;

/// Adult eupnea, breaths per minute, inclusive.
pub const BPM_MIN: f32 = 10.0;
pub const BPM_MAX: f32 = 20.0;

/// Why a plan or a byte string was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    NonFinite(&'static str),
    OutOfRange(&'static str),
    TooLong,
    BadBytes(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFinite(name) => write!(f, "{name} is not finite"),
            Self::OutOfRange(name) => write!(f, "{name} is out of range"),
            Self::TooLong => f.write_str("breath plan is longer than 10 minutes"),
            Self::BadBytes(what) => write!(f, "breath bytes: {what}"),
        }
    }
}

impl std::error::Error for Error {}

/// Steady speaker. Not a feeling, and not this moment's energy or fatigue.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VoiceBaseline {
    /// Quiet cycle length in 80 ms frames. 60 is 4.8 s, 12.5 breaths/min.
    pub quiet_period_frames: u32,
    /// Expiration / inspiration during quiet breathing. 2 is an I:E of 1:2.
    pub ie_ratio: f32,
    /// Modal speech pressure, cm H2O. The plan multiplies this, then clamps.
    pub speech_ps_cm_h2o: f32,
    /// Fant Rd of the steady voice. 1 is the modal row.
    pub modal_rd: f32,
    /// Velum leak when the caller did not ask for a nasal. 0 is closed.
    pub nasal_rest: f32,
    /// Jaw at rest, 0 closed, 1 open.
    pub mouth_rest: f32,
}

impl VoiceBaseline {
    pub const ADULT: Self = Self {
        quiet_period_frames: 60,
        ie_ratio: 2.0,
        speech_ps_cm_h2o: 7.0,
        modal_rd: 1.0,
        nasal_rest: 0.02,
        mouth_rest: 0.12,
    };
}

/// Body, for this plan. 0 is none of that factor, except health where 1 is well.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inside {
    /// Added expiratory drive. 0 leaves the baseline pressure alone.
    pub energy: f32,
    /// 0 rested, 1 spent. Lowers speech pressure.
    pub fatigue: f32,
    /// 1 well, 0 unwell. Not a diagnosis.
    pub health: f32,
}

impl Inside {
    pub const REST: Self = Self {
        energy: 0.0,
        fatigue: 0.0,
        health: 1.0,
    };
}

/// This moment, as plain numbers. Not a type from another crate.
///
/// Arousal is −1..=1 and 0 is still. Positive arousal shortens breath groups
/// and raises speech pressure. Negative arousal does not invent a slower breath.
/// Amusement, disgust, and fear are the only category rows with a published
/// breath direction. Anything else is left at 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmotionMoment {
    pub arousal: f32,
    pub amusement: f32,
    pub disgust: f32,
    pub fear: f32,
}

impl EmotionMoment {
    pub const STILL: Self = Self {
        arousal: 0.0,
        amusement: 0.0,
        disgust: 0.0,
        fear: 0.0,
    };
}

/// What the caller is doing for a run of frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpanKind {
    Talk,
    Pause,
}

/// A stretch of speech or of silence, already counted in 80 ms frames.
/// This crate does not tokenize text. A pause is a boundary the caller marked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub kind: SpanKind,
    pub frames: u32,
    /// 0..=1 during talk. Ignored on a pause.
    pub mouth: f32,
    /// 0 oral, 1 nasal, during talk. Ignored on a pause.
    pub nasal: f32,
}

impl Span {
    #[must_use]
    pub const fn talk(frames: u32, mouth: f32, nasal: f32) -> Self {
        Self {
            kind: SpanKind::Talk,
            frames,
            mouth,
            nasal,
        }
    }

    #[must_use]
    pub const fn pause(frames: u32) -> Self {
        Self {
            kind: SpanKind::Pause,
            frames,
            mouth: 0.0,
            nasal: 0.0,
        }
    }
}

/// Where the breath is. Speech is [`Phase::Exhale`] with [`Frame::speaking`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Inhale,
    Exhale,
    /// End-inspiratory or end-expiratory boundary. Not mid-phrase speech.
    Hold,
}

impl Phase {
    fn wire(self) -> u8 {
        match self {
            Self::Inhale => 1,
            Self::Exhale => 2,
            Self::Hold => 3,
        }
    }

    fn from_wire(byte: u8) -> Result<Self, Error> {
        match byte {
            1 => Ok(Self::Inhale),
            2 => Ok(Self::Exhale),
            3 => Ok(Self::Hold),
            _ => Err(Error::BadBytes("phase")),
        }
    }
}

/// One 80 ms control frame.
///
/// `subglottal_cm_h2o` is a model scale inside the envelopes in this module,
/// positive when air is driven out. `airflow_ml_s` uses the same sign.
/// `open_quotient` is Fant's quotient while [`Frame::speaking`] is set, and an
/// abduction fraction (folds open for air) otherwise.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub t_ms: u32,
    pub phase: Phase,
    pub speaking: bool,
    pub pause: bool,
    pub subglottal_cm_h2o: f32,
    pub airflow_ml_s: f32,
    pub open_quotient: f32,
    pub breathiness: f32,
    pub nasal_coupling: f32,
    pub mouth_openness: f32,
}

/// The frames of one plan, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub frames: Vec<Frame>,
}
