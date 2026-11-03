// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! # galion-heartbeat
//!
//! The 80 ms heartbeat: **one** `step()` per frame that carries the you band,
//! the me band and the feeling band ("a conversation is one heartbeat, not a
//! pipeline"). This crate owns the frame-loop API; [`Rika`] runs inside it on
//! the feeling band (D20, Q-RIKA.Q6: inside `step()`, no separate process).
//!
//! Hot-path contract (LOCKS `[frame] frame_ms = 80`):
//! * [`Heartbeat::step`] **never blocks**: no locks, no channels, no sleeps, no
//!   clock reads, no file or network I/O.
//! * [`Heartbeat::step`] **never allocates**: all state is fixed-size and lives
//!   in the struct (checked by `tests/zero_alloc.rs` with a counting allocator).
//! * **A late frame is a pause, never a glitch**: a frame flagged late by the
//!   scheduler (or one whose you-band PCM is not exactly [`FRAME_SAMPLES`]) is
//!   delivered with `paused = true`, the me band starts nothing new on it, and
//!   nothing catches up on the next frame. Rika is told (`late`) so it does not
//!   advance hysteresis on scheduler artefacts (D20 §5.1, R9).
//!
//! Pipeline timing: the feeling-band decision made in frame *t* is **applied**
//! (audible) in frame *t+1* ([`FrameOut::applied`]). With Rika's 2-frame
//! hysteresis this gives evidence *t* → armed *t+1* → audible *t+2* = 160 ms
//! (LOCKS `rika_land_max_ms`), and a user onset while armed → `yield_now` →
//! audible next frame (`rika_yield_max_frames = 1`).
//!
//! Rika is **OFF by default** ([`HeartbeatConfig::default`]); it also needs an
//! open session, SENSE **and** ADAPT consent bits and a permitted purpose from
//! the [`ConsentGate`] every frame. Without SENSE no PCM is read for affect.
//!
//! The feeling band on each [`FrameOut`] is [`galion_emotion::Feeling`]: the
//! live galion-emotion weight (OKLCH + persona prosody) for Samantha or Jarvis.
//! It is computed inside [`Heartbeat::step`] from the consented caller frame.
//! No phrase table. No accelerator. A missing SENSE bit clears it to rest.
//! Jarvis's hub bed is a me-band layer. [`Heartbeat::set_hub_layer`] toggles it
//! between frames. `step` does not mix it into `you_pcm`.

use galion_emotion::band::consent_bits;
use galion_emotion::{
    AffectDetector, ConsentGate, Cue, EmotionFrame, EnergyStubDetector, Feeling, FrameFeatures, OffReason, ProsodyControl, Purpose,
    Rika, RikaConfig, RikaCtx, RikaOutput, RikaReport, RikaState, VoicePersona,
};

#[cfg(feature = "presence")]
pub mod presence;

/// The heartbeat period. Locked (LOCKS.toml `[frame] frame_ms = 80`).
pub const FRAME_MS: u32 = 80;
/// Audio sample rate of the you/me bands (Mimi 24 kHz).
pub const SAMPLE_RATE: u32 = 24_000;
/// Samples in one 80 ms frame at 24 kHz.
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;

/// Heartbeat configuration. `Default` = Rika OFF (Q-RIKA.Q14: off until ≥ 5 must specs).
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub rika: RikaConfig,
    /// Run the RMS/ZCR `EnergyStubDetector` (inside the consent-gated feature
    /// read) when the host supplies no affect frame. Stub path only.
    pub stub_affect: bool,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self { rika: RikaConfig::default(), stub_affect: true }
    }
}

impl HeartbeatConfig {
    /// Rika enabled with the v0 (D20 [E]) thresholds — fixtures, gates, harness.
    pub fn rika_v0() -> Self {
        Self { rika: RikaConfig::v0_enabled(), ..Self::default() }
    }
}

/// One frame of input. Borrowed, so building it allocates nothing.
#[derive(Debug, Clone, Copy)]
pub struct FrameIn<'a> {
    /// You-band mono PCM for this frame, [-1, 1], ideally exactly [`FRAME_SAMPLES`].
    pub you_pcm: &'a [f32],
    /// Inbound affect from an upstream head (student). `None` → stub detector if enabled.
    pub affect: Option<EmotionFrame>,
    /// Future affect-head tension (primary Rika dimension, Q9). `None` on the stub path.
    pub tension: Option<f32>,
    /// Live consent bits (`band::consent_bits`).
    pub consent_bits: u8,
    pub purpose: Purpose,
    pub session_open: bool,
    /// The me band is voiced right now (output scheduler).
    pub me_speaking: bool,
    /// The scheduler marked this frame as having missed its deadline.
    pub late: bool,
}

/// One frame of output: what to apply now, and what was decided now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameOut {
    /// Frames delivered so far (0-based), not milliseconds.
    pub index: u32,
    /// Late frame → pause: me band holds (no new onset, no catch-up).
    pub paused: bool,
    /// Rika's decision this frame (takes effect next frame).
    pub decided: RikaOutput,
    /// Rika's decision from the previous frame, applied (audible) in this frame.
    pub applied: RikaOutput,
    /// The me band may start a new onset in this frame.
    pub me_onset_allowed: bool,
    /// Live persona conduct for this frame (you · me · feeling). Stack only.
    pub feeling: Feeling,
}

impl FrameOut {
    /// Lean-in audible in this frame (the presence `lean_in` signal).
    pub fn lean_in_audible(&self) -> bool {
        self.applied.lean_in > 0.0
    }
    /// Flush the me band in this frame (user onset while armed, decided last frame).
    pub fn yield_now(&self) -> bool {
        self.applied.yield_now
    }
    /// Bounded lean-in prosody for this frame (identity when no lean-in).
    pub fn prosody(&self, base: ProsodyControl) -> ProsodyControl {
        self.applied.apply_prosody(base)
    }
    /// Linear me-band gain for this frame (1.0 exactly when neutral; never > 1).
    pub fn me_gain(&self) -> f32 {
        self.applied.me_gain()
    }
    /// Persona prosody for this frame, then the applied Rika lean-in.
    /// This is the voice path. It is not a sentence.
    pub fn voice_prosody(&self) -> ProsodyControl {
        self.applied.apply_prosody(self.feeling.prosody)
    }
    /// You-band onset threshold delta in dB (≤ 0) for this frame.
    pub fn onset_threshold_db(&self) -> f32 {
        self.applied.onset_threshold_db
    }
    /// Console glyph for the feeling-band row (D20 §7).
    pub fn glyph(&self) -> char {
        self.decided.state.glyph()
    }
}

/// A neutral, OFF Rika output (the heartbeat's "nothing applied" value).
pub fn neutral_output(frame: u32, off: Option<OffReason>) -> RikaOutput {
    RikaOutput {
        frame,
        state: RikaState::Off,
        trigger: None,
        cue: Cue::None,
        lean_in: 0.0,
        hold_me_onset: false,
        yield_floor: false,
        yield_now: false,
        onset_threshold_db: 0.0,
        me_energy_db: 0.0,
        off,
    }
}

/// The 80 ms heartbeat. Plain struct, RAM only, fixed size.
#[derive(Debug)]
pub struct Heartbeat<G: ConsentGate> {
    cfg: HeartbeatConfig,
    gate: G,
    rika: Rika,
    detector: EnergyStubDetector,
    index: u32,
    pending: RikaOutput,
    paused_frames: u32,
    persona: VoicePersona,
    feeling: Feeling,
    /// Jarvis hub bed. Default on. Samantha ignores it. Not applied to `you_pcm`.
    hub_on: bool,
}

impl<G: ConsentGate> Heartbeat<G> {
    pub fn new(cfg: HeartbeatConfig, gate: G) -> Self {
        let rika = Rika::new(cfg.rika.clone());
        let persona = VoicePersona::Samantha;
        Self {
            cfg,
            gate,
            rika,
            detector: EnergyStubDetector::default(),
            index: 0,
            pending: neutral_output(0, Some(OffReason::Disabled)),
            paused_frames: 0,
            persona,
            feeling: Feeling::rest(persona),
            hub_on: true,
        }
    }

    /// Fresh session state (hang-up / new call). Wipes all affect-derived state.
    /// Not a hot-path call.
    pub fn reset(&mut self) {
        self.rika = Rika::new(self.cfg.rika.clone());
        self.detector.reset();
        self.index = 0;
        self.pending = neutral_output(0, Some(OffReason::Disabled));
        self.paused_frames = 0;
        self.feeling = Feeling::conduct(self.persona, None, self.hub_on);
    }

    /// Register for the next frames. Not called inside [`Self::step`].
    pub fn set_persona(&mut self, persona: VoicePersona) {
        self.persona = persona;
    }

    pub fn persona(&self) -> VoicePersona {
        self.persona
    }

    /// Jarvis character bed. `false` mutes the layer. It does not denoise a
    /// reference and it does not change caller audio. Not called inside [`Self::step`].
    pub fn set_hub_layer(&mut self, on: bool) {
        self.hub_on = on;
    }

    pub fn hub_layer_on(&self) -> bool {
        self.hub_on
    }

    pub fn gate(&self) -> &G {
        &self.gate
    }
    /// Swap/modify consent between frames (e.g. user withdraws). Not called inside `step`.
    pub fn gate_mut(&mut self) -> &mut G {
        &mut self.gate
    }
    pub fn config(&self) -> &HeartbeatConfig {
        &self.cfg
    }
    pub fn rika(&self) -> &Rika {
        &self.rika
    }
    pub fn rika_report(&self) -> &RikaReport {
        self.rika.report()
    }
    /// Rika lean-in latency (evidence → audible), from the live report. R1 = 160 ms.
    pub fn lean_in_ms(&self) -> Option<u32> {
        self.rika.report().lean_in_ms()
    }
    /// Q13: truthful answer to "were you waiting for me?".
    pub fn explain(&self) -> &'static str {
        self.rika.explain()
    }
    pub fn frames(&self) -> u32 {
        self.index
    }
    pub fn paused_frames(&self) -> u32 {
        self.paused_frames
    }

    /// Advance exactly one 80 ms frame. Never blocks, never allocates.
    pub fn step(&mut self, input: &FrameIn<'_>) -> FrameOut {
        let idx = self.index;
        self.index = self.index.wrapping_add(1);
        let paused = input.late || input.you_pcm.len() != FRAME_SAMPLES;
        if paused {
            self.paused_frames = self.paused_frames.saturating_add(1);
        }

        let ctx = RikaCtx {
            consent_bits: input.consent_bits,
            purpose: input.purpose,
            session_open: input.session_open,
            me_speaking: input.me_speaking,
            late: paused,
        };
        let pcm = input.you_pcm;
        let stub = self.cfg.stub_affect && input.affect.is_none();
        let (gate, detector) = (&self.gate, &mut self.detector);
        // Host-supplied affect, replaced by the stub only when Rika's closure runs.
        let mut seen = input.affect;
        // The feature closure (the PCM read) runs only after Rika's consent gate passes.
        let decided = self.rika.step(ctx, gate, || {
            let affect = match input.affect {
                Some(a) => Some(a),
                None if stub => detector.detect(pcm, SAMPLE_RATE, idx.wrapping_mul(FRAME_MS), gate, input.purpose).ok(),
                None => None,
            };
            seen = affect;
            let mut f = FrameFeatures::from_pcm(pcm).with_affect(affect);
            f.tension = input.tension;
            f
        });
        if decided.off.is_some() {
            // OFF → no affect-derived state survives (detector EMA included).
            self.detector.reset();
        }

        // Feeling is the same 80 ms step. SENSE off → rest. A late frame holds.
        let sense = input.consent_bits & consent_bits::SENSE != 0;
        let hub_on = self.hub_on;
        if !sense {
            self.feeling = Feeling::conduct(self.persona, None, hub_on);
        } else if !paused {
            self.feeling = Feeling::conduct(self.persona, seen.as_ref(), hub_on);
        } else if self.feeling.persona != self.persona {
            self.feeling = Feeling::conduct(self.persona, None, hub_on);
        } else {
            self.feeling.hub = galion_emotion::HubLayer::character(self.persona, hub_on);
        }

        let applied = core::mem::replace(&mut self.pending, decided);
        // Late frame = pause: nothing new starts. Held breath: no onset while armed/held.
        let me_onset_allowed = !paused && !decided.hold_me_onset && !applied.hold_me_onset;
        let feeling = self.feeling;
        FrameOut { index: idx, paused, decided, applied, me_onset_allowed, feeling }
    }

    /// Host reports whether it actually started a me-band onset this frame.
    pub fn note_me_onset(&mut self, out: &FrameOut, started: bool) {
        self.rika.note_me_onset(&out.decided, started);
    }
}

/// SENSE | ADAPT — the bits Rika requires (LOCKS `rika_requires`).
pub const RIKA_BITS: u8 = consent_bits::SENSE | consent_bits::ADAPT;
