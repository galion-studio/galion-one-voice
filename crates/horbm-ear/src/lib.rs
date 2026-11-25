// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Ear-side prosody for HORBM.
//!
//! One call is one 80 ms frame: 16 kHz, 1280 mono samples. That is the
//! capture quantum beside the locked heartbeat (`LOCKS.toml` `[clock]
//! frame_ms` is 80). Playback stays 24 kHz and is not this crate.
//!
//! The ear starts on. [`Ear::new`] senses. [`Ear::deny`], [`EarConfig`] with
//! `sense: false`, and `GALION_EAR_SENSE=0|off|deny` ([`Ear::from_env`]) are
//! the off switch. While sensing is off, or the purpose is workplace or
//! education, [`Ear::push`] returns [`FrameOut::Denied`] and does not read
//! the samples. The pitch detector and the scalar rings are dropped when the
//! switch closes. Nothing in this crate writes a file, opens a socket, or
//! keeps a copy of the PCM.
//!
//! Pitch is [`pitch_detection::detector::yin::YINDetector`] (de Cheveigné and
//! Kawahara, 2002, as that crate implements it). That crate's clarity number
//! is not a 0..=1 score, so voicing is the normalised autocorrelation at the
//! period it returned. Energy, pause, the speaking-rate estimate, jitter, and
//! shimmer are the rest of the glue. The open-space point is a fixed coupling,
//! not a category list. [`port::ListenerOffer`] is the acoustic point the
//! emotion core can take later. [`ListenerOffer::heard_axes`] reads valence,
//! arousal, and strength, or nothing when one of them is non-finite. A
//! denied frame has no offer. [`Ear::push`] does not step the core, and
//! neither does that read.
//! [`link::push_frame`] steps it with an inbound intent that mixes only when
//! adapt is granted as well. [`listen::hear_frame`] is the configured
//! persona's path: a separate listener reading, then that persona's own
//! response. It does not write the baseline. An id with no emotion profile
//! is not given another persona's profile. The caller then keeps the reading
//! on the persona-neutral [`listen::UNIVERSAL`] path only.
//!
//! [`eot::Scorer`] turns those scalars, plus the latest ASR partial, into a
//! [`eot::TurnMsg`]. Thresholds are `eot.toml`, compiled in. A denied frame
//! is [`eot::TurnMsg::PauseOnly`], which is the conductor's existing hang.
//! [`eot::ListenCue`] marks a clause-final fall that is still a hold, a
//! question rise, or a filler. Nothing in the scorer reads a weight or a
//! sound file, and it does not play a backchannel.

#![forbid(unsafe_code)]

mod dsp;
pub mod eot;
pub mod link;
pub mod listen;
pub mod port;

pub use eot::{EotConfig, ListenCue, ProsodyCue, Scorer, TurnMsg};
pub use galion_emotion::consent_bits;
pub use galion_emotion::Purpose;
pub use port::{ListenerOffer, ListenerPort};

/// Capture rate of one ear frame.
pub const SAMPLE_RATE: u32 = 16_000;
/// Locked heartbeat. Same value as `galion-emotion-core` `FRAME_MS`.
pub const FRAME_MS: u32 = 80;
/// Samples in one ear frame. `16_000 * 80 / 1000`.
pub const FRAME_SAMPLES: usize = 1_280;

/// This layer has no store. `remember` does not open one.
pub const PERSISTS: bool = false;

/// Process environment that closes the ear. Unset means on.
/// `0`, `off`, and `deny` close it. Anything else leaves it on.
pub const EAR_SENSE_ENV: &str = "GALION_EAR_SENSE";

const _: () = assert!(SAMPLE_RATE as usize * FRAME_MS as usize / 1000 == FRAME_SAMPLES);
const _: () = assert!(FRAME_MS == galion_emotion_core::FRAME_MS);

/// What one granted frame produced, or why it produced nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameOut {
    /// Sense is off, or the purpose is prohibited. The samples were not read.
    Denied,
    /// Sense is on, but the buffer is not one finite ear frame. Rings stay as they were.
    Rejected,
    Heard(ProsodyFrame),
}

/// Acoustic features plus the listener offer. Copy. No PCM.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProsodyFrame {
    /// Hertz. 0 when [`Self::voiced`] is false.
    pub f0_hz: f32,
    pub voiced: bool,
    /// Normalised autocorrelation at the YIN period, 0..=1. 0 when unvoiced.
    pub voicing: f32,
    /// Full-frame RMS, linear.
    pub energy: f32,
    /// Envelope onsets per second over the scalar window (at most 2 s).
    pub speaking_rate_hz: f32,
    /// Trailing quiet run, in milliseconds.
    pub pause_ms: u32,
    /// Relative local jitter. 0 when there are not enough cycles.
    pub jitter: f32,
    /// Relative local shimmer. 0 when there are not enough cycles.
    pub shimmer: f32,
    pub offer: ListenerOffer,
}

/// In-memory switch. [`EarConfig::default`] is sensing on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EarConfig {
    pub sense: bool,
}

impl Default for EarConfig {
    fn default() -> Self {
        Self { sense: true }
    }
}

/// One ear. Starts sensing. Not shared across threads: the pitch crate
/// borrows its scratch on the call.
pub struct Ear {
    consent: u8,
    purpose: Purpose,
    memory: Option<dsp::Memory>,
}

impl std::fmt::Debug for Ear {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ear")
            .field("consent", &self.consent)
            .field("purpose", &self.purpose)
            .field("sensing", &self.sensing())
            .field("frames", &self.frames_analysed())
            .finish()
    }
}

impl Default for Ear {
    fn default() -> Self {
        Self::new()
    }
}

impl Ear {
    /// Session start. Sensing is on, for a personal assistant. Adapt,
    /// remember, and share stay off. Workplace and education are not this
    /// default; pass them to [`Self::set_grant`] and the ear closes.
    pub fn new() -> Self {
        Self {
            consent: consent_bits::SENSE,
            purpose: Purpose::PersonalAssistant,
            memory: Some(dsp::Memory::new()),
        }
    }

    /// [`Self::new`], then apply an explicit off value.
    ///
    /// `None`, `""`, `1`, `on`, and `allow` stay on. `0`, `off`, and `deny`
    /// close the ear. Matching is trimmed and case-insensitive.
    pub fn from_env(value: Option<&str>) -> Self {
        if env_closes(value) {
            Self::shut()
        } else {
            Self::new()
        }
    }

    /// [`Self::from_env`] on [`EAR_SENSE_ENV`]. Unset stays on.
    pub fn from_process_env() -> Self {
        match std::env::var(EAR_SENSE_ENV) {
            Ok(value) => Self::from_env(Some(&value)),
            Err(_) => Self::new(),
        }
    }

    pub fn from_config(config: EarConfig) -> Self {
        if config.sense {
            Self::new()
        } else {
            Self::shut()
        }
    }

    fn shut() -> Self {
        Self {
            consent: 0,
            purpose: Purpose::PersonalAssistant,
            memory: None,
        }
    }

    pub fn consent(&self) -> u8 {
        self.consent
    }

    pub fn purpose(&self) -> Purpose {
        self.purpose
    }

    /// True only while a grant is open and the analyser is alive.
    pub fn sensing(&self) -> bool {
        self.memory.is_some()
    }

    /// Accepted frames since the current grant. 0 when the ear is shut.
    pub fn frames_analysed(&self) -> u32 {
        self.memory.as_ref().map(|m| m.frames()).unwrap_or(0)
    }

    /// PCM samples retained by this crate. Always 0.
    pub fn retained_samples(&self) -> usize {
        0
    }

    /// Install a grant. Low four bits only; the affect-label nibble is ignored.
    ///
    /// The ear opens only when the sense bit is set and `purpose` is not
    /// workplace or education. Any other call drops the analyser and the
    /// scalar rings. A grant that stays open does not reset them.
    pub fn set_grant(&mut self, consent: u8, purpose: Purpose) {
        let consent = consent & 0x0f;
        let open = consent & consent_bits::SENSE != 0 && purpose_allows(purpose);
        if !open {
            self.memory = None;
        } else if self.memory.is_none() {
            self.memory = Some(dsp::Memory::new());
        }
        self.consent = consent;
        self.purpose = purpose;
    }

    /// Close the ear and drop its rings. The purpose is left as it was.
    pub fn deny(&mut self) {
        self.set_grant(0, self.purpose);
    }

    /// One frame. Denied returns before the slice is read.
    pub fn push(&mut self, pcm: &[f32]) -> FrameOut {
        match self.memory.as_mut() {
            None => FrameOut::Denied,
            Some(memory) => match accept(pcm) {
                None => FrameOut::Rejected,
                Some(()) => FrameOut::Heard(memory.push(pcm, self.consent)),
            },
        }
    }

    /// [`Self::push`], then hand the offer to `port` when a frame was heard.
    /// A denied or rejected frame does not call the port.
    pub fn push_with(&mut self, pcm: &[f32], port: &mut dyn ListenerPort) -> FrameOut {
        let out = self.push(pcm);
        if let FrameOut::Heard(frame) = &out {
            port.take_listener(&frame.offer);
        }
        out
    }
}

fn purpose_allows(purpose: Purpose) -> bool {
    !matches!(purpose, Purpose::Workplace | Purpose::Education)
}

fn env_closes(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("0") | Some("off") | Some("deny")
    )
}

fn accept(pcm: &[f32]) -> Option<()> {
    if pcm.len() != FRAME_SAMPLES {
        return None;
    }
    if pcm.iter().any(|s| !s.is_finite()) {
        return None;
    }
    Some(())
}
