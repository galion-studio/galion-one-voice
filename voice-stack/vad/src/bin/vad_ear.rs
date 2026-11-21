// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Silero ear for samantha-local.
//!
//! One process, stdin commands, stdout lines. The model path is not printed.
//! `GALION_SILERO_VAD` or `--model` selects the ONNX file. A missing file
//! still answers, with `backend=energy`, so the parent can see that Silero
//! did not load.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use galion_sam_vad::{Event, Listen, TurnCfg, FRAME_SAMPLES};

fn main() {
    let mut model = std::env::var_os("GALION_SILERO_VAD")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" => model = args.next().map(PathBuf::from),
            "--help" | "-h" => {
                eprintln!("vad-ear [--model FILE]");
                return;
            }
            _ => {}
        }
    }
    let cfg = TurnCfg::from_env();
    let mut ear = match &model {
        Some(path) => Listen::open(Some(path), cfg),
        None => Listen::from_env(),
    };
    let mut out = io::stdout();
    let _ = writeln!(out, "ready backend={}", ear.backend().name());
    let _ = out.flush();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line == "quit" {
            break;
        }
        if line == "reset" {
            ear.reset();
            let _ = writeln!(out, "ok");
            let _ = out.flush();
            continue;
        }
        let Some(hex) = line.strip_prefix("frame ") else {
            let _ = writeln!(out, "err bad-command");
            let _ = out.flush();
            continue;
        };
        let Some(frame) = decode_hex(hex) else {
            let _ = writeln!(out, "err bad-frame");
            let _ = out.flush();
            continue;
        };
        if frame.len() != FRAME_SAMPLES {
            let _ = writeln!(out, "err bad-frame");
            let _ = out.flush();
            continue;
        }
        match ear.push(&frame, false, "") {
            Ok(report) => {
                let event = match report.event {
                    Event::SpeechStart => "start",
                    Event::SpeechEnd => "end",
                    _ => "none",
                };
                let speaking = u8::from(report.speaking);
                let _ = writeln!(
                    out,
                    "score prob={:.3} speaking={speaking} event={event}",
                    report.prob.clamp(0.0, 1.0)
                );
            }
            Err(_) => {
                let _ = writeln!(out, "err bad-frame");
            }
        }
        let _ = out.flush();
    }
}

fn decode_hex(hex: &str) -> Option<Vec<i16>> {
    if hex.len() % 4 != 0 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Option<Vec<u8>> = hex
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect();
    let bytes = bytes?;
    Some(
        bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect(),
    )
}
