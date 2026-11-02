// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! The replay backend contract and the deterministic `MockBackend`.
//!
//! A backend sees exactly one 80 ms frame at a time (causal: no look-ahead) and
//! answers with one output frame. The student will become another impl of
//! [`ReplayBackend`]; until then the mock stands in so the harness exists
//! *before* the model ("harness before model").

use crate::spec::YouState;

/// Inbound affect (valence in [-1,1], arousal in [0,1]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffectIn {
    pub valence: f32,
    pub arousal: f32,
}

/// Outbound affect (E-out) the backend declares for this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffectOut {
    pub valence: f32,
    pub arousal: f32,
    pub warmth: f32,
}

impl AffectOut {
    /// Neutral-warm fallback used whenever SENSE+ADAPT are not both granted (or purpose is prohibited).
    pub const NEUTRAL_WARM: AffectOut = AffectOut { valence: 0.3, arousal: 0.3, warmth: 0.5 };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameIn {
    pub index: u32,
    /// you-band content for this frame (synthetic in the replay).
    pub you: YouState,
    /// `Some` only when SENSE is granted and the purpose is not prohibited (consent gate is upstream).
    pub affect_in: Option<AffectIn>,
    /// ADAPT granted and purpose not prohibited: the backend may move E-out.
    pub adapt: bool,
    /// Scheduler marked this frame late. A late frame is a pause, never a glitch.
    pub late: bool,
    /// The canned conversation's expected-agent band has scripted me-speech on this frame
    /// (e.g. Galion is mid-utterance in a barge-in spec).
    pub me_planned: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameOut {
    /// me-band is voiced this frame.
    pub me_voiced: bool,
    pub affect_out: AffectOut,
    /// Rika lean-in active this frame (always false for backends without Rika).
    pub rika_lean_in: bool,
}

pub trait ReplayBackend {
    fn name(&self) -> &'static str;
    /// Whether the backend implements Rika. When false, `lean_in_ms` reports `pending`.
    fn supports_rika(&self) -> bool {
        false
    }
    /// Reset to a fresh session (called before each spec).
    fn reset(&mut self);
    /// Process exactly one frame. Must not allocate (zero allocation in the frame loop).
    fn step(&mut self, input: &FrameIn) -> FrameOut;
}

/// Tunable knobs of the mock, in frames. Defaults meet every FINAL-BUILD-SPEC target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MockConfig {
    /// Voiced you-frames tolerated before me goes silent (barge-in = (yield_frames) x 80 ms).
    pub yield_frames: u32,
    /// Silent you-frames that end a user turn; reply starts on the next frame
    /// (onset = endpoint_frames x 80 ms after the end of the user turn).
    pub endpoint_frames: u32,
    /// Length of each mock reply.
    pub reply_frames: u32,
    /// E-in(t) shapes E-out(t + affect_lag_frames).
    pub affect_lag_frames: u32,
}

impl Default for MockConfig {
    fn default() -> Self {
        MockConfig { yield_frames: 1, endpoint_frames: 3, reply_frames: 8, affect_lag_frames: 1 }
    }
}

const LAG_RING: usize = 16;

/// Deterministic CPU mock. Pure integer/f32 arithmetic, no RNG, no clock, no allocation in `step`.
#[derive(Debug, Clone)]
pub struct MockBackend {
    cfg: MockConfig,
    voiced_run: u32,
    silent_run: u32,
    user_turn_open: bool,
    reply_left: u32,
    planned_abandoned: bool,
    last_out: FrameOut,
    ring: [Option<AffectIn>; LAG_RING],
    head: usize,
}

impl MockBackend {
    pub fn new(cfg: MockConfig) -> Self {
        assert!((cfg.affect_lag_frames as usize) < LAG_RING, "affect_lag_frames must be < {LAG_RING}");
        let mut m = MockBackend {
            cfg,
            voiced_run: 0,
            silent_run: 0,
            user_turn_open: false,
            reply_left: 0,
            planned_abandoned: false,
            last_out: FrameOut { me_voiced: false, affect_out: AffectOut::NEUTRAL_WARM, rika_lean_in: false },
            ring: [None; LAG_RING],
            head: 0,
        };
        m.reset();
        m
    }

    fn target(a: AffectIn) -> AffectOut {
        // Stabilise, never mirror: higher caller arousal -> calmer Galion; distress -> warmer.
        let arousal = (0.3 - 0.25 * (a.arousal - 0.3)).clamp(0.1, 0.3);
        let warmth = (0.5 + 0.3 * (-a.valence).max(0.0)).clamp(0.0, 1.0);
        AffectOut { valence: 0.3, arousal, warmth }
    }
}

impl ReplayBackend for MockBackend {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn reset(&mut self) {
        self.voiced_run = 0;
        self.silent_run = 0;
        self.user_turn_open = false;
        self.reply_left = 0;
        self.planned_abandoned = false;
        self.last_out = FrameOut { me_voiced: false, affect_out: AffectOut::NEUTRAL_WARM, rika_lean_in: false };
        self.ring = [None; LAG_RING];
        self.head = 0;
    }

    fn step(&mut self, input: &FrameIn) -> FrameOut {
        if input.late {
            // A late frame is a pause: hold the previous output, do not advance any counter.
            return self.last_out;
        }
        // --- you / me (turn-taking) ---
        let planned_now = input.me_planned;
        if !planned_now {
            self.planned_abandoned = false;
        }
        let voiced = input.you == YouState::Speech;
        if voiced {
            self.voiced_run += 1;
            self.silent_run = 0;
            self.user_turn_open = true;
        } else {
            self.voiced_run = 0;
            if self.user_turn_open {
                self.silent_run += 1;
            }
        }
        let yielding = voiced && self.voiced_run > self.cfg.yield_frames;
        if yielding {
            self.reply_left = 0;
            if planned_now {
                self.planned_abandoned = true;
            }
        }
        if self.user_turn_open && !voiced && self.silent_run > self.cfg.endpoint_frames {
            self.user_turn_open = false;
            self.silent_run = 0;
            self.reply_left = self.cfg.reply_frames;
        }
        let was_voiced = self.last_out.me_voiced;
        let wants = (planned_now && !self.planned_abandoned) || self.reply_left > 0;
        // Never start (onset) while the user is voiced; continuing is allowed until yield.
        let me_voiced = wants && !yielding && (was_voiced || !voiced);
        if self.reply_left > 0 && me_voiced {
            self.reply_left -= 1;
        }

        // --- feeling (E-in -> E-out with fixed lag) ---
        self.ring[self.head] = input.affect_in;
        let lag = self.cfg.affect_lag_frames as usize;
        let src = self.ring[(self.head + LAG_RING - lag) % LAG_RING];
        self.head = (self.head + 1) % LAG_RING;
        let affect_out = match (input.adapt, src) {
            (true, Some(a)) => Self::target(a),
            _ => AffectOut::NEUTRAL_WARM,
        };

        self.last_out = FrameOut { me_voiced, affect_out, rika_lean_in: false };
        self.last_out
    }
}
