// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use galion_sam_asr::{
    compiled_with, cpu_model, final_text, open_engine, parse_manifest, read_wav, run_stream,
    unix_to_utc, wav_to_16k_mono, word_error, AsrKind, ClipScore, EngineOpts, HostInfo, RunScore,
    HEARTBEAT_MS, HEARTBEAT_SAMPLES, SAMPLE_RATE,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("s03-asr-eval: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("eval/cc0-ljspeech.tsv");
    let mut audio_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("weights/cc0");
    let mut backend = "vosk".to_string();
    let mut model: Option<PathBuf> = None;
    let mut lib: Option<PathBuf> = None;
    let mut bin: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut chunk_ms: u32 = HEARTBEAT_MS;
    let mut threads: Option<u32> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut next = |flag: &str| -> Result<String, String> {
            args.next().ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--manifest" => manifest = PathBuf::from(next("--manifest")?),
            "--audio" => audio_dir = PathBuf::from(next("--audio")?),
            "--backend" => backend = next("--backend")?,
            "--model" => model = Some(PathBuf::from(next("--model")?)),
            "--lib" => lib = Some(PathBuf::from(next("--lib")?)),
            "--bin" => bin = Some(PathBuf::from(next("--bin")?)),
            "--out" => out = Some(PathBuf::from(next("--out")?)),
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
    if chunk_ms == 0 {
        return Err("--chunk-ms must be at least 1".into());
    }
    let text = std::fs::read_to_string(&manifest)
        .map_err(|err| format!("cannot read {}: {err}", manifest.display()))?;
    let clips = parse_manifest(&text).map_err(|err| err.to_string())?;
    let mut opts = EngineOpts::backend(&backend);
    opts.model = model;
    opts.lib = lib;
    opts.bin = bin;
    opts.threads = threads;
    let load_started = Instant::now();
    let (mut engine, model_path, thread_count) =
        open_engine(&opts).map_err(|err| err.to_string())?;
    let load_us = load_started.elapsed().as_micros() as u64;
    let chunk_samples = (u64::from(SAMPLE_RATE) * u64::from(chunk_ms) / 1000) as usize;
    if chunk_samples == 0 {
        return Err("chunk is zero samples".into());
    }
    let mut scored = Vec::new();
    for clip in clips {
        let path = audio_dir.join(format!("{}.wav", clip.id));
        if !path.is_file() {
            return Err(format!(
                "missing {}. Run voice-stack/asr/scripts/fetch-cc0.sh. No score was invented.",
                path.display()
            ));
        }
        let wav = read_wav(&path).map_err(|err| err.to_string())?;
        let pcm = wav_to_16k_mono(&wav).map_err(|err| err.to_string())?;
        engine.reset().map_err(|err| err.to_string())?;
        let report =
            run_stream(engine.as_mut(), &pcm, chunk_samples).map_err(|err| err.to_string())?;
        let hypothesis = final_text(&report.events);
        scored.push(ClipScore {
            id: clip.id,
            wer: word_error(&clip.text, &hypothesis),
            reference: clip.text,
            hypothesis,
            source_hz: wav.sample_rate,
            source_channels: wav.channels,
            audio_samples: report.audio_samples,
            wall_us: report.wall_us,
            finish_us: report.finish_us,
            chunk_wall_us: report.chunk_wall_us,
            first_partial_wall_us: report.first_partial_wall_us,
            first_partial_audio_ms: report.first_partial_audio_ms,
            partials: report
                .events
                .iter()
                .filter(|e| e.kind == AsrKind::Partial)
                .count(),
            finals: report
                .events
                .iter()
                .filter(|e| e.kind == AsrKind::Final)
                .count(),
        });
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let run = RunScore {
        measured_at_utc: unix_to_utc(secs),
        host: HostInfo {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            cpu: cpu_model(),
            cpus: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
            rustc: compiled_with().into(),
        },
        test_set: "LJ Speech 1.1 clips LJ001-0001 through LJ001-0012, in dataset order. Not the full 24 hours.".into(),
        license: "Public domain (Keith Ito / LibriVox, Linda Johnson). Mozilla Data Collective catalogs the same set as CC0-1.0.".into(),
        backend: engine.name().to_string(),
        model: model_path,
        threads: thread_count,
        chunk_ms,
        load_us,
        notes: vec![
            "micro_wer is (substitutions + deletions + insertions) / reference_words on this clip list only.".into(),
            "RTF is the sum of push+finish wall time divided by audio duration after resample. Model load is load_us and is not inside RTF.".into(),
            "Chunk latency is the wall time of each push. finish is separate. A chunk over 80000 us missed an 80 ms heartbeat.".into(),
            "The hypothesis is the concatenation of non-empty finals. Partials are snapshots and are not appended.".into(),
            "Source audio is 16-bit PCM. It is downmixed and linearly resampled to 16000 Hz mono before the engine.".into(),
            format!(
                "One heartbeat is {HEARTBEAT_MS} ms ({HEARTBEAT_SAMPLES} samples at {SAMPLE_RATE} Hz)."
            ),
        ],
        clips: scored,
    };
    let summary = run.summary();
    println!("{summary}");
    if let Some(path) = out {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
            }
        }
        std::fs::write(&path, run.to_pretty_json())
            .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
        eprintln!("wrote {}", path.display());
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "s03-asr-eval [--backend vosk|vosk-worker|whisper] [--manifest FILE] [--audio DIR]\n\
         [--model PATH] [--lib PATH] [--bin PATH] [--chunk-ms 80] [--threads N] [--out FILE.json]\n\
         \n\
         Scores a local clip list. Missing audio or a missing model is an error.\n\
         This program does not invent a word-error rate.\n\
         whisper requires a build with --features whisper."
    );
}
