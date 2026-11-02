// Galion Voice — Perpetual Open MIT License
//! CAPTURE-stage mic sources (A11 wave 2).
//!
//! [`MicSource`] fills one 80 ms mono f32 frame per heartbeat. Implementations:
//!
//! * [`SynthMic`] — deterministic voice-ish tone (180 Hz + harmonic, loudness swell);
//!   the pre-wave-2 behaviour of `TensorTimingFeed`, still the default.
//! * [`PcmMic`] — plays back a decoded PCM buffer (loops), linearly resampled to the
//!   pipeline rate. [`PcmMic::from_wav`] (feature `wav-mic`) decodes a RIFF/WAVE file
//!   with the **galion-presence** loader (`galion_presence::wav`, shared code, no new
//!   parser): integer PCM 8/16/24/32-bit, mono/stereo (downmixed), 8–192 kHz.
//! * [`LiveMic`] — **stub**. The box has no audio device and the console has no audio
//!   backend dependency (no cpal/ALSA); `open()` always returns
//!   [`MicError::NoDevice`]. A real device backend implements [`MicSource`] later;
//!   nothing downstream changes.
//!
//! Dependency-free (WAV decoding is the only feature-gated part).

/// One mic frame per heartbeat.
pub trait MicSource {
    /// Fill `out` (one frame at [`MicSource::sample_rate`]) for heartbeat `tick`.
    fn fill(&mut self, tick: u64, out: &mut [f32]);
    fn sample_rate(&self) -> u32;
    fn name(&self) -> &'static str;
}

impl<M: MicSource + ?Sized> MicSource for Box<M> {
    fn fill(&mut self, tick: u64, out: &mut [f32]) {
        (**self).fill(tick, out)
    }
    fn sample_rate(&self) -> u32 {
        (**self).sample_rate()
    }
    fn name(&self) -> &'static str {
        (**self).name()
    }
}

/// Why a mic could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicError {
    /// Live capture is stubbed (no device / no audio backend on this build).
    NoDevice(&'static str),
    /// WAV/PCM fixture could not be used.
    Fixture(String),
    /// Feature not compiled in.
    Unsupported(&'static str),
}

impl std::fmt::Display for MicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MicError::NoDevice(s) | MicError::Unsupported(s) => f.write_str(s),
            MicError::Fixture(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for MicError {}

/// Deterministic synthetic mic (default CAPTURE input).
#[derive(Clone, Copy, Debug)]
pub struct SynthMic {
    pub sample_rate: u32,
}

impl Default for SynthMic {
    fn default() -> Self {
        Self { sample_rate: 24_000 }
    }
}

impl MicSource for SynthMic {
    fn fill(&mut self, tick: u64, out: &mut [f32]) {
        let n = out.len();
        let sr = self.sample_rate as f32;
        // Voice-ish synthetic mic: 180 Hz + harmonic, loudness swells over ~2 s.
        let amp = 0.04 + 0.22 * ((tick as f32 * 0.25).sin() * 0.5 + 0.5);
        for (i, x) in out.iter_mut().enumerate() {
            let t = (tick as usize * n + i) as f32 / sr;
            let w = t * 180.0 * std::f32::consts::TAU;
            *x = amp * (w.sin() + 0.3 * (2.0 * w).sin()) / 1.3;
        }
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn name(&self) -> &'static str {
        "synth"
    }
}

/// Fixture playback: a mono PCM buffer, resampled once to `target_rate`, played
/// frame by frame and looped (or silent after the end with `looped = false`).
#[derive(Clone, Debug)]
pub struct PcmMic {
    samples: Vec<f32>,
    target_rate: u32,
    /// Source rate of the fixture (for diagnostics).
    pub source_rate: u32,
    pub looped: bool,
}

impl PcmMic {
    /// `samples` mono f32 in [-1, 1] at `source_rate`; output at `target_rate`.
    pub fn new(samples: &[f32], source_rate: u32, target_rate: u32) -> Result<Self, MicError> {
        if samples.is_empty() {
            return Err(MicError::Fixture("mic fixture: no samples".into()));
        }
        if source_rate == 0 || target_rate == 0 {
            return Err(MicError::Fixture("mic fixture: sample rate 0".into()));
        }
        Ok(Self { samples: resample_linear(samples, source_rate, target_rate), target_rate, source_rate, looped: true })
    }

    /// Decode a WAV fixture with the shared galion-presence loader.
    #[cfg(feature = "wav-mic")]
    pub fn from_wav(path: &std::path::Path, target_rate: u32) -> Result<Self, MicError> {
        let pcm = galion_presence::wav::read_wav(path).map_err(MicError::Fixture)?;
        Self::new(&pcm.samples, pcm.sample_rate, target_rate)
    }

    #[cfg(not(feature = "wav-mic"))]
    pub fn from_wav(_path: &std::path::Path, _target_rate: u32) -> Result<Self, MicError> {
        Err(MicError::Unsupported("WAV mic needs the `wav-mic` cargo feature"))
    }

    /// Resampled length in samples.
    pub fn len(&self) -> usize {
        self.samples.len()
    }
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

impl MicSource for PcmMic {
    fn fill(&mut self, tick: u64, out: &mut [f32]) {
        let n = out.len();
        let len = self.samples.len();
        let start = tick as usize * n;
        for (i, x) in out.iter_mut().enumerate() {
            let k = start + i;
            *x = if self.looped { self.samples[k % len] } else { self.samples.get(k).copied().unwrap_or(0.0) };
        }
    }
    fn sample_rate(&self) -> u32 {
        self.target_rate
    }
    fn name(&self) -> &'static str {
        "wav"
    }
}

/// Live microphone — **stubbed**. See module docs.
#[derive(Debug)]
pub struct LiveMic {
    _private: (),
}

impl LiveMic {
    pub const STUB_REASON: &'static str =
        "live mic is stubbed: no audio device on this box and no audio backend compiled in; use --mic-wav PATH (fixture) or the synth mic";

    /// Always `Err(MicError::NoDevice)` until a device backend lands.
    pub fn open() -> Result<LiveMic, MicError> {
        Err(MicError::NoDevice(Self::STUB_REASON))
    }
}

/// Linear-interpolation resampler (fixtures only; not a production SRC).
pub fn resample_linear(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.len() < 2 {
        return x.to_vec();
    }
    let out_len = ((x.len() as u64 * to as u64) / from as u64).max(1) as usize;
    let step = from as f64 / to as f64;
    (0..out_len)
        .map(|i| {
            let pos = i as f64 * step;
            let j = pos as usize;
            let f = (pos - j as f64) as f32;
            let a = x[j.min(x.len() - 1)];
            let b = x[(j + 1).min(x.len() - 1)];
            a + f * (b - a)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_mic_is_stubbed() {
        let e = LiveMic::open().unwrap_err();
        assert!(matches!(e, MicError::NoDevice(_)));
        assert!(e.to_string().contains("stubbed"));
    }

    #[test]
    fn pcm_mic_loops_and_resamples() {
        let src: Vec<f32> = (0..800).map(|i| i as f32 / 800.0).collect(); // 100 ms @ 8 kHz
        let mut m = PcmMic::new(&src, 8_000, 24_000).unwrap();
        assert_eq!(m.len(), 2400);
        assert_eq!(m.sample_rate(), 24_000);
        let mut a = vec![0.0; 1920];
        m.fill(0, &mut a);
        assert!(a.windows(2).all(|w| w[1] >= w[0]), "ramp preserved by linear resample");
        let mut b = vec![0.0; 1920];
        m.fill(1, &mut b);
        assert_eq!(b[480], a[0], "frame 1 wraps around the 2400-sample loop");
        m.looped = false;
        m.fill(1, &mut b);
        assert_eq!(b[1919], 0.0, "no loop → silence past the end");
        assert!(PcmMic::new(&[], 8_000, 24_000).is_err());
    }

    #[test]
    fn synth_mic_matches_legacy_tone() {
        let mut m = SynthMic::default();
        let mut a = vec![0.0; 1920];
        m.fill(3, &mut a);
        let amp = 0.04 + 0.22 * ((3.0f32 * 0.25).sin() * 0.5 + 0.5);
        let t = (3 * 1920 + 7) as f32 / 24_000.0;
        let w = t * 180.0 * std::f32::consts::TAU;
        assert_eq!(a[7], amp * (w.sin() + 0.3 * (2.0 * w).sin()) / 1.3);
    }

    #[cfg(feature = "wav-mic")]
    #[test]
    fn wav_fixture_feeds_mic() {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/fixtures/p7-turn-mono-8k16.wav");
        let mut m = PcmMic::from_wav(&p, 24_000).expect("committed galion-presence fixture decodes");
        assert_eq!(m.source_rate, 8_000);
        let mut f = vec![0.0; 1920];
        let mut peak = 0.0f32;
        for t in 0..(m.len() / 1920) as u64 {
            m.fill(t, &mut f);
            peak = peak.max(f.iter().fold(0.0, |a, x| a.max(x.abs())));
        }
        assert!(peak > 0.01, "fixture has speech-level audio, peak {peak}");
        assert!(matches!(PcmMic::from_wav(std::path::Path::new("/nonexistent.wav"), 24_000), Err(MicError::Fixture(_))));
    }
}
