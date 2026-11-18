// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Corruption checks for one rendered clip, or one 80 ms frame stream.
//!
//! Call [`admit`] between sound out and listening-back. A failed report is
//! the rejection: the samples must not be played and must not be passed on.
//!
//! Reused, not rewritten:
//! - `ebur128` 0.1.11 (MIT) for ITU-R BS.1770 true peak
//! - the galion-hygiene sample-jump click
//! - `galion-articulation` TractPrint v2 and praatfan HNR (Boersma 1993)
//! - a caller-supplied ECAPA cosine from `galion-ecapa` (this crate does not run it)
//!
//! DNSMOS, UTMOS, NISQA, Silero, and webrtc VAD are not linked. Silence uses
//! the energy mask already in `galion-articulation` select. Neural scores
//! stay behind the `neural` feature and do not download weights.

#![forbid(unsafe_code)]

mod checks;
mod config;
mod neural;
mod report;
mod speaker;
mod wav;

pub use config::{load as load_config, Config};
pub use report::{Check, FrameSpan, Report, SCHEMA};
pub use speaker::Baseline;
pub use wav::{read_f32_le, read_wav, Wav};

/// Samples the guard should judge. Interleaved if `channels` > 1.
pub struct Pcm<'a> {
    pub interleaved: &'a [f32],
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub bits_per_sample: Option<u16>,
}

/// What the caller already knows. The baseline and the cosine are optional.
/// When both are absent, speaker drift is a stub and does not open or close the gate.
pub struct Context<'a> {
    pub baseline: Option<Baseline<'a>>,
    /// Cosine from galion-ecapa, or any ECAPA-TDNN embedder the caller already ran.
    pub embedding_cos: Option<f64>,
    /// A finished utterance should not end in a hard mute. A live 80 ms stream
    /// that has not ended yet sets this false.
    pub expect_release: bool,
    /// Path passed to an external neural scorer. Ignored unless feature `neural` is on.
    pub wav_path: Option<&'a std::path::Path>,
}

impl Context<'_> {
    pub fn clip() -> Self {
        Self {
            baseline: None,
            embedding_cos: None,
            expect_release: true,
            wav_path: None,
        }
    }
}

/// Judge one clip. `verdict` is `fail` when any required check fails.
pub fn check(pcm: Pcm<'_>, cfg: &Config, ctx: &Context<'_>) -> Result<Report, String> {
    let prepared = checks::prepare(
        pcm.interleaved,
        pcm.sample_rate_hz,
        pcm.channels,
        pcm.bits_per_sample,
        cfg.tick_ms,
    )?;
    let rows = vec![
        checks::format(&prepared, cfg),
        checks::finite(&prepared),
        checks::denormal(&prepared),
        checks::dc(&prepared, cfg.dc.max_dbfs),
        checks::clipping(&prepared, cfg.clip.linear),
        checks::true_peak(&prepared, cfg.clip.max_true_peak_dbtp),
        checks::click(&prepared, cfg.click.jump),
        checks::ar_click(&prepared, cfg.click.ar_order, cfg.click.ar_residual),
        checks::dropout(&prepared, cfg),
        checks::repeated_frame(&prepared, cfg),
        checks::babble(&prepared, cfg),
        speaker::buzz(&prepared, cfg.buzz.min_hnr_db),
        checks::cutoff(&prepared, cfg, ctx.expect_release),
        speaker::drift(&prepared, ctx.baseline.as_ref(), ctx.embedding_cos, cfg),
        neural::quality(&cfg.neural, ctx.wav_path),
    ];
    Ok(Report::from_checks(
        rows,
        prepared.sample_rate_hz,
        prepared.channels,
        cfg.tick_ms,
        prepared.n_frames,
        prepared.raw.len(),
    ))
}

/// Fail closed. `Err` is the report, and the clip must not go to listening-back.
pub fn admit(pcm: Pcm<'_>, cfg: &Config, ctx: &Context<'_>) -> Result<Report, Box<Report>> {
    let sample_rate_hz = pcm.sample_rate_hz;
    let channels = pcm.channels;
    let samples = pcm.interleaved.len();
    let tick_ms = cfg.tick_ms;
    match check(pcm, cfg, ctx) {
        Ok(report) if !report.bad => Ok(report),
        Ok(report) => Err(Box::new(report)),
        Err(e) => Err(Box::new(Report::from_checks(
            vec![Check {
                name: "format",
                status: "fail",
                score: Some(0.0),
                value: None,
                limit: None,
                unit: "",
                reason: e,
                frames: Vec::new(),
                method: "admit refuses a clip it cannot read",
            }],
            sample_rate_hz,
            channels,
            tick_ms,
            0,
            samples,
        ))),
    }
}

/// One 80 ms frame, mono.
pub fn check_frames(
    frames: &[&[f32]],
    sample_rate_hz: u32,
    cfg: &Config,
    ctx: &Context<'_>,
) -> Result<Report, String> {
    let fl = checks::frame_len(sample_rate_hz, cfg.tick_ms).unwrap_or(0);
    let mut mono = Vec::new();
    let mut short = false;
    for frame in frames {
        if fl == 0 || frame.len() != fl {
            short = true;
        }
        mono.extend_from_slice(frame);
    }
    let mut report = check(
        Pcm {
            interleaved: &mono,
            sample_rate_hz,
            channels: 1,
            bits_per_sample: None,
        },
        cfg,
        ctx,
    )?;
    if short {
        if let Some(row) = report.checks.iter_mut().find(|c| c.name == "format") {
            row.status = "fail";
            row.score = Some(0.0);
            row.reason = format!(
                "a frame is not {fl} samples ({} ms at {sample_rate_hz} Hz)",
                cfg.tick_ms
            );
        }
        report.bad = true;
        report.verdict = "fail";
        report.reason = report
            .checks
            .iter()
            .filter(|c| c.status == "fail")
            .map(|c| format!("{}: {}", c.name, c.reason))
            .collect::<Vec<_>>()
            .join("; ");
    }
    Ok(report)
}

/// Hook for `galion-mne`: sound-out PCM in, a report out.
/// `Err` means do not call listening-back.
pub fn before_listen(
    samples: &[f32],
    sample_rate_hz: u32,
    cfg: &Config,
    baseline: Option<Baseline<'_>>,
    embedding_cos: Option<f64>,
) -> Result<Report, Box<Report>> {
    admit(
        Pcm {
            interleaved: samples,
            sample_rate_hz,
            channels: 1,
            bits_per_sample: None,
        },
        cfg,
        &Context {
            baseline,
            embedding_cos,
            expect_release: true,
            wav_path: None,
        },
    )
}
