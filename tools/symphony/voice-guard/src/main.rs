// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Galion Labs.
//! galion-voice-guard check --wav clip.wav [--baseline ref.wav] [--config configs/symphony/voice-guard.toml]

use galion_voice_guard::{
    admit, load_config, read_f32_le, read_wav, Baseline, Config, Context, Pcm,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("galion-voice-guard: {e}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_default();
    if cmd != "check" {
        return Err("usage: galion-voice-guard check --wav FILE [--baseline FILE] [--embedding-cos FLOAT] [--config FILE] [--out FILE] [--raw --sr HZ] [--no-release]".into());
    }
    let mut wav: Option<PathBuf> = None;
    let mut raw: Option<PathBuf> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut config: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut embedding: Option<f64> = None;
    let mut sr: Option<u32> = None;
    let mut release = true;
    let rest: Vec<String> = args.collect();
    let mut i = 0;
    while i < rest.len() {
        let key = rest[i].as_str();
        let val = rest.get(i + 1).map(String::as_str);
        match key {
            "--wav" => wav = Some(need(val, "--wav")?.into()),
            "--raw" => raw = Some(need(val, "--raw")?.into()),
            "--baseline" => baseline = Some(need(val, "--baseline")?.into()),
            "--config" => config = Some(need(val, "--config")?.into()),
            "--out" => out = Some(need(val, "--out")?.into()),
            "--embedding-cos" => {
                embedding = Some(
                    need(val, "--embedding-cos")?
                        .parse()
                        .map_err(|_| "--embedding-cos is not a number".to_string())?,
                );
            }
            "--sr" => {
                sr = Some(
                    need(val, "--sr")?
                        .parse()
                        .map_err(|_| "--sr is not an integer".to_string())?,
                );
            }
            "--no-release" => {
                release = false;
                i += 1;
                continue;
            }
            other => return Err(format!("unknown argument {other}")),
        }
        i += 2;
    }
    let cfg = resolve_config(config.as_deref())?;
    let (interleaved, rate, channels, bits) = if let Some(path) = &wav {
        let w = read_wav(path)?;
        (
            w.interleaved,
            w.sample_rate_hz,
            w.channels,
            Some(w.bits_per_sample),
        )
    } else if let Some(path) = &raw {
        let samples = read_f32_le(path)?;
        let rate = sr.ok_or("--raw needs --sr")?;
        (samples, rate, 1, None)
    } else {
        return Err("--wav or --raw is required".into());
    };
    let base = match &baseline {
        Some(path) => {
            let w = read_wav(path)?;
            if w.channels != 1 {
                let mono = downmix(&w.interleaved, w.channels);
                Some((mono, w.sample_rate_hz))
            } else {
                Some((w.interleaved, w.sample_rate_hz))
            }
        }
        None => None,
    };
    let held_base = base;
    let ctx = Context {
        baseline: held_base.as_ref().map(|(s, r)| Baseline {
            samples: s,
            sample_rate_hz: *r,
        }),
        embedding_cos: embedding,
        expect_release: release,
        wav_path: wav.as_deref(),
    };
    let report = match admit(
        Pcm {
            interleaved: &interleaved,
            sample_rate_hz: rate,
            channels,
            bits_per_sample: bits,
        },
        &cfg,
        &ctx,
    ) {
        Ok(r) | Err(r) => r,
    };
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    } else {
        println!("{text}");
    }
    Ok(if report.bad {
        ExitCode::from(1)
    } else {
        ExitCode::from(0)
    })
}

fn need<'a>(val: Option<&'a str>, flag: &str) -> Result<&'a str, String> {
    val.ok_or_else(|| format!("{flag} needs a value"))
}

fn resolve_config(path: Option<&Path>) -> Result<Config, String> {
    if let Some(path) = path {
        return load_config(path);
    }
    let local = Path::new("configs/symphony/voice-guard.toml");
    if local.is_file() {
        return load_config(local);
    }
    Config::bundled()
}

fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    let ch = channels as usize;
    interleaved
        .chunks_exact(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect()
}
