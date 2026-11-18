// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! Optional neural quality scores.
//!
//! DNSMOS, UTMOS, and NISQA are not linked. The default build does not spawn
//! a process and does not download weights. Feature `neural` plus a command
//! already on disk is the only path that runs a scorer.

use crate::checks;
use crate::config::NeuralCfg;
use crate::report::Check;
use std::path::Path;

pub fn quality(cfg: &NeuralCfg, wav: Option<&Path>) -> Check {
    let method = "optional external scorer (DNSMOS / UTMOS / NISQA style). Not linked. No weights are fetched";
    if !cfg!(feature = "neural") {
        return checks::stub(
            "neural_quality",
            "feature `neural` is off. DNSMOS, UTMOS, and NISQA are not run and no weights are downloaded",
            method,
        );
    }
    if !cfg.enabled || cfg.command.trim().is_empty() {
        return checks::stub(
            "neural_quality",
            "feature `neural` is on and configs/symphony/voice-guard.toml [neural] has no command",
            method,
        );
    }
    let Some(wav) = wav else {
        return checks::stub(
            "neural_quality",
            "no wav path. The external scorer is a file tool",
            method,
        );
    };
    let mut cmd = std::process::Command::new(&cfg.command);
    cmd.arg(wav)
        .env("HF_HUB_OFFLINE", "1")
        .env("TRANSFORMERS_OFFLINE", "1");
    match cmd.output() {
        Ok(out) if out.status.success() => Check {
            name: "neural_quality",
            status: "pass",
            score: None,
            value: None,
            limit: None,
            unit: "",
            reason: format!(
                "external scorer exited 0 ({} bytes of stdout attached by the caller, not parsed into a gate)",
                out.stdout.len()
            ),
            frames: Vec::new(),
            method,
        },
        Ok(out) => Check {
            name: "neural_quality",
            status: "fail",
            score: Some(0.0),
            value: None,
            limit: None,
            unit: "",
            reason: format!(
                "external scorer failed ({}); {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).chars().take(240).collect::<String>()
            ),
            frames: Vec::new(),
            method,
        },
        Err(e) => Check {
            name: "neural_quality",
            status: "fail",
            score: Some(0.0),
            value: None,
            limit: None,
            unit: "",
            reason: format!("external scorer could not start: {e}"),
            frames: Vec::new(),
            method,
        },
    }
}
