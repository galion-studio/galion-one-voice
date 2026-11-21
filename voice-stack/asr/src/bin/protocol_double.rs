// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Protocol peer for tests. It is not a recognizer. It answers the
//! `galion-asr-vosk/1` handshake with a fixed partial and final.

use std::io::{BufRead, Write};

fn main() {
    emit(r#"{"type":"ready","protocol":"galion-asr-vosk/1","model":"double","frame_samples":320}"#);
    let mut line = String::new();
    let n = std::io::stdin().lock().read_line(&mut line).unwrap_or(0);
    if n == 0 {
        return;
    }
    emit(r#"{"type":"partial","text":"hello there","t_ms":20}"#);
    emit(
        r#"{"type":"final","text":"hello there","t_ms":40,"words":[{"word":"hello","start":0.0,"end":0.2,"conf":0.5},{"word":"there","start":0.2,"end":0.4,"conf":0.75}]}"#,
    );
}

fn emit(line: &str) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}
