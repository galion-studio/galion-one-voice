// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! WAV loading and high-quality rational windowed-sinc (Kaiser) resampling.
use anyhow::{bail, Context, Result};

pub struct Audio {
    pub sr: u32,
    pub samples: Vec<f32>, // mono, [-1, 1]
}

pub fn load_wav(path: &str) -> Result<Audio> {
    let mut r = hound::WavReader::open(path).with_context(|| format!("open {path}"))?;
    let spec = r.spec();
    let ch = spec.channels.max(1) as usize;
    let raw: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|v| v as f32 * scale)).collect::<Result<_, _>>()?
        }
    };
    if raw.is_empty() {
        bail!("{path}: empty audio");
    }
    let samples = if ch == 1 {
        raw
    } else {
        raw.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect()
    };
    Ok(Audio { sr: spec.sample_rate, samples })
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    let q = x * x / 4.0;
    for k in 1..50 {
        term *= q / (k as f64 * k as f64);
        sum += term;
        if term < 1e-12 * sum {
            break;
        }
    }
    sum
}

/// Polyphase windowed-sinc resampler (Kaiser beta=8.6, 32 zero crossings, cutoff 0.95*Nyquist of the lower rate).
pub fn resample(x: &[f32], sr_in: u32, sr_out: u32) -> Vec<f32> {
    if sr_in == sr_out {
        return x.to_vec();
    }
    let g = gcd(sr_in as u64, sr_out as u64);
    let up = (sr_out as u64 / g) as usize; // L
    let down = (sr_in as u64 / g) as usize; // M
    let scale = (sr_out as f64 / sr_in as f64).min(1.0);
    let fc = 0.5 * scale * 0.95; // cycles per input sample
    let zeros = 32.0;
    let half = (zeros / (2.0 * fc) * 0.5).ceil() as isize; // half width in input samples
    let beta = 8.6;
    let i0b = bessel_i0(beta);
    let taps = (2 * half + 1) as usize;
    // filters[p][k]: phase p = fractional offset p/up
    let mut filters = vec![vec![0f32; taps]; up];
    for p in 0..up {
        let frac = p as f64 / up as f64;
        let mut s = 0.0;
        for k in 0..taps {
            let t = (k as isize - half) as f64 - frac; // input-sample distance
            let sinc = if t.abs() < 1e-12 { 2.0 * fc } else { (2.0 * std::f64::consts::PI * fc * t).sin() / (std::f64::consts::PI * t) };
            let r = t / (half as f64 + 1.0);
            let w = if r.abs() >= 1.0 { 0.0 } else { bessel_i0(beta * (1.0 - r * r).sqrt()) / i0b };
            let v = sinc * w;
            filters[p][k] = v as f32;
            s += v;
        }
        for v in filters[p].iter_mut() {
            *v = (*v as f64 / s) as f32; // unity DC gain per phase
        }
    }
    let n_out = ((x.len() as u64 * up as u64) / down as u64) as usize;
    let mut y = Vec::with_capacity(n_out);
    let n = x.len() as isize;
    for j in 0..n_out {
        let num = j * down;
        let base = (num / up) as isize;
        let p = num % up;
        let f = &filters[p];
        let start = base - half;
        let mut acc = 0f32;
        if start >= 0 && start + taps as isize <= n {
            let seg = &x[start as usize..start as usize + taps];
            for k in 0..taps {
                acc += seg[k] * f[k];
            }
        } else {
            for k in 0..taps {
                let i = start + k as isize;
                if i >= 0 && i < n {
                    acc += x[i as usize] * f[k];
                }
            }
        }
        y.push(acc);
    }
    y
}
