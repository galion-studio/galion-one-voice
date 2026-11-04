// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Post-synthesis emotion DSP for the Piper path (owner t641u/t644u).
//!
//! * **Pitch**: streaming linear-interpolation resample by `ratio = 2^(st/12)`.
//!   Resampling raises pitch *and* shortens the audio by `1/ratio`; the caller
//!   sends Piper `length_scale × ratio` (see
//!   `galion_emotion::PiperProsodyVector::compensated_length_scale`) so the
//!   planned pace is kept. Formants move with the pitch; within the persona
//!   bounds (Samantha ±3 st, Jarvis ±1.5 st) that is a mild timbre shift.
//! * **Energy**: gain with a cubic soft knee above 0.8 FS (no hard clipping).
//! * **Pause**: [`EmotiveShaper::silence`] writes zeros; nothing is buffered.
//!
//! [`EmotiveShaper::process`] is allocation-free: the caller owns the output
//! slice (size it with [`EmotiveShaper::max_out`]); state between chunks is
//! one previous sample and a fractional read position, so streaming chunks
//! from Piper join without clicks.

/// Streaming pitch/energy shaper. One per utterance stream.
#[derive(Debug, Clone, Copy)]
pub struct EmotiveShaper {
    ratio: f32,
    gain: f32,
    /// Read position relative to the start of the next chunk (may be < 0: inside `prev`).
    pos: f64,
    prev: f32,
    primed: bool,
}

impl EmotiveShaper {
    /// `pitch_st` semitones (clamped ±6), `energy` gain (clamped 0.25..2).
    pub fn new(pitch_st: f32, energy: f32) -> Self {
        let st = if pitch_st.is_finite() { pitch_st.clamp(-6.0, 6.0) } else { 0.0 };
        let g = if energy.is_finite() { energy.clamp(0.25, 2.0) } else { 1.0 };
        Self { ratio: (st / 12.0).exp2(), gain: g, pos: 0.0, prev: 0.0, primed: false }
    }
    pub fn ratio(&self) -> f32 {
        self.ratio
    }
    pub fn is_identity(&self) -> bool {
        (self.ratio - 1.0).abs() < 1e-6 && (self.gain - 1.0).abs() < 1e-6
    }
    /// Upper bound of output samples for an input chunk of `n`.
    pub fn max_out(&self, n: usize) -> usize {
        (n as f64 / self.ratio as f64).ceil() as usize + 2
    }
    /// Write `ms` of silence at `rate` into `out`; returns samples written.
    pub fn silence(ms: u32, rate: u32, out: &mut [i16]) -> usize {
        let n = ((ms as u64 * rate as u64) / 1000) as usize;
        let n = n.min(out.len());
        out[..n].fill(0);
        n
    }

    #[inline]
    fn shape(&self, x: f32) -> i16 {
        let y = x * self.gain / 32768.0;
        let a = y.abs();
        let s = if a <= 0.8 {
            y
        } else {
            // cubic knee: maps 0.8..∞ smoothly into 0.8..0.98 (headroom, never full scale)
            let t = ((a - 0.8) / 0.6).min(1.0);
            (0.8 + 0.18 * (1.5 * t - 0.5 * t * t * t)) * y.signum()
        };
        (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16
    }

    /// Process one chunk. Returns samples written to `out` (never more than `max_out(input.len())`).
    pub fn process(&mut self, input: &[i16], out: &mut [i16]) -> usize {
        if input.is_empty() {
            return 0;
        }
        if (self.ratio - 1.0).abs() < 1e-6 {
            let n = input.len().min(out.len());
            for (o, &x) in out[..n].iter_mut().zip(input) {
                *o = self.shape(x as f32);
            }
            return n;
        }
        if !self.primed {
            self.prev = input[0] as f32;
            self.primed = true;
        }
        let step = self.ratio as f64;
        let len = input.len() as f64;
        let mut w = 0usize;
        // Sample at position p: p in [-1, 0) interpolates prev→input[0].
        while self.pos < len - 1.0 + 1e-9 && w < out.len() {
            let p = self.pos;
            let (a, b, frac) = if p < 0.0 {
                (self.prev, input[0] as f32, (p + 1.0) as f32)
            } else {
                let i = p.floor() as usize;
                let i1 = (i + 1).min(input.len() - 1);
                (input[i] as f32, input[i1] as f32, (p - i as f64) as f32)
            };
            out[w] = self.shape(a + (b - a) * frac);
            w += 1;
            self.pos += step;
        }
        self.prev = input[input.len() - 1] as f32;
        self.pos -= len;
        w
    }
}

/// Fundamental frequency estimate (Hz) of voiced frames by normalised
/// autocorrelation; median over frames. For the listening-set report and tests.
pub fn median_f0(pcm: &[i16], rate: u32, fmin: f32, fmax: f32) -> Option<f32> {
    let frame = (rate as f32 * 0.04) as usize;
    let hop = frame / 2;
    let lag_min = (rate as f32 / fmax) as usize;
    let lag_max = (rate as f32 / fmin) as usize;
    if pcm.len() < frame + lag_max {
        return None;
    }
    let mut f0s = Vec::new();
    let mut s = 0;
    while s + frame + lag_max < pcm.len() {
        let x = &pcm[s..s + frame + lag_max];
        let e0: f64 = x[..frame].iter().map(|&v| (v as f64).powi(2)).sum();
        if e0 / frame as f64 > 300.0f64.powi(2) {
            let mut best = (0.0f64, 0usize);
            for lag in lag_min..=lag_max {
                let mut num = 0.0f64;
                let mut el = 0.0f64;
                for i in 0..frame {
                    num += x[i] as f64 * x[i + lag] as f64;
                    el += (x[i + lag] as f64).powi(2);
                }
                let r = num / (e0 * el).sqrt().max(1.0);
                if r > best.0 {
                    best = (r, lag);
                }
            }
            if best.0 > 0.6 {
                f0s.push(rate as f32 / best.1 as f32);
            }
        }
        s += hop;
    }
    if f0s.is_empty() {
        return None;
    }
    f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Some(f0s[f0s.len() / 2])
}

/// RMS of the PCM in dBFS (silence-excluded: frames below −50 dBFS are skipped).
pub fn active_rms_dbfs(pcm: &[i16], rate: u32) -> f32 {
    let frame = (rate / 50).max(1) as usize;
    let (mut sum, mut n) = (0.0f64, 0usize);
    for c in pcm.chunks(frame) {
        let e: f64 = c.iter().map(|&v| (v as f64 / 32768.0).powi(2)).sum::<f64>() / c.len() as f64;
        if e > 1e-5 {
            sum += e * c.len() as f64;
            n += c.len();
        }
    }
    if n == 0 { -120.0 } else { (10.0 * (sum / n as f64).log10()) as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, rate: u32, secs: f32, amp: f32) -> Vec<i16> {
        (0..(rate as f32 * secs) as usize)
            .map(|i| (amp * (2.0 * std::f32::consts::PI * hz * i as f32 / rate as f32).sin() * 32767.0) as i16)
            .collect()
    }

    #[test]
    fn pitch_up_two_semitones_measures_right() {
        let rate = 22050;
        let x = tone(200.0, rate, 1.0, 0.5);
        let mut sh = EmotiveShaper::new(2.0, 1.0);
        let mut out = vec![0i16; sh.max_out(x.len())];
        let n = sh.process(&x, &mut out);
        let f = median_f0(&out[..n], rate, 60.0, 500.0).unwrap();
        let want = 200.0 * 2f32.powf(2.0 / 12.0);
        assert!((f - want).abs() / want < 0.02, "{f} vs {want}");
        let dur = n as f32 / x.len() as f32;
        assert!((dur - 1.0 / sh.ratio()).abs() < 0.01, "duration shrinks by 1/ratio");
    }

    #[test]
    fn chunked_equals_whole() {
        let x = tone(173.0, 22050, 0.5, 0.4);
        let mut a = EmotiveShaper::new(-1.5, 0.9);
        let mut whole = vec![0i16; a.max_out(x.len())];
        let nw = a.process(&x, &mut whole);
        let mut b = EmotiveShaper::new(-1.5, 0.9);
        let mut buf = vec![0i16; b.max_out(x.len())];
        let mut joined = Vec::new();
        for c in x.chunks(997) {
            let n = b.process(c, &mut buf);
            joined.extend_from_slice(&buf[..n]);
        }
        assert!((joined.len() as i64 - nw as i64).abs() <= 1);
        let m = joined.len().min(nw);
        let maxd = joined[..m].iter().zip(&whole[..m]).map(|(p, q)| (*p as i32 - *q as i32).abs()).max().unwrap();
        assert!(maxd <= 2, "chunk seams differ by {maxd}");
    }

    #[test]
    fn energy_gain_and_soft_knee() {
        let x = tone(220.0, 22050, 0.3, 0.3);
        let mut sh = EmotiveShaper::new(0.0, 0.8);
        let mut out = vec![0i16; sh.max_out(x.len())];
        let n = sh.process(&x, &mut out);
        let d = active_rms_dbfs(&out[..n], 22050) - active_rms_dbfs(&x, 22050);
        assert!((d - 20.0 * 0.8f32.log10()).abs() < 0.1, "{d}");
        let loud = tone(220.0, 22050, 0.3, 0.95);
        let mut sh = EmotiveShaper::new(0.0, 1.5);
        let n = sh.process(&loud, &mut out);
        assert!(out[..n].iter().all(|&v| v > i16::MIN && v < i16::MAX), "no hard clip");
    }

    #[test]
    fn identity_is_bit_exact() {
        let x = tone(150.0, 22050, 0.2, 0.5);
        let mut sh = EmotiveShaper::new(0.0, 1.0);
        assert!(sh.is_identity());
        let mut out = vec![0i16; sh.max_out(x.len())];
        let n = sh.process(&x, &mut out);
        assert_eq!(&out[..n], &x[..]);
    }

    #[test]
    fn silence_counts() {
        let mut out = [7i16; 4000];
        assert_eq!(EmotiveShaper::silence(100, 22050, &mut out), 2205);
        assert!(out[..2205].iter().all(|&v| v == 0));
    }
}
