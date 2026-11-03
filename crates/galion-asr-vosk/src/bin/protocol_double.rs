// Galion Voice — Perpetual Open MIT License
// Copyright (c) 2026 Maciej Grajczyk.
//! Protocol peer for tests. Not speech recognition. The screen script does
//! not launch this binary, and it never emits a duration-bucket sentence.
//!
//! Final text is `n=<sample count> grammar=<phrase count>`.

use galion_asr_vosk::{encode_final, encode_partial, encode_ready, parse_utterance, Word};
use std::io::{self, BufRead, Write};

fn main() {
    let mut out = io::stdout();
    let _ = writeln!(out, "{}", encode_ready("protocol-double"));
    let _ = out.flush();
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let req = match parse_utterance(&line) {
            Ok(r) => r,
            Err(e) => {
                let _ = writeln!(out, "{}", galion_asr_vosk::encode_error(&e));
                let _ = out.flush();
                std::process::exit(1);
            }
        };
        let g = req.grammar.as_ref().map(|p| p.len()).unwrap_or(0);
        let _ = writeln!(out, "{}", encode_partial("frame", 20));
        let text = format!("n={} grammar={g}", req.pcm.len());
        let words = vec![Word {
            word: "n".into(),
            start: 0.0,
            end: 0.02,
            conf: 1.0,
        }];
        let _ = writeln!(out, "{}", encode_final(&text, 40, &words));
        let _ = out.flush();
    }
}
