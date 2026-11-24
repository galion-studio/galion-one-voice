// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Synthetic fixtures. Sine, noise, a formant resonator, a chord, a room tail.
//!
//! These are not a voice, not a recording of a person, and not a clone.
//! Nothing here is written into the repository.

const PI: f64 = std::f64::consts::PI;

fn xorshift(state: &mut u64) -> u64 {
    if *state == 0 {
        *state = 0x9E37_79B9_7F4A_7C15;
    }
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn gauss(state: &mut u64) -> f64 {
    let u1 = ((xorshift(state) as f64) + 1.0) / (u64::MAX as f64 + 2.0);
    let u2 = (xorshift(state) as f64) / (u64::MAX as f64 + 1.0);
    let u1 = u1.clamp(1e-12, 1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
}

pub fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    (x.iter()
        .map(|s| {
            let v = *s as f64;
            v * v
        })
        .sum::<f64>()
        / x.len() as f64)
        .sqrt()
}

pub fn scale_rms(x: &mut [f32], target: f64) {
    let r = rms(x);
    if r > 0.0 && target.is_finite() {
        let g = target / r;
        for s in x {
            *s = (*s as f64 * g) as f32;
        }
    }
}

pub fn white(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = seed | 1;
    (0..n).map(|_| gauss(&mut rng) as f32).collect()
}

/// Kellet pink filter. Warm-up samples are dropped so the slope is the steady one.
pub fn pink(n: usize, seed: u64) -> Vec<f32> {
    let mut rng = seed | 1;
    let mut b0 = 0.0;
    let mut b1 = 0.0;
    let mut b2 = 0.0;
    let mut b3 = 0.0;
    let mut b4 = 0.0;
    let mut b5 = 0.0;
    let mut b6 = 0.0;
    let warm = 4096usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n + warm {
        let w = gauss(&mut rng);
        b0 = 0.99886 * b0 + w * 0.0555179;
        b1 = 0.99332 * b1 + w * 0.0750759;
        b2 = 0.96900 * b2 + w * 0.1538520;
        b3 = 0.86650 * b3 + w * 0.3104856;
        b4 = 0.55000 * b4 + w * 0.5329522;
        b5 = -0.7616 * b5 - w * 0.0168980;
        let p = b0 + b1 + b2 + b3 + b4 + b5 + b6 + w * 0.5362;
        b6 = w * 0.115926;
        if i >= warm {
            out.push(p as f32);
        }
    }
    out
}

pub fn sine(n: usize, sr: u32, hz: f64, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| amp * (2.0 * PI * hz * i as f64 / f64::from(sr)).sin() as f32)
        .collect()
}

pub fn chord(n: usize, sr: u32, hz: &[f64], amp: f32) -> Vec<f32> {
    if hz.is_empty() {
        return vec![0.0; n];
    }
    let each = amp / (hz.len() as f32).sqrt();
    let mut y = vec![0.0f32; n];
    for &f in hz {
        let tone = sine(n, sr, f, each);
        for (a, b) in y.iter_mut().zip(tone.iter()) {
            *a += *b;
        }
    }
    y
}

fn resonator(x: &[f64], sr: u32, hz: f64, bw: f64) -> Vec<f64> {
    let r = (-PI * bw / f64::from(sr)).exp();
    let theta = 2.0 * PI * hz / f64::from(sr);
    let a1 = 2.0 * r * theta.cos();
    let a2 = -(r * r);
    let mut y = Vec::with_capacity(x.len());
    let mut y1 = 0.0;
    let mut y2 = 0.0;
    for &s in x {
        let v = s + a1 * y1 + a2 * y2;
        y2 = y1;
        y1 = v;
        y.push(v);
    }
    y
}

/// Pulse train through two-pole resonators. `formants` is `(hz, bandwidth)`.
/// The result is scaled to `rms_target`. It is a filter, not a person.
pub fn formant_voice(
    n: usize,
    sr: u32,
    f0_hz: f64,
    rms_target: f64,
    formants: &[(f64, f64)],
) -> Vec<f32> {
    let mut x = vec![0.0f64; n];
    let period = (f64::from(sr) / f0_hz).round().max(1.0);
    let mut i = 0.0;
    while (i as usize) < n {
        x[i as usize] = 1.0;
        i += period;
    }
    for &(hz, bw) in formants {
        x = resonator(&x, sr, hz, bw);
    }
    let mut y: Vec<f32> = x.into_iter().map(|v| v as f32).collect();
    scale_rms(&mut y, rms_target);
    y
}

/// Add `src` at `at`, with a linear fade on `src` so the join is not a click.
pub fn place(dst: &mut [f32], src: &[f32], at: usize, fade: usize) {
    if at >= dst.len() || src.is_empty() {
        return;
    }
    let n = src.len().min(dst.len() - at);
    let fade = fade.min(n / 2).max(1);
    for i in 0..n {
        let mut g = 1.0f32;
        if i < fade {
            g = i as f32 / fade as f32;
        } else if i + fade >= n {
            g = (n - 1 - i) as f32 / fade as f32;
        }
        dst[at + i] += src[i] * g.max(0.0);
    }
}

/// Noise throughout, plus a formant region at a known SNR.
/// SNR is 20 log10(rms_speech / rms_noise) before the signals are added.
pub fn noise_mix(sr: u32, snr_db: f64, pink_noise: bool, seed: u64) -> Vec<f32> {
    let n = sr as usize * 4;
    let noise_rms = 0.01;
    let mut bed = if pink_noise {
        pink(n, seed)
    } else {
        white(n, seed)
    };
    scale_rms(&mut bed, noise_rms);
    let speech_n = sr as usize * 2;
    let speech = formant_voice(
        speech_n,
        sr,
        140.0,
        noise_rms * 10f64.powf(snr_db / 20.0),
        &[(700.0, 90.0), (1500.0, 120.0), (2500.0, 150.0)],
    );
    place(&mut bed, &speech, sr as usize, sr as usize / 100);
    bed
}

/// Sustain, then an exponential tail, then a quiet bed.
/// Amplitude falls by 60 dB in `rt60_s` (factor `exp(-6.907755 * t / rt60)`).
pub fn room_tail(sr: u32, rt60_s: f64, seed: u64) -> Vec<f32> {
    let sustain = (f64::from(sr) * 1.2) as usize;
    let decay = (f64::from(sr) * (rt60_s * 2.5).max(0.8)) as usize;
    let tail = (f64::from(sr) * 0.8) as usize;
    let n = sustain + decay + tail;
    let mut x = white(n, seed);
    let sus = 0.08f32;
    let floor = sus * 10f32.powf(-70.0 / 20.0);
    for (i, sample) in x.iter_mut().enumerate() {
        let g = if i < sustain {
            sus
        } else if i < sustain + decay {
            let t = (i - sustain) as f64 / f64::from(sr);
            (sus * (-6.907_755_278_98 * t / rt60_s).exp() as f32).max(floor)
        } else {
            floor
        };
        *sample *= g;
    }
    let fade = (f64::from(sr) * 0.02) as usize;
    for (i, sample) in x.iter_mut().enumerate().take(fade.min(n)) {
        *sample *= i as f32 / fade as f32;
    }
    x
}
