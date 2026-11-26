// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! HORBM conductor.
//!
//! Eager end-of-turn, epoch rollback, barge-in at the 200 ms lock, and a
//! wordless reaction made of silence or one generated sine. MnE intent comes
//! in. A read-back goes out.
//!
//! A deeper question may also ask Galion Research. The local brain answers
//! first. The side-band result is merged only when it arrives within
//! [`clock::SIDE_BAND_CAP_MS`] on the virtual frame clock. The default client
//! is offline and reports unavailable on the ask. A session that has not
//! denied the mark posts a start when that request opens and an end when it
//! merges, drops, or is cancelled. Remember, learn, and in-band learning stay
//! denied.
//!
//! While the user holds the turn, [`Conductor::tick_listen`] may nod: one
//! short backchannel on the quiet frame after a clause-final fall. That nod
//! does not take the floor and is not a barge-in. `GALION_BACKCHANNEL=0|off`
//! closes it. A denied ear (`PauseOnly`) does not nod, because the fall and
//! the question rise are both pitch.
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

pub mod backchannel;
pub mod brain;
pub mod clock;
pub mod conductor;
pub mod lexicon;
pub mod mne;
pub mod reaction;
pub mod research;
pub mod stamps;

pub use backchannel::{
    listener_on_bus, Affect, Backchannel, BackchannelConfig, BackchannelKind,
    ListenCue, ListenMood, BACKCHANNEL_ENV, BACKCHANNEL_GAP_MS, BACKCHANNEL_PEAK_MAX,
    SPEECH_PEAK_MIN,
};
pub use brain::{ClauseBrain, FakeBrain, PickedClause};
pub use clock::{
    VirtualClock, BARGE_IN_MAX_MS, COMMIT_SILENCE_FRAMES, DUCK_MS, FRAME_MS, HANG_FRAMES,
    MIXER_RESUME_QUIET_MS, MIXER_YIELD_MS, PAUSE_MS, SIDE_BAND_CAP_MS, STABLE_TICKS,
};
pub use conductor::{Conductor, Ear, Floor, TickEvent, TickOut, TurnMsg};
pub use galion_sideband::{
    MarkEdge, MarkKind, SidebandConsent, SidebandMark, LEARN_IN_BAND, LEARN_LOCK, MARK_SOURCE,
    REMEMBER_LOCK,
};
pub use lexicon::is_listed_backchannel;
pub use mne::{read_back, stand_in_listen_intent};
pub use reaction::{clip, synthetic_clip, Reaction, ReactionKind, REACTION_SAMPLES};
pub use research::{
    cap_decision, classify, AskResult, CapDecision, FakeResearch, OfflineClient, PollResult,
    ResearchClient, SideBandView, SideIntent,
};
pub use stamps::{stamp_log_is_lengths_only, Domain04Stamps};

/// This crate is not on the frame-step path. The constant is the reminder.
pub const RUNS_INSIDE_HEARTBEAT_STEP: bool = false;
