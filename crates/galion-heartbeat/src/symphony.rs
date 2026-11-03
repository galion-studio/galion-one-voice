// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Live streaming sectors on the one 80 ms heartbeat.
//!
//! Clone, persona, and the one dot do not own a clock. The host calls
//! [`crate::Heartbeat::step`], then [`SymphonyWire::step`], once per frame.
//! This step never blocks and never allocates. A late frame is a pause:
//! the me slot is not consumed, the persona hold does not advance, and the
//! one-dot mark stays where it was.
//!
//! Off this step, on purpose: clone synthesis (the child process), persona
//! reply text, paint, and HTTP. Those may allocate. They must not run inside
//! either `step`.

use crate::{FrameOut, FRAME_MS, FRAME_SAMPLES};

/// Tick budget for one heartbeat step plus this wire. A measurement above
/// this fails the harness. It is not a second clock and it is not a lock file.
pub const STEP_BUDGET_US: u32 = 800;

pub const BAND_YOU: &str = "you";
pub const BAND_ME: &str = "me";
pub const BAND_FEELING: &str = "feeling";
/// Official band names, in order. Count is 3.
pub const BANDS: [&str; 3] = [BAND_YOU, BAND_ME, BAND_FEELING];

/// Consecutive sensed frames at or above [`PERSONA_HOLD_E`] before a persona
/// id may change. One frame never flips it. Same count as the persona floor.
pub const PERSONA_HOLD_FRAMES: u8 = 8;
pub const PERSONA_HOLD_E: f32 = 0.65;
pub const PERSONA_RELEASE_E: f32 = 0.50;

const _: () = assert!(FRAME_SAMPLES <= u16::MAX as usize);

/// The three live sectors. Inference and paint are not in this list's step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sector {
    Clone,
    Persona,
    OneDot,
}

impl Sector {
    pub const ALL: [Sector; 3] = [Sector::Clone, Sector::Persona, Sector::OneDot];

    pub const fn name(self) -> &'static str {
        match self {
            Sector::Clone => "clone",
            Sector::Persona => "persona",
            Sector::OneDot => "one-dot",
        }
    }
}

/// One-dot mark for this frame. Speak wins when the me band emitted samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DotMark {
    #[default]
    Idle,
    Listening,
    Speaking,
}

impl DotMark {
    pub const fn name(self) -> &'static str {
        match self {
            DotMark::Idle => "idle",
            DotMark::Listening => "listening",
            DotMark::Speaking => "speaking",
        }
    }
}

/// Borrowed you / me / feeling view. The caller owns every buffer.
#[derive(Clone, Copy, Debug)]
pub struct BandView<'a> {
    /// You band is voiced this frame (speech, not room tone).
    pub you_active: bool,
    /// Feeling sample decided this frame. Applied on the next frame.
    pub feeling_e: f32,
    /// SENSE consent. Without it the feeling sample is dropped.
    pub sense: bool,
    /// Precomputed me-band PCM waiting in the clone or reply slot.
    /// Empty when this frame has nothing queued.
    /// Must not alias `me_out` in [`SymphonyWire::step`].
    pub me_slot: &'a [f32],
}

/// What the three sectors do on this frame. Plain `Copy` data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SectorOut {
    pub index: u32,
    pub frame_ms: u32,
    pub paused: bool,
    pub dot: DotMark,
    /// Samples taken from the me slot. Zero on a pause, a yield, or an empty slot.
    /// Never more than one frame.
    pub clone_samples: u16,
    /// Feeling sample applied this frame (the previous frame's decision).
    pub feeling_applied: f32,
    pub persona_hold: u8,
    /// True only after [`PERSONA_HOLD_FRAMES`] consecutive holds.
    pub persona_may_flip: bool,
    pub me_onset_allowed: bool,
    /// Host should drop the rest of the queued me slot after this call returns.
    /// The drop itself is not done here.
    pub release_slot: bool,
}

/// Fixed-size sector state. No heap.
#[derive(Clone, Copy, Debug)]
pub struct SymphonyWire {
    hold: u8,
    pending_e: f32,
    pending_sense: bool,
    me_open: bool,
    dot: DotMark,
}

impl Default for SymphonyWire {
    fn default() -> Self {
        Self::new()
    }
}

impl SymphonyWire {
    pub const fn new() -> Self {
        Self {
            hold: 0,
            pending_e: 0.0,
            pending_sense: false,
            me_open: false,
            dot: DotMark::Idle,
        }
    }

    /// Whether this frame may pull one me-band frame from the clone or reply queue.
    /// Does not touch the queue and does not change wire state.
    pub fn me_emit(&self, frame: &FrameOut, dest_len: usize) -> bool {
        dest_len == FRAME_SAMPLES
            && !frame.paused
            && !frame.yield_now()
            && (frame.me_onset_allowed || self.me_open)
    }

    /// One 80 ms sector frame. Never blocks. Never allocates.
    ///
    /// `me_out` receives at most [`FRAME_SAMPLES`] samples from `bands.me_slot`
    /// when [`Self::me_emit`] is true. A pause leaves `me_out` untouched.
    /// A slot longer than one frame is not drained past the first frame.
    pub fn step(&mut self, frame: &FrameOut, bands: BandView<'_>, me_out: &mut [f32]) -> SectorOut {
        let paused = frame.paused || me_out.len() != FRAME_SAMPLES;
        let release = frame.yield_now();
        let emit = self.me_emit(frame, me_out.len());
        let mut copied = 0usize;
        if emit && !bands.me_slot.is_empty() {
            let n = bands.me_slot.len().min(FRAME_SAMPLES);
            me_out[..n].copy_from_slice(&bands.me_slot[..n]);
            if n < FRAME_SAMPLES {
                me_out[n..].fill(0.0);
            }
            copied = n;
        }
        if release {
            self.me_open = false;
        } else if !paused {
            self.me_open = copied > 0;
        }

        let applied = if self.pending_sense {
            self.pending_e
        } else {
            0.0
        };
        if !paused {
            if bands.sense {
                let e = if bands.feeling_e.is_finite() {
                    bands.feeling_e.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                self.pending_e = e;
                self.pending_sense = true;
                if e >= PERSONA_HOLD_E {
                    self.hold = self.hold.saturating_add(1);
                } else if e <= PERSONA_RELEASE_E {
                    self.hold = 0;
                }
            } else {
                self.pending_e = 0.0;
                self.pending_sense = false;
                self.hold = 0;
            }
        }

        let dot = if paused {
            self.dot
        } else if copied > 0 {
            DotMark::Speaking
        } else if bands.you_active {
            DotMark::Listening
        } else {
            DotMark::Idle
        };
        self.dot = dot;

        SectorOut {
            index: frame.index,
            frame_ms: FRAME_MS,
            paused,
            dot,
            clone_samples: copied as u16,
            feeling_applied: applied,
            persona_hold: self.hold,
            persona_may_flip: self.hold >= PERSONA_HOLD_FRAMES,
            me_onset_allowed: frame.me_onset_allowed,
            release_slot: release,
        }
    }
}
