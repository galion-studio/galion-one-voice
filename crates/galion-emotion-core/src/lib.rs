// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Moment-to-moment emotion core for articulation V2 / MnE V2.
//!
//! The state is a point and a velocity on four axes. It is not a Cowen
//! category and not the twelve emotion-v2 names. Those names are a
//! projection in [`project`]. The speaker's baseline is loaded once and
//! [`EmotionCore::step`] does not write it.
//!
//! One step is one 80 ms frame on [`clock::VirtualClock`]. Inside factors
//! (breath, energy, fatigue, health) move the moment. Intent arrives as a
//! 72-byte [`IntentMsg`]. The inner-life layer ([`inner`]) keeps cares,
//! wants, and expectations and emits that same message. A surprise nudge
//! is a pull on it. Consent is the low four bits of EmotionWire byte 7,
//! enforced through `galion-emotion`. Learning is a separate lock key.
//!
//! CPU only. No model, no audio, no thread. The hot path does not allocate.
#![forbid(unsafe_code)]

mod axes;
mod baseline;
mod clock;
mod conductor;
mod factors;
mod inner;
mod intent;
mod moment;
mod project;
mod standin;
mod state;
mod wire;

pub use axes::{APPROACH, AROUSAL, AXES, AXIS_NAME, DOMINANCE, VALENCE};
pub use baseline::{BaselineBits, VoiceBaseline};
pub use clock::{VirtualClock, DT_S, FRAME_MS};
pub use conductor::ConductorCue;
pub use factors::{
    factor_offset, standin_breath, Breath, BreathPhase, Inside, BREATH_CYCLE_FRAMES,
    BREATH_FLOW_AROUSAL, BREATH_VOLUME_AROUSAL, ENERGY_APPROACH, ENERGY_AROUSAL, ENERGY_REST,
    FATIGUE_APPROACH, FATIGUE_AROUSAL, FATIGUE_DOMINANCE, FATIGUE_REST, FATIGUE_VALENCE,
    HEALTH_AROUSAL, HEALTH_DOMINANCE, HEALTH_REST, HEALTH_VALENCE, VOLUME_REST,
};
pub use inner::{
    learn_permitted, Care, Expect, InnerLife, TurnIn, TurnOut, Want, CARE_PULL, CARE_SLOTS,
    CUE_BYTES, EXPECT_HOLD, INBOUND_FOLD, LABEL_BYTES, LEX_VALENCE, SEED_CARE, SEED_EXPECT,
    SEED_STRENGTH, SEED_WANT, STARTLE_AROUSAL, STRENGTH_DECAY, STRENGTH_STEP, SURPRISE_MIN,
    TURN_ALLOC_BUDGET, WANT_SLOTS, WORD_SURPRISE,
};
pub use intent::{
    IntentMsg, IntentSource, INTENT_BYTES, INTENT_MAGIC, INTENT_VERSION, MASK_BREATH, MASK_ENERGY,
    MASK_FATIGUE, MASK_HEALTH,
};
pub use moment::MomentState;
pub use project::{
    arousal_bipolar, project_universal, UniversalPole, PROJECTION_SIGMA, UNIVERSAL, UNIVERSAL_LEN,
};
pub use standin::{
    load_standins, InnerLifeStandin, MemoryCard, StandinError, StandinWorld, FILE_INNER,
    FILE_INTENT, FILE_MEMORY, FILE_PERSONA, SCHEMA as STANDIN_SCHEMA,
};
pub use state::{EmotionCore, DAMPING, STEP_ALLOC_BUDGET, STEP_FLOP_BUDGET, STIFFNESS, VEL_LIMIT};
pub use wire::{admit_pull, local_outbound, may_remember, may_share};

/// Failures of the frame step. Copy, so the hot path does not build a String.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreError {
    OffClock { expected_ms: u32, got_ms: u32 },
    BadIntent,
    ClockOverflow,
}
