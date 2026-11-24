// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! S02 — voice activity and end of turn for Samantha (`galion-sam-vad`).
//!
//! Mic audio arrives as 20 ms frames at 16 kHz (320 samples). The frame size
//! was copied from the archived lane
//! `archive/2026-11-24/crates/galion-v2-live/src/bin/call.rs` (24 kHz resampled
//! to 16 kHz before `Vad::push`). The process that speaks today is
//! `samantha-local`, which does not link that crate. Four of those frames are
//! one 80 ms heartbeat (`crates/galion-heartbeat` `FRAME_MS`).
//!
//! The score comes from Silero VAD v5 on the CPU (ONNX Runtime, no GPU
//! provider). If the model file is missing or will not load, the score comes
//! from the energy gate in `archive/2026-11-24/crates/galion-v2-live/src/vad.rs`. The turn rules
//! (onset, hangover, speculate / commit / hold, barge-in) sit on top of either
//! score. See [`turn`].
//!
//! This crate does not synthesize speech, does not open `GALION_SAMANTHA_REF`,
//! and does not emit a line of dialogue. Emotion stays on the live mood path.
//! The one-dot can read [`FrameReport::level`] later. Nothing here serves HTTP
//! or installs a systemd unit.
//!
//! Set `OMP_NUM_THREADS=1` and `OMP_WAIT_POLICY=PASSIVE` in the process
//! environment before the first Silero load. The prebuilt ONNX Runtime uses
//! OpenMP and ignores the intra-op thread count.

#![deny(unsafe_code)]

mod audio;
mod clock;
mod energy;
mod silero;
mod turn;

pub use audio::{parse_wav, read_wav, resample, rms_db, to_mono, Wav};
pub use turn::{sounds_unfinished, Event, Turn, TurnCfg, TurnOut};

use std::path::{Path, PathBuf};

use energy::Energy;
use silero::Silero;

/// 20 ms ear. Matches the live call's VAD frame.
pub const FRAME_MS: u32 = 20;
/// Silero's 16 kHz rate, and the rate `call.rs` resamples to before the VAD.
pub const SAMPLE_RATE: u32 = 16_000;
/// Samples in one [`FRAME_MS`] frame at [`SAMPLE_RATE`].
pub const FRAME_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_MS as usize) / 1000;
/// The heartbeat period. Locked in `crates/galion-heartbeat` (`FRAME_MS = 80`).
pub const HEARTBEAT_MS: u32 = 80;
/// How many VAD frames fill one heartbeat.
pub const FRAMES_PER_HEARTBEAT: u32 = HEARTBEAT_MS / FRAME_MS;

const _: () = assert!(FRAME_SAMPLES == 320);
const _: () = assert!(FRAMES_PER_HEARTBEAT == 4);

#[derive(Debug)]
pub struct VadError(String);

impl VadError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl std::fmt::Display for VadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for VadError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Energy,
    Silero,
}

impl Backend {
    pub fn name(self) -> &'static str {
        match self {
            Backend::Energy => "energy",
            Backend::Silero => "silero",
        }
    }
}

/// One 20 ms frame after the turn rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameReport {
    pub frame: u64,
    pub t_ms: u32,
    pub heartbeat: u64,
    pub prob: f32,
    pub rms_db: f32,
    pub speaking: bool,
    pub backend: Backend,
    pub event: Event,
    pub barge_in: bool,
    pub overlap_ms: u32,
    pub silent_ms: u32,
    pub voiced_ms: u32,
    /// Thread CPU time for this `push`, including a Silero window when one ran.
    pub cpu_ns: u64,
    /// Wall time for this `push`.
    pub wall_ns: u64,
    /// Thread CPU time inside the ONNX call. 0 when this frame did not run the model.
    pub model_cpu_ns: u64,
    pub ran_model: bool,
}

impl FrameReport {
    /// Speech probability clamped to 0..1. This is the level the one-dot paints.
    pub fn level(self) -> f32 {
        self.prob.clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimeStats {
    pub n: usize,
    pub min_ns: u64,
    pub p50_ns: u64,
    pub p95_ns: u64,
    pub max_ns: u64,
    pub mean_ns: u64,
}

pub fn summarize_ns(samples: &[u64]) -> TimeStats {
    if samples.is_empty() {
        return TimeStats {
            n: 0,
            min_ns: 0,
            p50_ns: 0,
            p95_ns: 0,
            max_ns: 0,
            mean_ns: 0,
        };
    }
    let mut v = samples.to_vec();
    v.sort_unstable();
    let pct = |p: f64| {
        let i = ((v.len() - 1) as f64 * p).round() as usize;
        v[i.min(v.len() - 1)]
    };
    let sum: u128 = v.iter().map(|&n| n as u128).sum();
    TimeStats {
        n: v.len(),
        min_ns: v[0],
        p50_ns: pct(0.50),
        p95_ns: pct(0.95),
        max_ns: *v.last().unwrap(),
        mean_ns: (sum / v.len() as u128) as u64,
    }
}

enum Kind {
    Energy(Energy),
    Silero(Silero),
}

/// One listener: a score backend plus the turn rules.
pub struct Listen {
    kind: Kind,
    turn: Turn,
    backend: Backend,
    fallback: Option<String>,
    warm_cpu_ns: u64,
    model_io: Option<String>,
}

impl Listen {
    /// Energy gate only. No ONNX Runtime.
    pub fn energy(cfg: TurnCfg) -> Self {
        Self::energy_with_reason(cfg, None)
    }

    fn energy_with_reason(cfg: TurnCfg, reason: Option<String>) -> Self {
        Self {
            kind: Kind::Energy(Energy::default()),
            turn: Turn::new(cfg),
            backend: Backend::Energy,
            fallback: reason,
            warm_cpu_ns: 0,
            model_io: None,
        }
    }

    /// Load Silero. Does not fall back: a bad model is an error, so a bench
    /// cannot quietly report energy numbers as Silero numbers.
    pub fn silero(path: &Path, cfg: TurnCfg) -> Result<Self, VadError> {
        let model = Silero::load(path)?;
        let warm = model.warm_cpu_ns();
        let io = model.io().to_string();
        Ok(Self {
            kind: Kind::Silero(model),
            turn: Turn::new(cfg),
            backend: Backend::Silero,
            fallback: None,
            warm_cpu_ns: warm,
            model_io: Some(io),
        })
    }

    /// Silero when `model` exists and loads. Otherwise the energy gate, with
    /// the reason in [`Self::fallback_reason`].
    pub fn open(model: Option<&Path>, cfg: TurnCfg) -> Self {
        let Some(path) = model else {
            return Self::energy_with_reason(cfg, Some("no silero model path".into()));
        };
        if !path.is_file() {
            return Self::energy_with_reason(
                cfg,
                Some(format!("silero model not found: {}", path.display())),
            );
        }
        match Self::silero(path, cfg) {
            Ok(listener) => listener,
            Err(e) => Self::energy_with_reason(cfg, Some(e.to_string())),
        }
    }

    /// `GALION_SILERO_VAD` if set, otherwise `models/silero_vad.onnx` next to
    /// this crate when that file has been downloaded. Energy if neither loads.
    pub fn from_env() -> Self {
        let cfg = TurnCfg::from_env();
        if let Some(p) = std::env::var_os("GALION_SILERO_VAD") {
            if !p.is_empty() {
                return Self::open(Some(Path::new(&p)), cfg);
            }
        }
        let bundled = bundled_model_path();
        if bundled.is_file() {
            Self::open(Some(&bundled), cfg)
        } else {
            Self::energy_with_reason(
                cfg,
                Some(
                    "GALION_SILERO_VAD is unset and models/silero_vad.onnx is not downloaded"
                        .into(),
                ),
            )
        }
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    pub fn warm_cpu_ns(&self) -> u64 {
        self.warm_cpu_ns
    }

    pub fn model_io(&self) -> Option<&str> {
        self.model_io.as_deref()
    }

    pub fn cfg(&self) -> TurnCfg {
        self.turn.cfg()
    }

    pub fn speaking(&self) -> bool {
        self.turn.speaking()
    }

    pub fn reset(&mut self) {
        self.turn.reset();
        match &mut self.kind {
            Kind::Energy(e) => *e = Energy::default(),
            Kind::Silero(s) => s.reset_state(),
        }
    }

    /// One 20 ms, 16 kHz, mono s16 frame.
    pub fn push(
        &mut self,
        frame: &[i16],
        me_speaking: bool,
        transcript: &str,
    ) -> Result<FrameReport, VadError> {
        if frame.len() != FRAME_SAMPLES {
            return Err(VadError::new(format!(
                "frame is {} samples, want {FRAME_SAMPLES} (20 ms at {SAMPLE_RATE} Hz)",
                frame.len()
            )));
        }
        let wall0 = std::time::Instant::now();
        let cpu0 = clock::thread_cpu_ns();
        let rms_db = audio::rms_db(frame) as f32;
        let score = match &mut self.kind {
            Kind::Energy(e) => silero::FrameScore {
                prob: e.push_i16(frame),
                model_cpu_ns: 0,
                ran_model: false,
            },
            Kind::Silero(s) => s.push_i16(frame)?,
        };
        let out = self.turn.push(score.prob, me_speaking, transcript);
        Ok(FrameReport {
            frame: out.frame,
            t_ms: out.t_ms,
            heartbeat: out.heartbeat,
            prob: score.prob,
            rms_db,
            speaking: out.speaking,
            backend: self.backend,
            event: out.event,
            barge_in: out.barge_in,
            overlap_ms: out.overlap_ms,
            silent_ms: out.silent_ms,
            voiced_ms: out.voiced_ms,
            cpu_ns: clock::since_ns(cpu0),
            wall_ns: wall0.elapsed().as_nanos() as u64,
            model_cpu_ns: score.model_cpu_ns,
            ran_model: score.ran_model,
        })
    }

    /// Resample to 16 kHz if needed, then cut 20 ms frames. The tail is padded
    /// with zeros, which is silence, so the last partial frame can close.
    pub fn push_pcm(
        &mut self,
        samples: &[i16],
        rate: u32,
        me_speaking: bool,
        transcript: &str,
    ) -> Result<Vec<FrameReport>, VadError> {
        let pcm = if rate == SAMPLE_RATE {
            samples.to_vec()
        } else {
            audio::resample(samples, rate, SAMPLE_RATE)
        };
        let mut out = Vec::with_capacity(pcm.len() / FRAME_SAMPLES + 1);
        if pcm.is_empty() {
            return Ok(out);
        }
        let mut i = 0;
        while i < pcm.len() {
            let mut frame = [0i16; FRAME_SAMPLES];
            let n = (pcm.len() - i).min(FRAME_SAMPLES);
            frame[..n].copy_from_slice(&pcm[i..i + n]);
            out.push(self.push(&frame, me_speaking, transcript)?);
            i += FRAME_SAMPLES;
        }
        Ok(out)
    }
}

/// `models/silero_vad.onnx` beside this crate. The fetch script writes it there.
/// It is gitignored.
pub fn bundled_model_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("models")
        .join("silero_vad.onnx")
}

/// `samples/` beside this crate, or `GALION_VAD_SAMPLES` when that directory exists.
pub fn samples_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GALION_VAD_SAMPLES") {
        let p = PathBuf::from(p);
        if p.is_dir() {
            return Some(p);
        }
    }
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples");
    if p.is_dir() {
        Some(p)
    } else {
        None
    }
}

/// 20 ms at 24 kHz (480 samples) down to the 16 kHz ear. This is the conversion
/// `call.rs` does before `Vad::push`.
pub fn frame_from_24k(frame: &[i16]) -> Result<Vec<i16>, VadError> {
    if frame.len() != 480 {
        return Err(VadError::new(format!(
            "24 kHz frame is {} samples, want 480 (20 ms)",
            frame.len()
        )));
    }
    Ok(resample(frame, 24_000, SAMPLE_RATE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| ((i as f64 * 0.35).sin() * amp as f64) as i16)
            .collect()
    }

    #[test]
    fn rejects_a_short_frame() {
        let mut ear = Listen::energy(TurnCfg::default());
        let err = ear.push(&[0i16; 10], false, "").unwrap_err();
        assert!(err.to_string().contains("320"));
    }

    #[test]
    fn missing_model_falls_back_to_energy_without_loading_ort() {
        let ear = Listen::open(Some(Path::new("no-such-silero.onnx")), TurnCfg::default());
        assert_eq!(ear.backend(), Backend::Energy);
        assert!(ear.fallback_reason().unwrap().contains("not found"));
    }

    #[test]
    fn energy_start_and_hangover_on_a_tone() {
        let mut ear = Listen::energy(TurnCfg::default());
        for _ in 0..2 {
            let r = ear.push(&tone(FRAME_SAMPLES, 8000), false, "").unwrap();
            assert_ne!(r.event, Event::SpeechStart);
            assert!(r.cpu_ns < 20_000_000, "energy frame took {} ns", r.cpu_ns);
        }
        let r = ear.push(&tone(FRAME_SAMPLES, 8000), false, "").unwrap();
        assert_eq!(r.event, Event::SpeechStart);
        for _ in 0..24 {
            let r = ear.push(&[0i16; FRAME_SAMPLES], false, "").unwrap();
            assert_ne!(r.event, Event::SpeechEnd);
        }
        let end = ear.push(&[0i16; FRAME_SAMPLES], false, "").unwrap();
        assert_eq!(end.event, Event::SpeechEnd);
    }

    #[test]
    fn energy_barge_in_while_she_is_speaking() {
        let mut ear = Listen::energy(TurnCfg::default());
        let mut barge = None;
        for i in 0..6 {
            let r = ear.push(&tone(FRAME_SAMPLES, 8000), true, "").unwrap();
            if r.barge_in {
                barge = Some(i);
            }
        }
        assert_eq!(barge, Some(3));
    }

    #[test]
    fn twenty_four_khz_frame_is_320_samples() {
        let y = frame_from_24k(&[1000i16; 480]).unwrap();
        assert_eq!(y.len(), FRAME_SAMPLES);
    }

    #[test]
    fn energy_frames_stay_inside_the_20_ms_budget() {
        let mut ear = Listen::energy(TurnCfg::default());
        let mut cpu = Vec::new();
        for i in 0..200 {
            let frame = if i < 40 {
                tone(FRAME_SAMPLES, 8000)
            } else {
                vec![0i16; FRAME_SAMPLES]
            };
            let me = i > 10 && i < 30;
            let r = ear.push(&frame, me, "").unwrap();
            cpu.push(r.cpu_ns);
        }
        let s = summarize_ns(&cpu);
        eprintln!(
            "energy debug frames n={} cpu_us min={} p50={} p95={} max={}",
            s.n,
            s.min_ns / 1000,
            s.p50_ns / 1000,
            s.p95_ns / 1000,
            s.max_ns / 1000
        );
        assert!(
            s.max_ns < 20_000_000,
            "energy max frame cpu {} ns exceeds 20 ms",
            s.max_ns
        );
        assert!(s.max_ns < 80_000_000);
    }

    #[test]
    fn cc0_speech_is_detected_when_the_script_has_been_run() {
        let Some(dir) = samples_dir() else {
            eprintln!("no samples dir; skip CC0 (run scripts/fetch-cc0.sh)");
            return;
        };
        let mut wavs: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("wav"))
            .collect();
        wavs.sort();
        if wavs.is_empty() {
            eprintln!("samples dir empty; skip CC0");
            return;
        }
        eprintln!("cc0 files: {}", wavs.len());
        let model = bundled_model_path();
        let have_silero = model.is_file();
        for path in &wavs {
            let wav = read_wav(path).unwrap();
            let mono = to_mono(&wav);
            let name = path.file_name().unwrap().to_string_lossy();
            let mut energy = Listen::energy(TurnCfg::default());
            let ev = energy.push_pcm(&mono, wav.rate, false, "").unwrap();
            assert!(
                ev.iter().any(|r| r.event == Event::SpeechStart),
                "{name}: energy did not hear speech (peak rms {:.1} dB)",
                mono.chunks(FRAME_SAMPLES)
                    .map(rms_db)
                    .fold(-120.0, f64::max)
            );
            if have_silero {
                let mut sil = Listen::silero(&model, TurnCfg::default()).unwrap();
                let ev = sil.push_pcm(&mono, wav.rate, false, "").unwrap();
                assert!(
                    ev.iter().any(|r| r.event == Event::SpeechStart),
                    "{name}: silero did not hear speech"
                );
                let mut cpu = Vec::new();
                let mut model_cpu = Vec::new();
                // A second pass, state reset, so the assertion is on streaming frames
                // after the load warmup inside `silero()`.
                sil.reset();
                let pcm = if wav.rate == SAMPLE_RATE {
                    mono
                } else {
                    resample(&mono, wav.rate, SAMPLE_RATE)
                };
                let mut i = 0;
                while i < pcm.len() {
                    let mut frame = [0i16; FRAME_SAMPLES];
                    let n = (pcm.len() - i).min(FRAME_SAMPLES);
                    frame[..n].copy_from_slice(&pcm[i..i + n]);
                    let r = sil.push(&frame, false, "").unwrap();
                    cpu.push(r.cpu_ns);
                    if r.ran_model {
                        model_cpu.push(r.model_cpu_ns);
                    }
                    i += FRAME_SAMPLES;
                }
                let s = summarize_ns(&cpu);
                let m = summarize_ns(&model_cpu);
                eprintln!(
                    "{name}: silero frames {} cpu_us p50={} p95={} max={}  windows {} model_cpu_us p50={} p95={} max={}",
                    s.n,
                    s.p50_ns / 1000,
                    s.p95_ns / 1000,
                    s.max_ns / 1000,
                    m.n,
                    m.p50_ns / 1000,
                    m.p95_ns / 1000,
                    m.max_ns / 1000
                );
                assert!(
                    s.p95_ns < 20_000_000,
                    "{name}: p95 frame cpu {} ns",
                    s.p95_ns
                );
                assert!(
                    m.p95_ns < 20_000_000,
                    "{name}: p95 silero window {} ns",
                    m.p95_ns
                );
            }
        }
    }
}
