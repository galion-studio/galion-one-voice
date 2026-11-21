// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::PathBuf;

use galion_s03_asr::{
    open_engine, read_wav, run_stream, wav_to_16k_mono, EngineOpts, HEARTBEAT_MS, SAMPLE_RATE,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("s03-asr: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut wav: Option<PathBuf> = None;
    let mut backend = "vosk".to_string();
    let mut model: Option<PathBuf> = None;
    let mut lib: Option<PathBuf> = None;
    let mut bin: Option<PathBuf> = None;
    let mut chunk_ms: u32 = HEARTBEAT_MS;
    let mut threads: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut next = |flag: &str| -> Result<String, String> {
            args.next().ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                eprintln!(
                    "s03-asr --wav FILE [--backend vosk|vosk-worker|whisper] [--model PATH]\n\
                     [--lib PATH] [--bin PATH] [--chunk-ms 80] [--threads N]\n\
                     \n\
                     Prints one JSON object per partial or final on stdout.\n\
                     A missing model is an error. This program does not invent words."
                );
                return Ok(());
            }
            "--wav" => wav = Some(PathBuf::from(next("--wav")?)),
            "--backend" => backend = next("--backend")?,
            "--model" => model = Some(PathBuf::from(next("--model")?)),
            "--lib" => lib = Some(PathBuf::from(next("--lib")?)),
            "--bin" => bin = Some(PathBuf::from(next("--bin")?)),
            "--chunk-ms" => {
                chunk_ms = next("--chunk-ms")?
                    .parse()
                    .map_err(|_| "--chunk-ms is not an integer".to_string())?;
            }
            "--threads" => {
                threads = Some(
                    next("--threads")?
                        .parse()
                        .map_err(|_| "--threads is not an integer".to_string())?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let wav = wav.ok_or_else(|| "--wav is required".to_string())?;
    if chunk_ms == 0 {
        return Err("--chunk-ms must be at least 1".into());
    }
    let decoded = read_wav(&wav).map_err(|err| err.to_string())?;
    let pcm = wav_to_16k_mono(&decoded).map_err(|err| err.to_string())?;
    let mut opts = EngineOpts::backend(backend);
    opts.model = model;
    opts.lib = lib;
    opts.bin = bin;
    opts.threads = threads;
    let (mut engine, model_path, _) = open_engine(&opts).map_err(|err| err.to_string())?;
    let chunk_samples = (u64::from(SAMPLE_RATE) * u64::from(chunk_ms) / 1000) as usize;
    let report = run_stream(engine.as_mut(), &pcm, chunk_samples).map_err(|err| err.to_string())?;
    eprintln!(
        "backend={} model={} source_hz={} samples={} wall_us={}",
        engine.name(),
        model_path,
        decoded.sample_rate,
        report.audio_samples,
        report.wall_us
    );
    for event in &report.events {
        println!("{}", event.to_json_line());
    }
    Ok(())
}
