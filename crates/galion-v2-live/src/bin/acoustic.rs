// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! v2-acoustic: measurable tone/tonality/colour/contrast features of WAVs.
//! F0 (YIN, 40 ms frames), F0 range in semitones, loudness (RMS dB) and its
//! dynamic range, spectral centroid (colour/brightness), voiced ratio and a
//! syllable-rate proxy (energy-envelope peaks/s). Usage: v2-acoustic a.wav b.wav …
use galion_v2_live::{audio, stats::{pct, r1}};
use serde_json::json;

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len(); let mut j = 0;
    for i in 1..n { let mut b = n >> 1; while j & b != 0 { j ^= b; b >>= 1; } j |= b; if i < j { re.swap(i, j); im.swap(i, j); } }
    let mut len = 2;
    while len <= n {
        let a = -2.0 * std::f64::consts::PI / len as f64;
        for s in (0..n).step_by(len) { for k in 0..len / 2 {
            let (c, si) = ((a * k as f64).cos(), (a * k as f64).sin());
            let (ur, ui) = (re[s + k], im[s + k]);
            let (vr, vi) = (re[s + k + len / 2] * c - im[s + k + len / 2] * si, re[s + k + len / 2] * si + im[s + k + len / 2] * c);
            re[s + k] = ur + vr; im[s + k] = ui + vi; re[s + k + len / 2] = ur - vr; im[s + k + len / 2] = ui - vi;
        } }
        len <<= 1;
    }
}

fn yin(x: &[f64], sr: f64) -> Option<f64> {
    let (minp, maxp) = ((sr / 500.0) as usize, (sr / 60.0) as usize);
    let w = x.len() - maxp; if w < maxp { return None; }
    let mut d = vec![0.0; maxp + 1];
    for tau in 1..=maxp { d[tau] = (0..w).map(|i| (x[i] - x[i + tau]).powi(2)).sum(); }
    let mut cum = 0.0; let mut dn = vec![1.0; maxp + 1];
    for tau in 1..=maxp { cum += d[tau]; dn[tau] = d[tau] * tau as f64 / cum.max(1e-12); }
    let mut tau = minp;
    while tau < maxp { if dn[tau] < 0.15 { while tau + 1 < maxp && dn[tau + 1] < dn[tau] { tau += 1; } break; } tau += 1; }
    if tau >= maxp { return None; }
    let (a, b, c) = (dn[tau - 1], dn[tau], dn[tau + 1]);
    let den = a + c - 2.0 * b;
    let t = if den.abs() > 1e-12 { tau as f64 + (a - c) / (2.0 * den) } else { tau as f64 };
    Some(sr / t)
}

fn analyze(path: &str) -> serde_json::Value {
    let w = match audio::read_wav(path) { Ok(w) => w, Err(e) => return json!({"file": path, "error": e}) };
    let sr = 16_000u32;
    let x: Vec<f64> = audio::resample(&audio::to_mono(&w), w.rate, sr).iter().map(|&s| s as f64 / 32768.0).collect();
    let (hop, win) = (160usize, 640usize);
    let mut f0 = vec![]; let mut db = vec![]; let mut cent = vec![]; let mut env = vec![];
    let mut i = 0;
    let peak_db = x.chunks(win).map(|c| 10.0 * (c.iter().map(|v| v * v).sum::<f64>() / c.len() as f64).max(1e-12).log10()).fold(-120.0, f64::max);
    while i + win <= x.len() {
        let fr = &x[i..i + win];
        let e = 10.0 * (fr.iter().map(|v| v * v).sum::<f64>() / win as f64).max(1e-12).log10();
        env.push(e);
        if e > peak_db - 30.0 {
            db.push(e);
            if let Some(f) = yin(fr, sr as f64) { if (60.0..=500.0).contains(&f) { f0.push(f); } }
            let n = 1024; let mut re = vec![0.0; n]; let mut im = vec![0.0; n];
            for (k, v) in fr.iter().enumerate() { re[k] = v * (0.5 - 0.5 * (2.0 * std::f64::consts::PI * k as f64 / win as f64).cos()); }
            fft(&mut re, &mut im);
            let (mut num, mut den) = (0.0, 0.0);
            for k in 1..n / 2 { let m = (re[k] * re[k] + im[k] * im[k]).sqrt(); num += m * k as f64 * sr as f64 / n as f64; den += m; }
            if den > 0.0 { cent.push(num / den); }
        }
        i += hop;
    }
    // syllable proxy: peaks in smoothed envelope ≥ 6 dB prominence, ≥ 100 ms apart
    let sm: Vec<f64> = (0..env.len()).map(|k| { let a = k.saturating_sub(3); let b = (k + 4).min(env.len()); env[a..b].iter().sum::<f64>() / (b - a) as f64 }).collect();
    let mut peaks = 0; let mut last = -100i64;
    for k in 1..sm.len().saturating_sub(1) {
        if sm[k] > sm[k - 1] && sm[k] >= sm[k + 1] && sm[k] > peak_db - 25.0 && (k as i64 - last) >= 10 {
            let lo = sm[k.saturating_sub(15)..k].iter().cloned().fold(f64::INFINITY, f64::min);
            if sm[k] - lo >= 6.0 { peaks += 1; last = k as i64; }
        }
    }
    let speech_s = db.len() as f64 * hop as f64 / sr as f64;
    let st: Vec<f64> = f0.iter().map(|f| 12.0 * (f / 100.0).log2()).collect();
    json!({"file": path, "dur_s": r1(x.len() as f64 / sr as f64), "speech_s": r1(speech_s),
        "f0_median_hz": r1(pct(&f0, 50.0)), "f0_range_st_p10_p90": r1(pct(&st, 90.0) - pct(&st, 10.0)),
        "f0_sd_st": r1({ let m = st.iter().sum::<f64>() / st.len().max(1) as f64; (st.iter().map(|s| (s - m).powi(2)).sum::<f64>() / st.len().max(1) as f64).sqrt() }),
        "loud_db_median": r1(pct(&db, 50.0)), "loud_range_db_p10_p90": r1(pct(&db, 90.0) - pct(&db, 10.0)),
        "centroid_hz_median": r1(pct(&cent, 50.0)), "voiced_ratio": r1(f0.len() as f64 / db.len().max(1) as f64 * 100.0) / 100.0,
        "syll_per_s": r1(peaks as f64 / speech_s.max(0.1))})
}

fn main() {
    let v: Vec<_> = std::env::args().skip(1).map(|p| analyze(&p)).collect();
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
}
