// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! Protocol stand-in for tests. Same stdin/stdout/stderr contract as
//! Piper 2023.11.14-2: one JSON line in, raw s16le out, then
//! `Real-time factor:` on stderr. Not a voice. Ignores `--model` and the rest.

use galion_tts_piper::stand_in_pcm;
use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let text = json_text(&line);
        let pcm = stand_in_pcm(&text);
        let mut bytes = Vec::with_capacity(pcm.len() * 2);
        for s in &pcm {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
        // Odd first write, so the parent must reassemble a split sample.
        if !bytes.is_empty() {
            let _ = stdout.write_all(&bytes[..1]);
            let _ = stdout.flush();
            let _ = stdout.write_all(&bytes[1..]);
            let _ = stdout.flush();
        }
        let _ = writeln!(
            stderr,
            "Real-time factor: 0.10 (infer=0.01 sec, audio=0.20 sec)"
        );
        let _ = stderr.flush();
    }
}

fn json_text(line: &str) -> String {
    let key = "\"text\"";
    let Some(i) = line.find(key) else {
        return String::new();
    };
    let rest = &line[i + key.len()..];
    let Some(q) = rest.find('"') else {
        return String::new();
    };
    let mut out = String::new();
    let mut chars = rest[q + 1..].chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            break;
        }
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => break,
            }
        } else {
            out.push(c);
        }
    }
    out
}
