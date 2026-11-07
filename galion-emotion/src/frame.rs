//! Core affect representation: dimensional (valence/arousal/dominance) first,
//! categorical label second. Dimensional is cheaper, continuous and blends
//! naturally over time — the right primitive for a real-time band.
//!
//! A live window is one 80 ms frame of 24 kHz mono audio (1920 samples).
//! `live_frame` under `cfg(test)` names that arithmetic. It is the clock,
//! not a measured score.

/// Coarse categorical label, derived from VAD. Kept small on purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Affect {
    Neutral = 0,
    Calm = 1,
    Joy = 2,
    Excited = 3,
    Sad = 4,
    Tired = 5,
    Angry = 6,
    Anxious = 7,
}

impl Affect {
    pub fn from_u8(v: u8) -> Affect {
        match v {
            1 => Affect::Calm,
            2 => Affect::Joy,
            3 => Affect::Excited,
            4 => Affect::Sad,
            5 => Affect::Tired,
            6 => Affect::Angry,
            7 => Affect::Anxious,
            _ => Affect::Neutral,
        }
    }
}

/// One affect estimate for a window of speech.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmotionFrame {
    /// Monotonic sequence number within a stream.
    pub seq: u32,
    /// Start of the analysed window, ms since stream start.
    pub t_ms: u32,
    /// Window length in ms.
    pub dur_ms: u16,
    /// -1.0 (negative) .. +1.0 (positive)
    pub valence: f32,
    /// 0.0 (calm/low energy) .. 1.0 (aroused/high energy)
    pub arousal: f32,
    /// 0.0 (submissive) .. 1.0 (dominant)
    pub dominance: f32,
    /// 0.0 .. 1.0 — consumers MUST treat low confidence as "unknown".
    pub confidence: f32,
}

impl EmotionFrame {
    pub fn neutral(seq: u32, t_ms: u32, dur_ms: u16) -> Self {
        Self { seq, t_ms, dur_ms, valence: 0.0, arousal: 0.3, dominance: 0.5, confidence: 0.0 }
    }

    /// Clamp all dimensions into their legal ranges (NaN -> neutral value).
    pub fn clamped(mut self) -> Self {
        fn c(x: f32, lo: f32, hi: f32, d: f32) -> f32 {
            if x.is_nan() { d } else { x.clamp(lo, hi) }
        }
        self.valence = c(self.valence, -1.0, 1.0, 0.0);
        self.arousal = c(self.arousal, 0.0, 1.0, 0.3);
        self.dominance = c(self.dominance, 0.0, 1.0, 0.5);
        self.confidence = c(self.confidence, 0.0, 1.0, 0.0);
        self
    }

    /// Map VAD to a coarse label. Below 0.2 confidence -> Neutral.
    pub fn label(&self) -> Affect {
        if self.confidence < 0.2 {
            return Affect::Neutral;
        }
        let (v, a) = (self.valence, self.arousal);
        match (v, a) {
            (v, a) if v >= 0.25 && a >= 0.6 => Affect::Excited,
            (v, _) if v >= 0.25 => Affect::Joy,
            (v, a) if v <= -0.25 && a >= 0.6 && self.dominance >= 0.5 => Affect::Angry,
            (v, a) if v <= -0.25 && a >= 0.6 => Affect::Anxious,
            (v, a) if v <= -0.25 && a < 0.35 => Affect::Tired,
            (v, _) if v <= -0.25 => Affect::Sad,
            (_, a) if a < 0.25 => Affect::Calm,
            _ => Affect::Neutral,
        }
    }

    /// Exponential moving average toward `next` (alpha in 0..=1). Smooths jitter
    /// so expression never "flickers" between moods.
    pub fn ema(&self, next: &EmotionFrame, alpha: f32) -> EmotionFrame {
        let a = alpha.clamp(0.0, 1.0);
        let l = |x: f32, y: f32| x + a * (y - x);
        EmotionFrame {
            seq: next.seq,
            t_ms: next.t_ms,
            dur_ms: next.dur_ms,
            valence: l(self.valence, next.valence),
            arousal: l(self.arousal, next.arousal),
            dominance: l(self.dominance, next.dominance),
            confidence: l(self.confidence, next.confidence),
        }
        .clamped()
    }
}

/// Live heartbeat window. Compiled only for tests so the affect types stay
/// the production surface. 80 ms at 24 kHz is 1920 mono samples, 12.5 Hz.
#[cfg(test)]
pub mod live_frame {
    /// Heartbeat period. Same quantum as LOCKS `[clock] frame_ms`.
    pub const FRAME_MS: u32 = 80;
    /// You / me / feeling audio rate.
    pub const SAMPLE_RATE_HZ: u32 = 24_000;
    /// `SAMPLE_RATE_HZ * FRAME_MS / 1000`. Exact: the product divides 1000.
    pub const FRAME_SAMPLES: usize = (SAMPLE_RATE_HZ as usize * FRAME_MS as usize) / 1000;
    /// 12.5 Hz written as millihertz so the test does not use a float literal.
    pub const FRAME_MHZ: u32 = 12_500;

    /// Samples in one frame. `None` when the window does not land on a sample.
    pub fn samples_in(sample_rate_hz: u32, frame_ms: u32) -> Option<usize> {
        let ticks = u64::from(sample_rate_hz) * u64::from(frame_ms);
        if ticks % 1000 != 0 {
            None
        } else {
            Some((ticks / 1000) as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(v: f32, a: f32, d: f32, c: f32) -> EmotionFrame {
        EmotionFrame {
            seq: 0,
            t_ms: 0,
            dur_ms: live_frame::FRAME_MS as u16,
            valence: v,
            arousal: a,
            dominance: d,
            confidence: c,
        }
    }

    #[test]
    fn live_frame_is_80ms_at_24khz() {
        assert_eq!(live_frame::FRAME_MS, 80);
        assert_eq!(live_frame::SAMPLE_RATE_HZ, 24_000);
        assert_eq!(live_frame::samples_in(live_frame::SAMPLE_RATE_HZ, live_frame::FRAME_MS), Some(1920));
        assert_eq!(live_frame::FRAME_SAMPLES, 1920);
        // 12.5 Hz: 2 * 1000 ms = 25 * 80 ms, and millihertz is 12_500.
        assert_eq!(2 * 1000, 25 * live_frame::FRAME_MS);
        assert_eq!(live_frame::FRAME_MHZ, 12_500);
        assert_eq!(u64::from(live_frame::SAMPLE_RATE_HZ) * u64::from(live_frame::FRAME_MS) % 1000, 0);
        assert_eq!(live_frame::samples_in(24_001, live_frame::FRAME_MS), None);
        let n = EmotionFrame::neutral(1, live_frame::FRAME_MS, live_frame::FRAME_MS as u16);
        assert_eq!(n.dur_ms, live_frame::FRAME_MS as u16);
        assert_eq!(n.t_ms, live_frame::FRAME_MS);
        assert_eq!(n.confidence, 0.0);
        assert_eq!(n.label(), Affect::Neutral);
    }

    #[test]
    fn clamp_handles_out_of_range_and_nan() {
        let x = f(5.0, -1.0, f32::NAN, 2.0).clamped();
        assert_eq!(x.valence, 1.0);
        assert_eq!(x.arousal, 0.0);
        assert_eq!(x.dominance, 0.5);
        assert_eq!(x.confidence, 1.0);
    }

    #[test]
    fn labels() {
        assert_eq!(f(0.8, 0.9, 0.5, 0.9).label(), Affect::Excited);
        assert_eq!(f(0.5, 0.3, 0.5, 0.9).label(), Affect::Joy);
        assert_eq!(f(-0.7, 0.9, 0.8, 0.9).label(), Affect::Angry);
        assert_eq!(f(-0.7, 0.9, 0.2, 0.9).label(), Affect::Anxious);
        assert_eq!(f(-0.5, 0.1, 0.5, 0.9).label(), Affect::Tired);
        assert_eq!(f(-0.5, 0.5, 0.5, 0.9).label(), Affect::Sad);
        assert_eq!(f(0.0, 0.1, 0.5, 0.9).label(), Affect::Calm);
        assert_eq!(f(-0.9, 0.9, 0.9, 0.1).label(), Affect::Neutral, "low confidence = unknown");
    }

    #[test]
    fn ema_moves_partially() {
        let a = f(0.0, 0.0, 0.5, 0.5);
        let b = f(1.0, 1.0, 0.5, 0.5);
        let m = a.ema(&b, 0.25);
        assert!((m.valence - 0.25).abs() < 1e-6);
        assert!((m.arousal - 0.25).abs() < 1e-6);
    }

    #[test]
    fn affect_u8_roundtrip() {
        for v in 0u8..8 {
            assert_eq!(Affect::from_u8(v) as u8, v);
        }
        assert_eq!(Affect::from_u8(200), Affect::Neutral);
    }
}
