// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Frame sizes taken from the crates this sector does not reimplement.
//!
//! * [`FRAME_MS`] is `galion_heartbeat::FRAME_MS` (LOCKS `[clock] frame_ms = 80`).
//! * [`MIC_RATE`] and [`MIC_FRAME_SAMPLES`] are `galion_asr_vosk` (16 kHz, 20 ms).
//! * [`TTS_RATE`] is `galion_v2_live::piper::PIPER_HZ` (Piper raw s16le).

/// Heartbeat period. Mood ticks and TTS slices use this.
pub const FRAME_MS: u32 = 80;
/// Browser mic and the Vosk worker rate.
pub const MIC_RATE: u32 = 16_000;
/// One Vosk frame: 20 ms at 16 kHz.
pub const MIC_FRAME_SAMPLES: usize = 320;
/// Piper raw output rate (`PIPER_HZ`).
pub const TTS_RATE: u32 = 22_050;
/// One 80 ms slice at [`TTS_RATE`]. 22050 * 80 / 1000 = 1764.
pub const TTS_FRAME_SAMPLES: usize = 1_764;
/// How many 80 ms slices the unwired mouth emits so the socket can pace them.
pub const HOLD_SLICES: usize = 4;

const _: () = assert!(MIC_RATE * 20 / 1000 == MIC_FRAME_SAMPLES as u32);
const _: () = assert!(TTS_RATE * FRAME_MS / 1000 == TTS_FRAME_SAMPLES as u32);

/// Samples in one `frame_ms` slice at `rate`. Truncates, matching the integer
/// sizes above (both pairs divide evenly).
pub fn samples_per_frame(rate: u32, frame_ms: u32) -> usize {
    (rate as usize).saturating_mul(frame_ms as usize) / 1000
}

/// Split `pcm` into consecutive slices of `frame_ms` at `rate`.
/// A short tail is kept so the last flag can still mark the end.
pub fn slice_pcm(pcm: &[i16], rate: u32, frame_ms: u32) -> Vec<Vec<i16>> {
    let n = samples_per_frame(rate, frame_ms);
    if n == 0 || pcm.is_empty() {
        return Vec::new();
    }
    pcm.chunks(n).map(|c| c.to_vec()).collect()
}

/// RMS of s16 samples, scaled to 0..=1 against full scale.
pub fn rms(pcm: &[i16]) -> f32 {
    if pcm.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for s in pcm {
        let x = f64::from(*s);
        acc += x * x;
    }
    (acc / pcm.len() as f64).sqrt() as f32 / 32768.0
}

/// A steady tone used by the unwired mouth. It is not a voice.
/// `hz` and `amp` (0..=1) come from the live pad so the level follows mood.
/// A 2 ms fade at each end keeps the splice from clicking.
pub fn tone(n: usize, rate: u32, hz: f32, amp: f32) -> Vec<i16> {
    if n == 0 || rate == 0 {
        return Vec::new();
    }
    let amp = if amp.is_finite() {
        amp.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let hz = if hz.is_finite() {
        hz.clamp(40.0, 2000.0)
    } else {
        220.0
    };
    let fade_n = ((rate as usize) * 2 / 1000).max(1);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let env = edge_fade(i, n, fade_n);
        let t = i as f32 / rate as f32;
        let s = (t * hz * std::f32::consts::TAU).sin() * amp * env;
        let sample = (s * 32767.0).round();
        let sample = sample.clamp(-32767.0, 32767.0);
        out.push(sample as i16);
    }
    out
}

fn edge_fade(i: usize, n: usize, fade_n: usize) -> f32 {
    let fade_n = fade_n.min(n / 2).max(1);
    if i < fade_n {
        i as f32 / fade_n as f32
    } else if i + fade_n >= n {
        (n - 1 - i) as f32 / fade_n as f32
    } else {
        1.0
    }
}

/// Four 80 ms slices. Pitch follows valence. Level stays high enough to hear.
/// This is the hold mouth: Piper is not in this process.
pub fn hold_tone(valence: f32) -> Vec<i16> {
    let hz = 196.0 + valence.clamp(0.0, 1.0) * 80.0;
    tone(TTS_FRAME_SAMPLES * HOLD_SLICES, TTS_RATE, hz, 0.45)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_matches_piper_frame() {
        let pcm = vec![1i16; TTS_FRAME_SAMPLES * HOLD_SLICES];
        let slices = slice_pcm(&pcm, TTS_RATE, FRAME_MS);
        assert_eq!(slices.len(), 4);
        assert!(slices.iter().all(|s| s.len() == TTS_FRAME_SAMPLES));
    }

    #[test]
    fn short_tail_is_kept() {
        let slices = slice_pcm(&[1, 2, 3], 16_000, 80);
        assert_eq!(slices.len(), 1);
        assert_eq!(slices[0], vec![1, 2, 3]);
    }

    #[test]
    fn tone_is_audible_in_the_middle() {
        let pcm = hold_tone(0.25);
        assert_eq!(pcm.len(), TTS_FRAME_SAMPLES * HOLD_SLICES);
        let mid = pcm[pcm.len() / 2].unsigned_abs();
        assert!(mid > 1000, "mid sample {mid} is silent");
        assert_eq!(pcm[0], 0, "fade starts at zero");
    }

    #[test]
    fn rms_of_constant_frame() {
        let frame = vec![8000i16; MIC_FRAME_SAMPLES];
        let r = rms(&frame);
        assert!(r > 0.2 && r < 0.3, "{r}");
        assert_eq!(rms(&[]), 0.0);
    }
}
