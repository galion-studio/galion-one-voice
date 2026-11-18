// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Signal checks. Each one calls a cited meter. Thresholds stay in config.

use crate::config::Config;
use crate::report::{merge_frames, round6, Check, FrameSpan};
use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

pub struct Prepared {
    pub raw: Vec<f32>,
    /// Non-finite and subnormal samples replaced so later meters stay finite.
    pub clean: Vec<f32>,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u16>,
    pub frame_len: usize,
    pub n_frames: u32,
}

pub fn prepare(
    interleaved: &[f32],
    sample_rate_hz: u32,
    channels: u16,
    bits_per_sample: Option<u16>,
    tick_ms: u32,
) -> Result<Prepared, String> {
    if channels == 0 {
        return Err("channels is 0".into());
    }
    if interleaved.len() % channels as usize != 0 {
        return Err("interleaved length is not a multiple of channels".into());
    }
    let ch = channels as usize;
    let n = interleaved.len() / ch;
    let mut raw = vec![0.0f32; n];
    for (i, frame) in interleaved.chunks_exact(ch).enumerate() {
        let mut s = 0.0f32;
        for c in frame {
            s += *c;
        }
        raw[i] = s / ch as f32;
    }
    let mut clean = raw.clone();
    let mut last = 0.0f32;
    for s in &mut clean {
        if !s.is_finite() || s.is_subnormal() {
            *s = last;
        } else {
            last = *s;
        }
    }
    let frame_len = frame_len(sample_rate_hz, tick_ms).unwrap_or(0);
    let n_frames = if frame_len == 0 || n == 0 {
        0
    } else {
        n.div_ceil(frame_len) as u32
    };
    Ok(Prepared {
        raw,
        clean,
        sample_rate_hz,
        channels,
        bits_per_sample,
        frame_len,
        n_frames,
    })
}

pub fn frame_len(sr: u32, tick_ms: u32) -> Option<usize> {
    let n = sr as u64 * tick_ms as u64;
    if sr == 0 || tick_ms == 0 || n % 1000 != 0 {
        None
    } else {
        Some((n / 1000) as usize)
    }
}

fn frame_of(sample: usize, frame_len: usize) -> u32 {
    if frame_len == 0 {
        0
    } else {
        (sample / frame_len) as u32
    }
}

fn spans_from_samples(samples: &[usize], frame_len: usize, n_frames: u32) -> Vec<FrameSpan> {
    if frame_len == 0 {
        return Vec::new();
    }
    let idxs: Vec<u32> = samples
        .iter()
        .map(|s| frame_of(*s, frame_len))
        .filter(|f| *f < n_frames)
        .collect();
    merge_frames(&idxs)
}

fn all_frames(n_frames: u32) -> Vec<FrameSpan> {
    if n_frames == 0 {
        Vec::new()
    } else {
        vec![FrameSpan {
            start: 0,
            end: n_frames,
        }]
    }
}

fn db20(amp: f64) -> f64 {
    if amp <= 1e-12 {
        -240.0
    } else {
        20.0 * amp.log10()
    }
}

fn rms_db(x: &[f32]) -> f64 {
    if x.is_empty() {
        return -240.0;
    }
    let mut s = 0.0f64;
    for v in x {
        let y = *v as f64;
        s += y * y;
    }
    let p = s / x.len() as f64;
    if p <= 1e-20 {
        -200.0
    } else {
        10.0 * p.log10()
    }
}

pub fn finite(p: &Prepared) -> Check {
    let mut bad = Vec::new();
    for (i, s) in p.raw.iter().enumerate() {
        if !s.is_finite() {
            bad.push(i);
        }
    }
    let n = p.raw.len().max(1) as f64;
    let score = 1.0 - bad.len() as f64 / n;
    let pass = bad.is_empty() && !p.raw.is_empty();
    Check {
        name: "finite",
        status: if pass { "pass" } else { "fail" },
        score: Some(round6(score.clamp(0.0, 1.0))),
        value: Some(bad.len() as f64),
        limit: Some(0.0),
        unit: "samples",
        reason: if p.raw.is_empty() {
            "empty clip".into()
        } else if pass {
            "every sample is finite".into()
        } else {
            format!("{} NaN or Inf sample(s)", bad.len())
        },
        frames: spans_from_samples(&bad, p.frame_len, p.n_frames),
        method: "IEEE 754: a PCM sample must be finite",
    }
}

pub fn denormal(p: &Prepared) -> Check {
    let mut bad = Vec::new();
    for (i, s) in p.raw.iter().enumerate() {
        if s.is_subnormal() {
            bad.push(i);
        }
    }
    let n = p.raw.len().max(1) as f64;
    let score = 1.0 - bad.len() as f64 / n;
    let pass = bad.is_empty();
    Check {
        name: "denormal",
        status: if pass { "pass" } else { "fail" },
        score: Some(round6(score.clamp(0.0, 1.0))),
        value: Some(bad.len() as f64),
        limit: Some(0.0),
        unit: "samples",
        reason: if pass {
            "no subnormal samples".into()
        } else {
            format!("{} subnormal sample(s)", bad.len())
        },
        frames: spans_from_samples(&bad, p.frame_len, p.n_frames),
        method: "IEEE 754 subnormal numbers are not valid full-scale PCM",
    }
}

pub fn dc(p: &Prepared, max_dbfs: f64) -> Check {
    if p.clean.is_empty() {
        return skipped(
            "dc",
            "empty clip",
            "sample mean, AES17 / galion-refcheck dc_offset_dbfs",
        );
    }
    let mean = p.clean.iter().map(|s| *s as f64).sum::<f64>() / p.clean.len() as f64;
    let db = db20(mean.abs());
    let pass = db <= max_dbfs;
    Check {
        name: "dc",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(db)),
        limit: Some(max_dbfs),
        unit: "dBFS",
        reason: if pass {
            format!("mean {db:.2} dBFS is at or under {max_dbfs:.0}")
        } else {
            format!("mean {db:.2} dBFS is above {max_dbfs:.0}")
        },
        frames: if pass {
            Vec::new()
        } else {
            all_frames(p.n_frames)
        },
        method: "sample mean in dBFS, same meter as galion-refcheck dc_offset_dbfs",
    }
}

pub fn clipping(p: &Prepared, linear: f64) -> Check {
    let mut bad = Vec::new();
    let mut peak = 0.0f64;
    for (i, s) in p.clean.iter().enumerate() {
        let a = (*s as f64).abs();
        peak = peak.max(a);
        if a >= linear {
            bad.push(i);
        }
    }
    let pass = bad.is_empty();
    Check {
        name: "clipping",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(db20(peak))),
        limit: Some(linear),
        unit: "dBFS",
        reason: if pass {
            format!(
                "sample peak {:.2} dBFS, no sample at or above {linear}",
                db20(peak)
            )
        } else {
            format!("{} sample(s) at or above {linear}", bad.len())
        },
        frames: spans_from_samples(&bad, p.frame_len, p.n_frames),
        method: "galion-hygiene clipped-sample count (|x| >= clip threshold)",
    }
}

pub fn true_peak(p: &Prepared, max_dbtp: f64) -> Check {
    if p.clean.is_empty() || p.sample_rate_hz == 0 {
        return fail_closed(
            "true_peak",
            "no samples to measure",
            "ebur128 true peak, ITU-R BS.1770",
        );
    }
    match peak_dbtp(&p.clean, p.sample_rate_hz) {
        Ok(db) => {
            let pass = db <= max_dbtp;
            let frames = if pass {
                Vec::new()
            } else {
                frames_over_true_peak(p, max_dbtp)
            };
            Check {
                name: "true_peak",
                status: if pass { "pass" } else { "fail" },
                score: Some(if pass { 1.0 } else { 0.0 }),
                value: Some(round6(db)),
                limit: Some(max_dbtp),
                unit: "dBTP",
                reason: if pass {
                    format!("{db:.2} dBTP is at or under {max_dbtp:.1}")
                } else {
                    format!("{db:.2} dBTP is above {max_dbtp:.1}")
                },
                frames,
                method: "ebur128 0.1.11 true peak (ITU-R BS.1770, 4x oversample under 96 kHz)",
            }
        }
        Err(e) => fail_closed("true_peak", &e, "ebur128 true peak, ITU-R BS.1770"),
    }
}

fn peak_dbtp(mono: &[f32], sr: u32) -> Result<f64, String> {
    let mut meter = ebur128::EbuR128::new(1, sr, ebur128::Mode::TRUE_PEAK)
        .map_err(|e| format!("ebur128: {e}"))?;
    meter
        .add_frames_f32(mono)
        .map_err(|e| format!("ebur128: {e}"))?;
    let lin = meter.true_peak(0).map_err(|e| format!("ebur128: {e}"))?;
    Ok(db20(lin))
}

fn frames_over_true_peak(p: &Prepared, max_dbtp: f64) -> Vec<FrameSpan> {
    if p.frame_len == 0 {
        return all_frames(p.n_frames);
    }
    let mut bad = Vec::new();
    for i in 0..p.n_frames as usize {
        let a = i * p.frame_len;
        let b = (a + p.frame_len).min(p.clean.len());
        if a >= b {
            continue;
        }
        if let Ok(db) = peak_dbtp(&p.clean[a..b], p.sample_rate_hz) {
            if db > max_dbtp {
                bad.push(i as u32);
            }
        }
    }
    if bad.is_empty() {
        all_frames(p.n_frames)
    } else {
        merge_frames(&bad)
    }
}

pub fn click(p: &Prepared, jump: f64) -> Check {
    let mut bad = Vec::new();
    let mut max_j = 0.0f64;
    for i in 1..p.clean.len() {
        let j = (p.clean[i] as f64 - p.clean[i - 1] as f64).abs();
        max_j = max_j.max(j);
        if j > jump {
            bad.push(i);
        }
    }
    let pass = bad.is_empty();
    Check {
        name: "click",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(max_j)),
        limit: Some(jump),
        unit: "linear",
        reason: if pass {
            format!("max sample jump {max_j:.4} is at or under {jump}")
        } else {
            format!("{} sample jump(s) above {jump}", bad.len())
        },
        frames: spans_from_samples(&bad, p.frame_len, p.n_frames),
        method: "galion-hygiene first difference |x[n]-x[n-1]|",
    }
}

pub fn ar_click(p: &Prepared, order: usize, residual_limit: f64) -> Check {
    let x: Vec<f64> = p.clean.iter().map(|s| *s as f64).collect();
    let order = order.min(x.len().saturating_sub(2));
    if order < 2 || x.len() < order + 8 {
        return skipped(
            "ar_click",
            "clip shorter than the AR window",
            "Godsill & Rayner AR residual",
        );
    }
    let a = lpc(&x, order);
    let mut bad = Vec::new();
    let mut max_e = 0.0f64;
    for n in order..x.len() {
        let mut pred = 0.0;
        for (k, ak) in a.iter().enumerate() {
            pred += ak * x[n - 1 - k];
        }
        let e = (x[n] - pred).abs();
        max_e = max_e.max(e);
        if e > residual_limit {
            bad.push(n);
        }
    }
    let pass = bad.is_empty();
    Check {
        name: "ar_click",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(max_e)),
        limit: Some(residual_limit),
        unit: "linear",
        reason: if pass {
            format!("max AR residual {max_e:.4} is at or under {residual_limit}")
        } else {
            format!("{} AR residual(s) above {residual_limit}", bad.len())
        },
        frames: spans_from_samples(&bad, p.frame_len, p.n_frames),
        method:
            "forward LPC residual (Godsill & Rayner AR click model; Levinson-Durbin, Markel & Gray)",
    }
}

/// Predictor coefficients a[1..=order] for x[n] ≈ sum a[k] x[n-k].
fn lpc(x: &[f64], order: usize) -> Vec<f64> {
    let n = x.len();
    let mut r = vec![0.0; order + 1];
    for (lag, bin) in r.iter_mut().enumerate() {
        let mut s = 0.0;
        for i in lag..n {
            s += x[i] * x[i - lag];
        }
        *bin = s;
    }
    if r[0] < 1e-18 {
        return vec![0.0; order];
    }
    let mut a = vec![0.0; order + 1];
    let mut prev = vec![0.0; order + 1];
    let mut err = r[0];
    for i in 1..=order {
        let mut acc = r[i];
        for j in 1..i {
            acc -= prev[j] * r[i - j];
        }
        let k = if err.abs() < 1e-18 {
            0.0
        } else {
            (acc / err).clamp(-0.999, 0.999)
        };
        a[i] = k;
        for j in 1..i {
            a[j] = prev[j] - k * prev[i - j];
        }
        err *= 1.0 - k * k;
        prev.copy_from_slice(&a);
    }
    a[1..].to_vec()
}

pub fn dropout(p: &Prepared, cfg: &Config) -> Check {
    let win = ((p.sample_rate_hz as u64 * cfg.dropout.window_ms as u64) / 1000).max(1) as usize;
    if p.clean.len() < win * 2 {
        return skipped(
            "dropout",
            "clip shorter than two silence windows",
            "refprep energy mask, articulation select PAUSE_S",
        );
    }
    let mut silent = Vec::new();
    let mut i = 0;
    while i + win <= p.clean.len() {
        silent.push(rms_db(&p.clean[i..i + win]) <= cfg.dropout.silence_dbfs);
        i += win;
    }
    let min_windows = ((cfg.dropout.min_gap_s * p.sample_rate_hz as f64) / win as f64)
        .ceil()
        .max(1.0) as usize;
    let mut bad_samples = Vec::new();
    if silent.iter().all(|s| *s) && silent.len() >= min_windows {
        bad_samples.extend(0..p.clean.len());
    }
    let mut run_start: Option<usize> = None;
    for (w, is_sil) in silent.iter().enumerate() {
        if *is_sil {
            if run_start.is_none() {
                run_start = Some(w);
            }
        } else if let Some(s) = run_start.take() {
            note_gap(s, w, min_windows, win, &mut bad_samples);
        }
    }
    // A trailing run is not an internal dropout. `run_start` left open is the tail.
    let _ = run_start;
    let pass = bad_samples.is_empty();
    let gap_s = if pass {
        0.0
    } else {
        bad_samples.len() as f64 / p.sample_rate_hz as f64
    };
    Check {
        name: "dropout",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(gap_s)),
        limit: Some(cfg.dropout.min_gap_s),
        unit: "s",
        reason: if pass {
            format!("no internal mute at or above {:.2} s", cfg.dropout.min_gap_s)
        } else {
            format!("internal mute of about {gap_s:.3} s")
        },
        frames: spans_from_samples(&bad_samples, p.frame_len, p.n_frames),
        method: "energy mask. Silence is RMS at or under the configured floor. Gap length is articulation select PAUSE_S (0.20 s in the bundled config)",
    }
}

fn note_gap(start: usize, end: usize, min_windows: usize, win: usize, out: &mut Vec<usize>) {
    // Internal: the caller only invokes this when a loud window follows, and `start > 0`
    // means a loud window came before. A leading mute is not a dropout.
    if start == 0 || end - start < min_windows {
        return;
    }
    let a = start * win;
    let b = end * win;
    out.extend(a..b);
}

pub fn repeated_frame(p: &Prepared, cfg: &Config) -> Check {
    let fl = p.frame_len;
    if fl == 0 || p.clean.len() < fl * cfg.r#loop.min_run_frames as usize {
        return skipped(
            "repeated_frame",
            "fewer frames than the minimum run",
            "normalised frame correlation, pitch lag excluded (Rabiner 1977)",
        );
    }
    let n = p.clean.len() / fl;
    let mut ncc_next = vec![0.0f64; n.saturating_sub(1)];
    for (i, slot) in ncc_next.iter_mut().enumerate() {
        *slot = ncc(
            &p.clean[i * fl..(i + 1) * fl],
            &p.clean[(i + 1) * fl..(i + 2) * fl],
        );
    }
    let mut bad = Vec::new();
    let mut i = 0;
    while i < ncc_next.len() {
        if ncc_next[i] < cfg.r#loop.ncc {
            i += 1;
            continue;
        }
        let start = i;
        while i < ncc_next.len() && ncc_next[i] >= cfg.r#loop.ncc {
            i += 1;
        }
        let run_frames = (i - start) + 1;
        if run_frames >= cfg.r#loop.min_run_frames as usize && !pitch_explains(p, start, cfg) {
            for f in start..start + run_frames {
                bad.push(f as u32);
            }
        }
    }
    let pass = bad.is_empty();
    let peak = ncc_next.iter().copied().fold(0.0f64, f64::max);
    Check {
        name: "repeated_frame",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(round6(peak)),
        limit: Some(cfg.r#loop.ncc),
        unit: "ncc",
        reason: if pass {
            format!("no {0}-frame copy above NCC {1}", cfg.r#loop.min_run_frames, cfg.r#loop.ncc)
        } else {
            format!("copied 80 ms frames, peak NCC {peak:.3}")
        },
        frames: merge_frames(&bad),
        method: "normalised correlation of successive 80 ms frames. A pitch lag that tiles the frame is not a loop (Rabiner 1977 autocorrelation)",
    }
}

fn pitch_explains(p: &Prepared, frame_index: usize, cfg: &Config) -> bool {
    let fl = p.frame_len;
    let a = frame_index * fl;
    let b = (a + fl).min(p.clean.len());
    let Some(lag) = pitch_lag(&p.clean[a..b], p.sample_rate_hz, cfg.r#loop.pitch_ncc) else {
        return false;
    };
    let rem = fl % lag;
    let tol = cfg.r#loop.pitch_lag_tolerance_samples;
    rem <= tol || lag - rem <= tol
}

fn pitch_lag(frame: &[f32], sr: u32, min_ncc: f64) -> Option<usize> {
    if frame.len() < 8 || sr == 0 {
        return None;
    }
    let min_lag = (sr as usize / 400).max(2);
    let max_lag = (sr as usize / 70).min(frame.len() / 2);
    if max_lag <= min_lag {
        return None;
    }
    let mut e0 = 0.0f64;
    for s in frame {
        e0 += (*s as f64) * (*s as f64);
    }
    if e0 < 1e-8 {
        return None;
    }
    let mut best = 0.0f64;
    let mut scores = Vec::with_capacity(max_lag - min_lag + 1);
    for l in min_lag..=max_lag {
        let mut dot = 0.0f64;
        let mut ea = 0.0f64;
        let mut eb = 0.0f64;
        for i in 0..frame.len() - l {
            let a = frame[i] as f64;
            let b = frame[i + l] as f64;
            dot += a * b;
            ea += a * a;
            eb += b * b;
        }
        let n = (ea * eb).sqrt();
        let c = if n < 1e-12 { 0.0 } else { dot / n };
        if c > best {
            best = c;
        }
        scores.push(c);
    }
    if best < min_ncc {
        return None;
    }
    // Shortest lag near the peak, so a harmonic multiple is not used as the period.
    scores
        .iter()
        .position(|c| *c >= best * 0.98)
        .map(|i| i + min_lag)
}

fn ncc(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f64;
    let mut ea = 0.0f64;
    let mut eb = 0.0f64;
    for i in 0..n {
        let x = a[i] as f64;
        let y = b[i] as f64;
        dot += x * y;
        ea += x * x;
        eb += y * y;
    }
    if ea < 1e-10 || eb < 1e-10 {
        return 0.0;
    }
    dot / (ea * eb).sqrt()
}

pub fn babble(p: &Prepared, cfg: &Config) -> Check {
    let fl = p.frame_len;
    if fl < 16 || p.clean.len() < fl {
        return skipped(
            "babble",
            "clip shorter than one frame",
            "Johnston 1988 spectral flatness",
        );
    }
    let mut planner = FftPlanner::<f64>::new();
    let nfft = fl.next_power_of_two();
    let fft = planner.plan_fft_forward(nfft);
    let mut flat = Vec::new();
    let n = p.clean.len() / fl;
    for i in 0..n {
        let frame = &p.clean[i * fl..(i + 1) * fl];
        if rms_db(frame) < cfg.flatness.speech_floor_dbfs {
            continue;
        }
        flat.push(spectral_flatness(frame, fft.as_ref(), nfft));
    }
    if flat.is_empty() {
        return skipped(
            "babble",
            "no frame above the speech-floor RMS",
            "Johnston 1988 spectral flatness",
        );
    }
    flat.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let med = flat[flat.len() / 2];
    let pass = med <= cfg.flatness.max_median;
    let score = (1.0 - med).clamp(0.0, 1.0);
    Check {
        name: "babble",
        status: if pass { "pass" } else { "fail" },
        score: Some(round6(score)),
        value: Some(round6(med)),
        limit: Some(cfg.flatness.max_median),
        unit: "flatness",
        reason: if pass {
            format!("median spectral flatness {med:.3} is at or under {}", cfg.flatness.max_median)
        } else {
            format!("noise-like spectrum, median flatness {med:.3} (babble)")
        },
        frames: if pass { Vec::new() } else { all_frames(p.n_frames) },
        method: "spectral flatness, geometric mean / arithmetic mean (Johnston 1988). rustfft. Near 1 is white noise",
    }
}

fn spectral_flatness(frame: &[f32], fft: &dyn Fft<f64>, nfft: usize) -> f64 {
    let mut buf = vec![Complex::new(0.0, 0.0); nfft];
    let m = frame.len();
    for (i, s) in frame.iter().enumerate() {
        let hann = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / m as f64).cos();
        buf[i] = Complex::new(*s as f64 * hann, 0.0);
    }
    fft.process(&mut buf);
    let mut log_sum = 0.0f64;
    let mut sum = 0.0f64;
    let mut n = 0.0f64;
    for bin in buf.iter().take(nfft / 2 + 1).skip(1) {
        let p = bin.norm_sqr();
        if p <= 1e-20 {
            continue;
        }
        log_sum += p.ln();
        sum += p;
        n += 1.0;
    }
    if n < 1.0 || sum <= 0.0 {
        return 0.0;
    }
    (log_sum / n).exp() / (sum / n)
}

pub fn cutoff(p: &Prepared, cfg: &Config, expect_release: bool) -> Check {
    if !expect_release {
        return skipped(
            "cutoff",
            "release not required on this stream",
            "click jump into a trailing mute",
        );
    }
    let min_tail =
        ((p.sample_rate_hz as u64 * cfg.cutoff.min_tail_ms as u64) / 1000).max(1) as usize;
    let silence = 10f64.powf(cfg.dropout.silence_dbfs / 20.0) as f32;
    if p.clean.len() <= min_tail + 1 {
        return skipped(
            "cutoff",
            "clip shorter than the tail window",
            "click jump into a trailing mute",
        );
    }
    let mut cut = p.clean.len();
    while cut > 0 && p.clean[cut - 1].abs() <= silence {
        cut -= 1;
    }
    let tail = p.clean.len() - cut;
    let jump = if cut > 0 && cut < p.clean.len() {
        (p.clean[cut] as f64 - p.clean[cut - 1] as f64).abs()
    } else {
        0.0
    };
    let hit = tail >= min_tail && cut > 0 && jump > cfg.cutoff.jump;
    Check {
        name: "cutoff",
        status: if hit { "fail" } else { "pass" },
        score: Some(if hit { 0.0 } else { 1.0 }),
        value: Some(round6(jump)),
        limit: Some(cfg.cutoff.jump),
        unit: "linear",
        reason: if hit {
            format!(
                "hard mute of {} ms after a jump of {jump:.3}",
                (tail as f64 * 1000.0 / p.sample_rate_hz as f64).round()
            )
        } else {
            "no jump into a trailing mute".into()
        },
        frames: if hit {
            spans_from_samples(&[cut.saturating_sub(1)], p.frame_len, p.n_frames)
        } else {
            Vec::new()
        },
        method: "galion-hygiene sample jump at the boundary of a trailing mute",
    }
}

pub fn format(p: &Prepared, cfg: &Config) -> Check {
    let mut why = Vec::new();
    if p.raw.is_empty() {
        why.push("empty clip".to_string());
    }
    if p.sample_rate_hz == 0
        || !cfg
            .format
            .allowed_sample_rate_hz
            .contains(&p.sample_rate_hz)
    {
        why.push(format!(
            "sample rate {} Hz is not in {:?}",
            p.sample_rate_hz, cfg.format.allowed_sample_rate_hz
        ));
    }
    if p.channels != cfg.format.channels {
        why.push(format!(
            "channels {} , config wants {}",
            p.channels, cfg.format.channels
        ));
    }
    if let Some(bits) = p.bits_per_sample {
        if !cfg.format.allowed_bits.contains(&bits) {
            why.push(format!(
                "bit depth {bits} is not in {:?}",
                cfg.format.allowed_bits
            ));
        }
    }
    if p.sample_rate_hz > 0 && frame_len(p.sample_rate_hz, cfg.tick_ms).is_none() {
        why.push(format!(
            "{} ms is not an integer number of samples at {} Hz",
            cfg.tick_ms, p.sample_rate_hz
        ));
    }
    let pass = why.is_empty();
    Check {
        name: "format",
        status: if pass { "pass" } else { "fail" },
        score: Some(if pass { 1.0 } else { 0.0 }),
        value: Some(p.sample_rate_hz as f64),
        limit: None,
        unit: "Hz",
        reason: if pass {
            format!("{} Hz, {} ch", p.sample_rate_hz, p.channels)
        } else {
            why.join("; ")
        },
        frames: if pass { Vec::new() } else { all_frames(p.n_frames) },
        method: "header or caller rate, channel count, and bit depth against configs/symphony/voice-guard.toml",
    }
}

fn skipped(name: &'static str, reason: &str, method: &'static str) -> Check {
    Check {
        name,
        status: "skipped",
        score: None,
        value: None,
        limit: None,
        unit: "",
        reason: reason.into(),
        frames: Vec::new(),
        method,
    }
}

fn fail_closed(name: &'static str, reason: &str, method: &'static str) -> Check {
    Check {
        name,
        status: "fail",
        score: Some(0.0),
        value: None,
        limit: None,
        unit: "",
        reason: reason.into(),
        frames: Vec::new(),
        method,
    }
}

pub fn stub(name: &'static str, reason: &str, method: &'static str) -> Check {
    Check {
        name,
        status: "stub",
        score: None,
        value: None,
        limit: None,
        unit: "",
        reason: reason.into(),
        frames: Vec::new(),
        method,
    }
}
