// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Steady baseline of the kept target regions.
//!
//! F0 is the autocorrelation peak. Formants are LPC peaks on that same audio.
//! This is the voice that stays put. It is not the 80 ms emotion moment.

use crate::dsp::{self, Feat};
use crate::profile::{Segment, VoiceProfile};

const PI: f64 = std::f64::consts::PI;

pub(crate) fn measure_voice(samples: &[f32], sr: u32, segments: &[Segment]) -> VoiceProfile {
    let kept = gather(samples, segments);
    let kept_samples = kept.len();
    if kept.len() < dsp::ms_samples(sr, dsp::WIN_MS) {
        return VoiceProfile {
            f0_hz: 0.0,
            f1_hz: 0.0,
            f2_hz: 0.0,
            f3_hz: 0.0,
            spectral_centroid_hz: 0.0,
            hnr_db: 0.0,
            voiced_fraction: 0.0,
            kept_samples,
        };
    }
    let feats = dsp::frame_features(&kept, sr);
    let voiced: Vec<&Feat> = feats
        .iter()
        .filter(|f| f.harmonicity >= 0.40 && f.f0_hz > 0.0)
        .collect();
    let f0s: Vec<f64> = voiced.iter().map(|f| f.f0_hz).collect();
    let cents: Vec<f64> = voiced.iter().map(|f| f.centroid_hz).collect();
    let hnrs: Vec<f64> = voiced.iter().filter_map(|f| hnr(f.harmonicity)).collect();
    let fraction = if feats.is_empty() {
        0.0
    } else {
        voiced.len() as f64 / feats.len() as f64
    };
    let (f1, f2, f3) = formants(&kept, sr);
    VoiceProfile {
        f0_hz: dsp::median(&f0s),
        f1_hz: f1,
        f2_hz: f2,
        f3_hz: f3,
        spectral_centroid_hz: dsp::median(&cents),
        hnr_db: dsp::median(&hnrs),
        voiced_fraction: fraction,
        kept_samples,
    }
}

fn gather(samples: &[f32], segments: &[Segment]) -> Vec<f32> {
    let mut out = Vec::new();
    for s in segments {
        if !s.kept {
            continue;
        }
        let a = s.start_sample.min(samples.len());
        let b = s.end_sample.min(samples.len());
        if b > a {
            out.extend_from_slice(&samples[a..b]);
        }
    }
    out
}

fn hnr(harmonicity: f64) -> Option<f64> {
    if !(0.05..0.999).contains(&harmonicity) {
        return None;
    }
    Some(10.0 * (harmonicity / (1.0 - harmonicity)).log10())
}

fn formants(samples: &[f32], sr: u32) -> (f64, f64, f64) {
    let win = dsp::ms_samples(sr, dsp::WIN_MS).max(8);
    let hop = dsp::ms_samples(sr, dsp::HOP_MS).max(1);
    if samples.len() < win {
        return (0.0, 0.0, 0.0);
    }
    let mut f1s = Vec::new();
    let mut f2s = Vec::new();
    let mut f3s = Vec::new();
    let mut i = 0usize;
    while i + win <= samples.len() {
        if let Some(peaks) = lpc_peaks(&samples[i..i + win], sr) {
            if peaks.len() >= 3 {
                f1s.push(peaks[0]);
                f2s.push(peaks[1]);
                f3s.push(peaks[2]);
            }
        }
        i += hop;
    }
    (dsp::median(&f1s), dsp::median(&f2s), dsp::median(&f3s))
}

fn lpc_peaks(frame: &[f32], sr: u32) -> Option<Vec<f64>> {
    let x = preemph_hann(frame);
    let order = (sr / 1000).clamp(8, 14) as usize;
    if x.len() <= order + 2 {
        return None;
    }
    let mut r = vec![0.0; order + 1];
    for (lag, slot) in r.iter_mut().enumerate() {
        let mut s = 0.0;
        for n in 0..x.len() - lag {
            s += x[n] * x[n + lag];
        }
        *slot = s;
    }
    if r[0] <= 1e-12 {
        return None;
    }
    let a = levinson(&r, order);
    let nfft = 2048usize;
    let mut mag = vec![0.0; nfft / 2 + 1];
    for (bin, m) in mag.iter_mut().enumerate() {
        let w = 2.0 * PI * bin as f64 / nfft as f64;
        let mut re = 0.0;
        let mut im = 0.0;
        for (k, ak) in a.iter().enumerate() {
            let t = w * k as f64;
            re += ak * t.cos();
            im -= ak * t.sin();
        }
        let den = re * re + im * im;
        *m = if den > 1e-18 { 1.0 / den.sqrt() } else { 0.0 };
    }
    Some(pick_peaks(&mag, sr, nfft))
}

fn preemph_hann(frame: &[f32]) -> Vec<f64> {
    let n = frame.len();
    let mean = frame.iter().map(|s| *s as f64).sum::<f64>() / n as f64;
    let mut x = vec![0.0; n];
    for i in 0..n {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos();
        x[i] = (f64::from(frame[i]) - mean) * w;
    }
    let mut y = vec![0.0; n];
    y[0] = x[0];
    for i in 1..n {
        y[i] = x[i] - 0.97 * x[i - 1];
    }
    y
}

/// `A(z) = sum a[k] z^{-k}` with `a[0] = 1`. The envelope is `1/A`.
fn levinson(r: &[f64], order: usize) -> Vec<f64> {
    let mut a = vec![0.0; order + 1];
    a[0] = 1.0;
    let mut e = r[0];
    for i in 1..=order {
        if e.abs() < 1e-18 {
            break;
        }
        let mut acc = 0.0;
        for j in 0..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / e;
        let mut next = vec![0.0; order + 1];
        for j in 0..=i {
            let left = if j <= order { a[j] } else { 0.0 };
            let right = if i >= j { a[i - j] } else { 0.0 };
            next[j] = left + k * right;
        }
        a = next;
        e *= 1.0 - k * k;
    }
    a
}

fn pick_peaks(mag: &[f64], sr: u32, nfft: usize) -> Vec<f64> {
    let bin_hz = f64::from(sr) / nfft as f64;
    let k_lo = ((200.0 / bin_hz).ceil() as usize).max(1);
    let k_hi = ((4000.0 / bin_hz).floor() as usize).min(mag.len().saturating_sub(2));
    let mut peaks: Vec<(f64, f64)> = Vec::new();
    if k_hi <= k_lo {
        return Vec::new();
    }
    for k in k_lo..=k_hi {
        if mag[k] >= mag[k - 1] && mag[k] > mag[k + 1] {
            let y0 = mag[k - 1];
            let y1 = mag[k];
            let y2 = mag[k + 1];
            let den = y0 - 2.0 * y1 + y2;
            let delta = if den.abs() > 1e-12 {
                (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let hz = (k as f64 + delta) * bin_hz;
            peaks.push((hz, y1));
        }
    }
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let mut kept = Vec::new();
    for (hz, _) in peaks {
        if kept.iter().all(|k: &f64| (hz - k).abs() >= 200.0) {
            kept.push(hz);
        }
        if kept.len() == 3 {
            break;
        }
    }
    kept.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    kept
}
