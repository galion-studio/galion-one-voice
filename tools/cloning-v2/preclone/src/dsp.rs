// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Shared frame measurements. One window, one hop, used by voice and segmentation.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

pub(crate) const WIN_MS: u32 = 40;
pub(crate) const HOP_MS: u32 = 10;

const PI: f64 = std::f64::consts::PI;

pub(crate) fn ms_samples(sr: u32, ms: u32) -> usize {
    ((u64::from(sr) * u64::from(ms)) / 1000) as usize
}

pub(crate) fn db10(power: f64) -> f64 {
    if power <= 1e-20 {
        -200.0
    } else {
        10.0 * power.log10()
    }
}

pub(crate) fn median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    }
}

pub(crate) fn quantile(xs: &[f64], p: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let t = p.clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let i = t.floor() as usize;
    let f = t - i as f64;
    if i + 1 >= v.len() {
        v[i]
    } else {
        v[i] * (1.0 - f) + v[i + 1] * f
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Feat {
    pub start: usize,
    pub db: f64,
    pub f0_hz: f64,
    pub harmonicity: f64,
    pub flatness: f64,
    pub residual_ratio: f64,
    pub centroid_hz: f64,
}

pub(crate) fn frame_features(samples: &[f32], sr: u32) -> Vec<Feat> {
    let win = ms_samples(sr, WIN_MS).max(8);
    let hop = ms_samples(sr, HOP_MS).max(1);
    if samples.len() < win || sr == 0 {
        return Vec::new();
    }
    let mut planner = FftPlanner::<f64>::new();
    let mut out = Vec::new();
    let mut start = 0usize;
    while start + win <= samples.len() {
        let raw = &samples[start..start + win];
        let mut power = 0.0;
        for s in raw {
            let v = *s as f64;
            power += v * v;
        }
        power /= win as f64;
        let shaped = shape(raw);
        let (f0, harm) = pitch(&shaped, sr, &mut planner);
        let spec = spectrum(&shaped, sr, &mut planner);
        let (flat, residual, centroid) = spectral_shape(&spec, sr, f0);
        out.push(Feat {
            start,
            db: db10(power),
            f0_hz: f0,
            harmonicity: harm,
            flatness: flat,
            residual_ratio: residual,
            centroid_hz: centroid,
        });
        start += hop;
    }
    out
}

fn shape(raw: &[f32]) -> Vec<f64> {
    let n = raw.len();
    let mean = raw.iter().map(|s| *s as f64).sum::<f64>() / n as f64;
    let mut x = Vec::with_capacity(n);
    let denom = (n as f64).max(1.0);
    for (i, s) in raw.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / denom).cos();
        x.push((*s as f64 - mean) * w);
    }
    x
}

/// Shortest strong autocorrelation peak in the speech F0 range.
/// The shortest peak avoids locking onto a subharmonic.
fn pitch(x: &[f64], sr: u32, planner: &mut FftPlanner<f64>) -> (f64, f64) {
    let n = x.len();
    let nfft = (n * 2).next_power_of_two().max(16);
    let fft = planner.plan_fft_forward(nfft);
    let ifft = planner.plan_fft_inverse(nfft);
    let mut buf = vec![Complex::new(0.0, 0.0); nfft];
    for (i, v) in x.iter().enumerate() {
        buf[i] = Complex::new(*v, 0.0);
    }
    fft.process(&mut buf);
    for c in &mut buf {
        *c = Complex::new(c.norm_sqr(), 0.0);
    }
    ifft.process(&mut buf);
    let r0 = buf[0].re;
    if r0 <= 1e-18 {
        return (0.0, 0.0);
    }
    let min_lag = (f64::from(sr) / 400.0).ceil() as usize;
    let max_lag = ((f64::from(sr) / 70.0).floor() as usize)
        .min(nfft - 2)
        .min(n.saturating_sub(1));
    if min_lag == 0 || max_lag <= min_lag {
        return (0.0, 0.0);
    }
    let mut max_r = 0.0f64;
    for bin in buf.iter().take(max_lag + 1).skip(min_lag) {
        max_r = max_r.max(bin.re / r0);
    }
    if max_r < 0.25 {
        return (0.0, max_r.max(0.0));
    }
    let thresh = max_r * 0.90;
    let mut chosen = 0usize;
    for lag in min_lag..=max_lag {
        let r = buf[lag].re / r0;
        if r < thresh {
            continue;
        }
        let prev = buf[lag - 1].re / r0;
        let next = buf[lag + 1].re / r0;
        if r >= prev && r >= next {
            chosen = lag;
            break;
        }
    }
    if chosen == 0 {
        return (0.0, max_r.max(0.0));
    }
    let y0 = buf[chosen - 1].re / r0;
    let y1 = buf[chosen].re / r0;
    let y2 = buf[chosen + 1].re / r0;
    let den = y0 - 2.0 * y1 + y2;
    let delta = if den.abs() > 1e-12 {
        (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let lag_f = chosen as f64 + delta;
    if lag_f <= 1.0 {
        return (0.0, y1.max(0.0));
    }
    (f64::from(sr) / lag_f, y1.max(0.0))
}

struct Spec {
    mag: Vec<f64>,
    nfft: usize,
}

fn spectrum(x: &[f64], _sr: u32, planner: &mut FftPlanner<f64>) -> Spec {
    let nfft = x.len().next_power_of_two().max(1024);
    let fft = planner.plan_fft_forward(nfft);
    let mut buf = vec![Complex::new(0.0, 0.0); nfft];
    for (i, v) in x.iter().enumerate() {
        buf[i] = Complex::new(*v, 0.0);
    }
    fft.process(&mut buf);
    let mag = buf.iter().take(nfft / 2 + 1).map(|c| c.norm()).collect();
    Spec { mag, nfft }
}

fn spectral_shape(spec: &Spec, sr: u32, f0: f64) -> (f64, f64, f64) {
    let nfft = spec.nfft;
    let bin_hz = f64::from(sr) / nfft as f64;
    let k_lo = ((80.0 / bin_hz).ceil() as usize).max(1);
    let k_hi = ((5000.0 / bin_hz).floor() as usize).min(spec.mag.len().saturating_sub(1));
    if k_hi <= k_lo {
        return (1.0, 1.0, 0.0);
    }
    let mut log_sum = 0.0;
    let mut lin_sum = 0.0;
    let mut harm = 0.0;
    let mut total = 0.0;
    let mut centroid_num = 0.0;
    let mut centroid_den = 0.0;
    let mut n = 0.0;
    for k in k_lo..=k_hi {
        let m = spec.mag[k];
        let p = m * m;
        log_sum += p.max(1e-20).ln();
        lin_sum += p;
        n += 1.0;
        let hz = k as f64 * bin_hz;
        total += p;
        centroid_num += hz * m;
        centroid_den += m;
        if on_harmonic(hz, f0, bin_hz) {
            harm += p;
        }
    }
    let flat = if lin_sum > 0.0 && n > 0.0 {
        (log_sum / n).exp() / (lin_sum / n)
    } else {
        1.0
    };
    let residual = if total > 0.0 {
        (1.0 - harm / total).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let centroid = if centroid_den > 0.0 {
        centroid_num / centroid_den
    } else {
        0.0
    };
    (flat.clamp(0.0, 1.0), residual, centroid)
}

fn on_harmonic(hz: f64, f0: f64, bin_hz: f64) -> bool {
    if f0 <= 0.0 || hz <= 0.0 {
        return false;
    }
    let k = (hz / f0).round();
    if k < 1.0 {
        return false;
    }
    (hz - k * f0).abs() <= (bin_hz * 2.5).max(0.04 * k * f0)
}

/// Mean power spectrum of one chunk, for the noise-colour slope.
pub(crate) fn power_spectrum(chunk: &[f32], nfft: usize) -> Vec<f64> {
    let nfft = nfft.max(16).next_power_of_two();
    if chunk.len() < nfft {
        return vec![0.0; nfft / 2 + 1];
    }
    let off = (chunk.len() - nfft) / 2;
    let slice = &chunk[off..off + nfft];
    let mean = slice.iter().map(|s| *s as f64).sum::<f64>() / nfft as f64;
    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(nfft);
    let mut buf = vec![Complex::new(0.0, 0.0); nfft];
    for (i, s) in slice.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / nfft as f64).cos();
        buf[i] = Complex::new((*s as f64 - mean) * w, 0.0);
    }
    fft.process(&mut buf);
    buf.iter()
        .take(nfft / 2 + 1)
        .map(|c| c.norm_sqr())
        .collect()
}

#[cfg(test)]
mod probe {
    #[test]
    fn a_formant_pulse_is_one_harmonic_series() {
        let x = crate::synth::formant_voice(
            16_000,
            16_000,
            140.0,
            0.05,
            &[(500.0, 80.0), (1500.0, 110.0), (2500.0, 140.0)],
        );
        let feats = super::frame_features(&x, 16_000);
        let mid = &feats[feats.len() / 2];
        assert!((mid.f0_hz - 140.0).abs() < 2.0, "{}", mid.f0_hz);
        assert!(mid.harmonicity > 0.6, "{}", mid.harmonicity);
        assert!(mid.residual_ratio < 0.2, "{}", mid.residual_ratio);
    }
}
