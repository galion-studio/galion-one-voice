// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Prosody over a short window of frame summaries. The window stores scalars
//! (RMS, a voiced bit, F0). It does not store samples.
//!
//! Speaking rate counts peaks in a smoothed energy envelope (10 ms hop,
//! 70 ms moving average so a single pitch cycle does not look like a
//! syllable), at least 100 ms apart. A pause is a quiet run of at least
//! 200 ms. Articulation rate is peaks per second of the rest of the window.
//! Speech rate is peaks per second of the whole window, pauses included.

/// Quiet subframe. A 0.04 sine (RMS ≈ 0.028) stays above this; digital silence does not.
pub const SILENCE_RMS: f32 = 0.012;
/// Subframe hop. Eight of these fill one 80 ms frame at 24 kHz (240 samples).
pub const SUB_MS: u32 = 10;
pub const SUBS_PER_FRAME: usize = 8;
/// Envelope points kept. 256 × 10 ms = 2.56 s.
pub const ENV_LEN: usize = 256;
/// Frame summaries kept. 32 × 80 ms = 2.56 s.
pub const FRAME_RING: usize = 32;
/// Minimum gap between syllable peaks.
pub const MIN_PEAK_GAP: usize = 10;
/// Quiet run that counts as a pause.
pub const PAUSE_MIN_SUBS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRec {
    pub f0_hz: f32,
    pub voiced: bool,
    pub harmonicity: f32,
    pub zcr: f32,
    pub rms: f32,
}

impl FrameRec {
    pub const SILENT: FrameRec = FrameRec {
        f0_hz: 0.0,
        voiced: false,
        harmonicity: 0.0,
        zcr: 0.0,
        rms: 0.0,
    };
}

/// Explainable prosody. Rates are hertz (events per second), not a score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prosody {
    pub f0_hz: f32,
    pub f0_range_hz: f32,
    pub rms: f32,
    pub rms_dbfs: f32,
    pub speech_rate_hz: f32,
    pub articulation_hz: f32,
    pub pause_ratio: f32,
    pub longest_pause_ms: u32,
    pub voiced_ratio: f32,
    pub zcr: f32,
    pub harmonicity: f32,
    pub nuclei: u32,
    pub voiced_frames: u32,
    pub window_ms: u32,
}

impl Prosody {
    pub const EMPTY: Prosody = Prosody {
        f0_hz: 0.0,
        f0_range_hz: 0.0,
        rms: 0.0,
        rms_dbfs: -120.0,
        speech_rate_hz: 0.0,
        articulation_hz: 0.0,
        pause_ratio: 0.0,
        longest_pause_ms: 0,
        voiced_ratio: 0.0,
        zcr: 0.0,
        harmonicity: 0.0,
        nuclei: 0,
        voiced_frames: 0,
        window_ms: 0,
    };
}

pub fn dbfs(rms: f32) -> f32 {
    if rms <= 1e-8 {
        -120.0
    } else {
        20.0 * rms.log10()
    }
}

/// `env` and `env_voiced` are oldest-first. `frames` is oldest-first.
pub fn summarize(env: &[f32], env_voiced: &[bool], frames: &[FrameRec]) -> Prosody {
    let n = env.len().min(env_voiced.len());
    if n == 0 || frames.is_empty() {
        return Prosody::EMPTY;
    }
    let window_ms = (n as u32) * SUB_MS;
    let mut voiced_subs = 0usize;
    let mut sum_voiced_rms = 0.0f32;
    let mut i = 0;
    while i < n {
        let r = if env[i].is_finite() {
            env[i].max(0.0)
        } else {
            0.0
        };
        if env_voiced[i] && r >= SILENCE_RMS {
            voiced_subs += 1;
            sum_voiced_rms += r;
        }
        i += 1;
    }
    // 70 ms moving average. A 10 ms RMS hop still wobbles with the pitch
    // cycle (a 180 Hz period is about 5.6 ms), and that wobble was counting
    // as extra syllables on a slow amplitude sweep.
    let mut smooth = [0.0f32; ENV_LEN];
    let radius = 3usize;
    i = 0;
    while i < n {
        let lo = i.saturating_sub(radius);
        let hi = (i + radius + 1).min(n);
        let mut acc = 0.0f32;
        let mut k = lo;
        while k < hi {
            acc += env[k];
            k += 1;
        }
        smooth[i] = acc / (hi - lo) as f32;
        i += 1;
    }
    let mut max_smooth = 0.0f32;
    i = 0;
    while i < n {
        if smooth[i] > max_smooth {
            max_smooth = smooth[i];
        }
        i += 1;
    }
    let thresh = (0.55 * max_smooth).max(0.02);
    let mut nuclei = 0u32;
    let mut last_peak: isize = -1_000;
    i = 1;
    while i + 1 < n {
        let r = smooth[i];
        // A steady tone's 10 ms RMS still wobbles a few percent with the
        // pitch cycle. A syllable has to rise at least 40% over the minimum
        // of the previous 300 ms, which a flat tone does not do.
        let mut prior_min = r;
        let back = i.saturating_sub(30);
        let mut k = back;
        while k < i {
            if smooth[k] < prior_min {
                prior_min = smooth[k];
            }
            k += 1;
        }
        let prominent = prior_min <= 1e-4 || r >= prior_min * 1.40;
        if env_voiced[i]
            && prominent
            && r >= thresh
            && r > smooth[i - 1]
            && r >= smooth[i + 1]
            && (i as isize - last_peak) >= MIN_PEAK_GAP as isize
        {
            nuclei += 1;
            last_peak = i as isize;
        }
        i += 1;
    }

    let mut longest = 0u32;
    let mut pause_subs = 0usize;
    let mut run = 0usize;
    let flush = |run: usize, longest: &mut u32, pause_subs: &mut usize| {
        let ms = (run as u32) * SUB_MS;
        if ms > *longest {
            *longest = ms;
        }
        if run >= PAUSE_MIN_SUBS {
            *pause_subs += run;
        }
    };
    i = 0;
    while i < n {
        let quiet = !env_voiced[i] || env[i] < SILENCE_RMS;
        if quiet {
            run += 1;
        } else if run > 0 {
            flush(run, &mut longest, &mut pause_subs);
            run = 0;
        }
        i += 1;
    }
    if run > 0 {
        flush(run, &mut longest, &mut pause_subs);
    }

    let mut f0s = [0.0f32; FRAME_RING];
    let mut nf = 0usize;
    let mut harm = 0.0f32;
    let mut zcr = 0.0f32;
    let mut voiced_frames = 0u32;
    for fr in frames {
        if fr.voiced && fr.f0_hz.is_finite() && fr.f0_hz > 0.0 {
            if nf < FRAME_RING {
                f0s[nf] = fr.f0_hz;
                nf += 1;
            }
            harm += fr.harmonicity;
            zcr += fr.zcr;
            voiced_frames += 1;
        }
    }
    let (f0, range) = if nf >= 4 {
        let slice = &mut f0s[..nf];
        insertion_sort(slice);
        let mid = slice[nf / 2];
        let p10 = slice[nf / 10];
        let p90 = slice[((nf - 1) * 9) / 10];
        (mid, (p90 - p10).max(0.0))
    } else {
        (0.0, 0.0)
    };
    let vf = voiced_frames.max(1) as f32;
    let speech_s = (window_ms as f32) / 1000.0;
    let pause_s = (pause_subs as f32) * (SUB_MS as f32) / 1000.0;
    let speaking_s = (speech_s - pause_s).max(0.0);
    let rms = if voiced_subs > 0 {
        sum_voiced_rms / voiced_subs as f32
    } else {
        0.0
    };
    Prosody {
        f0_hz: f0,
        f0_range_hz: range,
        rms,
        rms_dbfs: dbfs(rms),
        speech_rate_hz: if speech_s > 0.0 {
            nuclei as f32 / speech_s
        } else {
            0.0
        },
        articulation_hz: if speaking_s > 0.05 {
            nuclei as f32 / speaking_s
        } else {
            0.0
        },
        pause_ratio: if n > 0 {
            pause_subs as f32 / n as f32
        } else {
            0.0
        },
        longest_pause_ms: longest,
        voiced_ratio: voiced_subs as f32 / n as f32,
        zcr: if voiced_frames > 0 { zcr / vf } else { 0.0 },
        harmonicity: if voiced_frames > 0 { harm / vf } else { 0.0 },
        nuclei,
        voiced_frames,
        window_ms,
    }
}

fn insertion_sort(v: &mut [f32]) {
    let mut i = 1;
    while i < v.len() {
        let key = v[i];
        let mut j = i;
        while j > 0 && v[j - 1] > key {
            v[j] = v[j - 1];
            j -= 1;
        }
        v[j] = key;
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dbfs_of_silence_and_full_scale() {
        assert_eq!(dbfs(0.0), -120.0);
        assert!((dbfs(1.0) - 0.0).abs() < 1e-4);
        assert!((dbfs(0.1) + 20.0).abs() < 1e-3);
    }
}
