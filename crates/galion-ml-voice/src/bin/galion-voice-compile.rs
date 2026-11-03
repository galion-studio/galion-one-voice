// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Compile Chatterbox clone PCM into a Galion `.gq` voice clip.
//!
//! This process does not load PyTorch and does not speak. Clone generation
//! stays in the out-of-process worker. The file this writes is what the
//! symphony opens.

use galion_ml_voice::artifact::{self, ArtError};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") || args.is_empty() {
        usage();
        return if args.is_empty() {
            ExitCode::from(2)
        } else {
            ExitCode::SUCCESS
        };
    }
    match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("galion-voice-compile: {err}");
            ExitCode::from(code_for(&err))
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, ArtError> {
    let mut persona = None;
    let mut input = None;
    let mut out = None;
    let mut rate: u32 = 24_000;
    let mut channels: u16 = 1;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--persona" => persona = Some(need(args, &mut i, "--persona")?),
            "--input" => input = Some(PathBuf::from(need(args, &mut i, "--input")?)),
            "--out" => out = Some(PathBuf::from(need(args, &mut i, "--out")?)),
            "--rate" => rate = parse_u32(need(args, &mut i, "--rate")?)?,
            "--channels" => channels = parse_u16(need(args, &mut i, "--channels")?)?,
            other => {
                return Err(ArtError::BadInput(format!("unknown argument {other}")));
            }
        }
        i += 1;
    }
    let persona = persona.ok_or_else(|| ArtError::BadInput("missing --persona".into()))?;
    let input = input.ok_or_else(|| ArtError::BadInput("missing --input".into()))?;
    let out = out.unwrap_or_else(|| artifact::artifact_path(&persona));
    let rep = artifact::compile_file(&input, &out, &persona, rate, channels)?;
    println!(
        "compiled {} frames={} samples={} sha256={} pcm_bytes={}",
        rep.path.display(),
        rep.n_frames,
        rep.n_samples,
        rep.sha256_hex,
        rep.pcm_bytes
    );
    Ok(ExitCode::SUCCESS)
}

fn need<'a>(args: &'a [String], i: &mut usize, flag: &str) -> Result<&'a str, ArtError> {
    *i += 1;
    args.get(*i)
        .map(String::as_str)
        .ok_or_else(|| ArtError::BadInput(format!("{flag} needs a value")))
}

fn parse_u32(s: &str) -> Result<u32, ArtError> {
    s.parse()
        .map_err(|_| ArtError::BadInput(format!("not a u32: {s}")))
}

fn parse_u16(s: &str) -> Result<u16, ArtError> {
    s.parse()
        .map_err(|_| ArtError::BadInput(format!("not a u16: {s}")))
}

fn code_for(err: &ArtError) -> u8 {
    match err {
        ArtError::Refused(_) => 3,
        ArtError::Io(_) | ArtError::BadClip(_) | ArtError::Sidecar(_) => 1,
        ArtError::BadPersona(_) | ArtError::BadInput(_) => 2,
    }
}

fn usage() {
    eprintln!(
        "\
galion-voice-compile — clone PCM → Galion voice clip (.gq)

  galion-voice-compile --persona samantha|jarvis --input CLONE --out voices/compiled/<persona>.gq

CLONE is worker stdout or a file the worker could have written:
  .wav     16-bit PCM, mono or stereo (stereo is averaged, not stripped)
  .gci     Galion clone intermediate (not a runtime file)
  .pcm .s16le .raw   raw s16le; --rate (default 24000) and --channels (default 1)

The runtime path is voices/compiled/<persona>.gq plus a .sha256 sidecar.
Chatterbox safetensors, .pt, and GGML-family containers are refused.
CPU only ({flagship}; {plus}; {ultra}). PyTorch is not started.
",
        flagship = artifact::HOST_FLAGSHIP,
        plus = artifact::HOST_PLUS,
        ultra = artifact::HOST_ULTRA,
    );
}
