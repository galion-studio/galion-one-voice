// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! 80 ms frames at 24 kHz. Same sizes as `galion-heartbeat`:
//! `FRAME_MS = 80`, `SAMPLE_RATE = 24_000`, `FRAME_SAMPLES = 1920`.
//! A short tail is returned with `filled < 1920` and the rest zero.
//! That tail is a pause, not a stretched frame.

/// Heartbeat period. Locked to the same 80 ms as `galion-heartbeat`.
pub const HEARTBEAT_MS: u32 = 80;
/// Lane rate. Piper native output stays 22050; this is the resample target.
pub const HEARTBEAT_HZ: u32 = 24_000;
/// Samples in one 80 ms frame at 24 kHz.
pub const HEARTBEAT_FRAME: usize = 1_920;

const _: () = assert!(HEARTBEAT_MS as usize * HEARTBEAT_HZ as usize / 1000 == HEARTBEAT_FRAME);

/// One heartbeat frame. `filled < HEARTBEAT_FRAME` is a short tail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame80 {
    pub samples: [i16; HEARTBEAT_FRAME],
    pub filled: usize,
}

/// Linear resample. Same rounding as `galion-ml-voice` `resample_linear`.
pub fn resample_linear(input: &[i16], from: u32, to: u32) -> Vec<i16> {
    if from == 0 || to == 0 || input.is_empty() {
        return Vec::new();
    }
    if from == to {
        return input.to_vec();
    }
    let out_len =
        ((input.len() as u64 * u64::from(to) + u64::from(from) / 2) / u64::from(from)) as usize;
    let last = input.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let num = i as u64 * u64::from(from);
        let idx = (num / u64::from(to)) as usize;
        let frac = (num % u64::from(to)) as f64 / f64::from(to);
        let a = f64::from(input[idx.min(last)]);
        let b = f64::from(input[(idx + 1).min(last)]);
        let v = a + (b - a) * frac;
        out.push(v.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16);
    }
    out
}

/// Resample to 24 kHz and slice into 80 ms frames.
pub fn frames_80ms(pcm: &[i16], rate: u32) -> Vec<Frame80> {
    let at = if rate == HEARTBEAT_HZ {
        pcm.to_vec()
    } else {
        resample_linear(pcm, rate, HEARTBEAT_HZ)
    };
    if at.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < at.len() {
        let n = (at.len() - i).min(HEARTBEAT_FRAME);
        let mut samples = [0i16; HEARTBEAT_FRAME];
        samples[..n].copy_from_slice(&at[i..i + n]);
        out.push(Frame80 { samples, filled: n });
        i += n;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighty_ms_is_1920_samples() {
        assert_eq!(HEARTBEAT_FRAME, 1920);
        let frames = frames_80ms(&vec![7i16; 1920], HEARTBEAT_HZ);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].filled, 1920);
        assert!(frames[0].samples.iter().all(|&s| s == 7));
        let tail = frames_80ms(&vec![1i16; 1921], HEARTBEAT_HZ);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[1].filled, 1);
        assert_eq!(tail[1].samples[0], 1);
        assert_eq!(tail[1].samples[1], 0);
    }

    #[test]
    fn one_second_of_22050_lands_on_24000() {
        let up = resample_linear(&vec![0i16; 22050], 22050, 24000);
        assert_eq!(up.len(), 24000);
        assert_eq!(resample_linear(&[1, 2, 3], 22050, 22050), vec![1, 2, 3]);
    }
}
