// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Room and noise. The floor and the Schroeder RT60 are refcheck's functions.
//! SNR turns that floor and the active-window level back into a power ratio.

use galion_refcheck::dsp::{estimate_rt60, noise_floor_dbfs};
use galion_refcheck::spec::{ReverbSpec, WindowSpec};

use crate::dsp::{self, db10};
use crate::profile::EnvironmentProfile;

const WIN_MS: u32 = 400;
const HOP_MS: u32 = 100;

pub(crate) fn measure_room(samples: &[f32], sr: u32) -> EnvironmentProfile {
    let floor = noise_floor(samples, sr);
    let active = active_level_dbfs(samples, sr, floor);
    let snr = match (active, floor) {
        (Some(a), Some(n)) => snr_from_levels(a, n),
        _ => None,
    };
    let (rt60, note) = match estimate_rt60_s(samples, sr) {
        Ok(v) => (Some(v), String::new()),
        Err(e) => (None, e),
    };
    let tilt = noise_tilt_db_per_octave(samples, sr, floor);
    EnvironmentProfile {
        noise_floor_dbfs: floor,
        active_level_dbfs: active,
        snr_db: snr,
        rt60_s: rt60,
        rt60_note: note,
        noise_tilt_db_per_octave: tilt,
        noise_color: color_of(tilt).to_string(),
        music_bed: false,
    }
}

pub fn noise_floor(samples: &[f32], sr: u32) -> Option<f64> {
    let window = WindowSpec {
        max_dbfs: 0.0,
        window_ms: WIN_MS,
        hop_ms: HOP_MS,
    };
    noise_floor_dbfs(samples, sr, &window)
}

/// Schroeder T20 from `galion_refcheck::dsp::estimate_rt60`, extrapolated to RT60.
pub fn estimate_rt60_s(samples: &[f32], sr: u32) -> Result<f64, String> {
    let spec = ReverbSpec {
        max_rt60_s: 5.0,
        fit_start_db: -5.0,
        fit_end_db: -20.0,
        frame_ms: 10,
        hop_ms: 5,
    };
    estimate_rt60(samples, sr, &spec)
}

pub fn estimate_snr_db(samples: &[f32], sr: u32) -> Option<f64> {
    let floor = noise_floor(samples, sr)?;
    let active = active_level_dbfs(samples, sr, Some(floor))?;
    snr_from_levels(active, floor)
}

/// Active windows measure speech plus noise. `10 log10(10^(diff/10) - 1)` removes the noise.
fn snr_from_levels(active_db: f64, floor_db: f64) -> Option<f64> {
    let ratio = 10f64.powf((active_db - floor_db) / 10.0);
    if ratio <= 1.001 {
        None
    } else {
        Some(10.0 * (ratio - 1.0).log10())
    }
}

fn window_powers(samples: &[f32], sr: u32) -> Vec<(usize, f64)> {
    let win = dsp::ms_samples(sr, WIN_MS);
    let hop = dsp::ms_samples(sr, HOP_MS).max(1);
    if win == 0 || samples.len() < win {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + win <= samples.len() {
        let mut acc = 0.0;
        for s in &samples[i..i + win] {
            let v = *s as f64;
            acc += v * v;
        }
        out.push((i, acc / win as f64));
        i += hop;
    }
    out
}

fn active_level_dbfs(samples: &[f32], sr: u32, floor: Option<f64>) -> Option<f64> {
    let floor = floor?;
    let mut levels = Vec::new();
    for (_, power) in window_powers(samples, sr) {
        let db = db10(power);
        if db >= floor + 6.0 {
            levels.push(db);
        }
    }
    if levels.len() < 2 {
        return None;
    }
    Some(dsp::median(&levels))
}

fn noise_tilt_db_per_octave(samples: &[f32], sr: u32, floor: Option<f64>) -> Option<f64> {
    let floor = floor?;
    let win = dsp::ms_samples(sr, WIN_MS);
    let mut ranges = Vec::new();
    for (start, power) in window_powers(samples, sr) {
        if (db10(power) - floor).abs() <= 3.0 {
            ranges.push(start);
        }
    }
    if ranges.is_empty() || win < 2048 {
        return None;
    }
    let nfft = 4096.min(win).next_power_of_two();
    if nfft > win {
        return None;
    }
    let step = (ranges.len() / 6).max(1);
    let mut acc = vec![0.0; nfft / 2 + 1];
    let mut used = 0usize;
    let mut i = 0usize;
    while i < ranges.len() && used < 6 {
        let start = ranges[i];
        let chunk = &samples[start..start + win];
        let psd = dsp::power_spectrum(chunk, nfft);
        if psd.len() == acc.len() {
            for (a, p) in acc.iter_mut().zip(psd.iter()) {
                *a += *p;
            }
            used += 1;
        }
        i += step;
    }
    if used == 0 {
        return None;
    }
    let bin_hz = f64::from(sr) / nfft as f64;
    let k_lo = ((200.0 / bin_hz).ceil() as usize).max(1);
    let k_hi = ((4000.0 / bin_hz).floor() as usize).min(acc.len().saturating_sub(1));
    if k_hi <= k_lo + 4 {
        return None;
    }
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (k, power) in acc.iter().enumerate().take(k_hi + 1).skip(k_lo) {
        let f = k as f64 * bin_hz;
        xs.push(f.log2());
        ys.push(db10(power / used as f64));
    }
    Some(slope(&xs, &ys))
}

fn slope(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mx = x.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..x.len() {
        let dx = x[i] - mx;
        num += dx * (y[i] - my);
        den += dx * dx;
    }
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

pub(crate) fn color_of(tilt: Option<f64>) -> &'static str {
    match tilt {
        Some(t) if t > -1.5 => "white",
        Some(t) if t < -2.2 => "pink",
        _ => "unknown",
    }
}
