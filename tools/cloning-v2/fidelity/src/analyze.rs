//! Frame SNR, voicing, and pitch. Shared by segment selection and the MOS proxy.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::audio;

pub const F0_LO: f32 = 70.0;
pub const F0_HI: f32 = 400.0;
pub const PITCH_BINS: usize = 8;

#[derive(Clone, Debug)]
pub struct Frame {
    pub center: usize,
    pub rms: f32,
    pub voiced: bool,
    pub f0_hz: f32,
}

#[derive(Clone, Debug)]
pub struct Analysis {
    pub sr: u32,
    pub frame_len: usize,
    pub hop: usize,
    pub frames: Vec<Frame>,
    pub noise_floor: f32,
    pub stationary: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub snr_db: f64,
    pub voiced_ratio: f64,
    pub pitch_coverage: f64,
}

pub fn pitch_bin(f0: f32) -> Option<usize> {
    if !(f0 >= F0_LO && f0 <= F0_HI) {
        return None;
    }
    let t = (f0.ln() - F0_LO.ln()) / (F0_HI.ln() - F0_LO.ln());
    let idx = (t * PITCH_BINS as f32).floor() as usize;
    Some(idx.min(PITCH_BINS - 1))
}

pub fn analyze(samples: &[f32], sr: u32) -> Analysis {
    let owned;
    let y = if samples.iter().any(|v| !v.is_finite()) {
        owned = audio::sanitize(samples);
        owned.as_slice()
    } else {
        samples
    };
    if sr < 1000 || y.is_empty() {
        return Analysis {
            sr,
            frame_len: 0,
            hop: 0,
            frames: Vec::new(),
            noise_floor: 1e-6,
            stationary: true,
        };
    }
    let frame_len = ((sr as f32) * 0.025).round().max(64.0) as usize;
    let hop = ((sr as f32) * 0.020).round().max(32.0) as usize;
    let mut starts = Vec::new();
    if y.len() <= frame_len {
        starts.push(0usize);
    } else {
        let mut s = 0usize;
        while s + frame_len <= y.len() {
            starts.push(s);
            s += hop;
        }
        if starts.is_empty() {
            starts.push(0);
        }
    }
    let rms: Vec<f32> = starts
        .iter()
        .map(|&s| {
            let end = (s + frame_len).min(y.len());
            audio::rms(&y[s..end])
        })
        .collect();
    let p10 = audio::percentile(&rms, 0.10).max(1e-6);
    let p90 = audio::percentile(&rms, 0.90).max(p10);
    let range_db = 20.0 * (p90 / p10).log10();
    let stationary = range_db < 6.0;
    let noise_floor = p10;
    // p10 is only a noise floor when few frames sit on it. A large cluster near p10
    // is quieter speech, and an RMS gate would mark that speech unvoiced.
    let near_floor = rms.iter().filter(|r| **r <= p10 * 2.0).count();
    let floor_is_noise = (near_floor as f32) / (rms.len().max(1) as f32) < 0.25;
    let rms_gate = if stationary || !floor_is_noise {
        1e-4
    } else {
        (2.5 * noise_floor).max(1e-4)
    };

    let n_fft = (frame_len * 2).next_power_of_two().max(2048);
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n_fft);
    let inverse = planner.plan_fft_inverse(n_fft);

    let mut frames = Vec::with_capacity(starts.len());
    for (i, &s) in starts.iter().enumerate() {
        let end = (s + frame_len).min(y.len());
        let center = s + (end - s) / 2;
        let (voiced, f0) = if rms[i] >= rms_gate {
            pitch_of(&y[s..end], sr, n_fft, forward.as_ref(), inverse.as_ref())
        } else {
            (false, 0.0)
        };
        frames.push(Frame {
            center,
            rms: rms[i],
            voiced,
            f0_hz: f0,
        });
    }
    Analysis {
        sr,
        frame_len,
        hop,
        frames,
        noise_floor,
        stationary,
    }
}

fn pitch_of(
    frame: &[f32],
    sr: u32,
    n_fft: usize,
    forward: &dyn Fft<f64>,
    inverse: &dyn Fft<f64>,
) -> (bool, f32) {
    if frame.len() < 32 {
        return (false, 0.0);
    }
    let mean = frame.iter().sum::<f32>() / frame.len() as f32;
    let mut buf = vec![Complex::new(0.0, 0.0); n_fft];
    for (i, &v) in frame.iter().enumerate() {
        if i >= n_fft {
            break;
        }
        buf[i] = Complex::new((v - mean) as f64, 0.0);
    }
    forward.process(&mut buf);
    for bin in &mut buf {
        let power = bin.re * bin.re + bin.im * bin.im;
        *bin = Complex::new(power, 0.0);
    }
    inverse.process(&mut buf);
    let scale = 1.0 / n_fft as f64;
    let acf0 = buf[0].re * scale;
    if acf0 < 1e-8 {
        return (false, 0.0);
    }
    let lag_min = (sr as f32 / F0_HI).round().max(1.0) as usize;
    let lag_max = ((sr as f32 / F0_LO).round() as usize)
        .min(frame.len().saturating_sub(2))
        .min(n_fft / 2);
    if lag_max <= lag_min {
        return (false, 0.0);
    }
    let mut best_lag = lag_min;
    let mut best = f64::NEG_INFINITY;
    for lag in lag_min..=lag_max {
        let value = buf[lag].re * scale / acf0;
        if value > best {
            best = value;
            best_lag = lag;
        }
    }
    if best < 0.50 {
        return (false, 0.0);
    }
    (true, sr as f32 / best_lag as f32)
}

pub fn region_stats(samples: &[f32], analysis: &Analysis, start: usize, end: usize) -> Region {
    let end = end.min(samples.len()).max(start);
    let slice = &samples[start..end];
    let win_rms = audio::rms(slice);
    let frames: Vec<&Frame> = analysis
        .frames
        .iter()
        .filter(|frame| frame.center >= start && frame.center < end)
        .collect();
    let voiced_ratio = if frames.is_empty() {
        0.0
    } else {
        frames.iter().filter(|frame| frame.voiced).count() as f64 / frames.len() as f64
    };
    let mut seen = [false; PITCH_BINS];
    let mut voiced_n = 0usize;
    for frame in &frames {
        if frame.voiced {
            if let Some(bin) = pitch_bin(frame.f0_hz) {
                seen[bin] = true;
                voiced_n += 1;
            }
        }
    }
    let pitch_coverage = if voiced_n == 0 {
        0.0
    } else {
        seen.iter().filter(|on| **on).count() as f64 / PITCH_BINS as f64
    };
    let snr_db = if analysis.stationary {
        if voiced_ratio >= 0.55 {
            28.0
        } else {
            0.0
        }
    } else {
        20.0 * ((win_rms as f64 + 1e-8) / (analysis.noise_floor as f64 + 1e-8)).log10()
    };
    Region {
        snr_db,
        voiced_ratio,
        pitch_coverage,
    }
}

pub fn median_f0(samples: &[f32], sr: u32) -> Option<f32> {
    let analysis = analyze(samples, sr);
    let mut values: Vec<f32> = analysis
        .frames
        .iter()
        .filter(|frame| frame.voiced)
        .map(|frame| frame.f0_hz)
        .collect();
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Some(values[values.len() / 2])
}

pub fn voiced_ratio(samples: &[f32], sr: u32) -> f64 {
    let analysis = analyze(samples, sr);
    if analysis.frames.is_empty() {
        return 0.0;
    }
    analysis.frames.iter().filter(|frame| frame.voiced).count() as f64
        / analysis.frames.len() as f64
}

pub fn welch_power(samples: &[f32], n_fft: usize) -> Vec<f64> {
    let n_fft = n_fft.max(16);
    let hop = (n_fft / 2).max(1);
    let window: Vec<f64> = (0..n_fft)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n_fft as f64).cos())
        .collect();
    let mut planner = FftPlanner::<f64>::new();
    let forward = planner.plan_fft_forward(n_fft);
    let mut acc = vec![0.0f64; n_fft / 2 + 1];
    if samples.is_empty() {
        return acc;
    }
    let mut frames = 0u32;
    let mut pos = 0usize;
    loop {
        let mut buf = vec![Complex::new(0.0, 0.0); n_fft];
        let mut any = false;
        for i in 0..n_fft {
            if pos + i < samples.len() {
                buf[i] = Complex::new(samples[pos + i] as f64 * window[i], 0.0);
                any = true;
            }
        }
        if !any {
            break;
        }
        forward.process(&mut buf);
        for (k, bin) in buf.iter().take(n_fft / 2 + 1).enumerate() {
            acc[k] += bin.re * bin.re + bin.im * bin.im;
        }
        frames += 1;
        if pos + n_fft >= samples.len() || frames > 100_000 {
            break;
        }
        pos += hop;
    }
    let denom = frames.max(1) as f64;
    for value in &mut acc {
        *value /= denom;
    }
    acc
}

pub fn spectral_flatness(power: &[f64]) -> f64 {
    let bins: Vec<f64> = power
        .iter()
        .skip(1)
        .copied()
        .filter(|p| *p > 1e-20)
        .collect();
    if bins.is_empty() {
        return 1.0;
    }
    let mean = bins.iter().sum::<f64>() / bins.len() as f64;
    if mean <= 0.0 {
        return 1.0;
    }
    let geometric = (bins.iter().map(|p| p.ln()).sum::<f64>() / bins.len() as f64).exp();
    (geometric / mean).clamp(0.0, 1.0)
}

pub fn clip_fraction(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.iter().filter(|v| v.abs() >= 0.99).count() as f64 / samples.len() as f64
}
