// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Take renderers.
//!
//! `synthetic` writes a 432 Hz tone so the stitch, bed, and master can be
//! tested with no model. `command` runs an argv template. The default cbx
//! template calls the existing Chatterbox CLI with the owner's reference wav.

use crate::audio::read_wav_mono;
use crate::command::{display_path, fill_argv, run_cmd, tail, ToolError};
use crate::device::DeviceKind;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AudioKind {
    #[serde(rename = "synthetic-tone")]
    SyntheticTone,
    #[serde(rename = "runtime")]
    Runtime,
}

pub struct RenderedTake {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub device_used: String,
    pub note: String,
    pub kind: AudioKind,
}

pub struct RenderJob<'a> {
    pub line_index: usize,
    pub take_index: usize,
    pub text: &'a str,
    pub emotion: &'a str,
    pub reference: &'a Path,
    pub out_wav: &'a Path,
    pub seed: u64,
    pub temperature: f64,
    pub device_ep: &'a str,
    pub device_kind: DeviceKind,
    pub sample_rate: u32,
}

pub trait TakeRenderer {
    fn name(&self) -> &str;
    fn render(&mut self, job: &RenderJob<'_>) -> Result<RenderedTake, ToolError>;
}

pub struct SyntheticRenderer;

impl TakeRenderer for SyntheticRenderer {
    fn name(&self) -> &str {
        "synthetic"
    }

    fn render(&mut self, job: &RenderJob<'_>) -> Result<RenderedTake, ToolError> {
        let words = job.text.split_whitespace().count().max(1);
        let seconds = 0.70 + 0.08 * words as f64;
        let n = ((f64::from(job.sample_rate) * seconds).round() as usize).max(1);
        let sr = f64::from(job.sample_rate);
        let samples = (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                (0.12 * (2.0 * std::f64::consts::PI * 432.0 * t).sin()) as f32
            })
            .collect();
        Ok(RenderedTake {
            samples,
            sample_rate: job.sample_rate,
            device_used: job.device_ep.to_string(),
            note: "Synthetic 432 Hz tone. This is a test signal.".into(),
            kind: AudioKind::SyntheticTone,
        })
    }
}

pub struct CommandRenderer {
    pub template: Vec<String>,
    pub retry_cpu: bool,
}

impl TakeRenderer for CommandRenderer {
    fn name(&self) -> &str {
        "command"
    }

    fn render(&mut self, job: &RenderJob<'_>) -> Result<RenderedTake, ToolError> {
        match self.once(job, job.device_ep) {
            Ok(take) => Ok(take),
            Err(ToolError::Fatal(err)) => Err(ToolError::Fatal(err)),
            Err(ToolError::Take(err)) if self.retry_cpu && job.device_kind == DeviceKind::Cuda => {
                let mut second = self.once(job, "cpu")?;
                second.device_used = "cpu".into();
                second.note = format!(
                    "cuda failed ({}); retried on cpu. {}",
                    tail(&err, 180),
                    second.note
                );
                Ok(second)
            }
            Err(err) => Err(err),
        }
    }
}

impl CommandRenderer {
    fn once(&self, job: &RenderJob<'_>, device: &str) -> Result<RenderedTake, ToolError> {
        if let Some(parent) = job.out_wav.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| ToolError::Fatal(format!("{}: {err}", parent.display())))?;
        }
        let vars = [
            ("text", job.text.to_string()),
            ("emotion", job.emotion.to_string()),
            ("reference", display_path(job.reference)),
            ("out", display_path(job.out_wav)),
            ("device", device.to_string()),
            ("seed", job.seed.to_string()),
            ("temperature", format!("{:.4}", job.temperature)),
            ("take", job.take_index.to_string()),
            ("line", job.line_index.to_string()),
        ];
        let vars_ref: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let argv = fill_argv(&self.template, &vars_ref)?;
        let output = run_cmd(&argv)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Take(format!(
                "{} exited {}: {}",
                argv.first().map(String::as_str).unwrap_or("command"),
                output.status,
                tail(&stderr, 400)
            )));
        }
        let (samples, sample_rate) = read_wav_mono(job.out_wav).map_err(ToolError::Take)?;
        if samples.is_empty() {
            return Err(ToolError::Take("renderer wrote an empty wav".into()));
        }
        Ok(RenderedTake {
            samples,
            sample_rate,
            device_used: device.to_string(),
            note: format!(
                "runtime {}",
                argv.first().map(String::as_str).unwrap_or("command")
            ),
            kind: AudioKind::Runtime,
        })
    }
}
