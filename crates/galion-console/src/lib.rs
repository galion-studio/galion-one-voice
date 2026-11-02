// Galion Voice — Perpetual Open MIT License
//! galion-console: terminal-default visualization layer for Galion Voice.
//!
//! Hybrid rendering plan (locked): **terminal (ratatui over SSH) is the default**,
//! an optional native window and optional web console come later and will reuse
//! the same [`ConsoleState`] model + [`MetricsSource`] trait — only the renderer differs.
//!
//! Data flows one way: `MetricsSource -> ConsoleState -> ui::draw`.
//! The `galion-tensor` crate (or a bench runner / job runner) will implement
//! [`MetricsSource`] later; today [`SyntheticSource`] feeds placeholder data.
//!
//! The [`heartbeat`] module holds the YES / NO widget: key presses arm an answer
//! that commits as a [`HeartbeatEvent`] on the next 80 ms tick (`y`/`n`, Left/Right).
//!
//! The [`live`] module (A11) wires heartbeat / tensor / emotion stubs into the
//! console: one latency number per stage, `no reading` when SENSE is off,
//! persisted YES/NO answers (JSONL) and `NO_COLOR`. The [`feeds`] module holds
//! the real feeds (features `tensor-feed`, `emotion-feed`; both default-on).
//! The [`mic`] module holds CAPTURE sources: synthetic tone, WAV/PCM fixture
//! playback (feature `wav-mic`, galion-presence loader) and the stubbed live mic.
//! The [`dotviz`] module (feature `dot-viz`, W21-04) bridges heartbeat `FrameOut` →
//! galion-render one dot (PNG / terminal / web) + live timing / emotion bars.
//! The [`turn_worker`] module (feature `phone-turn`) runs PhoneLane off the 80 ms thread.
//! [`phone_host`] (same feature) is what the screen demo polls each tick.

#[cfg(feature = "dot-viz")]
pub mod dotviz;
#[cfg(feature = "phone-turn")]
pub mod phone_host;
#[cfg(feature = "phone-turn")]
pub mod turn_worker;
pub mod feeds;
pub mod heartbeat;
pub mod live;
pub mod mic;
pub mod model;
pub mod source;
pub mod ui;

pub use heartbeat::{Answer, HeartbeatEvent, YesNo, FRAME_MS};
pub use live::{
    AffectFeed, AffectSample, AnswerLog, ComposedFeed, LiveFeed, LiveSource, LiveView, NoAffect, Stage, StageTiming, StubAffect,
    StubFeed, StubTiming, TimingFeed,
};
pub use mic::{LiveMic, MicError, MicSource, PcmMic, SynthMic};
pub use model::{BenchSample, ConsoleState, JobState, JobStatus, TensorFrame};
pub use source::{MetricsSource, SyntheticSource};
