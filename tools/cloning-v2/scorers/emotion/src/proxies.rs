//! Pure-Rust acoustic emotion proxies.
//! All frame features at 16 kHz, hop 10 ms. Jitter/shimmer use the native-rate signal.
use rustfft::{num_complex::Complex32, FftPlanner};
use serde::Serialize;

pub const SR: usize = 16000;
pub const HOP: usize = 160; // 10 ms
const F0_MIN: f32 = 60.0;
const F0_MAX: f32 = 500.0;

#[derive(Serialize, Clone, Debug, Default)]
pub struct Proxies {
    // pitch
    pub voiced_frac: f32,
    pub f0_mean_hz: f32,
    pub f0_median_hz: f32,
    pub f0_sd_st: f32,
    pub f0_range_st_p5_p95: f32,
    pub f0_range_st_minmax: f32,
    // energy
    pub energy_dr_db: f32,
    pub energy_sd_db: f32,
    pub rms_dbfs: f32,
    // timing
    pub speech_span_s: f32,
    pub syllables: u32,
    pub speech_rate_syl_s: f32,
    pub articulation_rate_syl_s: f32,
    pub pause_count: u32,
    pub pause_count_ge300ms: u32,
    pub pause_mean_ms: f32,
    pub pause_max_ms: f32,
    pub pause_total_s: f32,
    pub pause_ratio: f32,
    // voice quality
    pub jitter_local_pct: f32,
    pub shimmer_local_pct: f32,
    pub shimmer_local_db: f32,
    pub glottal_cycles: u32,
    // spectral balance (voiced LTAS)
    pub spectral_tilt_db_oct: f32,
    pub alpha_ratio_db: f32,
    pub hammarberg_db: f32,
}

fn percentile(v: &[f32], p: f32) -> f32 {
    if v.is_empty() {
        return f32::NAN;
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pos = p / 100.0 * (s.len() - 1) as f32;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f32)
}
fn mean(v: &[f32]) -> f32 {
    if v.is_empty() { f32::NAN } else { v.iter().sum::<f32>() / v.len() as f32 }
}
fn sd(v: &[f32]) -> f32 {
    if v.len() < 2 {
        return f32::NAN;
    }
    let m = mean(v);
    (v.iter().map(|x| (x - m).powi(2)).sum::<f32>() / (v.len() - 1) as f32).sqrt()
}

/// Frame RMS in dBFS, 25 ms window centered at each 10 ms hop.
pub fn frame_db(x: &[f32]) -> Vec<f32> {
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

/// YIN F0 tracker (de Cheveigné & Kawahara 2002). Returns per-frame F0 (0 = unvoiced).
pub fn yin_f0(x: &[f32], db: &[f32], gate_db: f32) -> (Vec<f32>, Vec<f32>) {
    let tau_min = (SR as f32 / F0_MAX) as usize;
    let tau_max = (SR as f32 / F0_MIN).ceil() as usize;
    let w = 400; // 25 ms integration window
    let n = db.len();
    let mut f0 = vec![0f32; n];
    let mut aper = vec![1f32; n];
    let mut d = vec![0f32; tau_max + 2];
    for i in 0..n {
        if db[i] < gate_db {
            continue;
        }
        let c = i * HOP;
        let a = c.saturating_sub(w / 2);
        if a + w + tau_max + 1 > x.len() {
            continue;
        }
        let fr = &x[a..a + w + tau_max + 1];
        // difference function
        d[0] = 0.0;
        for tau in 1..=tau_max + 1 {
            let mut s = 0f32;
            let (p, q) = (&fr[..w], &fr[tau..tau + w]);
            for k in 0..w {
                let e = p[k] - q[k];
                s += e * e;
            }
            d[tau] = s;
        }
        // cumulative mean normalized difference
        let mut run = 0f32;
        let mut cm = vec![1f32; tau_max + 2];
        for tau in 1..=tau_max + 1 {
            run += d[tau];
            cm[tau] = if run > 0.0 { d[tau] * tau as f32 / run } else { 1.0 };
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
            // global min fallback, accept if aperiodicity moderate
            let (bt, bv) = (tau_min..=tau_max).map(|t| (t, cm[t])).fold((0, f32::MAX), |acc, v| if v.1 < acc.1 { v } else { acc });
            if bv < 0.30 {
                best = Some(bt);
            }
        }
        if let Some(t) = best {
            let (y0, y1, y2) = (cm[t - 1], cm[t], cm[t + 1]);
            let den = y0 - 2.0 * y1 + y2;
            let off = if den.abs() > 1e-9 { 0.5 * (y0 - y2) / den } else { 0.0 };
            aper[i] = y1;
            let tt = t as f32 + off.clamp(-1.0, 1.0);
            let f = SR as f32 / tt;
            if (F0_MIN..=F0_MAX).contains(&f) {
                f0[i] = f;
            }
        }
    }
    // median-of-3 smoothing on voiced frames, then octave-jump correction against running median
    let raw = f0.clone();
    for i in 1..n.saturating_sub(1) {
        if raw[i] > 0.0 && raw[i - 1] > 0.0 && raw[i + 1] > 0.0 {
            let mut t = [raw[i - 1], raw[i], raw[i + 1]];
            t.sort_by(|a, b| a.partial_cmp(b).unwrap());
            f0[i] = t[1];
        }
    }
    let voiced: Vec<f32> = f0.iter().copied().filter(|&v| v > 0.0).collect();
    if !voiced.is_empty() {
        let med = percentile(&voiced, 50.0);
        for v in f0.iter_mut() {
            if *v > 0.0 {
                if *v > med * 1.9 { *v *= 0.5; } else if *v < med / 1.9 { *v *= 2.0; }
                if *v < F0_MIN || *v > F0_MAX { *v = 0.0; }
            }
        }
    }
    // drop voiced runs shorter than 3 frames (30 ms)
    let mut i = 0;
    while i < n {
        if f0[i] > 0.0 {
            let s = i;
            while i < n && f0[i] > 0.0 { i += 1; }
            if i - s < 3 { for v in &mut f0[s..i] { *v = 0.0; } }
        } else {
            i += 1;
        }
    }
    (f0, aper)
}

/// Cycle-level jitter/shimmer (Praat-like "local", periodic-cc style): successive glottal
/// periods are found by maximizing normalized cross-correlation of one-period windows,
/// guided by the frame F0 track; amplitudes are per-cycle absolute peaks.
fn jitter_shimmer(x: &[f32], sr: usize, f0: &[f32]) -> (f32, f32, f32, u32) {
    let hop_n = sr as f64 * HOP as f64 / SR as f64; // native samples per frame
    let (mut jit_num, mut jit_den, mut jit_n) = (0f64, 0f64, 0usize);
    let (mut sh_num, mut sh_den, mut sh_db, mut sh_n) = (0f64, 0f64, 0f64, 0usize);
    let mut cycles = 0u32;
    let n = f0.len();
    let pmin = sr as f64 / F0_MAX as f64;
    let pmax = sr as f64 / F0_MIN as f64;
    let mut i = 0;
    while i < n {
        if f0[i] <= 0.0 { i += 1; continue; }
        let s = i;
        while i < n && f0[i] > 0.0 { i += 1; }
        if i - s < 5 { continue; }
        let a = (s as f64 * hop_n) as usize;
        let b = ((i as f64 * hop_n) as usize).min(x.len());
        let seg = &x[a..b];
        let f0_at = |pos: f64| -> f32 {
            let fi = ((a as f64 + pos) / hop_n).round() as usize;
            f0[fi.clamp(s, i - 1)]
        };
        let ncc = |p: usize, q: usize, len: usize| -> f64 {
            let (u, v) = (&seg[p..p + len], &seg[q..q + len]);
            let (mut uv, mut uu, mut vv) = (0f64, 0f64, 0f64);
            for k in 0..len {
                uv += (u[k] * v[k]) as f64; uu += (u[k] * u[k]) as f64; vv += (v[k] * v[k]) as f64;
            }
            if uu > 0.0 && vv > 0.0 { uv / (uu * vv).sqrt() } else { 0.0 }
        };
        // anchor at the absolute peak in the first period
        let t0 = (sr as f32 / f0_at(0.0)) as usize;
        if 3 * t0 >= seg.len() { continue; }
        let anchor = (0..t0).max_by(|&p, &q| seg[p].abs().partial_cmp(&seg[q].abs()).unwrap()).unwrap();
        let sign = if seg[anchor] >= 0.0 { 1.0f32 } else { -1.0 };
        let mut cur = anchor;
        let mut cur_f = cur as f64;
        // pass 1: NCC cycle tracking -> approximate glottal marks + local period
        let mut marks: Vec<(f64, f64)> = vec![(cur_f, sr as f64 / f0_at(cur_f) as f64)];
        loop {
            let t = (sr as f32 / f0_at(cur_f)) as f64;
            let len = t.round() as usize;
            let lo = (0.8 * t).floor() as usize;
            let hi = (1.2 * t).ceil() as usize;
            if cur + hi + len + 1 >= seg.len() { break; }
            let mut best = (lo, f64::MIN);
            let mut vals = vec![0f64; hi + 2];
            for tau in lo.saturating_sub(1)..=hi + 1 {
                vals[tau] = ncc(cur, cur + tau, len);
            }
            for tau in lo..=hi {
                if vals[tau] > best.1 { best = (tau, vals[tau]); }
            }
            if best.1 < 0.5 { break; } // lost periodicity
            let tb = best.0;
            let (y0, y1, y2) = (vals[tb - 1], vals[tb], vals[tb + 1]);
            let den = y0 - 2.0 * y1 + y2;
            let off = if den.abs() > 1e-12 { (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5) } else { 0.0 };
            cur_f += tb as f64 + off;
            cur = cur_f.round() as usize;
            marks.push((cur_f, tb as f64 + off));
        }
        // pass 2 (Praat-like): snap each mark to the same-polarity waveform peak within ±15% period
        let mut pk: Vec<(f64, f64)> = Vec::with_capacity(marks.len());
        for &(m, t) in &marks {
            let w = (0.15 * t).round() as isize;
            let c = m.round() as isize;
            let (a0, b0) = ((c - w).max(1) as usize, ((c + w) as usize).min(seg.len() - 2));
            if b0 <= a0 { continue; }
            let p = (a0..=b0).max_by(|&p, &q| (sign * seg[p]).partial_cmp(&(sign * seg[q])).unwrap()).unwrap();
            let (y0, y1, y2) = ((sign * seg[p - 1]) as f64, (sign * seg[p]) as f64, (sign * seg[p + 1]) as f64);
            let den = y0 - 2.0 * y1 + y2;
            let off = if den.abs() > 1e-12 { (0.5 * (y0 - y2) / den).clamp(-0.5, 0.5) } else { 0.0 };
            pk.push((p as f64 + off, y1 - 0.25 * (y0 - y2) * off));
        }
        let periods: Vec<f64> = pk.windows(2).map(|w| w[1].0 - w[0].0).collect();
        let amps: Vec<f64> = pk.iter().map(|v| v.1).collect();
        cycles += periods.len() as u32;
        for k in 1..periods.len() {
            let (p1, p2) = (periods[k - 1], periods[k]);
            if p1 < pmin || p1 > pmax || p2 < pmin || p2 > pmax || p1.max(p2) / p1.min(p2) > 1.3 { continue; }
            jit_num += (p1 - p2).abs(); jit_den += (p1 + p2) * 0.5; jit_n += 1;
        }
        for k in 1..amps.len() {
            let (a1, a2) = (amps[k - 1], amps[k]);
            if a1 <= 0.0 || a2 <= 0.0 || a1.max(a2) / a1.min(a2) > 1.6 { continue; }
            sh_num += (a1 - a2).abs(); sh_den += (a1 + a2) * 0.5; sh_db += (20.0 * (a2 / a1).log10()).abs(); sh_n += 1;
        }
    }
    let j = if jit_n > 0 { (100.0 * jit_num / jit_den) as f32 } else { f32::NAN };
    let sh = if sh_n > 0 { (100.0 * sh_num / sh_den) as f32 } else { f32::NAN };
    let sdb = if sh_n > 0 { (sh_db / sh_n as f64) as f32 } else { f32::NAN };
    (j, sh, sdb, cycles)
}

/// Voiced-frame long-term average spectrum → tilt (dB/oct over 1/3-oct bands 125–6300 Hz), alpha ratio, Hammarberg index.
fn spectral(x: &[f32], f0: &[f32]) -> (f32, f32, f32) {
    let nfft = 512;
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(nfft);
    let win: Vec<f32> = (0..nfft).map(|k| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * k as f32 / nfft as f32).cos()).collect();
    let mut ltas = vec![0f64; nfft / 2 + 1];
    let mut cnt = 0usize;
    let mut buf = vec![Complex32::new(0.0, 0.0); nfft];
    for (i, &f) in f0.iter().enumerate() {
        if f <= 0.0 { continue; }
        let c = i * HOP;
        if c < nfft / 2 || c + nfft / 2 > x.len() { continue; }
        let fr = &x[c - nfft / 2..c + nfft / 2];
        for k in 0..nfft { buf[k] = Complex32::new(fr[k] * win[k], 0.0); }
        fft.process(&mut buf);
        for k in 0..=nfft / 2 { ltas[k] += buf[k].norm_sqr() as f64; }
        cnt += 1;
    }
    if cnt == 0 { return (f32::NAN, f32::NAN, f32::NAN); }
    let bin_hz = SR as f64 / nfft as f64;
    let band_e = |lo: f64, hi: f64| -> f64 {
        let (a, b) = ((lo / bin_hz).ceil() as usize, ((hi / bin_hz).floor() as usize).min(nfft / 2));
        (a..=b).map(|k| ltas[k]).sum::<f64>() / cnt as f64
    };
    let band_max_db = |lo: f64, hi: f64| -> f64 {
        let (a, b) = ((lo / bin_hz).ceil() as usize, ((hi / bin_hz).floor() as usize).min(nfft / 2));
        (a..=b).map(|k| 10.0 * (ltas[k] / cnt as f64 + 1e-20).log10()).fold(f64::MIN, f64::max)
    };
    // 1/3-octave regression
    let mut xs = Vec::new(); let mut ys = Vec::new();
    let mut fc = 125.0f64;
    while fc <= 6300.0 {
        let lo = fc / 2f64.powf(1.0 / 6.0); let hi = fc * 2f64.powf(1.0 / 6.0);
        let (a, b) = ((lo / bin_hz).ceil() as usize, ((hi / bin_hz).floor() as usize).min(nfft / 2));
        if b >= a {
            let e = (a..=b).map(|k| ltas[k]).sum::<f64>() / ((b - a + 1) as f64 * cnt as f64);
            xs.push(fc.log2()); ys.push(10.0 * (e + 1e-20).log10());
        }
        fc *= 2f64.powf(1.0 / 3.0);
    }
    let mx = xs.iter().sum::<f64>() / xs.len() as f64; let my = ys.iter().sum::<f64>() / ys.len() as f64;
    let sxy: f64 = xs.iter().zip(&ys).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = xs.iter().map(|a| (a - mx).powi(2)).sum();
    let tilt = (sxy / sxx) as f32;
    let alpha = (10.0 * (band_e(1000.0, 5000.0) / band_e(50.0, 1000.0)).log10()) as f32;
    let hamm = (band_max_db(0.0, 2000.0) - band_max_db(2000.0, 5000.0)) as f32;
    (tilt, alpha, hamm)
}

pub fn compute(x16: &[f32], native: &[f32], native_sr: usize) -> Proxies {
    let db = frame_db(x16);
    let n = db.len();
    let max_db = db.iter().cloned().fold(f32::MIN, f32::max);
    // speech activity: within 35 dB of the loudest frame and above -55 dBFS
    let act_thr = (max_db - 35.0).max(-55.0);
    let (f0, _aper) = yin_f0(x16, &db, act_thr);
    let mut active: Vec<bool> = db.iter().map(|&d| d > act_thr).collect();
    // fill active gaps < 50 ms (short stop closures)
    let mut i = 0;
    while i < n {
        if !active[i] {
            let s = i; while i < n && !active[i] { i += 1; }
            if s > 0 && i < n && i - s < 5 { for v in &mut active[s..i] { *v = true; } }
        } else { i += 1; }
    }
    let first = active.iter().position(|&a| a).unwrap_or(0);
    let last = active.iter().rposition(|&a| a).unwrap_or(0);
    let span_s = (last + 1 - first) as f32 * HOP as f32 / SR as f32;
    // pauses ≥150 ms inside speech span
    let mut pauses: Vec<f32> = Vec::new();
    let mut i = first;
    while i <= last {
        if !active[i] {
            let s = i; while i <= last && !active[i] { i += 1; }
            let ms = (i - s) as f32 * 10.0;
            if ms >= 150.0 { pauses.push(ms); }
        } else { i += 1; }
    }
    let pause_total_s = pauses.iter().sum::<f32>() / 1000.0;
    // syllable nuclei: smoothed intensity peaks with ≥2 dB prominence on voiced frames, ≥100 ms apart
    let sm: Vec<f32> = (0..n).map(|i| {
        let a = i.saturating_sub(2); let b = (i + 3).min(n);
        db[a..b].iter().sum::<f32>() / (b - a) as f32
    }).collect();
    let voiced_db: Vec<f32> = (0..n).filter(|&i| f0[i] > 0.0).map(|i| sm[i]).collect();
    let peak_floor = percentile(&voiced_db, 50.0) - 10.0;
    let mut peaks: Vec<usize> = Vec::new();
    for i in 1..n.saturating_sub(1) {
        if !(sm[i] >= sm[i - 1] && sm[i] > sm[i + 1]) || f0[i] <= 0.0 || sm[i] < peak_floor { continue; }
        // prominence: min to left/right until a higher value or 300 ms
        let mut lmin = sm[i]; let mut j = i;
        while j > 0 && i - j < 30 { j -= 1; if sm[j] > sm[i] { break; } lmin = lmin.min(sm[j]); }
        let mut rmin = sm[i]; let mut j = i;
        while j + 1 < n && j - i < 30 { j += 1; if sm[j] > sm[i] { break; } rmin = rmin.min(sm[j]); }
        if sm[i] - lmin.max(rmin) < 2.0 { continue; }
        if let Some(&p) = peaks.last() {
            if i - p < 10 { if sm[i] > sm[p] { peaks.pop(); } else { continue; } }
        }
        peaks.push(i);
    }
    let voiced_f0: Vec<f32> = f0.iter().copied().filter(|&v| v > 0.0).collect();
    let med = percentile(&voiced_f0, 50.0);
    let st: Vec<f32> = voiced_f0.iter().map(|f| 12.0 * (f / med).log2()).collect();
    let act_db: Vec<f32> = db.iter().copied().filter(|&d| d > act_thr).collect();
    let total_e = x16.iter().map(|v| v * v).sum::<f32>() / x16.len().max(1) as f32;
    // (tested: restricting to YIN aperiodicity<0.10 frames or 4x upsampling did not improve Praat agreement)
    let (jit, shim, shim_db, cyc) = jitter_shimmer(native, native_sr, &f0);
    let (tilt, alpha, hamm) = spectral(x16, &f0);
    let speech_s = (span_s - pause_total_s).max(1e-3);
    Proxies {
        voiced_frac: voiced_f0.len() as f32 / (last + 1 - first).max(1) as f32,
        f0_mean_hz: mean(&voiced_f0),
        f0_median_hz: med,
        f0_sd_st: sd(&st),
        f0_range_st_p5_p95: percentile(&st, 95.0) - percentile(&st, 5.0),
        f0_range_st_minmax: percentile(&st, 100.0) - percentile(&st, 0.0),
        energy_dr_db: percentile(&act_db, 95.0) - percentile(&act_db, 5.0),
        energy_sd_db: sd(&act_db),
        rms_dbfs: 10.0 * (total_e + 1e-12).log10(),
        speech_span_s: span_s,
        syllables: peaks.len() as u32,
        speech_rate_syl_s: peaks.len() as f32 / span_s.max(1e-3),
        articulation_rate_syl_s: peaks.len() as f32 / speech_s,
        pause_count: pauses.len() as u32,
        pause_count_ge300ms: pauses.iter().filter(|&&p| p >= 300.0).count() as u32,
        pause_mean_ms: if pauses.is_empty() { 0.0 } else { mean(&pauses) },
        pause_max_ms: pauses.iter().cloned().fold(0.0, f32::max),
        pause_total_s,
        pause_ratio: pause_total_s / span_s.max(1e-3),
        jitter_local_pct: jit,
        shimmer_local_pct: shim,
        shimmer_local_db: shim_db,
        glottal_cycles: cyc,
        spectral_tilt_db_oct: tilt,
        alpha_ratio_db: alpha,
        hammarberg_db: hamm,
    }
}
