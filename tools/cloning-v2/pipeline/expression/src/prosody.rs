//! Prosody and voice-colour features at 16 kHz.
//!
//! Pitch is a YIN tracker (de Cheveigné & Kawahara). Spectral tilt is a
//! 1/3-octave regression on energy-active frames, so breathy or unvoiced
//! colour still has a slope. Singing is a sustained voiced phrase whose
//! F0 wiggles in the vibrato band (about 4–8 Hz).

use serde::{Deserialize, Serialize};

pub const SR: usize = 16_000;
pub const HOP: usize = 160; // 10 ms
const F0_MIN: f32 = 60.0;
const F0_MAX: f32 = 500.0;
const YIN_W: usize = 400; // 25 ms
const VIBRATO_HZ_LO: f64 = 4.0;
const VIBRATO_HZ_HI: f64 = 8.0;
const SUNG_MIN_S: f64 = 0.40;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProsodyStats {
    pub duration_s: f64,
    pub voiced_frac: f64,
    pub f0_mean_hz: f64,
    pub f0_median_hz: f64,
    pub f0_sd_st: f64,
    pub f0_range_st_p5_p95: f64,
    pub rms_db: f64,
    pub energy_sd_db: f64,
    pub spectral_tilt_db_oct: f64,
    pub vibrato_rate_hz: f64,
    pub vibrato_extent_cents: f64,
    pub sung_phrases: Vec<SungPhrase>,
    /// Downsampled voiced F0 (Hz). Unvoiced hops are omitted.
    pub f0_contour_hz: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SungPhrase {
    pub start_s: f64,
    pub end_s: f64,
    pub f0_median_hz: f64,
    pub vibrato_rate_hz: f64,
    pub vibrato_extent_cents: f64,
    pub f0_contour_hz: Vec<f64>,
}

impl ProsodyStats {
    pub fn empty() -> Self {
        Self {
            duration_s: 0.0,
            voiced_frac: 0.0,
            f0_mean_hz: 0.0,
            f0_median_hz: 0.0,
            f0_sd_st: 0.0,
            f0_range_st_p5_p95: 0.0,
            rms_db: -120.0,
            energy_sd_db: 0.0,
            spectral_tilt_db_oct: 0.0,
            vibrato_rate_hz: 0.0,
            vibrato_extent_cents: 0.0,
            sung_phrases: Vec::new(),
            f0_contour_hz: Vec::new(),
        }
    }
}

pub fn analyze_prosody(x16: &[f32]) -> ProsodyStats {
    if x16.len() < YIN_W + 8 {
        let mut s = ProsodyStats::empty();
        s.duration_s = x16.len() as f64 / SR as f64;
        s.rms_db = rms_db(x16);
        return s;
    }
    let db = frame_db(x16);
    let max_db = db.iter().cloned().fold(f32::MIN, f32::max);
    let gate = (max_db - 35.0).max(-55.0);
    let f0 = yin_f0(x16, &db, gate);
    let voiced: Vec<f32> = f0.iter().copied().filter(|v| *v > 0.0).collect();
    let voiced_frac = if db.is_empty() {
        0.0
    } else {
        voiced.len() as f64 / db.len() as f64
    };
    let (f0_mean, f0_median, f0_sd_st, f0_range) = f0_stats(&voiced);
    let active: Vec<f32> = db.iter().copied().filter(|d| *d > gate).collect();
    let phrases = sung_phrases(&f0);
    let (best_rate, best_extent) = phrases
        .iter()
        .max_by(|a, b| {
            a.vibrato_extent_cents
                .partial_cmp(&b.vibrato_extent_cents)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|p| (p.vibrato_rate_hz, p.vibrato_extent_cents))
        .unwrap_or((0.0, 0.0));
    ProsodyStats {
        duration_s: x16.len() as f64 / SR as f64,
        voiced_frac,
        f0_mean_hz: f0_mean,
        f0_median_hz: f0_median,
        f0_sd_st,
        f0_range_st_p5_p95: f0_range,
        rms_db: rms_db(x16),
        energy_sd_db: sample_sd(&active) as f64,
        spectral_tilt_db_oct: spectral_tilt(x16, &db, gate),
        vibrato_rate_hz: best_rate,
        vibrato_extent_cents: best_extent,
        f0_contour_hz: downsample_voiced(&f0, 48),
        sung_phrases: phrases,
    }
}

fn rms_db(x: &[f32]) -> f64 {
    if x.is_empty() {
        return -120.0;
    }
    let mean_sq = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len() as f64;
    10.0 * (mean_sq + 1e-12).log10()
}

fn frame_db(x: &[f32]) -> Vec<f32> {
    let win = 400;
    let n = x.len() / HOP + 1;
    (0..n)
        .map(|i| {
            let c = i * HOP;
            let a = c.saturating_sub(win / 2);
            let b = (c + win / 2).min(x.len());
            if b <= a {
                return -120.0;
            }
            let e = x[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a) as f32;
            10.0 * (e + 1e-12).log10()
        })
        .collect()
}

fn yin_f0(x: &[f32], db: &[f32], gate_db: f32) -> Vec<f32> {
    let tau_min = (SR as f32 / F0_MAX) as usize;
    let tau_max = (SR as f32 / F0_MIN).ceil() as usize;
    let n = db.len();
    let mut f0 = vec![0f32; n];
    let mut diff = vec![0f32; tau_max + 2];
    for i in 0..n {
        if db[i] < gate_db {
            continue;
        }
        let c = i * HOP;
        let a = c.saturating_sub(YIN_W / 2);
        if a + YIN_W + tau_max + 1 > x.len() {
            continue;
        }
        let fr = &x[a..a + YIN_W + tau_max + 1];
        diff[0] = 0.0;
        for tau in 1..=tau_max + 1 {
            let mut s = 0f32;
            for k in 0..YIN_W {
                let e = fr[k] - fr[k + tau];
                s += e * e;
            }
            diff[tau] = s;
        }
        let mut run = 0f32;
        let mut cm = vec![1f32; tau_max + 2];
        for tau in 1..=tau_max + 1 {
            run += diff[tau];
            cm[tau] = if run > 0.0 {
                diff[tau] * tau as f32 / run
            } else {
                1.0
            };
        }
        let thr = 0.15;
        let mut best = None;
        let mut tau = tau_min;
        while tau <= tau_max {
            if cm[tau] < thr {
                while tau + 1 <= tau_max && cm[tau + 1] < cm[tau] {
                    tau += 1;
                }
                best = Some(tau);
                break;
            }
            tau += 1;
        }
        if best.is_none() {
            let (bt, bv) = (tau_min..=tau_max)
                .map(|t| (t, cm[t]))
                .fold((0usize, f32::MAX), |acc, v| if v.1 < acc.1 { v } else { acc });
            if bv < 0.30 {
                best = Some(bt);
            }
        }
        if let Some(t) = best {
            if t == 0 || t + 1 >= cm.len() {
                continue;
            }
            let (y0, y1, y2) = (cm[t - 1], cm[t], cm[t + 1]);
            let den = y0 - 2.0 * y1 + y2;
            let off = if den.abs() > 1e-9 {
                (0.5 * (y0 - y2) / den).clamp(-1.0, 1.0)
            } else {
                0.0
            };
            let period = t as f32 + off;
            if period > 1.0 {
                let f = SR as f32 / period;
                if (F0_MIN..=F0_MAX).contains(&f) {
                    f0[i] = f;
                }
            }
        }
    }
    let raw = f0.clone();
    for i in 1..n.saturating_sub(1) {
        if raw[i] > 0.0 && raw[i - 1] > 0.0 && raw[i + 1] > 0.0 {
            let mut t = [raw[i - 1], raw[i], raw[i + 1]];
            t.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            f0[i] = t[1];
        }
    }
    let voiced: Vec<f32> = f0.iter().copied().filter(|v| *v > 0.0).collect();
    if !voiced.is_empty() {
        let med = percentile(&voiced, 50.0);
        for v in f0.iter_mut() {
            if *v > 0.0 {
                if *v > med * 1.9 {
                    *v *= 0.5;
                } else if *v < med / 1.9 {
                    *v *= 2.0;
                }
                if *v < F0_MIN || *v > F0_MAX {
                    *v = 0.0;
                }
            }
        }
    }
    let mut i = 0;
    while i < n {
        if f0[i] > 0.0 {
            let s = i;
            while i < n && f0[i] > 0.0 {
                i += 1;
            }
            if i - s < 3 {
                for v in &mut f0[s..i] {
                    *v = 0.0;
                }
            }
        } else {
            i += 1;
        }
    }
    f0
}

fn f0_stats(voiced: &[f32]) -> (f64, f64, f64, f64) {
    if voiced.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mean = voiced.iter().sum::<f32>() as f64 / voiced.len() as f64;
    let median = percentile(voiced, 50.0) as f64;
    let st: Vec<f32> = voiced
        .iter()
        .map(|f| 12.0 * (*f as f64 / median).log2() as f32)
        .collect();
    let sd = sample_sd(&st) as f64;
    let range = (percentile(&st, 95.0) - percentile(&st, 5.0)) as f64;
    (mean, median, sd, range)
}

fn sung_phrases(f0: &[f32]) -> Vec<SungPhrase> {
    let mut out = Vec::new();
    let n = f0.len();
    let mut i = 0;
    while i < n {
        if f0[i] <= 0.0 {
            i += 1;
            continue;
        }
        let s = i;
        while i < n && f0[i] > 0.0 {
            i += 1;
        }
        let dur = (i - s) as f64 * HOP as f64 / SR as f64;
        if dur + 1e-9 < SUNG_MIN_S {
            continue;
        }
        let run: Vec<f32> = f0[s..i].to_vec();
        let (rate, extent) = vibrato(&run);
        if rate <= 0.0 {
            continue;
        }
        out.push(SungPhrase {
            start_s: s as f64 * HOP as f64 / SR as f64,
            end_s: i as f64 * HOP as f64 / SR as f64,
            f0_median_hz: percentile(&run, 50.0) as f64,
            vibrato_rate_hz: rate,
            vibrato_extent_cents: extent,
            f0_contour_hz: downsample_voiced(&run, 32),
        });
    }
    out
}

/// Returns (rate_hz, extent_cents). Zero rate means no vibrato.
fn vibrato(f0: &[f32]) -> (f64, f64) {
    if f0.len() < 20 {
        return (0.0, 0.0);
    }
    let med = percentile(f0, 50.0);
    if med <= 0.0 {
        return (0.0, 0.0);
    }
    let cents: Vec<f64> = f0
        .iter()
        .map(|f| 1200.0 * (*f as f64 / med as f64).log2())
        .collect();
    let sd = {
        let m = cents.iter().sum::<f64>() / cents.len() as f64;
        let v = cents.iter().map(|c| (c - m).powi(2)).sum::<f64>() / cents.len() as f64;
        v.sqrt()
    };
    // A steady spoken vowel sits well under this. Vibrato of ~30 cents peak does not.
    if sd < 8.0 {
        return (0.0, 0.0);
    }
    let mean = cents.iter().sum::<f64>() / cents.len() as f64;
    let centered: Vec<f64> = cents.iter().map(|c| c - mean).collect();
    let energy = centered.iter().map(|c| c * c).sum::<f64>().max(1e-12);
    let hop_s = HOP as f64 / SR as f64;
    let lag_lo = (1.0 / (VIBRATO_HZ_HI * hop_s)).round() as usize;
    let lag_hi = (1.0 / (VIBRATO_HZ_LO * hop_s)).round() as usize;
    let mut best_lag = 0usize;
    let mut best_c = 0.0f64;
    for lag in lag_lo.max(2)..=lag_hi.min(centered.len() / 2) {
        let mut acc = 0.0;
        let mut e2 = 0.0;
        for i in 0..centered.len() - lag {
            acc += centered[i] * centered[i + lag];
            e2 += centered[i + lag] * centered[i + lag];
        }
        let norm = (energy * e2.max(1e-12)).sqrt();
        let c = acc / norm;
        if c > best_c {
            best_c = c;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best_c < 0.35 {
        return (0.0, 0.0);
    }
    let rate = 1.0 / (best_lag as f64 * hop_s);
    let p5 = percentile_f64(&cents, 5.0);
    let p95 = percentile_f64(&cents, 95.0);
    let extent = (p95 - p5) / 2.0;
    if !(15.0..=200.0).contains(&extent) {
        return (0.0, 0.0);
    }
    (rate, extent)
}

fn downsample_voiced(f0: &[f32], max_points: usize) -> Vec<f64> {
    let voiced: Vec<f64> = f0.iter().copied().filter(|v| *v > 0.0).map(|v| v as f64).collect();
    if voiced.len() <= max_points || max_points == 0 {
        return voiced;
    }
    (0..max_points)
        .map(|i| {
            let idx = (i as f64) * (voiced.len() - 1) as f64 / (max_points - 1) as f64;
            voiced[idx.round() as usize]
        })
        .collect()
}

fn spectral_tilt(x: &[f32], db: &[f32], gate: f32) -> f64 {
    let nfft = 512usize;
    let win: Vec<f32> = (0..nfft)
        .map(|k| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * k as f32 / nfft as f32).cos())
        .collect();
    let mut ltas = vec![0f64; nfft / 2 + 1];
    let mut cnt = 0usize;
    for (i, &d) in db.iter().enumerate() {
        if d < gate {
            continue;
        }
        let c = i * HOP;
        if c < nfft / 2 || c + nfft / 2 > x.len() {
            continue;
        }
        let fr = &x[c - nfft / 2..c + nfft / 2];
        let mut re = vec![0f32; nfft];
        let mut im = vec![0f32; nfft];
        for k in 0..nfft {
            re[k] = fr[k] * win[k];
        }
        fft_inplace(&mut re, &mut im);
        for k in 0..=nfft / 2 {
            ltas[k] += (re[k] as f64) * (re[k] as f64) + (im[k] as f64) * (im[k] as f64);
        }
        cnt += 1;
    }
    if cnt == 0 {
        return 0.0;
    }
    let bin_hz = SR as f64 / nfft as f64;
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    let mut fc = 125.0f64;
    while fc <= 6300.0 {
        let lo = fc / 2f64.powf(1.0 / 6.0);
        let hi = fc * 2f64.powf(1.0 / 6.0);
        let a = (lo / bin_hz).ceil() as usize;
        let b = ((hi / bin_hz).floor() as usize).min(nfft / 2);
        if b >= a {
            let e = (a..=b).map(|k| ltas[k]).sum::<f64>() / ((b - a + 1) as f64 * cnt as f64);
            xs.push(fc.log2());
            ys.push(10.0 * (e + 1e-20).log10());
        }
        fc *= 2f64.powf(1.0 / 3.0);
    }
    if xs.len() < 3 {
        return 0.0;
    }
    let mx = xs.iter().sum::<f64>() / xs.len() as f64;
    let my = ys.iter().sum::<f64>() / ys.len() as f64;
    let sxy: f64 = xs.iter().zip(&ys).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = xs.iter().map(|a| (a - mx).powi(2)).sum();
    if sxx.abs() < 1e-12 {
        0.0
    } else {
        sxy / sxx
    }
}

fn fft_inplace(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2usize;
    while len <= n {
        let ang = -2.0 * std::f32::consts::PI / len as f32;
        let (wlen_re, wlen_im) = (ang.cos(), ang.sin());
        let mut i = 0usize;
        while i < n {
            let (mut w_re, mut w_im) = (1f32, 0f32);
            for k in 0..len / 2 {
                let ur = re[i + k];
                let ui = im[i + k];
                let vr = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let vi = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let next_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = next_re;
            }
            i += len;
        }
        len <<= 1;
    }
}

fn percentile(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pos = (p / 100.0) * (s.len() - 1) as f32;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f32)
}

fn percentile_f64(v: &[f64], p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pos = (p / 100.0) * (s.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
}

fn sample_sd(v: &[f32]) -> f32 {
    if v.len() < 2 {
        return 0.0;
    }
    let m = v.iter().sum::<f32>() / v.len() as f32;
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / (v.len() - 1) as f32;
    var.sqrt()
}

pub fn sine(freq_hz: f64, seconds: f64, amp: f32) -> Vec<f32> {
    let n = (seconds * SR as f64).round() as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            (amp as f64 * (2.0 * std::f64::consts::PI * freq_hz * t).sin()) as f32
        })
        .collect()
}

pub fn vibrato_tone(freq_hz: f64, rate_hz: f64, extent_cents: f64, seconds: f64, amp: f32) -> Vec<f32> {
    let n = (seconds * SR as f64).round() as usize;
    let depth = extent_cents / 1200.0;
    let mut phase = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f64 / SR as f64;
        let inst = freq_hz * (2f64).powf(depth * (2.0 * std::f64::consts::PI * rate_hz * t).sin());
        phase += 2.0 * std::f64::consts::PI * inst / SR as f64;
        out.push((amp as f64 * phase.sin()) as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steady_sine_f0_and_level() {
        let x = sine(200.0, 1.0, 0.1);
        let p = analyze_prosody(&x);
        assert!((p.f0_median_hz - 200.0).abs() < 2.0, "median {}", p.f0_median_hz);
        assert!((p.f0_mean_hz - 200.0).abs() < 2.0, "mean {}", p.f0_mean_hz);
        // 10 log10(mean square) of a 0.1-peak sine is 20 log10(0.1/sqrt(2)).
        assert!((p.rms_db + 23.0103).abs() < 0.05, "rms {}", p.rms_db);
        assert!(p.sung_phrases.is_empty(), "steady tone is not sung");
        assert!(p.voiced_frac > 0.5);
        assert!(!p.f0_contour_hz.is_empty());
    }

    #[test]
    fn vibrato_marks_a_sung_phrase() {
        let x = vibrato_tone(220.0, 5.5, 50.0, 1.2, 0.2);
        let p = analyze_prosody(&x);
        assert!(
            (p.f0_median_hz - 220.0).abs() < 8.0,
            "median {}",
            p.f0_median_hz
        );
        assert!(!p.sung_phrases.is_empty(), "expected a sung phrase");
        let phrase = &p.sung_phrases[0];
        assert!(
            (4.5..=7.0).contains(&phrase.vibrato_rate_hz),
            "rate {}",
            phrase.vibrato_rate_hz
        );
        assert!(
            (20.0..=120.0).contains(&phrase.vibrato_extent_cents),
            "extent {}",
            phrase.vibrato_extent_cents
        );
        assert!(phrase.end_s - phrase.start_s >= 0.40);
        assert!(!phrase.f0_contour_hz.is_empty());
    }

    #[test]
    fn low_harmonics_tilt_below_bright_partials() {
        let mut low = vec![0f32; SR];
        let mut bright = vec![0f32; SR];
        for i in 0..SR {
            let t = i as f64 / SR as f64;
            let mut s = 0.0;
            for f in [150.0, 200.0, 250.0] {
                s += (2.0 * std::f64::consts::PI * f * t).sin();
            }
            low[i] = (0.15 * s) as f32;
            let mut b = 0.0;
            for f in [1800.0, 2400.0, 3200.0, 4200.0] {
                b += (2.0 * std::f64::consts::PI * f * t).sin();
            }
            bright[i] = (0.1 * b) as f32;
        }
        let tilt_low = analyze_prosody(&low).spectral_tilt_db_oct;
        let tilt_bright = analyze_prosody(&bright).spectral_tilt_db_oct;
        assert!(
            tilt_low < tilt_bright - 2.0,
            "low {tilt_low} bright {tilt_bright}"
        );
    }

    #[test]
    fn fft_impulse_is_flat() {
        let mut re = vec![0f32; 16];
        let mut im = vec![0f32; 16];
        re[0] = 1.0;
        fft_inplace(&mut re, &mut im);
        for (r, i) in re.iter().zip(im.iter()) {
            assert!((r - 1.0).abs() < 1e-4 && i.abs() < 1e-4, "{r} {i}");
        }
    }
}
