// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! Meters for the capture gate.
//!
//! Loudness is mono ITU-R BS.1770-4 (K-weighting, 400 ms blocks, absolute gate,
//! relative gate, channel weight 1). True peak is 4× windowed-sinc oversampling.
//! Reverb is a Schroeder T20 on the tail after the sustain. Spectral tilt is the
//! slope of power spectral density across octave bands on the 432 Hz grid.

use crate::spec::{ReverbSpec, SpectralSpec, WindowSpec};

const PI: f64 = std::f64::consts::PI;

pub fn ms_samples(sr: u32, ms: u32) -> usize {
    ((sr as u64 * ms as u64) / 1000) as usize
}

pub fn db10(power: f64) -> f64 {
    if power <= 1e-20 {
        -200.0
    } else {
        10.0 * power.log10()
    }
}

pub fn db20(amp: f64) -> f64 {
    if amp <= 1e-12 {
        -240.0
    } else {
        20.0 * amp.log10()
    }
}

struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0],
            b1: b[1],
            b2: b[2],
            a1: a[1],
            a2: a[2],
            z1: 0.0,
            z2: 0.0,
        }
    }
    fn process(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// High shelf then RLB high-pass. Coefficients follow BS.1770-4 at `sr`.
fn k_coeffs(sr: u32) -> ([f64; 3], [f64; 3], [f64; 3], [f64; 3]) {
    let sr = sr as f64;
    let f0 = 1681.974450955533;
    let g_db = 3.999843853973347;
    let q = 0.7071752369554196;
    let k = (PI * f0 / sr).tan();
    let vh = 10f64.powf(g_db / 20.0);
    let vb = vh.powf(0.4996667741545416);
    let a0 = 1.0 + k / q + k * k;
    let pre_b = [
        (vh + vb * k / q + k * k) / a0,
        2.0 * (k * k - vh) / a0,
        (vh - vb * k / q + k * k) / a0,
    ];
    let pre_a = [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0];

    let f0 = 38.13547087602444;
    let q = 0.5003270373238773;
    let k = (PI * f0 / sr).tan();
    let a0 = 1.0 + k / q + k * k;
    let hp_b = [1.0 / a0, -2.0 / a0, 1.0 / a0];
    let hp_a = [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0];
    (pre_b, pre_a, hp_b, hp_a)
}

pub fn k_weight(x: &[f32], sr: u32) -> Vec<f64> {
    let (pb, pa, hb, ha) = k_coeffs(sr);
    let mut shelf = Biquad::new(pb, pa);
    let mut hp = Biquad::new(hb, ha);
    let mut y = Vec::with_capacity(x.len());
    for &s in x {
        y.push(hp.process(shelf.process(s as f64)));
    }
    y
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

/// Integrated loudness in LUFS. `None` when no 400 ms block clears the absolute gate.
pub fn integrated_lufs(
    x: &[f32],
    sr: u32,
    block_ms: u32,
    hop_ms: u32,
    abs_gate: f64,
    rel_gate: f64,
) -> Option<f64> {
    if sr == 0 {
        return None;
    }
    let y = k_weight(x, sr);
    let block = ms_samples(sr, block_ms);
    let hop = ms_samples(sr, hop_ms).max(1);
    if block == 0 || y.len() < block {
        return None;
    }
    let mut zs = Vec::new();
    let mut i = 0;
    while i + block <= y.len() {
        let mut acc = 0.0;
        for s in &y[i..i + block] {
            acc += s * s;
        }
        zs.push(acc / block as f64);
        i += hop;
    }
    let loud = |z: f64| -0.691 + db10(z);
    let abs_keep: Vec<f64> = zs.iter().copied().filter(|z| loud(*z) > abs_gate).collect();
    if abs_keep.is_empty() {
        return None;
    }
    let ungated = loud(mean(&abs_keep));
    let rel: Vec<f64> = abs_keep
        .into_iter()
        .filter(|z| loud(*z) > ungated - rel_gate)
        .collect();
    if rel.is_empty() {
        return None;
    }
    Some(loud(mean(&rel)))
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-8 {
        1.0
    } else {
        (PI * x).sin() / (PI * x)
    }
}

fn phase_kernel(phase: f64, half: i32) -> Vec<f64> {
    let mut w = Vec::with_capacity((half * 2 + 1) as usize);
    let mut sum = 0.0;
    for k in -half..=half {
        let dist = k as f64 - phase;
        let hann = if dist.abs() >= half as f64 {
            0.0
        } else {
            0.5 * (1.0 + (PI * dist / half as f64).cos())
        };
        let c = sinc(dist) * hann;
        w.push(c);
        sum += c;
    }
    if sum.abs() > 1e-12 {
        for c in &mut w {
            *c /= sum;
        }
    }
    w
}

/// Linear true-peak amplitude (not dB). 4× windowed-sinc, plus the sample peak.
pub fn true_peak_linear(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let mut peak = 0.0f64;
    for &s in x {
        peak = peak.max((s as f64).abs());
    }
    let half = 32i32;
    let kernels = [
        phase_kernel(0.25, half),
        phase_kernel(0.5, half),
        phase_kernel(0.75, half),
    ];
    let n = x.len() as i32;
    let start = half;
    let end = n - half - 1;
    if end <= start {
        return peak;
    }
    for i in start..end {
        for kern in &kernels {
            let mut acc = 0.0;
            for (t, c) in kern.iter().enumerate() {
                let idx = i + t as i32 - half;
                acc += x[idx as usize] as f64 * c;
            }
            peak = peak.max(acc.abs());
        }
    }
    peak
}

pub fn true_peak_dbtp(x: &[f32]) -> f64 {
    db20(true_peak_linear(x))
}

/// RMS of the quietest window, in dBFS. `None` when the file is shorter than one window.
pub fn noise_floor_dbfs(x: &[f32], sr: u32, window: &WindowSpec) -> Option<f64> {
    let win = ms_samples(sr, window.window_ms);
    let hop = ms_samples(sr, window.hop_ms).max(1);
    if win == 0 || x.len() < win {
        return None;
    }
    let mut quiet = f64::MAX;
    let mut i = 0;
    while i + win <= x.len() {
        let mut acc = 0.0;
        for s in &x[i..i + win] {
            let v = *s as f64;
            acc += v * v;
        }
        quiet = quiet.min(acc / win as f64);
        i += hop;
    }
    Some(db10(quiet))
}

pub fn dc_offset_dbfs(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let mean = x.iter().map(|s| *s as f64).sum::<f64>() / x.len() as f64;
    db20(mean.abs())
}

pub fn duration_s(n: usize, sr: u32) -> f64 {
    if sr == 0 {
        0.0
    } else {
        n as f64 / sr as f64
    }
}

fn percentile(xs: &[f64], p: f64) -> f64 {
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    if v.is_empty() {
        return 0.0;
    }
    let t = (p / 100.0).clamp(0.0, 1.0) * (v.len() - 1) as f64;
    let i = t.floor() as usize;
    let f = t - i as f64;
    if i + 1 >= v.len() {
        v[i]
    } else {
        v[i] * (1.0 - f) + v[i + 1] * f
    }
}

fn smooth_hold(x: &[f64], win: usize) -> Vec<f64> {
    let w = if win % 2 == 0 { win + 1 } else { win };
    let h = w / 2;
    let n = x.len();
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut s = 0.0;
        for k in 0..w {
            let j = (i as isize + k as isize - h as isize).clamp(0, n as isize - 1) as usize;
            s += x[j];
        }
        y[i] = s / w as f64;
    }
    y
}

/// Schroeder T20, in seconds, measured on the tail after the sustain.
pub fn estimate_rt60(x: &[f32], sr: u32, spec: &ReverbSpec) -> Result<f64, String> {
    let frame = ms_samples(sr, spec.frame_ms);
    let hop = ms_samples(sr, spec.hop_ms).max(1);
    if frame == 0 || x.len() < frame * 4 {
        return Err("shorter than four reverb frames".into());
    }
    let mut idx = Vec::new();
    let mut db = Vec::new();
    let mut i = 0;
    while i + frame <= x.len() {
        let mut acc = 0.0;
        for s in &x[i..i + frame] {
            let v = *s as f64;
            acc += v * v;
        }
        idx.push(i);
        db.push(db10(acc / frame as f64));
        i += hop;
    }
    let sm = smooth_hold(&db, 5);
    let noise = percentile(&sm, 20.0);
    let loud: Vec<f64> = sm.iter().copied().filter(|v| *v > noise + 15.0).collect();
    if loud.is_empty() {
        return Err("no sustain above the noise floor".into());
    }
    let sustain = percentile(&loud, 50.0);
    if sustain - noise < 30.0 {
        return Err("dynamic range is under 30 dB, so the tail is not visible".into());
    }
    let knee_f = sm
        .iter()
        .rposition(|v| *v >= sustain - 2.0)
        .ok_or("no sustain knee")?;
    let knee = idx[knee_f];
    if x.len().saturating_sub(knee) < ms_samples(sr, 200) {
        return Err("no trailing quiet after the sustain".into());
    }
    let quiet_win = ms_samples(sr, 400).max(frame);
    let npow = quietest_power(x, quiet_win);
    let tail = &x[knee..];
    let mut y2 = Vec::with_capacity(tail.len());
    for s in tail {
        let p = (*s as f64) * (*s as f64) - npow;
        y2.push(p.max(0.0));
    }
    let mut rev = vec![0.0; y2.len()];
    let mut acc = 0.0;
    for i in (0..y2.len()).rev() {
        acc += y2[i];
        rev[i] = acc;
    }
    if rev[0] <= 0.0 {
        return Err("tail energy is at the noise floor".into());
    }
    let edc: Vec<f64> = rev.iter().map(|e| db10(e / rev[0])).collect();
    let cross = |level: f64| edc.iter().position(|v| *v <= level);
    let i_start = cross(spec.fit_start_db)
        .ok_or_else(|| format!("tail does not fall to {} dB", spec.fit_start_db))?;
    let i_end = cross(spec.fit_end_db).ok_or_else(|| {
        format!(
            "tail does not fall to {} dB above the noise floor",
            spec.fit_end_db
        )
    })?;
    if i_end <= i_start {
        return Err("reverb fit interval is empty".into());
    }
    let t: Vec<f64> = (i_start..=i_end).map(|i| i as f64 / sr as f64).collect();
    let y: Vec<f64> = edc[i_start..=i_end].to_vec();
    let slope = lin_slope(&t, &y);
    if slope >= -1.0 {
        return Err("decay slope is flat".into());
    }
    Ok(-60.0 / slope)
}

fn quietest_power(x: &[f32], win: usize) -> f64 {
    if win == 0 || x.len() < win {
        return 0.0;
    }
    let mut quiet = f64::MAX;
    let hop = (win / 4).max(1);
    let mut i = 0;
    while i + win <= x.len() {
        let mut acc = 0.0;
        for s in &x[i..i + win] {
            let v = *s as f64;
            acc += v * v;
        }
        quiet = quiet.min(acc / win as f64);
        i += hop;
    }
    quiet
}

fn lin_slope(t: &[f64], y: &[f64]) -> f64 {
    let n = t.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mt = t.iter().sum::<f64>() / n;
    let my = y.iter().sum::<f64>() / n;
    let mut num = 0.0;
    let mut den = 0.0;
    for i in 0..t.len() {
        let dt = t[i] - mt;
        num += dt * (y[i] - my);
        den += dt * dt;
    }
    if den == 0.0 {
        0.0
    } else {
        num / den
    }
}

/// In-place radix-2 FFT. `im` is the imaginary part.
pub fn fft(re: &mut [f64], im: &mut [f64]) {
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
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * PI / len as f64;
        let wlen_re = ang.cos();
        let wlen_im = ang.sin();
        let mut i = 0;
        while i < n {
            let mut w_re = 1.0;
            let mut w_im = 0.0;
            for k in 0..len / 2 {
                let u_re = re[i + k];
                let u_im = im[i + k];
                let v_re = re[i + k + len / 2] * w_re - im[i + k + len / 2] * w_im;
                let v_im = re[i + k + len / 2] * w_im + im[i + k + len / 2] * w_re;
                re[i + k] = u_re + v_re;
                im[i + k] = u_im + v_im;
                re[i + k + len / 2] = u_re - v_re;
                im[i + k + len / 2] = u_im - v_im;
                let nw_re = w_re * wlen_re - w_im * wlen_im;
                w_im = w_re * wlen_im + w_im * wlen_re;
                w_re = nw_re;
            }
            i += len;
        }
        len <<= 1;
    }
}

pub fn ifft(re: &mut [f64], im: &mut [f64]) {
    for v in im.iter_mut() {
        *v = -*v;
    }
    fft(re, im);
    let n = re.len() as f64;
    for i in 0..re.len() {
        re[i] /= n;
        im[i] = -im[i] / n;
    }
}

fn hann(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            if n <= 1 {
                1.0
            } else {
                0.5 - 0.5 * (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos()
            }
        })
        .collect()
}

fn active_mask(x: &[f32], sr: u32, down_db: f64) -> Vec<bool> {
    let frame = ms_samples(sr, 50).max(1);
    let hop = (frame / 2).max(1);
    let mut rms = Vec::new();
    let mut starts = Vec::new();
    let mut i = 0;
    while i + frame <= x.len() {
        let mut acc = 0.0;
        for s in &x[i..i + frame] {
            let v = *s as f64;
            acc += v * v;
        }
        rms.push((acc / frame as f64).sqrt());
        starts.push(i);
        i += hop;
    }
    let peak = rms.iter().copied().fold(0.0_f64, f64::max);
    let thr = peak * 10f64.powf(-down_db / 20.0);
    let mut mask = vec![false; x.len()];
    for (j, start) in starts.into_iter().enumerate() {
        if rms[j] >= thr {
            let end = (start + frame).min(x.len());
            for m in &mut mask[start..end] {
                *m = true;
            }
        }
    }
    mask
}

/// Power-spectral-density tilt in dB/octave on the reference-frequency octave grid.
pub fn spectral_tilt(
    x: &[f32],
    sr: u32,
    reference_hz: f64,
    spec: &SpectralSpec,
) -> Result<(f64, String), String> {
    let n = spec.fft_size;
    if x.len() < n {
        return Err("shorter than one spectral frame".into());
    }
    let mask = active_mask(x, sr, spec.active_down_db);
    let win = hann(n);
    let wsum: f64 = win.iter().map(|w| w * w).sum::<f64>().max(1e-12);
    let mut acc = vec![0.0; n / 2 + 1];
    let mut count = 0usize;
    let hop = n / 2;
    let mut i = 0;
    while i + n <= x.len() {
        let active = mask[i..i + n].iter().filter(|m| **m).count();
        if active * 2 >= n {
            let mut re = vec![0.0; n];
            let mut im = vec![0.0; n];
            for k in 0..n {
                re[k] = x[i + k] as f64 * win[k];
            }
            fft(&mut re, &mut im);
            for k in 0..=n / 2 {
                acc[k] += (re[k] * re[k] + im[k] * im[k]) / wsum;
            }
            count += 1;
        }
        i += hop;
    }
    if count == 0 {
        return Err("no active region for the spectrum".into());
    }
    for a in &mut acc {
        *a /= count as f64;
    }
    let freqs: Vec<f64> = (0..=n / 2)
        .map(|k| k as f64 * sr as f64 / n as f64)
        .collect();
    let mut raw: Vec<(f64, f64)> = Vec::new();
    for oct in spec.octave_min..=spec.octave_max {
        let center = reference_hz * 2f64.powi(oct);
        let lo = center / 2f64.sqrt();
        let hi = center * 2f64.sqrt();
        if lo >= sr as f64 / 2.0 {
            continue;
        }
        let mut sum = 0.0;
        let mut bins = 0usize;
        for (k, f) in freqs.iter().enumerate() {
            if *f >= lo && *f < hi {
                sum += acc[k];
                bins += 1;
            }
        }
        if bins > 0 {
            raw.push((center, sum / bins as f64));
        }
    }
    if raw.is_empty() {
        return Err("no octave band inside the Nyquist range".into());
    }
    let peak = raw.iter().map(|(_, p)| *p).fold(0.0_f64, f64::max);
    let floor = peak * 10f64.powf(-60.0 / 10.0);
    let kept: Vec<(f64, f64)> = raw
        .into_iter()
        .filter(|(_, p)| *p > floor && *p > 0.0)
        .collect();
    if kept.len() < 4 {
        return Err(format!(
            "only {} octave bands are above the floor",
            kept.len()
        ));
    }
    let xs: Vec<f64> = kept.iter().map(|(c, _)| c.log2()).collect();
    let ys: Vec<f64> = kept.iter().map(|(_, p)| db10(*p)).collect();
    let slope = lin_slope(&xs, &ys);
    let mut detail = String::new();
    for (c, p) in &kept {
        if !detail.is_empty() {
            detail.push(' ');
        }
        detail.push_str(&format!("{:.0}:{:.1}", c, db10(*p)));
    }
    Ok((slope, detail))
}

/// Dominant frequency in Hz, parabolic interpolation on a power-of-two FFT.
pub fn dominant_hz(x: &[f32], sr: u32) -> Result<f64, String> {
    let mut n = 65536usize;
    while n > x.len() {
        n /= 2;
    }
    if n < 2048 {
        return Err("shorter than 2048 samples".into());
    }
    let start = (x.len() - n) / 2;
    let win = hann(n);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    for i in 0..n {
        re[i] = x[start + i] as f64 * win[i];
    }
    fft(&mut re, &mut im);
    let mag = |k: usize| (re[k] * re[k] + im[k] * im[k]).sqrt();
    let mut best_k = 1usize;
    let mut best = 0.0;
    for k in 1..n / 2 {
        let m = mag(k);
        if m > best {
            best = m;
            best_k = k;
        }
    }
    let a = mag(best_k - 1);
    let b = mag(best_k);
    let c = mag(best_k + 1);
    let denom = a - 2.0 * b + c;
    let delta = if denom.abs() < 1e-18 {
        0.0
    } else {
        0.5 * (a - c) / denom
    };
    Ok((best_k as f64 + delta) * sr as f64 / n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(sr: u32, secs: f64, hz: f64, amp: f64, phase_samples: f64) -> Vec<f32> {
        let n = (sr as f64 * secs) as usize;
        (0..n)
            .map(|i| {
                (amp * (2.0 * PI * hz * ((i as f64 + phase_samples) / sr as f64)).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn mono_1khz_full_scale_is_about_minus_3_lufs() {
        let x = sine(48_000, 3.0, 1000.0, 1.0, 0.0);
        let l = integrated_lufs(&x, 48_000, 400, 100, -70.0, 10.0).unwrap();
        assert!((l - (-3.01)).abs() < 0.15, "lufs {l}");
    }

    #[test]
    fn true_peak_finds_the_intersample_peak_at_fs_over_4() {
        let x = sine(48_000, 0.5, 12_000.0, 1.0, 0.5);
        let sample_peak = x.iter().map(|s| s.abs()).fold(0.0f32, f32::max) as f64;
        let tp = true_peak_linear(&x);
        assert!(sample_peak < 0.8, "sample peak {sample_peak}");
        assert!(
            tp > sample_peak + 0.15,
            "true peak {tp} should clear the samples"
        );
        assert!((tp - 1.0).abs() < 0.08, "true peak {tp}");
    }

    #[test]
    fn fft_impulse_is_flat_and_roundtrips() {
        let mut re = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let mut im = vec![0.0; 8];
        fft(&mut re, &mut im);
        for i in 0..8 {
            assert!((re[i] - 1.0).abs() < 1e-9, "re {i} {}", re[i]);
            assert!(im[i].abs() < 1e-9);
        }
        ifft(&mut re, &mut im);
        assert!((re[0] - 1.0).abs() < 1e-9);
        for i in 1..8 {
            assert!(re[i].abs() < 1e-9 && im[i].abs() < 1e-9);
        }
    }
}
