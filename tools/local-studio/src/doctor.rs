// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Report which local runtimes are visible. Nothing is downloaded and no
//! reference clip is opened.

use crate::command::discover;
use crate::config::{Backend, StudioConfig};
use crate::device::{choose, DeviceRequest};
use crate::{
    BED_HZ, BED_UNDER_DB, ROLLOUT_ECAPA_MIN, ROLLOUT_EMOTION_MIN, ROLLOUT_WER_MAX, TARGET_LUFS,
    TRUE_PEAK_DBTP,
};
use serde_json::{json, Value};
use std::path::PathBuf;

pub fn report(config: Option<&StudioConfig>, nvidia: bool) -> Value {
    let tools = discover();
    let request = config
        .map(|cfg| cfg.render.device)
        .unwrap_or(DeviceRequest::Auto);
    let device = match choose(request, nvidia) {
        Ok(choice) => json!(choice),
        Err(message) => json!({ "error": message, "nvidia": nvidia }),
    };
    let backend = config
        .map(|cfg| match cfg.render.backend {
            Backend::Cbx => "cbx",
            Backend::Command => "command",
            Backend::Synthetic => "synthetic",
        })
        .unwrap_or("cbx");
    let whisper = command_or_bin(
        config.map(|cfg| cfg.whisper.command.as_slice()),
        &tools.whisper,
    );
    let ecapa = command_or_bin(config.map(|cfg| cfg.ecapa.command.as_slice()), &tools.ecapa);
    let emotion = command_or_bin(
        config.map(|cfg| cfg.emotion.command.as_slice()),
        &tools.emotion,
    );
    let renderer = match backend {
        "synthetic" => Some("synthetic".to_string()),
        "command" => config.and_then(|cfg| cfg.render.command.first().cloned()),
        _ => config
            .and_then(|cfg| cfg.render.command.first().cloned())
            .or_else(|| tools.cbx.as_ref().map(|path| path.display().to_string())),
    };
    let formats = config
        .map(|cfg| cfg.output.formats.clone())
        .unwrap_or_else(|| vec!["wav".into(), "opus".into(), "mp3".into()]);
    let needs_ffmpeg = formats.iter().any(|fmt| fmt == "opus" || fmt == "mp3");
    let ffmpeg = tools.ffmpeg.as_ref().map(|path| path.display().to_string());
    let ready = renderer.is_some()
        && whisper.is_some()
        && ecapa.is_some()
        && emotion.is_some()
        && (!needs_ffmpeg || ffmpeg.is_some());
    json!({
        "tool": "galion-local-studio",
        "os": std::env::consts::OS,
        "device": device,
        "backend": backend,
        "renderer": renderer,
        "scorers": {
            "whisper": whisper,
            "ecapa": ecapa,
            "emotion": emotion
        },
        "ffmpeg": ffmpeg,
        "needs_ffmpeg": needs_ffmpeg,
        "gates": {
            "wer_max": config.map(|cfg| cfg.gates.wer_max).unwrap_or(ROLLOUT_WER_MAX),
            "ecapa_min": config.map(|cfg| cfg.gates.ecapa_min).unwrap_or(ROLLOUT_ECAPA_MIN),
            "emotion_min": config.map(|cfg| cfg.gates.emotion_min).unwrap_or(ROLLOUT_EMOTION_MIN),
            "ecapa_comparison": "greater-than"
        },
        "delivery": {
            "lufs": TARGET_LUFS,
            "true_peak_dbtp": TRUE_PEAK_DBTP,
            "bed_hz": BED_HZ,
            "bed_under_db": BED_UNDER_DB
        },
        "ready_to_render": ready,
        "weights": "not-downloaded"
    })
}

fn command_or_bin(command: Option<&[String]>, discovered: &Option<PathBuf>) -> Option<String> {
    if let Some(argv) = command {
        if let Some(bin) = argv.first() {
            if !bin.is_empty() {
                return Some(bin.clone());
            }
        }
    }
    discovered.as_ref().map(|path| path.display().to_string())
}

pub fn nvidia_from_host() -> bool {
    crate::device::nvidia_visible()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StudioConfig;

    #[test]
    fn doctor_names_the_rollout_gates() {
        let cfg = StudioConfig::parse(include_str!("../examples/studio.toml")).unwrap();
        let body = report(Some(&cfg), false);
        assert_eq!(body["gates"]["wer_max"], 0.018);
        assert_eq!(body["gates"]["ecapa_min"], 0.7057);
        assert_eq!(body["delivery"]["bed_hz"], 432.0);
        assert_eq!(body["weights"], "not-downloaded");
        assert_eq!(body["device"]["ep"], "cpu");
    }
}
