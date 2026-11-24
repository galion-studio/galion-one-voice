// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! HORBM phase 5 conductor.
//!
//! Eager end-of-turn, epoch rollback, barge-in at the 200 ms lock, and a
//! wordless reaction made of silence or one generated sine. MnE intent comes
//! in. A read-back goes out.
//!
//! Time is a virtual frame counter ([`clock::VirtualClock`]). One tick is
//! 80 ms, the locked frame. The host runs [`Conductor::tick`] on the turn
//! worker. It does not run inside the heartbeat frame function.
//!
//! The brain is [`brain::ClauseBrain`]. Tests use [`brain::FakeBrain`].
//! The phase 4 crate is not on `main`, so this crate does not name it.
//!
//! # Safety
//!
//! No `unsafe`. No SIMD. No async runtime. No socket.

#![forbid(unsafe_code)]

pub mod brain;
pub mod clock;
pub mod conductor;
pub mod lexicon;
pub mod mne;
pub mod reaction;
pub mod stamps;

pub use brain::{ClauseBrain, FakeBrain, PickedClause};
pub use clock::{
    VirtualClock, BARGE_IN_MAX_MS, COMMIT_SILENCE_FRAMES, DUCK_MS, FRAME_MS, HANG_FRAMES,
    MIXER_RESUME_QUIET_MS, MIXER_YIELD_MS, PAUSE_MS, STABLE_TICKS,
};
pub use conductor::{Conductor, Ear, Floor, TickEvent, TickOut};
pub use lexicon::is_listed_backchannel;
pub use mne::{read_back, stand_in_listen_intent};
pub use reaction::{clip, synthetic_clip, Reaction, ReactionKind, REACTION_SAMPLES};
pub use stamps::{stamp_log_is_lengths_only, Domain04Stamps};

/// This crate is not on the frame-step path. The constant is the reminder.
pub const RUNS_INSIDE_HEARTBEAT_STEP: bool = false;
