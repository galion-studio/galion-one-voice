// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Glue around `pitch-detection`'s YIN detector.
//!
//! The detector is the pitch. Everything else is a scalar: RMS, a 10 ms
//! envelope for rate and pause, and cycle peaks for jitter and shimmer.
//! The open-space point is a fixed map. It was not fit on a corpus.
//!
//! Jitter and shimmer are the relative local forms: the mean absolute
//! difference of successive periods (or peak amplitudes) divided by the
//! mean. Boersma, "Accurate short-term analysis of the fundamental frequency
//! and the harmonics-to-noise ratio of a sampled sound", Proceedings of the
//! Institute of Phonetic Sciences of the University of Amsterdam 17 (1993).
//!
//! Arousal follows the direction Juslin and Laukka report as the reliable
//! vocal cue (louder, higher, faster). Valence is the weak axis: roughness
//! and a long pause pull it down. Juslin and Laukka, Psychological Bulletin
//! 129(5), 2003. No name is assigned.

use pitch_detection::detector::yin::YINDetector;
use pitch_detection::detector::PitchDetector;

use crate::port::{ListenerOffer, APPROACH, AROUSAL, AXES, DOMINANCE, VALENCE};
use crate::{FRAME_SAMPLES, SAMPLE_RATE};

const HOP: usize = 160;
const HOPS_PER_FRAME: usize = FRAME_SAMPLES / HOP;
/// 200 × 10 ms = 2 s of envelope scalars. Not samples.
const HOP_RING: usize = 200;
const SILENCE_RMS: f32 = 0.012;
/// Onsets closer than this are one event. 10 hops = 100 ms.
const MIN_GAP_HOPS: usize = 10;
const F0_MIN_HZ: f32 = 70.0;
const F0_MAX_HZ: f32 = 450.0;
const POWER_FLOOR: f32 = 1.0e-3;
const CLARITY_MIN: f32 = 0.75;

const _: () = assert!(HOP * HOPS_PER_FRAME == FRAME_SAMPLES);

pub(crate) struct Memory {
    pitch: YINDetector<f32>,
    hops: [f32; HOP_RING],
    hop_len: usize,
    hop_next: usize,
    pause_hops: u32,
    t_ms: u32,
    frames: u32,
}

impl Memory {
    pub(crate) fn new() -> Self {
        Self {
            pitch: YINDetector::new(FRAME_SAMPLES, FRAME_SAMPLES / 2),
            hops: [0.0; HOP_RING],
            hop_len: 0,
            hop_next: 0,
            pause_hops: 0,
            t_ms: 0,
            frames: 0,
        }
    }

    pub(crate) fn frames(&self) -> u32 {
        self.frames
    }

    pub(crate) fn push(&mut self, pcm: &[f32], consent: u8) -> crate::ProsodyFrame {
        let energy = rms(pcm);
        self.push_hops(pcm);
        let (f0_hz, voicing, voiced) = self.pitch_of(pcm, energy);
        let (jitter, shimmer) = if voiced {
            roughness(pcm, f0_hz)
        } else {
            (0.0, 0.0)
        };
        let heard = Heard {
            energy,
            f0_hz,
            voiced,
            voicing,
            speaking_rate_hz: self.rate(),
            pause_ms: self.pause_hops.saturating_mul(10),
            jitter,
            shimmer,
        };
        let t_ms = self.t_ms;
        let offer = map_offer(t_ms, consent, &heard);
        self.t_ms = self.t_ms.wrapping_add(crate::FRAME_MS);
        self.frames = self.frames.wrapping_add(1);
        crate::ProsodyFrame {
            f0_hz: heard.f0_hz,
            voiced: heard.voiced,
            voicing: heard.voicing,
            energy: heard.energy,
            speaking_rate_hz: heard.speaking_rate_hz,
            pause_ms: heard.pause_ms,
            jitter: heard.jitter,
            shimmer: heard.shimmer,
            offer,
        }
    }

    fn pitch_of(&mut self, pcm: &[f32], energy: f32) -> (f32, f32, bool) {
        if energy < SILENCE_RMS {
            return (0.0, 0.0, false);
        }
        // The crate's `clarity` is not a 0..=1 score in this build: a pure
        // sine comes back with the right hertz and a negative clarity. Voicing
        // is the normalised autocorrelation at the period YIN returned.
        let Some(found) = self
            .pitch
            .get_pitch(pcm, SAMPLE_RATE as usize, POWER_FLOOR, CLARITY_MIN)
        else {
            return (0.0, 0.0, false);
        };
        let hz = found.frequency;
        if !(hz.is_finite() && (F0_MIN_HZ..=F0_MAX_HZ).contains(&hz)) {
            return (0.0, 0.0, false);
        }
        let voicing = period_correlation(pcm, hz);
        if voicing >= 0.55 {
            (hz, voicing, true)
        } else {
            (0.0, 0.0, false)
        }
    }

    fn push_hops(&mut self, pcm: &[f32]) {
        let mut hop_i = 0;
        while hop_i < HOPS_PER_FRAME {
            let offset = hop_i * HOP;
            let hop = &pcm[offset..offset + HOP];
            let r = rms(hop);
            self.hops[self.hop_next] = r;
            self.hop_next = (self.hop_next + 1) % HOP_RING;
            if self.hop_len < HOP_RING {
                self.hop_len += 1;
            }
            if r < SILENCE_RMS {
                self.pause_hops = self.pause_hops.saturating_add(1);
            } else {
                self.pause_hops = 0;
            }
            hop_i += 1;
        }
    }

    fn rate(&self) -> f32 {
        let mut env = [0.0f32; HOP_RING];
        let n = self.hop_len;
        let start = if n == HOP_RING { self.hop_next } else { 0 };
        let mut i = 0;
        while i < n {
            env[i] = self.hops[(start + i) % HOP_RING];
            i += 1;
        }
        rate_of(&env[..n])
    }
}

fn rate_of(env: &[f32]) -> f32 {
    if env.len() < 2 {
        return 0.0;
    }
    let mut max = 0.0f32;
    for r in env {
        if *r > max {
            max = *r;
        }
    }
    let thresh = (0.45 * max).max(SILENCE_RMS);
    let mut nuclei = 0u32;
    let mut last: Option<usize> = None;
    let mut i = 1;
    while i < env.len() {
        if env[i - 1] < thresh && env[i] >= thresh {
            let far = last.map(|p| i - p >= MIN_GAP_HOPS).unwrap_or(true);
            if far {
                nuclei += 1;
                last = Some(i);
            }
        }
        i += 1;
    }
    let secs = env.len() as f32 * 0.010;
    if secs <= 0.0 {
        0.0
    } else {
        nuclei as f32 / secs
    }
}

/// Normalised autocorrelation at the nearest integer period. 1 is periodic.
fn period_correlation(pcm: &[f32], hz: f32) -> f32 {
    let tau = (SAMPLE_RATE as f32 / hz).round() as usize;
    if tau < 2 || tau >= pcm.len() / 2 {
        return 0.0;
    }
    let n = pcm.len() - tau;
    let mut dot = 0.0f32;
    let mut e0 = 0.0f32;
    let mut e1 = 0.0f32;
    let mut i = 0;
    while i < n {
        let a = pcm[i];
        let b = pcm[i + tau];
        dot += a * b;
        e0 += a * a;
        e1 += b * b;
        i += 1;
    }
    let denom = (e0 * e1).sqrt();
    if denom < 1.0e-8 {
        0.0
    } else {
        (dot / denom).clamp(0.0, 1.0)
    }
}

fn rms(pcm: &[f32]) -> f32 {
    let mut acc = 0.0f32;
    for x in pcm {
        acc += x * x;
    }
    (acc / pcm.len() as f32).sqrt()
}

fn roughness(pcm: &[f32], f0: f32) -> (f32, f32) {
    let period = SAMPLE_RATE as f32 / f0;
    if !(period.is_finite() && (32.0..=400.0).contains(&period)) {
        return (0.0, 0.0);
    }
    let min_dist = (period * 0.55) as usize;
    let mut pos = [0usize; 40];
    let mut amp = [0.0f32; 40];
    let mut n = 0usize;
    let mut last = 0usize;
    let mut i = 1usize;
    while i + 1 < pcm.len() && n < pos.len() {
        let x = pcm[i];
        if x > pcm[i - 1]
            && x >= pcm[i + 1]
            && x > 0.0
            && (n == 0 || i.saturating_sub(last) >= min_dist)
        {
            pos[n] = i;
            amp[n] = x;
            last = i;
            n += 1;
        }
        i += 1;
    }
    if n < 4 {
        return (0.0, 0.0);
    }
    let nper = n - 1;
    let mut periods = [0.0f32; 39];
    let mut sum_t = 0.0f32;
    let mut k = 0;
    while k < nper {
        periods[k] = (pos[k + 1] - pos[k]) as f32;
        sum_t += periods[k];
        k += 1;
    }
    let mean_t = sum_t / nper as f32;
    if mean_t < 1.0 {
        return (0.0, 0.0);
    }
    let mut sum_abs_dt = 0.0f32;
    k = 0;
    while k + 1 < nper {
        sum_abs_dt += (periods[k] - periods[k + 1]).abs();
        k += 1;
    }
    let jitter = (sum_abs_dt / (nper - 1) as f32) / mean_t;

    let mut sum_a = 0.0f32;
    k = 0;
    while k < n {
        sum_a += amp[k];
        k += 1;
    }
    let mean_a = sum_a / n as f32;
    if mean_a < 1.0e-6 {
        return (0.0, 0.0);
    }
    let mut sum_abs_da = 0.0f32;
    k = 0;
    while k + 1 < n {
        sum_abs_da += (amp[k] - amp[k + 1]).abs();
        k += 1;
    }
    let shimmer = (sum_abs_da / (n - 1) as f32) / mean_a;
    (jitter.clamp(0.0, 1.0), shimmer.clamp(0.0, 1.0))
}

fn unit(x: f32, lo: f32, hi: f32) -> f32 {
    if !x.is_finite() || hi <= lo {
        return 0.0;
    }
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

struct Heard {
    energy: f32,
    f0_hz: f32,
    voiced: bool,
    voicing: f32,
    speaking_rate_hz: f32,
    pause_ms: u32,
    jitter: f32,
    shimmer: f32,
}

fn map_offer(t_ms: u32, consent: u8, heard: &Heard) -> ListenerOffer {
    let n_e = unit(heard.energy, 0.012, 0.25);
    let n_f = if heard.voiced {
        unit(heard.f0_hz, 80.0, 320.0)
    } else {
        0.0
    };
    let n_rate = unit(heard.speaking_rate_hz, 0.0, 6.0);
    let n_pause = unit(heard.pause_ms as f32, 0.0, 800.0);
    let n_rough = 0.5 * (unit(heard.jitter, 0.0, 0.08) + unit(heard.shimmer, 0.0, 0.25));
    let mut point = [0.0; AXES];
    point[VALENCE] =
        (0.05 + 0.16 * n_e + 0.10 * n_f + 0.08 * n_rate - 0.70 * n_rough - 0.40 * n_pause)
            .clamp(-1.0, 1.0);
    point[AROUSAL] =
        (0.06 + 0.52 * n_e + 0.22 * n_f + 0.18 * n_rate - 0.28 * n_pause).clamp(0.0, 1.0);
    point[DOMINANCE] =
        (0.20 + 0.45 * n_e + 0.10 * n_f + 0.10 * n_rate - 0.25 * n_pause).clamp(0.0, 1.0);
    point[APPROACH] =
        (0.10 * n_e + 0.45 * n_rate - 0.50 * n_pause - 0.15 * n_rough).clamp(-1.0, 1.0);
    let strength = if heard.voiced {
        (0.20 + 0.80 * heard.voicing) * n_e.clamp(0.15, 1.0)
    } else {
        0.35 * n_e
    };
    ListenerOffer {
        t_ms,
        consent: consent & 0x0f,
        point,
        strength: strength.clamp(0.0, 1.0),
    }
}
