// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Instant map from a clamped [`Pad`] to speaking parameters.
//!
//! Numbers reused, not refit:
//!
//! * Samantha `e0` register in `docs/galion-voice-v2/personas/voice/samantha.toml`:
//!   pitch 0 st, rate 1.00, loudness 1.00, pre-pause 0 ms. Tags are clone-only
//!   (`on_piper = false`, `on_clone = true`).
//! * Samantha column of `docs/galion-voice-v2/PERSONA-MOOD.md` and the
//!   `SAMANTHA` profile in `galion-emotion/src/persona_mood.rs`: pace, pitch,
//!   energy, noise and sentence-silence gains, and the hard bounds.
//! * Chatterbox worker clamps in `scripts/clone/chatterbox_worker.py`
//!   (`controls_of`, `speaking_rate_of`): energy 0.6..=1.2, pause 0..=400 ms,
//!   rate 0.8..=1.2. Exaggeration on that worker is 0..=2. This sector uses
//!   the narrower s711 band 0.48..=0.65.
//!
//! `da`, `dv` and `dd` are deltas from [`Pad::RESTING`], not from the older
//! persona rest arousal of 0.35. At the HORBM rest point the voice is the
//! `e0` line.
//!
//! The persona formula's `e` term is the user's empathy weight. This sector
//! does not invent `e` from the brain's own valence. Pre-pause still moves:
//! the published Samantha `e1` pause (250 ms) is the ceiling, scaled by how
//! far arousal sits below rest and by how far dominance sits below rest.

use crate::pad::Pad;

/// s711 Chatterbox exaggeration band. The worker itself allows 0..=2.
pub const EXAGGERATION_MIN: f32 = 0.48;
pub const EXAGGERATION_MAX: f32 = 0.65;
/// Inside the band, near the Chatterbox conventional default of 0.5.
pub const EXAGGERATION_REST: f32 = 0.52;

/// Samantha `e0` / `SAMANTHA.voice` baselines.
pub const BASE_LENGTH: f32 = 1.0;
pub const BASE_NOISE: f32 = 0.667;
pub const BASE_NOISE_W: f32 = 0.8;
pub const BASE_PITCH_ST: f32 = 0.0;
pub const BASE_ENERGY: f32 = 1.0;
pub const BASE_PRE_PAUSE_MS: f32 = 0.0;
pub const BASE_SENTENCE_SILENCE_S: f32 = 0.20;

/// Gains from the Samantha column. `pace_per_arousal` is negative: higher
/// arousal shortens `length_scale` (Piper: larger length is slower).
pub const PACE_PER_AROUSAL: f32 = -0.40;
pub const PITCH_PER_VALENCE: f32 = 1.6;
pub const PITCH_PER_AROUSAL: f32 = 2.2;
pub const ENERGY_PER_AROUSAL: f32 = 0.45;
pub const NOISE_PER_AROUSAL: f32 = 0.30;
pub const NOISE_W_PER_AROUSAL: f32 = 0.30;
/// Published sentence-silence slope: `base − 0.25·da`.
pub const SILENCE_PER_AROUSAL: f32 = 0.25;
/// Samantha `e1` pre-pause minus `e0` (`samantha.toml` 250 − 0).
pub const PAUSE_CEILING_MS: f32 = 250.0;

pub const LENGTH_MIN: f32 = 0.80;
pub const LENGTH_MAX: f32 = 1.30;
pub const NOISE_MIN: f32 = 0.2;
pub const NOISE_MAX: f32 = 0.9;
pub const NOISE_W_MIN: f32 = 0.2;
pub const NOISE_W_MAX: f32 = 1.0;
pub const SILENCE_MIN_S: f32 = 0.08;
pub const SILENCE_MAX_S: f32 = 0.45;
pub const PITCH_BOUND_ST: f32 = 3.0;
/// Same floor and ceiling the Chatterbox worker applies to energy.
/// The floor is the whisper guard: this sector never asks for a quieter voice.
pub const ENERGY_MIN: f32 = 0.60;
pub const ENERGY_MAX: f32 = 1.20;
pub const PAUSE_MIN_MS: f32 = 0.0;
/// Worker cap. The persona ceiling ([`PAUSE_CEILING_MS`]) is tighter.
pub const PAUSE_WORKER_MAX_MS: f32 = 400.0;
/// `speaking_rate_of` in the Chatterbox worker.
pub const WORKER_RATE_MIN: f32 = 0.8;
pub const WORKER_RATE_MAX: f32 = 1.2;

/// Tokens named in `samantha.toml` `[tags].allowed`. `[inhale]` and `[giggle]`
/// are not on that list (`unknown_tags = "off"`). This sector still emits
/// them as mood hints. A strict reader of the persona file drops them.
pub const PERSONA_ALLOWED_TAGS: &[&str] = &["[laugh]", "[chuckle]"];

/// Mood hint for the clone worker. Never written into the reply string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Inhale,
    Giggle,
}

impl Tag {
    pub fn token(self) -> &'static str {
        match self {
            Tag::Inhale => "[inhale]",
            Tag::Giggle => "[giggle]",
        }
    }

    /// `samantha.toml` `[tags].on_piper`. Piper would speak the token.
    pub fn on_piper(self) -> bool {
        false
    }

    /// `samantha.toml` `[tags].on_clone`.
    pub fn on_clone(self) -> bool {
        true
    }

    pub fn persona_allows(self) -> bool {
        PERSONA_ALLOWED_TAGS.contains(&self.token())
    }
}

/// Whisper is not a Samantha speaking mode. No token, no gain under the floor.
pub const WHISPER_TOKENS: &[&str] = &["[whisper]", "[whispering]"];

pub fn is_whisper_token(token: &str) -> bool {
    WHISPER_TOKENS.iter().any(|w| token.eq_ignore_ascii_case(w))
}

/// One instantaneous speaking packet. `tag` is always `None` from [`map`];
/// the clock fills it after the hold and the cooldown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeakingParams {
    pub exaggeration: f32,
    pub length_scale: f32,
    pub noise_scale: f32,
    pub noise_w: f32,
    pub sentence_silence_s: f32,
    pub pre_pause_ms: f32,
    pub pitch_shift_st: f32,
    pub energy: f32,
    /// `1 / length_scale`. Above 1 is faster, matching the worker's `prosody.rate`.
    pub rate: f32,
    /// `rate` after the worker's 0.8..=1.2 clamp.
    pub worker_rate: f32,
    pub tag: Option<Tag>,
}

impl SpeakingParams {
    /// Piper spawn length after the post-synthesis pitch resample.
    /// `PERSONA-MOOD.md`: `length_scale · 2^(st/12)`.
    pub fn piper_length_scale(self) -> f32 {
        self.length_scale * (self.pitch_shift_st / 12.0).exp2()
    }

    /// Milliseconds the worker can put in `prosody.pre_pause_ms` (an integer).
    pub fn pre_pause_ms_i(self) -> u32 {
        let ms = if self.pre_pause_ms.is_finite() {
            self.pre_pause_ms.clamp(PAUSE_MIN_MS, PAUSE_WORKER_MAX_MS)
        } else {
            0.0
        };
        ms.round() as u32
    }

    /// Pause expressed in 80 ms frames, for a scheduler that speaks on the tick.
    pub fn pre_pause_frames(self) -> u32 {
        (self.pre_pause_ms_i() as f32 / crate::TICK_MS as f32).round() as u32
    }

    pub fn whisper(self) -> bool {
        false
    }

    pub fn in_bounds(self) -> bool {
        in_range(self.exaggeration, EXAGGERATION_MIN, EXAGGERATION_MAX)
            && in_range(self.length_scale, LENGTH_MIN, LENGTH_MAX)
            && in_range(self.noise_scale, NOISE_MIN, NOISE_MAX)
            && in_range(self.noise_w, NOISE_W_MIN, NOISE_W_MAX)
            && in_range(self.sentence_silence_s, SILENCE_MIN_S, SILENCE_MAX_S)
            && in_range(self.pre_pause_ms, PAUSE_MIN_MS, PAUSE_CEILING_MS)
            && in_range(self.pitch_shift_st, -PITCH_BOUND_ST, PITCH_BOUND_ST)
            && in_range(self.energy, ENERGY_MIN, ENERGY_MAX)
            && self.rate.is_finite()
            && self.rate > 0.0
            && in_range(self.worker_rate, WORKER_RATE_MIN, WORKER_RATE_MAX)
            && self
                .tag
                .map(|t| !is_whisper_token(t.token()))
                .unwrap_or(true)
            && !self.whisper()
    }
}

fn in_range(x: f32, lo: f32, hi: f32) -> bool {
    x.is_finite() && x >= lo && x <= hi
}

/// Map one already-decided mood. Does not read reply text.
pub fn map(pad: Pad) -> SpeakingParams {
    let pad = pad.clamped();
    let da = pad.arousal - Pad::RESTING.arousal;
    let dv = pad.valence - Pad::RESTING.valence;
    let dd = pad.dominance - Pad::RESTING.dominance;

    let exaggeration = (EXAGGERATION_REST + 0.10 * da / 0.70 + 0.06 * dv / 0.75 + 0.02 * dd)
        .clamp(EXAGGERATION_MIN, EXAGGERATION_MAX);

    let length_scale = (BASE_LENGTH * (1.0 + PACE_PER_AROUSAL * da)).clamp(LENGTH_MIN, LENGTH_MAX);

    let noise_scale = (BASE_NOISE + NOISE_PER_AROUSAL * da + 0.05 * dv).clamp(NOISE_MIN, NOISE_MAX);
    let noise_w = (BASE_NOISE_W + NOISE_W_PER_AROUSAL * da).clamp(NOISE_W_MIN, NOISE_W_MAX);
    let sentence_silence_s =
        (BASE_SENTENCE_SILENCE_S - SILENCE_PER_AROUSAL * da).clamp(SILENCE_MIN_S, SILENCE_MAX_S);

    let pitch_shift_st = (BASE_PITCH_ST + PITCH_PER_VALENCE * dv + PITCH_PER_AROUSAL * da)
        .clamp(-PITCH_BOUND_ST, PITCH_BOUND_ST);

    let energy = (BASE_ENERGY * (1.0 + ENERGY_PER_AROUSAL * da) * dominance_energy(pad.dominance))
        .clamp(ENERGY_MIN, ENERGY_MAX);

    let pre_pause_ms = pre_pause(pad).clamp(PAUSE_MIN_MS, PAUSE_CEILING_MS);

    let rate = 1.0 / length_scale;
    let worker_rate = rate.clamp(WORKER_RATE_MIN, WORKER_RATE_MAX);

    SpeakingParams {
        exaggeration,
        length_scale,
        noise_scale,
        noise_w,
        sentence_silence_s,
        pre_pause_ms,
        pitch_shift_st,
        energy,
        rate,
        worker_rate,
        tag: None,
    }
}

/// Rest dominance is unity. Lower dominance softens, higher dominance is
/// more present. The result is clamped to [`ENERGY_MIN`]..=[`ENERGY_MAX`]
/// by the caller, which is also the worker's energy clamp.
fn dominance_energy(dominance: f32) -> f32 {
    let tilt = |d: f32| 0.85 + 0.30 * d;
    tilt(dominance) / tilt(Pad::RESTING.dominance)
}

fn pre_pause(pad: Pad) -> f32 {
    let air = if pad.arousal < Pad::RESTING.arousal {
        (Pad::RESTING.arousal - pad.arousal) / Pad::RESTING.arousal
    } else {
        0.0
    };
    let soft = if pad.dominance < Pad::RESTING.dominance {
        (Pad::RESTING.dominance - pad.dominance) / Pad::RESTING.dominance
    } else {
        0.0
    };
    // 0.75 at rest dominance, 1.0 when dominance is on the floor.
    PAUSE_CEILING_MS * air * (0.75 + 0.25 * soft)
}

/// Playful, bright, not collapsed and not pushing. A calm smile does not open it.
pub fn giggle_open(pad: Pad) -> bool {
    let pad = pad.clamped();
    pad.valence >= 0.55 && pad.arousal >= 0.48 && (0.40..=0.80).contains(&pad.dominance)
}

/// A breath before a warm, unhurried line. Not a gasp, not a whisper.
pub fn inhale_open(pad: Pad, pre_pause_ms: f32) -> bool {
    let pad = pad.clamped();
    let pause = if pre_pause_ms.is_finite() {
        pre_pause_ms
    } else {
        0.0
    };
    pad.arousal <= 0.36
        && (0.10..=0.70).contains(&pad.valence)
        && pad.dominance >= 0.32
        && (pause >= 40.0 || pad.arousal <= 0.28)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest_pad() -> Pad {
        Pad::RESTING
    }

    #[test]
    fn rest_is_the_samantha_e0_line() {
        let p = map(rest_pad());
        assert_eq!(p.exaggeration, EXAGGERATION_REST);
        assert_eq!(p.length_scale, BASE_LENGTH);
        assert_eq!(p.noise_scale, BASE_NOISE);
        assert_eq!(p.noise_w, BASE_NOISE_W);
        assert_eq!(p.sentence_silence_s, BASE_SENTENCE_SILENCE_S);
        assert_eq!(p.pre_pause_ms, BASE_PRE_PAUSE_MS);
        assert_eq!(p.pitch_shift_st, BASE_PITCH_ST);
        assert_eq!(p.energy, BASE_ENERGY);
        assert_eq!(p.rate, 1.0);
        assert_eq!(p.worker_rate, 1.0);
        assert_eq!(p.piper_length_scale(), BASE_LENGTH);
        assert!(p.tag.is_none());
        assert!(p.in_bounds());
        assert!(!p.whisper());
    }

    #[test]
    fn high_arousal_is_faster_and_clamped() {
        let p = map(Pad {
            arousal: 1.0,
            valence: Pad::RESTING.valence,
            dominance: Pad::RESTING.dominance,
        });
        assert_eq!(p.length_scale, LENGTH_MIN);
        assert_eq!(p.energy, ENERGY_MAX);
        assert!(p.noise_scale > BASE_NOISE);
        assert_eq!(p.noise_w, NOISE_W_MAX);
        assert_eq!(p.sentence_silence_s, SILENCE_MIN_S);
        assert_eq!(p.pre_pause_ms, 0.0);
        assert!(p.exaggeration > EXAGGERATION_REST);
        assert!(p.exaggeration <= EXAGGERATION_MAX);
        assert!(p.worker_rate <= WORKER_RATE_MAX);
        assert!(p.in_bounds());
    }

    #[test]
    fn low_arousal_slows_and_adds_air() {
        let p = map(Pad {
            arousal: 0.0,
            valence: Pad::RESTING.valence,
            dominance: Pad::RESTING.dominance,
        });
        assert!(p.length_scale > BASE_LENGTH);
        assert!(p.length_scale <= LENGTH_MAX);
        assert!(p.pre_pause_ms > 0.0);
        assert!(p.pre_pause_ms <= PAUSE_CEILING_MS);
        assert!(p.sentence_silence_s > BASE_SENTENCE_SILENCE_S);
        assert!(p.energy >= ENERGY_MIN);
        assert!(p.in_bounds());
    }

    #[test]
    fn valence_moves_pitch_and_not_the_reply() {
        let low = map(Pad {
            arousal: Pad::RESTING.arousal,
            valence: -0.2,
            dominance: Pad::RESTING.dominance,
        });
        let high = map(Pad {
            arousal: Pad::RESTING.arousal,
            valence: 0.8,
            dominance: Pad::RESTING.dominance,
        });
        assert!(high.pitch_shift_st > 0.0);
        assert!(low.pitch_shift_st < 0.0);
        assert!(high.pitch_shift_st > low.pitch_shift_st);
        assert!(high.exaggeration > low.exaggeration);
    }

    #[test]
    fn dominance_softens_without_a_whisper() {
        let low = map(Pad {
            arousal: 0.1,
            valence: Pad::RESTING.valence,
            dominance: 0.1,
        });
        let high = map(Pad {
            arousal: 0.1,
            valence: Pad::RESTING.valence,
            dominance: 0.9,
        });
        assert!(low.pre_pause_ms > high.pre_pause_ms);
        assert!(high.energy > low.energy);
        assert!(low.energy >= ENERGY_MIN);
        assert!(high.energy <= ENERGY_MAX);
        assert!(!low.whisper() && !high.whisper());
    }

    #[test]
    fn exaggeration_stays_in_the_s711_band() {
        let samples = [
            Pad::RESTING,
            Pad {
                arousal: 0.0,
                valence: -1.0,
                dominance: 0.0,
            },
            Pad {
                arousal: 1.0,
                valence: 1.0,
                dominance: 1.0,
            },
            Pad {
                arousal: 1.0,
                valence: -1.0,
                dominance: 0.2,
            },
            Pad {
                arousal: f32::NAN,
                valence: f32::INFINITY,
                dominance: -4.0,
            },
        ];
        for pad in samples {
            let ex = map(pad).exaggeration;
            assert!(
                (EXAGGERATION_MIN..=EXAGGERATION_MAX).contains(&ex),
                "{pad:?} -> {ex}"
            );
        }
    }

    #[test]
    fn gates_are_closed_at_rest_and_disjoint() {
        let rest = map(Pad::RESTING);
        assert!(!giggle_open(Pad::RESTING));
        assert!(!inhale_open(Pad::RESTING, rest.pre_pause_ms));
        for a in 0..=20 {
            for v in -8..=8 {
                for d in 0..=8 {
                    let pad = Pad {
                        arousal: a as f32 / 20.0,
                        valence: v as f32 / 8.0,
                        dominance: d as f32 / 8.0,
                    };
                    let pause = map(pad).pre_pause_ms;
                    assert!(
                        !(giggle_open(pad) && inhale_open(pad, pause)),
                        "both gates open at {pad:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn tags_are_not_whisper_and_not_on_piper() {
        for tag in [Tag::Inhale, Tag::Giggle] {
            assert!(!tag.on_piper());
            assert!(tag.on_clone());
            assert!(!tag.persona_allows());
            assert!(!is_whisper_token(tag.token()));
            assert!(tag.token().starts_with('[') && tag.token().ends_with(']'));
        }
        assert!(is_whisper_token("[whisper]"));
        assert!(is_whisper_token("[Whispering]"));
    }
}
